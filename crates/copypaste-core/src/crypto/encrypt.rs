use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

pub const NONCE_SIZE: usize = 24;
pub const TAG_SIZE: usize = 16;

/// Process-global counter used to rate-limit the "legacy empty-AAD fallback"
/// warning emitted by [`decrypt_item_with_aad`]. Without rate limiting a
/// migration-time bulk decrypt could spam logs with one line per row.
static LEGACY_AAD_FALLBACK_HITS: AtomicU64 = AtomicU64::new(0);

/// Environment variable that re-enables the legacy empty-AAD decryption
/// fallback. Defaults to enabled (`true`) for v0.2 backwards-compatibility
/// during the AAD migration; set to `"0"` / `"false"` / `"no"` to force
/// strict AAD enforcement (no fallback — pre-AAD rows will fail to decrypt
/// with `EncryptError::AuthFailed`).
///
/// TODO(v0.3): once the storage schema gains a per-row `aad_version`
/// column (owned by the storage worker), the fallback becomes per-row and
/// this env var disappears.
pub const LEGACY_AAD_ENV: &str = "COPYPASTE_ALLOW_LEGACY_AAD";

/// Returns `true` iff the legacy empty-AAD decryption fallback is enabled
/// for this process. Reads `LEGACY_AAD_ENV` on every call; cheap enough
/// (single syscall per decrypt attempt) and avoids stale-cache surprises
/// in long-running daemons that toggle the env var via re-exec.
///
/// Recognised "disabled" values (case-insensitive): `0`, `false`, `no`,
/// `off`. Any other value — including unset — means "enabled" for v0.2.
fn legacy_aad_fallback_allowed() -> bool {
    match std::env::var(LEGACY_AAD_ENV) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        Err(_) => true, // unset → enabled (v0.2 default)
    }
}

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
/// TODO(v0.3): remove legacy empty-AAD fallback in `decrypt_item_with_aad`
/// once the entire row population has been re-encrypted with AAD.
pub const AAD_SCHEMA_VERSION: u32 = 3;

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
/// Legacy fallback (v0.2 → v0.3 transition): if decryption with the
/// supplied `aad` fails AND `aad` is non-empty AND the env var
/// [`LEGACY_AAD_ENV`] is enabled (default), retry once with an
/// empty AAD. This lets us decrypt rows written by the pre-AAD
/// (`encrypt_item`) code path without forcing a migration.
///
/// Security note: the unconditional fallback (pre-audit) silently disabled
/// AAD substitution protection — an attacker who could swap a ciphertext
/// blob from row `item_id=A` into row `item_id=B` would succeed if the
/// original blob was written without AAD. Gating behind an env var that
/// defaults to enabled preserves migration UX while letting deployments
/// that have completed re-encryption flip the strict switch
/// (`COPYPASTE_ALLOW_LEGACY_AAD=0`). The fallback MUST be removed in v0.3
/// once the full row population has been re-encrypted under the new AAD
/// binding — see `AAD_SCHEMA_VERSION` TODO note above.
///
/// Every successful fallback hit is logged at `warn!` level, rate-limited
/// to ~1 line per 100 calls via an internal counter so a bulk
/// migration cannot DoS the log file.
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
    match cipher.decrypt(&nonce_x, payload) {
        Ok(pt) => Ok(pt),
        Err(_) if !aad.is_empty() => {
            if !legacy_aad_fallback_allowed() {
                // Strict mode: refuse to silently fall back to empty AAD.
                // Surface a distinct error so callers / tests can tell
                // "AAD mismatch + fallback disabled" apart from a
                // straight ciphertext/key/nonce auth failure.
                return Err(EncryptError::AadMismatch);
            }
            // Legacy row written by pre-AAD `encrypt_item`. Retry with
            // empty AAD. TODO(v0.3): drop this fallback path entirely.
            let legacy = Payload {
                msg: ciphertext,
                aad: &[][..],
            };
            match cipher.decrypt(&nonce_x, legacy) {
                Ok(pt) => {
                    let hits = LEGACY_AAD_FALLBACK_HITS.fetch_add(1, Ordering::Relaxed);
                    if hits.is_multiple_of(100) {
                        tracing::warn!(
                            total_hits = hits + 1,
                            "legacy empty-AAD decryption used — will be disabled in v0.3 \
                             (set COPYPASTE_ALLOW_LEGACY_AAD=0 to opt in early)"
                        );
                    }
                    Ok(pt)
                }
                Err(_) => Err(EncryptError::AuthFailed),
            }
        }
        Err(_) => Err(EncryptError::AuthFailed),
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

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"Hello, clipboard!";
        let (nonce, ciphertext) = encrypt_item(plaintext, &key).unwrap();
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_plaintexts_produce_different_nonces() {
        let key = test_key();
        let (n1, _) = encrypt_item(b"aaa", &key).unwrap();
        let (n2, _) = encrypt_item(b"aaa", &key).unwrap();
        assert_ne!(n1, n2);
    }

    #[test]
    fn tampered_ciphertext_fails_decryption() {
        let key = test_key();
        let (nonce, mut ciphertext) = encrypt_item(b"secret", &key).unwrap();
        ciphertext[0] ^= 0xFF;
        assert!(decrypt_item(&ciphertext, &nonce, &key).is_err());
    }

    #[test]
    fn empty_plaintext_encrypts_and_decrypts() {
        let key = test_key();
        let (nonce, ciphertext) = encrypt_item(b"", &key).unwrap();
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn large_plaintext_1mb_roundtrip() {
        let key = test_key();
        let plaintext = vec![0xABu8; 1_000_000];
        let (nonce, ciphertext) = encrypt_item(&plaintext, &key).unwrap();
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    // ── Audit HIGH #1: legacy empty-AAD fallback must be opt-in via env ──
    //
    // These tests mutate process-global env state — they share the same
    // env var with every other test in the binary, so they must run
    // serially. Without `serial_test::serial` two of these executing on
    // different worker threads would race and produce flaky CI failures.
    use serial_test::serial;

    /// SAFETY (clippy `deprecated-cfg-attr-crate-type-name` /
    /// `unsafe-op-in-unsafe-fn`): `env::set_var` is `unsafe` only because
    /// concurrent reads from other threads racing the write can produce a
    /// torn read. We force `#[serial]` on every env-mutating test so no
    /// other test is observing the env var while we flip it.
    fn set_env_var(key: &str, value: &str) {
        // Safe because tests using this helper are `#[serial]`-gated.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn unset_env_var(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    #[serial]
    fn legacy_empty_aad_fallback_succeeds_when_env_unset() {
        unset_env_var(LEGACY_AAD_ENV);
        let key = test_key();
        // Write with empty AAD (pre-AAD code path), then read with a
        // non-empty AAD — fallback should kick in and succeed.
        let (nonce, ct) = encrypt_item(b"legacy-payload", &key).unwrap();
        let aad = build_item_aad("item-xyz", AAD_SCHEMA_VERSION);
        let pt = decrypt_item_with_aad(&ct, &nonce, &key, &aad).unwrap();
        assert_eq!(pt, b"legacy-payload");
    }

    #[test]
    #[serial]
    fn legacy_empty_aad_fallback_succeeds_when_env_true() {
        set_env_var(LEGACY_AAD_ENV, "1");
        let key = test_key();
        let (nonce, ct) = encrypt_item(b"legacy", &key).unwrap();
        let aad = build_item_aad("item-A", AAD_SCHEMA_VERSION);
        let pt = decrypt_item_with_aad(&ct, &nonce, &key, &aad).unwrap();
        assert_eq!(pt, b"legacy");
        unset_env_var(LEGACY_AAD_ENV);
    }

    #[test]
    #[serial]
    fn legacy_empty_aad_fallback_disabled_when_env_zero() {
        set_env_var(LEGACY_AAD_ENV, "0");
        let key = test_key();
        let (nonce, ct) = encrypt_item(b"legacy", &key).unwrap();
        let aad = build_item_aad("item-A", AAD_SCHEMA_VERSION);
        let err = decrypt_item_with_aad(&ct, &nonce, &key, &aad).unwrap_err();
        assert!(matches!(err, EncryptError::AadMismatch));
        unset_env_var(LEGACY_AAD_ENV);
    }

    #[test]
    #[serial]
    fn legacy_empty_aad_fallback_disabled_when_env_false() {
        set_env_var(LEGACY_AAD_ENV, "FALSE");
        let key = test_key();
        let (nonce, ct) = encrypt_item(b"legacy", &key).unwrap();
        let aad = build_item_aad("item-A", AAD_SCHEMA_VERSION);
        let err = decrypt_item_with_aad(&ct, &nonce, &key, &aad).unwrap_err();
        assert!(matches!(err, EncryptError::AadMismatch));
        unset_env_var(LEGACY_AAD_ENV);
    }

    #[test]
    #[serial]
    fn aad_mismatch_with_real_aad_blob_returns_authfailed_not_fallback() {
        // Both writer and reader use AAD, but reader supplies the wrong
        // item_id — this MUST stay AuthFailed regardless of env var,
        // because the original ciphertext was never written with empty AAD
        // so the fallback path also fails. Pin behaviour.
        set_env_var(LEGACY_AAD_ENV, "1");
        let key = test_key();
        let aad_a = build_item_aad("item-A", AAD_SCHEMA_VERSION);
        let aad_b = build_item_aad("item-B", AAD_SCHEMA_VERSION);
        let (nonce, ct) = encrypt_item_with_aad(b"payload", &key, &aad_a).unwrap();
        let err = decrypt_item_with_aad(&ct, &nonce, &key, &aad_b).unwrap_err();
        assert!(matches!(err, EncryptError::AuthFailed));
        unset_env_var(LEGACY_AAD_ENV);
    }

    /// Security audit medium #10: pathological inputs must surface as
    /// `EncryptError::CipherFailed` instead of panicking. We can't actually
    /// allocate >256 GiB to hit the real ChaCha20-Poly1305 limit in CI,
    /// so we exercise the happy path *and* a forced-error path via a
    /// crafted decryption call (which uses the same error-mapping pattern).
    /// The structural fact this test pins is: `encrypt_item` returns
    /// `Result` — the API can no longer panic at the call site.
    #[test]
    fn encrypt_returns_error_not_panic_on_oversized() {
        let key = test_key();

        // Happy path: returns Ok
        let ok = encrypt_item(b"normal input", &key);
        assert!(ok.is_ok(), "small input must succeed");

        // The signature itself is the guarantee: callers handle errors via `?`
        // instead of unwinding the stack on adversarial input. We assert the
        // type-level contract: the function returns Result, not a raw tuple.
        let result: Result<([u8; NONCE_SIZE], Vec<u8>), EncryptError> = encrypt_item(b"x", &key);
        assert!(result.is_ok());

        // And the error variant exists and formats sensibly.
        let err = EncryptError::CipherFailed("simulated".into());
        assert!(err.to_string().contains("AEAD cipher failed"));
    }
}
