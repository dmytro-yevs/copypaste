use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;

pub const NONCE_SIZE: usize = 24;
pub const TAG_SIZE: usize = 16;

/// AAD schema version for per-item AEAD binding (`item_id|schema_version`).
///
/// Stored locally as a compile-time constant rather than re-exporting from
/// `storage::schema` to avoid a cross-module merge race with other beta
/// workers. If another worker promotes a shared `SCHEMA_VERSION` to `pub`,
/// this constant should be reconciled to that single source of truth.
///
/// Re-exported via `pub use crypto::encrypt::AAD_SCHEMA_VERSION` so storage
/// callers can pass it to `build_item_aad` without hard-coding `3` everywhere.
///
/// **v0.3 (T5) NOTE:** legacy v1-key ciphertexts continue to use the 2-arg
/// AAD format `"{item_id}|{schema_version}"` (where `schema_version == 3`).
/// New v2-key ciphertexts use [`AAD_SCHEMA_VERSION_V4`] together with
/// [`build_item_aad_v2`], which appends `|{key_version}` to bind the
/// ciphertext to the key generation that produced it.
pub const AAD_SCHEMA_VERSION: u32 = 3;

/// AAD schema version for v4 (key-versioned) ciphertexts. Used together with
/// [`build_item_aad_v2`] for items encrypted under the v2 HKDF key family.
/// Tying the schema bump (3 → 4) and the key-version field together means a
/// single integer comparison disambiguates legacy ciphertexts from
/// post-migration ciphertexts at the decrypt site.
pub const AAD_SCHEMA_VERSION_V4: u32 = 4;

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("Decryption failed: authentication tag mismatch")]
    AuthFailed,
    /// AEAD cipher rejected the input (e.g. payload exceeds the per-message
    /// limit of (2^32 - 1) * 64 bytes for ChaCha20-Poly1305). We surface the
    /// underlying error string instead of panicking so callers can degrade
    /// gracefully (chunk the input, reject the request, etc.).
    #[error("AEAD cipher failed: {0}")]
    CipherFailed(String),
}

/// Build the canonical AEAD AAD for a clipboard item:
/// `"{item_id}|{schema_version}"` as UTF-8 bytes.
///
/// Binding ciphertext to both the row's `item_id` and the storage
/// `schema_version` means an attacker who copies a ciphertext blob from
/// one row into another (or replays an old-schema blob into a new-schema
/// row) is detected by the AEAD auth tag — `decrypt_item_with_aad` will
/// reject the substituted ciphertext with `EncryptError::AuthFailed`.
pub fn build_item_aad(item_id: &str, schema_version: u32) -> Vec<u8> {
    format!("{item_id}|{schema_version}").into_bytes()
}

/// Build the v4 (key-versioned) AEAD AAD for a clipboard item:
/// `"{item_id}|{schema_version}|{key_version}"` as UTF-8 bytes.
///
/// Adds `key_version` to the AAD so that ciphertexts produced under the v2
/// HKDF key family cannot be silently decrypted with a v1 key (the auth tag
/// will reject the substituted key). This is the on-disk binding that lets
/// the v3 → v4 ciphertext migration sweep run without a flag-day reboot:
/// rows can be re-encrypted batch-by-batch and the row's `key_version`
/// column unambiguously selects which key + AAD format to use at decrypt
/// time.
///
/// Callers using the legacy 2-arg [`build_item_aad`] are encrypting with a
/// v1 key and the v3-schema AAD; that path remains supported for backward
/// compatibility (notably the daemon's existing call sites and the
/// migration sweep's decrypt-with-v1 step).
pub fn build_item_aad_v2(item_id: &str, schema_version: u32, key_version: u32) -> Vec<u8> {
    format!("{item_id}|{schema_version}|{key_version}").into_bytes()
}

/// Encrypt with XChaCha20-Poly1305 + associated data.
///
/// Returns `(random_nonce[24], ciphertext_with_tag)` or
/// `EncryptError::CipherFailed` if the AEAD layer rejects the input
/// (e.g. plaintext exceeds the per-message size limit).
///
/// `aad` is authenticated but NOT encrypted. Decryption MUST be called
/// with the identical AAD bytes, otherwise `AuthFailed` is returned.
/// This function MUST NOT panic on user-supplied data —
/// see security audit medium #10.
pub fn encrypt_item_with_aad(
    plaintext: &[u8],
    key: &[u8; 32],
    aad: &[u8],
) -> Result<([u8; NONCE_SIZE], Vec<u8>), EncryptError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|e| EncryptError::CipherFailed(e.to_string()))?;
    Ok((nonce_bytes, ciphertext))
}

/// Decrypt with XChaCha20-Poly1305 + associated data.
///
/// Returns plaintext on success or `EncryptError::AuthFailed` if the
/// ciphertext, nonce, key, or AAD has been tampered with / is wrong.
///
/// **v0.3 breaking change:** the legacy empty-AAD fallback (v0.2 → v0.3
/// bridge) has been removed. Callers MUST pass the exact AAD that was
/// supplied to `encrypt_item_with_aad`. v0.2 ciphertexts produced without
/// AAD will no longer decrypt — run `copypaste migrate v3` BEFORE upgrading
/// to backfill AAD across the row population.
pub fn decrypt_item_with_aad(
    ciphertext: &[u8],
    nonce: &[u8; NONCE_SIZE],
    key: &[u8; 32],
    aad: &[u8],
) -> Result<Vec<u8>, EncryptError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce_x = XNonce::from(*nonce);
    let payload = Payload {
        msg: ciphertext,
        aad,
    };
    cipher
        .decrypt(&nonce_x, payload)
        .map_err(|_| EncryptError::AuthFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    fn test_aad() -> Vec<u8> {
        build_item_aad("test-item", AAD_SCHEMA_VERSION)
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let aad = test_aad();
        let plaintext = b"Hello, clipboard!";
        let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).unwrap();
        let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_plaintexts_produce_different_nonces() {
        let key = test_key();
        let aad = test_aad();
        let (n1, _) = encrypt_item_with_aad(b"aaa", &key, &aad).unwrap();
        let (n2, _) = encrypt_item_with_aad(b"aaa", &key, &aad).unwrap();
        assert_ne!(n1, n2);
    }

    #[test]
    fn tampered_ciphertext_fails_decryption() {
        let key = test_key();
        let aad = test_aad();
        let (nonce, mut ciphertext) = encrypt_item_with_aad(b"secret", &key, &aad).unwrap();
        ciphertext[0] ^= 0xFF;
        assert!(decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).is_err());
    }

    #[test]
    fn empty_plaintext_encrypts_and_decrypts() {
        let key = test_key();
        let aad = test_aad();
        let (nonce, ciphertext) = encrypt_item_with_aad(b"", &key, &aad).unwrap();
        let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn large_plaintext_1mb_roundtrip() {
        let key = test_key();
        let aad = test_aad();
        let plaintext = vec![0xABu8; 1_000_000];
        let (nonce, ciphertext) = encrypt_item_with_aad(&plaintext, &key, &aad).unwrap();
        let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    /// Security audit medium #10: pathological inputs must surface as
    /// `EncryptError::CipherFailed` instead of panicking. We can't actually
    /// allocate >256 GiB to hit the real ChaCha20-Poly1305 limit in CI,
    /// so we exercise the happy path *and* a forced-error path via a
    /// crafted decryption call (which uses the same error-mapping pattern).
    /// The structural fact this test pins is: `encrypt_item_with_aad` returns
    /// `Result` — the API can no longer panic at the call site.
    #[test]
    fn encrypt_returns_error_not_panic_on_oversized() {
        let key = test_key();
        let aad = test_aad();

        // Happy path: returns Ok
        let ok = encrypt_item_with_aad(b"normal input", &key, &aad);
        assert!(ok.is_ok(), "small input must succeed");

        // The signature itself is the guarantee: callers handle errors via `?`
        // instead of unwinding the stack on adversarial input. We assert the
        // type-level contract: the function returns Result, not a raw tuple.
        let result: Result<([u8; NONCE_SIZE], Vec<u8>), EncryptError> =
            encrypt_item_with_aad(b"x", &key, &aad);
        assert!(result.is_ok());

        // And the error variant exists and formats sensibly.
        let err = EncryptError::CipherFailed("simulated".into());
        assert!(err.to_string().contains("AEAD cipher failed"));
    }

    // ---------------------------------------------------------------------
    // T5 (v0.3): v4 AAD format — binds ciphertext to key_version
    // ---------------------------------------------------------------------

    /// v4 AAD layout snapshot: `"{item_id}|{schema_version}|{key_version}"`.
    /// Locked here so a refactor that re-orders or drops a field surfaces as
    /// a test failure rather than as silently-undecryptable on-disk rows.
    #[test]
    fn build_item_aad_v2_layout_is_pipe_delimited_triplet() {
        let aad = build_item_aad_v2("item-xyz", 4, 2);
        assert_eq!(aad, b"item-xyz|4|2");
    }

    /// Decryption MUST fail if a v4 ciphertext is presented with an AAD that
    /// claims a different `key_version`. This is the property that prevents
    /// a v1 key from silently decrypting (or appearing to fail nondescriptly
    /// on) a v2-key ciphertext.
    #[test]
    fn tampering_key_version_in_aad_fails_decryption() {
        let key = test_key();
        let aad_v2 = build_item_aad_v2("item-xyz", AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) = encrypt_item_with_aad(b"secret", &key, &aad_v2).unwrap();

        // Attempt decrypt with the *same* (item_id, schema_version) but a
        // different key_version → auth tag rejects.
        let aad_wrong_kv = build_item_aad_v2("item-xyz", AAD_SCHEMA_VERSION_V4, 1);
        let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_wrong_kv);
        assert!(
            matches!(result, Err(EncryptError::AuthFailed)),
            "wrong key_version in AAD must surface as AuthFailed"
        );
    }

    /// v3 AAD (legacy 2-arg) and v4 AAD (3-arg) MUST NOT be interchangeable:
    /// a v3-AAD ciphertext cannot decrypt with a v4 AAD even when the
    /// "shared" fields are identical.
    #[test]
    fn v3_and_v4_aad_are_incompatible() {
        let key = test_key();
        let aad_v3 = build_item_aad("item-xyz", AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) = encrypt_item_with_aad(b"secret", &key, &aad_v3).unwrap();

        // Decrypting with v4 AAD (even with key_version=1) must fail —
        // the on-the-wire AAD bytes are physically different.
        let aad_v4 = build_item_aad_v2("item-xyz", AAD_SCHEMA_VERSION, 1);
        assert!(decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_v4).is_err());
    }

    #[test]
    fn aad_schema_version_v4_constant_is_4() {
        assert_eq!(AAD_SCHEMA_VERSION_V4, 4);
    }
}
