//! Passphrase-derived shared sync key for cross-device cloud encryption.
//!
//! Items encrypted at rest use a per-device `local_enc_key` that never
//! leaves the device, so a second device cannot decrypt them. This module
//! provides a SHARED sync key derived deterministically from a user-supplied
//! passphrase so that any device that knows the passphrase can decrypt cloud
//! items produced by any other device with the same passphrase.
//!
//! # Security design
//!
//! We use **Argon2id** (hybrid of Argon2i and Argon2d) as the password-based
//! KDF because it provides both side-channel resistance (data-independent
//! memory access) and brute-force resistance (memory-hard).
//!
//! A **fixed domain-separation salt** (`ARGON2_SYNC_SALT`) is used instead of
//! a per-user random salt. This is intentional: a random per-user salt would
//! make key derivation non-deterministic across devices, breaking cross-device
//! decryptability. The Argon2id work factor (memory cost + time cost) provides
//! the brute-force resistance that a random salt would otherwise supply. The
//! fixed salt's only role here is domain separation — it ensures that the same
//! passphrase used in a different application or context produces a different
//! key.
//!
//! # Cloud ciphertext domain separation
//!
//! Cloud blobs use `CLOUD_AAD_SCHEMA_VERSION = 5`, which is strictly greater
//! than both the local v3 AAD schema and the v4 key-versioned AAD schema. This
//! ensures that a cloud ciphertext can never be silently decrypted as a local
//! ciphertext (and vice-versa) — the AEAD auth tag will reject the wrong AAD.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::encrypt::NONCE_SIZE;

// ─────────────────────────────────────────────────────────────────────────────
// Argon2id parameters
// ─────────────────────────────────────────────────────────────────────────────

/// Argon2id memory cost in kibibytes (19 MiB).
///
/// OWASP recommends a minimum of 19 MiB for interactive logins; we match that
/// floor. Higher values increase brute-force cost at the expense of memory on
/// each device that derives the key (acceptable for a one-time or rare
/// passphrase-entry flow).
pub const ARGON2_M_COST_KIB: u32 = 19_456;

/// Argon2id time cost (number of passes over memory).
///
/// Two passes provides additional mixing compared to a single pass without
/// doubling the total memory cost; this matches the OWASP recommendation for
/// Argon2id at the 19 MiB memory level.
pub const ARGON2_T_COST: u32 = 2;

/// Argon2id parallelism (number of independent lanes).
///
/// Set to 1 for simplicity and cross-platform reproducibility. A higher value
/// would not meaningfully increase security for our use-case and would make
/// the output harder to reproduce on single-threaded environments (e.g.
/// embedded or WASM targets).
pub const ARGON2_P_COST: u32 = 1;

/// Fixed domain-separation salt for sync-key derivation.
///
/// This salt is IDENTICAL across all devices and is NOT secret. Its purpose
/// is domain separation: the same passphrase typed into a different
/// application (or a different `info` context) derives a different key. A
/// random per-user salt is deliberately NOT used here because cross-device
/// key agreement requires both sides to derive the SAME key from the same
/// passphrase — a random salt would need to be transmitted and stored, which
/// undermines the zero-server-trust threat model.
///
/// Brute-force resistance is provided entirely by Argon2id's memory-hard
/// work factor (see `ARGON2_M_COST_KIB` / `ARGON2_T_COST`). The fixed salt
/// is NOT a shortcut around that protection.
///
/// Changing this constant is a hard-fork of all existing cloud ciphertexts —
/// every passphrase will derive a different key and existing blobs will
/// become unreadable. This test pins the value:
/// `ARGON2_SYNC_SALT == SHA-256(b"copypaste/cloud-sync-key/v1/argon2id-salt")`
pub const ARGON2_SYNC_SALT: &[u8; 32] = &[
    // SHA-256("copypaste/cloud-sync-key/v1/argon2id-salt")
    0xe1, 0xb5, 0x69, 0xc8, 0xe9, 0xd3, 0xc8, 0x22, 0xdf, 0x1c, 0xdf, 0x05, 0x09, 0x90, 0x4c, 0x07,
    0xe6, 0x13, 0x10, 0x60, 0x81, 0x43, 0x5d, 0x43, 0xa8, 0xd3, 0x3b, 0x63, 0x08, 0x00, 0x85, 0xbd,
];

// ─────────────────────────────────────────────────────────────────────────────
// AAD schema version for cloud ciphertexts
// ─────────────────────────────────────────────────────────────────────────────

/// AAD schema version used for cloud (sync) ciphertexts.
///
/// This is distinct from the local AAD schema versions (3 and 4) so that a
/// cloud blob can NEVER be silently decrypted as a local blob and vice-versa.
/// The AEAD auth tag will reject any cross-domain substitution attempt.
///
/// | version | usage |
/// |---------|-------|
/// | 3       | local v1-key ciphertexts |
/// | 4       | local v2-key ciphertexts |
/// | 5       | cloud (sync-key) ciphertexts ← this constant |
pub const CLOUD_AAD_SCHEMA_VERSION: u32 = 5;

/// Build the cloud AEAD AAD: `"{item_id}|{CLOUD_AAD_SCHEMA_VERSION}"`.
///
/// The format intentionally mirrors `build_item_aad` so the AAD-building
/// pattern is consistent, but uses a distinct schema-version constant so
/// cloud and local ciphertexts are physically incompatible.
fn build_cloud_aad(item_id: &str) -> Vec<u8> {
    format!("{item_id}|{CLOUD_AAD_SCHEMA_VERSION}").into_bytes()
}

// ─────────────────────────────────────────────────────────────────────────────
// SyncKey newtype
// ─────────────────────────────────────────────────────────────────────────────

/// A 32-byte symmetric key derived from a user passphrase via Argon2id.
///
/// Shared across devices that know the same passphrase; used exclusively for
/// cloud (relay) ciphertext encryption and decryption. Never stored or
/// transmitted directly — always re-derived on demand from the passphrase.
///
/// # Security
/// - Implements `ZeroizeOnDrop`: key bytes are scrubbed when the value is
///   dropped, limiting the window during which the key is in memory.
/// - Does NOT implement `Debug` or `Display` to prevent accidental logging.
/// - Does NOT implement `Clone` or `Copy` to prevent silent duplication.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SyncKey([u8; 32]);

impl SyncKey {
    /// Returns a reference to the inner 32-byte key.
    ///
    /// Intentionally named `as_bytes` rather than making the field `pub` so
    /// callers have to make a deliberate choice to access the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Construct a `SyncKey` from a raw 32-byte array.
    ///
    /// Use this when the caller already holds key bytes that were snapshotted
    /// from a live `SyncKey` (e.g. across a `spawn_blocking` boundary where
    /// `Arc<Mutex<SyncKey>>` cannot be sent).  The bytes are wrapped in a new
    /// `SyncKey` value so all accesses go through the type-safe `as_bytes`
    /// accessor and the bytes are zeroed on drop.
    ///
    /// # Security
    /// The caller is responsible for zeroing the source array after calling
    /// this function if it is no longer needed.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        SyncKey(bytes)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum passphrase length enforced by `derive_sync_key`.
///
/// A passphrase shorter than this is trivially brute-forceable even with
/// Argon2id's memory-hard work factor. Eight characters is a conservative
/// floor that blocks the worst cases (empty string, single char) without
/// being onerous for real users.
pub const MIN_PASSPHRASE_LEN: usize = 8;

/// Errors returned by sync-key derivation and cloud encrypt/decrypt.
#[derive(Debug, Error)]
pub enum SyncKeyError {
    /// Argon2id parameter configuration was rejected (should never happen with
    /// the hardcoded constants; surfaced as a non-panic error rather than
    /// `unwrap` per project convention).
    #[error("Argon2 parameter error: {0}")]
    Argon2Params(String),

    /// Argon2id hashing failed (out of memory or other runtime error).
    #[error("Argon2 hashing failed: {0}")]
    Argon2Hash(String),

    /// AEAD encryption failed (e.g. plaintext exceeds per-message limit).
    #[error("Cloud encryption failed: {0}")]
    EncryptFailed(String),

    /// AEAD decryption failed — ciphertext, nonce, key, or AAD is wrong.
    /// Indistinguishable from a tampered blob to prevent oracle attacks.
    #[error("Cloud decryption failed: authentication tag mismatch")]
    DecryptFailed,

    /// The cloud blob is shorter than the minimum required length
    /// (24-byte nonce prefix).
    #[error("Cloud blob too short: expected at least {NONCE_SIZE} bytes, got {0}")]
    BlobTooShort(usize),

    /// The passphrase is shorter than `MIN_PASSPHRASE_LEN` characters.
    /// Short passphrases are trivially brute-forceable; the IPC layer
    /// should surface this as a user-actionable validation error.
    #[error("passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters, got {0}")]
    PassphraseTooShort(usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Derive a 32-byte shared sync key from `passphrase` using Argon2id.
///
/// The derivation is **deterministic**: the same passphrase always produces
/// the same key on every device. This is the cross-device agreement property
/// required for cloud sync. See the module-level documentation for the
/// security rationale for the fixed salt.
///
/// # Errors
/// Returns `SyncKeyError::PassphraseTooShort` if `passphrase` is shorter than
/// `MIN_PASSPHRASE_LEN` characters (prevents trivial brute force).
/// Returns `SyncKeyError::Argon2Params` if the Argon2id parameter struct
/// cannot be built (only possible if the hardcoded constants are invalid),
/// or `SyncKeyError::Argon2Hash` if the hashing operation fails at runtime.
pub fn derive_sync_key(passphrase: &str) -> Result<SyncKey, SyncKeyError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    // Fix [HIGH]: reject trivially short passphrases before hashing so the
    // IPC layer can surface a user-actionable error rather than silently
    // accepting a 0- or 1-character passphrase that is brute-forceable.
    if passphrase.chars().count() < MIN_PASSPHRASE_LEN {
        return Err(SyncKeyError::PassphraseTooShort(passphrase.chars().count()));
    }

    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| SyncKeyError::Argon2Params(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key_bytes = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), ARGON2_SYNC_SALT, &mut key_bytes)
        .map_err(|e| SyncKeyError::Argon2Hash(e.to_string()))?;

    Ok(SyncKey(key_bytes))
}

/// Encrypt `plaintext` for cloud storage using `key`, binding the ciphertext
/// to `item_id` via AEAD AAD.
///
/// Output format: `nonce[24] || ciphertext_with_tag`.
///
/// A fresh 24-byte nonce is generated for every call — callers MUST NOT
/// reuse this function's output as the canonical nonce for a second call.
///
/// The AAD encodes `(item_id, CLOUD_AAD_SCHEMA_VERSION)` so the ciphertext
/// is domain-separated from local ciphertexts and is bound to a specific
/// item identity. Substituting the blob into a different item's slot or
/// decrypting with a local key will be rejected by the auth tag.
///
/// # Errors
/// Returns `SyncKeyError::EncryptFailed` if the AEAD layer rejects the
/// input (e.g. plaintext exceeds the ~256 GiB per-message limit).
pub fn encrypt_for_cloud(
    key: &SyncKey,
    item_id: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, SyncKeyError> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    // Safety: OsRng is infallible on all supported targets; if the OS RNG
    // fails the process is in an unrecoverable state regardless.
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);

    let aad = build_cloud_aad(item_id);
    let payload = Payload {
        msg: plaintext,
        aad: &aad,
    };

    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|e| SyncKeyError::EncryptFailed(e.to_string()))?;

    // Output: nonce || ciphertext+tag
    let mut blob = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

/// Decrypt a cloud blob produced by `encrypt_for_cloud`.
///
/// `blob` must be at least `NONCE_SIZE` (24) bytes long. The first 24 bytes
/// are interpreted as the XChaCha20-Poly1305 nonce; the remainder is the
/// authenticated ciphertext.
///
/// # Errors
/// - `SyncKeyError::BlobTooShort` — blob is shorter than 24 bytes.
/// - `SyncKeyError::DecryptFailed` — wrong key, wrong `item_id` AAD,
///   or tampered ciphertext/nonce.
pub fn decrypt_from_cloud(
    key: &SyncKey,
    item_id: &str,
    blob: &[u8],
) -> Result<Vec<u8>, SyncKeyError> {
    if blob.len() < NONCE_SIZE {
        return Err(SyncKeyError::BlobTooShort(blob.len()));
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_SIZE);
    // SAFETY: `split_at(NONCE_SIZE)` guarantees `nonce_bytes.len() == NONCE_SIZE == 24`.
    // `XNonce::from_slice` is the idiomatic infallible constructor for a known-length
    // slice; it panics only when the length differs from 24, which is structurally
    // impossible here. Prefer it over `try_into().expect()` to avoid the bare `expect`.
    let nonce = *XNonce::from_slice(nonce_bytes);

    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let aad = build_cloud_aad(item_id);
    let payload = Payload {
        msg: ciphertext,
        aad: &aad,
    };

    cipher
        .decrypt(&nonce, payload)
        .map_err(|_| SyncKeyError::DecryptFailed)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_key(passphrase: &str) -> SyncKey {
        derive_sync_key(passphrase).expect("derive_sync_key must succeed with valid passphrase")
    }

    // ── golden-byte: ARGON2_SYNC_SALT ────────────────────────────────────────

    /// ARGON2_SYNC_SALT must equal SHA-256(b"copypaste/cloud-sync-key/v1/argon2id-salt").
    /// Changing the constant is a hard-fork of all cloud ciphertexts; this test
    /// makes that a deliberate, visible act.
    #[test]
    fn argon2_sync_salt_is_sha256_of_canonical_input() {
        let expected = Sha256::digest(b"copypaste/cloud-sync-key/v1/argon2id-salt");
        assert_eq!(
            ARGON2_SYNC_SALT.as_ref(),
            expected.as_slice(),
            "ARGON2_SYNC_SALT must equal SHA-256(b\"copypaste/cloud-sync-key/v1/argon2id-salt\")"
        );
    }

    // ── derive_sync_key ──────────────────────────────────────────────────────

    /// Same passphrase must yield the same key on every call (cross-device
    /// agreement depends on this property).
    #[test]
    fn derive_sync_key_is_deterministic() {
        let k1 = make_key("correct horse battery staple");
        let k2 = make_key("correct horse battery staple");
        assert_eq!(k1.as_bytes(), k2.as_bytes());
    }

    /// Different passphrases must produce different keys.
    #[test]
    fn derive_sync_key_different_passphrases_produce_different_keys() {
        let k1 = make_key("passphrase-alpha");
        let k2 = make_key("passphrase-beta");
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    // ── round-trip ───────────────────────────────────────────────────────────

    /// Encrypt then decrypt with the SAME key and item_id must return the
    /// original plaintext.
    #[test]
    fn cloud_roundtrip_same_key_and_item_id() {
        let key = make_key("shared-secret");
        let item_id = "item-cloud-001";
        let plaintext = b"hello from the cloud";

        let blob = encrypt_for_cloud(&key, item_id, plaintext).unwrap();
        let recovered = decrypt_from_cloud(&key, item_id, &blob).unwrap();
        assert_eq!(recovered, plaintext);
    }

    /// Empty plaintext must also round-trip correctly.
    #[test]
    fn cloud_roundtrip_empty_plaintext() {
        let key = make_key("empty-test");
        let blob = encrypt_for_cloud(&key, "item-empty", b"").unwrap();
        let recovered = decrypt_from_cloud(&key, "item-empty", &blob).unwrap();
        assert_eq!(recovered, b"");
    }

    // ── wrong passphrase → decrypt fails ────────────────────────────────────

    /// Decrypting with a key derived from a different passphrase must fail.
    #[test]
    fn wrong_passphrase_decrypt_fails() {
        let key_enc = make_key("correct-passphrase");
        let key_dec = make_key("wrong-passphrase");
        let blob = encrypt_for_cloud(&key_enc, "item-x", b"secret data").unwrap();
        let result = decrypt_from_cloud(&key_dec, "item-x", &blob);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "wrong passphrase must produce DecryptFailed, got {:?}",
            result
        );
    }

    // ── tampered ciphertext fails ────────────────────────────────────────────

    /// Flipping a bit in the ciphertext body must cause auth-tag failure.
    #[test]
    fn tampered_ciphertext_fails() {
        let key = make_key("tamper-test");
        let mut blob = encrypt_for_cloud(&key, "item-tamper", b"important data").unwrap();
        // Flip a byte in the ciphertext portion (after the 24-byte nonce).
        blob[NONCE_SIZE] ^= 0xFF;
        let result = decrypt_from_cloud(&key, "item-tamper", &blob);
        assert!(matches!(result, Err(SyncKeyError::DecryptFailed)));
    }

    // ── wrong item_id AAD fails ──────────────────────────────────────────────

    /// Decrypting with a different item_id must fail (AAD mismatch).
    #[test]
    fn wrong_item_id_aad_fails() {
        let key = make_key("aad-test");
        let blob = encrypt_for_cloud(&key, "item-correct", b"payload").unwrap();
        let result = decrypt_from_cloud(&key, "item-wrong", &blob);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "wrong item_id in AAD must produce DecryptFailed"
        );
    }

    // ── nonce uniqueness ─────────────────────────────────────────────────────

    /// Two encryptions of the same plaintext with the same key must produce
    /// different nonces (and therefore different blobs).
    #[test]
    fn nonce_unique_across_two_encrypts() {
        let key = make_key("nonce-test");
        let item_id = "item-nonce";
        let plaintext = b"same plaintext";

        let blob1 = encrypt_for_cloud(&key, item_id, plaintext).unwrap();
        let blob2 = encrypt_for_cloud(&key, item_id, plaintext).unwrap();

        // Nonce is the first NONCE_SIZE bytes of the blob.
        assert_ne!(
            &blob1[..NONCE_SIZE],
            &blob2[..NONCE_SIZE],
            "two encrypts must use different nonces"
        );
        // The full blobs must differ too (nonces are embedded in the output).
        assert_ne!(blob1, blob2);
    }

    // ── blob format ──────────────────────────────────────────────────────────

    /// Cloud blob must start with exactly NONCE_SIZE bytes followed by
    /// ciphertext+tag (plaintext.len() + 16).
    #[test]
    fn blob_format_nonce_then_ciphertext_plus_tag() {
        let key = make_key("format-test");
        let plaintext = b"format check";
        let blob = encrypt_for_cloud(&key, "item-fmt", plaintext).unwrap();
        // blob length must be nonce(24) + plaintext(N) + tag(16)
        assert_eq!(blob.len(), NONCE_SIZE + plaintext.len() + 16);
    }

    // ── blob too short ───────────────────────────────────────────────────────

    /// A blob shorter than NONCE_SIZE must return BlobTooShort, not panic.
    #[test]
    fn blob_too_short_returns_error_not_panic() {
        let key = make_key("short-test");
        let short_blob = [0u8; 10];
        let result = decrypt_from_cloud(&key, "item-short", &short_blob);
        assert!(
            matches!(result, Err(SyncKeyError::BlobTooShort(10))),
            "expected BlobTooShort(10), got {:?}",
            result
        );
    }

    // ── cloud domain separation from local ───────────────────────────────────

    /// The cloud AAD schema version must be strictly greater than any local
    /// schema version (3 and 4) so cloud and local ciphertexts cannot collide.
    #[test]
    fn cloud_aad_schema_version_is_5() {
        assert_eq!(CLOUD_AAD_SCHEMA_VERSION, 5);
    }

    /// Cloud AAD bytes match the expected format.
    #[test]
    fn build_cloud_aad_format() {
        let aad = build_cloud_aad("item-abc");
        assert_eq!(aad, b"item-abc|5");
    }

    // ── parameter constants ──────────────────────────────────────────────────

    #[test]
    fn argon2_params_are_expected_values() {
        assert_eq!(ARGON2_M_COST_KIB, 19_456);
        assert_eq!(ARGON2_T_COST, 2);
        assert_eq!(ARGON2_P_COST, 1);
    }

    // ── SyncKey::from_bytes round-trip ───────────────────────────────────────

    /// A blob encrypted with a `SyncKey` produced by `derive_sync_key` must
    /// decrypt successfully using a `SyncKey` reconstructed from the same raw
    /// bytes via `from_bytes`. This is the code path used by the cloud download
    /// worker which snapshots the key bytes before crossing a `spawn_blocking`
    /// boundary.
    #[test]
    fn from_bytes_decrypts_blob_encrypted_by_derive_sync_key() {
        let passphrase = "correct-horse-battery-staple";
        let item_id = "round-trip-item-001";
        let plaintext = b"clipboard content for cloud sync round-trip";

        // Derive a key from the passphrase and encrypt.
        let original_key = derive_sync_key(passphrase).expect("derive must succeed");
        // Wrap in Zeroizing so the stack copy is scrubbed when it goes out of
        // scope, closing the window where raw key bytes sit on the stack unguarded.
        let key_bytes = zeroize::Zeroizing::new(*original_key.as_bytes());
        let blob =
            encrypt_for_cloud(&original_key, item_id, plaintext).expect("encrypt must succeed");

        // Reconstruct a SyncKey from the raw bytes (simulates the spawn_blocking
        // snapshot path) and verify the blob decrypts to the original plaintext.
        let reconstructed_key = SyncKey::from_bytes(*key_bytes);
        let decrypted = decrypt_from_cloud(&reconstructed_key, item_id, &blob)
            .expect("decrypt with from_bytes key must succeed");

        assert_eq!(
            decrypted, plaintext,
            "decrypted plaintext must match the original"
        );
    }

    /// `from_bytes` with the wrong key bytes must produce `DecryptFailed`, not
    /// a panic or incorrect plaintext.
    #[test]
    fn from_bytes_wrong_key_returns_decrypt_failed() {
        let key = derive_sync_key("correct-passphrase").expect("derive must succeed");
        let blob =
            encrypt_for_cloud(&key, "item-fb-wrong", b"secret").expect("encrypt must succeed");

        // Construct a key from all-zero bytes — should not decrypt correctly.
        let wrong_key = SyncKey::from_bytes([0u8; 32]);
        let result = decrypt_from_cloud(&wrong_key, "item-fb-wrong", &blob);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "wrong key via from_bytes must return DecryptFailed, got {:?}",
            result
        );
    }

    // ── passphrase length enforcement ────────────────────────────────────────

    /// Fix [HIGH]: passphrases shorter than MIN_PASSPHRASE_LEN must be rejected
    /// with `PassphraseTooShort` before Argon2id even runs.
    #[test]
    fn short_passphrase_returns_passphrase_too_short() {
        for short in &["", "a", "1234567"] {
            let result = derive_sync_key(short);
            assert!(
                matches!(result, Err(SyncKeyError::PassphraseTooShort(_))),
                "passphrase {:?} (len {}) must produce PassphraseTooShort",
                short,
                short.chars().count(),
            );
        }
    }

    /// A passphrase of exactly MIN_PASSPHRASE_LEN characters must succeed.
    #[test]
    fn passphrase_at_min_length_succeeds() {
        // "12345678" is exactly 8 chars — must not return PassphraseTooShort.
        assert!(
            derive_sync_key("12345678").is_ok(),
            "passphrase of exactly {MIN_PASSPHRASE_LEN} chars must succeed"
        );
    }

    /// The PassphraseTooShort error carries the actual char count.
    #[test]
    fn passphrase_too_short_error_contains_length() {
        match derive_sync_key("abc") {
            Err(SyncKeyError::PassphraseTooShort(n)) => assert_eq!(n, 3),
            _ => panic!("expected PassphraseTooShort(3), got Err variant or Ok"),
        }
    }
}
