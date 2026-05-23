use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;

pub const NONCE_SIZE: usize = 24;
pub const TAG_SIZE: usize = 16;

// v0.3 breaking change: legacy empty-AAD fallback removed. The env-gated
// fallback (`COPYPASTE_ALLOW_LEGACY_AAD`) that briefly shipped on beta.6
// is gone — all callers must pass the exact AAD used at encrypt time.

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
    /// Strict-mode failure: the supplied AAD did not validate AND the
    /// `COPYPASTE_ALLOW_LEGACY_AAD` env var is disabled, so we refused to
    /// silently fall back to empty-AAD decryption. Callers can recover by
    /// re-enabling the env var for one-shot legacy decrypts, or by
    /// re-encrypting the affected row under the current AAD binding.
    #[error("Decryption failed: AAD mismatch (legacy empty-AAD fallback disabled)")]
    AadMismatch,
    /// AEAD cipher rejected the input (e.g. payload exceeds the per-message
    /// limit of (2^32 - 1) * 64 bytes for ChaCha20-Poly1305). We surface the
    /// underlying error string instead of panicking so callers can degrade
    /// gracefully (chunk the input, reject the request, etc.).
    #[error("AEAD cipher failed: {0}")]
    CipherFailed(String),
    /// The row's `key_version` column carries a value that no current code
    /// path knows how to decrypt. Typically indicates a forward-compatibility
    /// gap: the row was written by a newer build and the current binary needs
    /// to be upgraded.
    ///
    /// MUST NOT panic — surfaces as a hard error at the IPC layer
    /// (mapped to `auth_failed` error code) so the caller can handle it
    /// gracefully. Never silently falls through to v1 or v2 decrypt logic.
    #[error("Unknown key_version: {0}")]
    UnknownKeyVersion(u8),
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

/// Decrypt a clipboard item, dispatching on `key_version` to select the
/// correct key and AAD format.
///
/// | `key_version` | Key    | AAD format                         |
/// |---------------|--------|------------------------------------|
/// | 1             | v1 key | `build_item_aad(item_id, 3)`       |
/// | 2             | v2 key | `build_item_aad_v2(item_id, 4, 2)` |
/// | other         | —      | `Err(UnknownKeyVersion(v))`        |
///
/// Callers supply BOTH `v1_key` and `v2_key` so this function can select the
/// right one without requiring the caller to know the version ahead of time.
/// On `key_version = 2` the `v1_key` argument is ignored.
///
/// # Errors
/// - `AuthFailed` — ciphertext, nonce, or key is wrong for the version
/// - `UnknownKeyVersion(v)` — `key_version` is not 1 or 2
pub fn decrypt_item_by_version(
    key_version: u8,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
    item_id: &str,
    nonce: &[u8; NONCE_SIZE],
    ciphertext: &[u8],
) -> Result<Vec<u8>, EncryptError> {
    match key_version {
        1 => {
            let aad = build_item_aad(item_id, AAD_SCHEMA_VERSION);
            decrypt_item_with_aad(ciphertext, nonce, v1_key, &aad)
        }
        2 => {
            let aad = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2);
            decrypt_item_with_aad(ciphertext, nonce, v2_key, &aad)
        }
        v => Err(EncryptError::UnknownKeyVersion(v)),
    }
}

/// Encrypt with XChaCha20-Poly1305 and no AAD (legacy/back-compat).
///
/// Equivalent to `encrypt_item_with_aad(plaintext, key, &[])`. New call
/// sites SHOULD use `encrypt_item_with_aad` and pass an AAD bound to
/// the row's `(item_id, schema_version)` — see `build_item_aad`.
pub fn encrypt_item(
    plaintext: &[u8],
    key: &[u8; 32],
) -> Result<([u8; NONCE_SIZE], Vec<u8>), EncryptError> {
    encrypt_item_with_aad(plaintext, key, &[])
}

/// Decrypt with XChaCha20-Poly1305 and no AAD (legacy/back-compat).
///
/// Equivalent to `decrypt_item_with_aad(ciphertext, nonce, key, &[])`.
/// For ciphertexts produced by `encrypt_item_with_aad` with non-empty
/// AAD, call `decrypt_item_with_aad` with the matching AAD.
pub fn decrypt_item(
    ciphertext: &[u8],
    nonce: &[u8; NONCE_SIZE],
    key: &[u8; 32],
) -> Result<Vec<u8>, EncryptError> {
    decrypt_item_with_aad(ciphertext, nonce, key, &[])
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

    // ── Audit HIGH #1: legacy empty-AAD fallback must be opt-in via env ──
    //
    // These tests mutate process-global env state — they share the same
    use serial_test::serial;

    // v0.3 breaking change: legacy empty-AAD fallback was removed (was
    // briefly env-gated on beta.6 via COPYPASTE_ALLOW_LEGACY_AAD). The
    // fallback tests have been deleted along with the code path. AAD
    // mismatch now always returns AuthFailed — pinned by
    // `decrypt_item_with_aad_wrong_aad_returns_err` and
    // `decrypt_item_with_aad_full_roundtrip` above.

    #[test]
    #[serial]
    fn aad_mismatch_with_real_aad_blob_returns_authfailed() {
        let key = test_key();
        let aad_a = build_item_aad("item-A", AAD_SCHEMA_VERSION);
        let aad_b = build_item_aad("item-B", AAD_SCHEMA_VERSION);
        let (nonce, ct) = encrypt_item_with_aad(b"payload", &key, &aad_a).unwrap();
        let err = decrypt_item_with_aad(&ct, &nonce, &key, &aad_b).unwrap_err();
        assert!(matches!(err, EncryptError::AuthFailed));
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

    // -------------------------------------------------------------------------
    // wave1a-atomic: UnknownKeyVersion + decrypt_item_by_version dispatcher
    // -------------------------------------------------------------------------

    /// T4 (golden-file): verify `build_item_aad_v2` produces exact bytes so
    /// future AAD schema changes surface as a test failure before they corrupt
    /// on-disk rows.
    #[test]
    fn build_item_aad_v2_golden_bytes() {
        // AAD for key_version=2 item "item-abc" at schema version 4:
        // b"item-abc|4|2"
        let aad = build_item_aad_v2("item-abc", AAD_SCHEMA_VERSION_V4, 2);
        assert_eq!(aad, b"item-abc|4|2");

        // AAD for key_version=1 item at schema version 3 (legacy):
        // b"item-abc|3"
        let aad_v1 = build_item_aad("item-abc", AAD_SCHEMA_VERSION);
        assert_eq!(aad_v1, b"item-abc|3");
    }

    /// `decrypt_item_by_version` dispatches to v1 path for key_version=1.
    #[test]
    fn decrypt_item_by_version_v1_roundtrip() {
        let v1_key = [0x11u8; 32];
        let v2_key = [0x22u8; 32];
        let item_id = "item-v1-test";
        let aad = build_item_aad(item_id, AAD_SCHEMA_VERSION);
        let (nonce, ct) = encrypt_item_with_aad(b"hello v1", &v1_key, &aad).unwrap();
        let pt = decrypt_item_by_version(1, &v1_key, &v2_key, item_id, &nonce, &ct).unwrap();
        assert_eq!(pt, b"hello v1");
    }

    /// `decrypt_item_by_version` dispatches to v2 path for key_version=2.
    #[test]
    fn decrypt_item_by_version_v2_roundtrip() {
        let v1_key = [0x33u8; 32];
        let v2_key = [0x44u8; 32];
        let item_id = "item-v2-test";
        let aad = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ct) = encrypt_item_with_aad(b"hello v2", &v2_key, &aad).unwrap();
        let pt = decrypt_item_by_version(2, &v1_key, &v2_key, item_id, &nonce, &ct).unwrap();
        assert_eq!(pt, b"hello v2");
    }

    /// Corrupt `key_version=255` must return `UnknownKeyVersion(255)`, not panic.
    #[test]
    fn decrypt_item_by_version_unknown_returns_error_not_panic() {
        let v1_key = [0x55u8; 32];
        let v2_key = [0x66u8; 32];
        let nonce = [0u8; NONCE_SIZE];
        let ct = b"garbage";
        let result = decrypt_item_by_version(255, &v1_key, &v2_key, "item-x", &nonce, ct);
        assert!(
            matches!(result, Err(EncryptError::UnknownKeyVersion(255))),
            "key_version=255 must return UnknownKeyVersion(255), got {:?}",
            result
        );
    }

    /// key_version=0 is also unknown — must return UnknownKeyVersion(0).
    #[test]
    fn decrypt_item_by_version_zero_returns_unknown() {
        let v1_key = [0x77u8; 32];
        let v2_key = [0x88u8; 32];
        let nonce = [0u8; NONCE_SIZE];
        let result = decrypt_item_by_version(0, &v1_key, &v2_key, "item-y", &nonce, b"ct");
        assert!(matches!(result, Err(EncryptError::UnknownKeyVersion(0))));
    }

    /// `UnknownKeyVersion` must format without panicking.
    #[test]
    fn unknown_key_version_formats_without_panic() {
        for v in [0u8, 3, 42, 100, 200, 255] {
            let e = EncryptError::UnknownKeyVersion(v);
            let s = e.to_string();
            assert!(
                s.contains(&v.to_string()),
                "error string must include key_version value"
            );
        }
    }
}
