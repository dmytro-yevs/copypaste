//! Passphrase-derived shared sync key for cross-device cloud encryption.
//!
//! Items encrypted at rest use a per-device `local_enc_key` that never
//! leaves the device, so a second device cannot decrypt them. This module
//! provides a SHARED sync key derived deterministically from a user-supplied
//! passphrase **and** the account's stable identifier so that any device that
//! knows the passphrase (and belongs to the same account) can decrypt cloud
//! items produced by any other device of that account.
//!
//! # Security design
//!
//! We use **Argon2id** (hybrid of Argon2i and Argon2d) as the password-based
//! KDF because it provides both side-channel resistance (data-independent
//! memory access) and brute-force resistance (memory-hard).
//!
//! ## The salt is per-account, never one global constant
//!
//! Argon2id's work factor raises the cost of *each individual* guess, but it
//! does **not** stop an attacker from *amortising* one precomputed dictionary
//! across many victims. With a single global salt an attacker who scrapes the
//! untrusted cloud's E2E ciphertext corpus could run a passphrase dictionary
//! through Argon2id **once** and try every resulting key against **every**
//! user's blobs — a classic precompute / rainbow-table amortisation that a
//! unique salt is specifically designed to defeat.
//!
//! A random per-user salt would defeat precompute but breaks cloud sync: two
//! devices that only share a passphrase could not agree on the same key without
//! transmitting and storing the salt. The resolution is a **deterministic
//! per-account salt**: we mix a stable account identifier (the Supabase
//! `user_id`, which both devices of an account already know) into the Argon2id
//! salt via HKDF. This keeps derivation **deterministic across the two devices
//! of the same account** (cloud sync still works) while making the salt
//! **unique per account**, so an attacker must restart the entire memory-hard
//! dictionary for every account — cross-user amortisation is gone.
//!
//! This is the **only** sync-key derivation. There is no global-salt variant,
//! no version dispatch, and no trial decryption: every cloud blob is encrypted
//! and decrypted under the one per-account key, so [`derive_sync_key`] always
//! requires a non-empty `account_id`.
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

/// Fixed input keying material (IKM) for the per-account Argon2id salt.
///
/// This constant is IDENTICAL across all devices and is NOT secret — it is only
/// the HKDF IKM from which `derive_per_account_salt` expands a unique salt per
/// account. It provides domain separation so the per-account salt can never
/// collide with any other HKDF use that shares `sha2`/`hkdf` (relay inbox,
/// storage, telemetry). The uniqueness that defeats cross-user precompute comes
/// from mixing the account id into the HKDF `info`, not from this constant.
///
/// The value is pinned via its SHA-256 preimage (see the golden test below):
/// `PER_ACCOUNT_SALT_IKM == SHA-256(b"copypaste/cloud-sync-key/per-account-salt-ikm")`.
/// Changing it is a hard-fork of every existing cloud ciphertext — every
/// passphrase would derive a different key — so the test makes any change a
/// deliberate, visible act.
pub const PER_ACCOUNT_SALT_IKM: &[u8; 32] = &[
    // SHA-256("copypaste/cloud-sync-key/per-account-salt-ikm")
    0x7c, 0xac, 0x08, 0xf5, 0x29, 0x43, 0x41, 0x2f, 0x83, 0x07, 0xa7, 0x2f, 0x21, 0x19, 0x2c, 0x95,
    0x46, 0xb3, 0xfe, 0x01, 0x3a, 0x69, 0x25, 0xb4, 0x1d, 0xd0, 0x12, 0x32, 0x33, 0x7a, 0xfe, 0x8a,
];

/// HKDF `info` string for the per-account Argon2id salt.
///
/// Purpose-separated (per the project's HKDF-info convention) so this salt can
/// never collide with the relay-inbox / storage / telemetry HKDF derivations
/// that share `sha2`/`hkdf`. The account id is appended to this prefix.
const SYNC_SALT_INFO: &[u8] = b"copypaste/cloud-sync-key/per-account-salt|";

/// Derive the **deterministic** per-account Argon2id salt.
///
/// `salt = HKDF-SHA256(ikm = PER_ACCOUNT_SALT_IKM, salt = None, info =
/// SYNC_SALT_INFO || account_id)`.
///
/// Properties:
/// - **Deterministic**: depends only on the (public) account id and the fixed
///   IKM, so both devices of the same account derive the identical salt and
///   therefore the identical sync key — cloud sync determinism is preserved.
/// - **Per-account**: two different account ids yield independent salts, so an
///   attacker cannot reuse one Argon2id precompute across accounts.
///
/// The account id is NOT secret (it is the Supabase `user_id`); it is mixed in
/// purely as a *salt*, not as key material. Using HKDF rather than raw
/// concatenation gives a fixed-length, well-distributed 32-byte salt regardless
/// of the account id's length or structure.
fn derive_per_account_salt(account_id: &str) -> [u8; 32] {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hk = Hkdf::<Sha256>::new(None, PER_ACCOUNT_SALT_IKM);
    let mut info = Vec::with_capacity(SYNC_SALT_INFO.len() + account_id.len());
    info.extend_from_slice(SYNC_SALT_INFO);
    info.extend_from_slice(account_id.as_bytes());

    let mut salt = [0u8; 32];
    // HKDF-SHA256 expand of 32 bytes (< 255*32) cannot fail; the only error
    // variant is "output too long". Surfaced as an unreachable expect with a
    // structural justification rather than propagated, matching the relay HKDF
    // call sites in `relay.rs`.
    hk.expand(&info, &mut salt)
        .expect("HKDF-SHA256 expand of 32 bytes is always valid");
    salt
}

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
/// Shared across devices of the same account that know the same passphrase; used
/// exclusively for cloud (relay) ciphertext encryption and decryption. Never
/// stored or transmitted directly — always re-derived on demand from the
/// passphrase + account id.
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

    /// Generate a fresh 32-byte key from the OS CSPRNG.
    ///
    /// Unlike [`derive_sync_key`], this key is NOT reproducible from a
    /// passphrase — callers must distribute the raw bytes to remaining paired
    /// devices via the existing pairing re-provision flow (QR re-scan or
    /// explicit key-share). This is the preferred path for automatic revocation
    /// (`revoke_peer` with an active cloud/relay backend) because it requires
    /// NO passphrase entry from the user.
    ///
    /// The key is filled by `OsRng` (which maps to `getrandom` / the OS
    /// entropy pool) so it is cryptographically unpredictable and unique.
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        SyncKey(bytes)
    }

    /// Constant-time comparison of this key's bytes against a candidate
    /// 32-byte key.
    ///
    /// Used by the provisioning-apply path to distinguish a ROUTINE pairing
    /// re-provision (incoming key identical → no-op) from a ROTATION
    /// re-provision (incoming key differs → replace). Per the project's
    /// security constraints, secret key material must never be compared with
    /// `==`; this uses `subtle::ConstantTimeEq` so the comparison does not leak
    /// information via timing.
    pub fn ct_eq_bytes(&self, other: &[u8; 32]) -> bool {
        use subtle::ConstantTimeEq;
        self.0.ct_eq(other).into()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum passphrase length enforced by the sync-key derivation entry point.
///
/// A passphrase shorter than this is brute-forceable even with Argon2id's
/// memory-hard work factor. Twelve characters raises the dictionary-attack
/// floor materially (the OWASP/NIST guidance for human-chosen secrets) while
/// staying enterable for a passphrase-style secret. Raising this only validates
/// NEW passphrase entry; it does not invalidate already-derived stored keys.
pub const MIN_PASSPHRASE_LEN: usize = 12;

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

    /// The account id was empty. The per-account salt requires a stable,
    /// non-empty account identifier (the Supabase `user_id`); an empty value
    /// would collapse the salt toward a shared constant and reintroduce the
    /// cross-user precompute weakness, so it is rejected loudly rather than
    /// silently producing a degenerate key.
    #[error("account id must not be empty for the per-account sync-key derivation")]
    EmptyAccountId,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Run Argon2id over `passphrase` with an explicit 32-byte `salt`.
///
/// Shared core of the public derivation entry point so the Argon2id
/// parameters, the length guard, and the zeroize-on-drop handling live in one
/// place.
fn derive_sync_key_with_salt(passphrase: &str, salt: &[u8; 32]) -> Result<SyncKey, SyncKeyError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    // Reject trivially short passphrases before hashing so the IPC layer can
    // surface a user-actionable error rather than silently accepting a short,
    // brute-forceable passphrase.
    if passphrase.chars().count() < MIN_PASSPHRASE_LEN {
        return Err(SyncKeyError::PassphraseTooShort(passphrase.chars().count()));
    }

    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| SyncKeyError::Argon2Params(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Wrap the derived-key stack buffer in Zeroizing so the bytes are scrubbed
    // on drop even if move semantics leave a stale copy behind (the SyncKey we
    // return owns its own copy and is itself ZeroizeOnDrop).
    let mut key_bytes = zeroize::Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key_bytes[..])
        .map_err(|e| SyncKeyError::Argon2Hash(e.to_string()))?;

    Ok(SyncKey(*key_bytes))
}

/// Derive the 32-byte shared sync key from `passphrase` and `account_id`.
///
/// This is the **single** sync-key derivation. It feeds Argon2id a deterministic
/// per-account salt derived from `account_id` (see `derive_per_account_salt`).
/// Because the salt is unique per account, an attacker who scrapes the untrusted
/// cloud's E2E ciphertext corpus cannot reuse a single Argon2id dictionary
/// precompute across accounts. Because the salt is deterministic, both devices
/// of the same account derive the identical key, so cloud sync still works.
///
/// `account_id` must be a STABLE identifier that BOTH devices of the account
/// agree on (the canonical `copypaste_supabase::supabase_account_id`, i.e.
/// `"<project_ref>|<user_id>"`). It is not secret — it is mixed in only as a
/// salt — but it must be identical on both devices or they will not agree on a
/// key, and it must be non-empty.
///
/// # Errors
/// - `SyncKeyError::EmptyAccountId` if `account_id` is empty.
/// - `SyncKeyError::PassphraseTooShort` if `passphrase` is shorter than
///   `MIN_PASSPHRASE_LEN` characters (prevents trivial brute force).
/// - `SyncKeyError::Argon2Params` if the Argon2id parameter struct cannot be
///   built (only possible if the hardcoded constants are invalid), or
///   `SyncKeyError::Argon2Hash` if the hashing operation fails at runtime.
pub fn derive_sync_key(passphrase: &str, account_id: &str) -> Result<SyncKey, SyncKeyError> {
    if account_id.is_empty() {
        return Err(SyncKeyError::EmptyAccountId);
    }
    let salt = derive_per_account_salt(account_id);
    derive_sync_key_with_salt(passphrase, &salt)
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

    // ── shared fixtures ──────────────────────────────────────────────────────

    const ACCOUNT_A: &str = "proj_abc|00000000-0000-0000-0000-0000000000aa";
    const ACCOUNT_B: &str = "proj_abc|00000000-0000-0000-0000-0000000000bb";
    const PASS: &str = "correct horse battery staple";

    fn make_key(passphrase: &str) -> SyncKey {
        derive_sync_key(passphrase, ACCOUNT_A).expect("derive_sync_key must succeed")
    }

    // ── golden-byte: PER_ACCOUNT_SALT_IKM ────────────────────────────────────

    /// PER_ACCOUNT_SALT_IKM must equal SHA-256(b"copypaste/cloud-sync-key/per-account-salt-ikm").
    /// Changing the constant is a hard-fork of all cloud ciphertexts; this test
    /// makes that a deliberate, visible act.
    #[test]
    fn per_account_salt_ikm_is_sha256_of_canonical_input() {
        let expected = Sha256::digest(b"copypaste/cloud-sync-key/per-account-salt-ikm");
        assert_eq!(
            PER_ACCOUNT_SALT_IKM.as_ref(),
            expected.as_slice(),
            "PER_ACCOUNT_SALT_IKM must equal SHA-256(b\"copypaste/cloud-sync-key/per-account-salt-ikm\")"
        );
    }

    // ── derive_sync_key ──────────────────────────────────────────────────────

    /// Same passphrase + same account must yield the same key on every call
    /// (cross-device agreement depends on this property).
    #[test]
    fn same_account_same_passphrase_is_deterministic() {
        let k1 = derive_sync_key(PASS, ACCOUNT_A).expect("derive 1");
        let k2 = derive_sync_key(PASS, ACCOUNT_A).expect("derive 2");
        assert_eq!(
            k1.as_bytes(),
            k2.as_bytes(),
            "same account + passphrase must be deterministic across devices"
        );
    }

    /// Different passphrases (same account) must produce different keys.
    #[test]
    fn different_passphrases_produce_different_keys() {
        let k1 = derive_sync_key("passphrase-alpha-xx", ACCOUNT_A).expect("derive 1");
        let k2 = derive_sync_key("passphrase-beta-xxx", ACCOUNT_A).expect("derive 2");
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    /// Two DIFFERENT account ids with the SAME passphrase must derive DIFFERENT
    /// keys — this is the property that defeats cross-user precompute.
    #[test]
    fn different_accounts_same_passphrase_derive_different_keys() {
        let key_a = derive_sync_key(PASS, ACCOUNT_A).expect("derive A");
        let key_b = derive_sync_key(PASS, ACCOUNT_B).expect("derive B");
        assert_ne!(
            key_a.as_bytes(),
            key_b.as_bytes(),
            "different accounts must not share a key (cross-user precompute would survive)"
        );
        // And a blob from account A must NOT decrypt under account B's key.
        let blob = encrypt_for_cloud(&key_a, "x", b"secret").expect("encrypt");
        assert!(
            matches!(
                decrypt_from_cloud(&key_b, "x", &blob),
                Err(SyncKeyError::DecryptFailed)
            ),
            "account B's key must not decrypt account A's blob"
        );
    }

    /// An empty account id is rejected with a clear, dedicated error — never a
    /// silently-degenerate key.
    #[test]
    fn empty_account_id_is_rejected() {
        assert!(matches!(
            derive_sync_key(PASS, ""),
            Err(SyncKeyError::EmptyAccountId)
        ));
    }

    // ── round-trip ───────────────────────────────────────────────────────────

    /// Encrypt then decrypt with the SAME key and item_id must return the
    /// original plaintext.
    #[test]
    fn cloud_roundtrip_same_key_and_item_id() {
        let key = make_key(PASS);
        let item_id = "item-cloud-001";
        let plaintext = b"hello from the cloud";

        let blob = encrypt_for_cloud(&key, item_id, plaintext).unwrap();
        let recovered = decrypt_from_cloud(&key, item_id, &blob).unwrap();
        assert_eq!(recovered, plaintext);
    }

    /// Empty plaintext must also round-trip correctly.
    #[test]
    fn cloud_roundtrip_empty_plaintext() {
        let key = make_key(PASS);
        let blob = encrypt_for_cloud(&key, "item-empty", b"").unwrap();
        let recovered = decrypt_from_cloud(&key, "item-empty", &blob).unwrap();
        assert_eq!(recovered, b"");
    }

    // ── wrong passphrase → decrypt fails ────────────────────────────────────

    /// Decrypting with a key derived from a different passphrase must fail.
    #[test]
    fn wrong_passphrase_decrypt_fails() {
        let key_enc = make_key("correct-passphrase-a");
        let key_dec = make_key("wrong-passphrase-bbb");
        let blob = encrypt_for_cloud(&key_enc, "item-x", b"secret data").unwrap();
        let result = decrypt_from_cloud(&key_dec, "item-x", &blob);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "wrong passphrase must produce DecryptFailed, got {result:?}"
        );
    }

    // ── tampered ciphertext fails ────────────────────────────────────────────

    /// Flipping a bit in the ciphertext body must cause auth-tag failure.
    #[test]
    fn tampered_ciphertext_fails() {
        let key = make_key(PASS);
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
        let key = make_key(PASS);
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
        let key = make_key(PASS);
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
        let key = make_key(PASS);
        let plaintext = b"format check";
        let blob = encrypt_for_cloud(&key, "item-fmt", plaintext).unwrap();
        // blob length must be nonce(24) + plaintext(N) + tag(16)
        assert_eq!(blob.len(), NONCE_SIZE + plaintext.len() + 16);
    }

    // ── blob too short ───────────────────────────────────────────────────────

    /// A blob shorter than NONCE_SIZE must return BlobTooShort, not panic.
    #[test]
    fn blob_too_short_returns_error_not_panic() {
        let key = make_key(PASS);
        let short_blob = [0u8; 10];
        let result = decrypt_from_cloud(&key, "item-short", &short_blob);
        assert!(
            matches!(result, Err(SyncKeyError::BlobTooShort(10))),
            "expected BlobTooShort(10), got {result:?}"
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
        let item_id = "round-trip-item-001";
        let plaintext = b"clipboard content for cloud sync round-trip";

        // Derive a key from the passphrase + account and encrypt.
        let original_key = derive_sync_key(PASS, ACCOUNT_A).expect("derive must succeed");
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
        let key = derive_sync_key(PASS, ACCOUNT_A).expect("derive must succeed");
        let blob =
            encrypt_for_cloud(&key, "item-fb-wrong", b"secret").expect("encrypt must succeed");

        // Construct a key from all-zero bytes — should not decrypt correctly.
        let wrong_key = SyncKey::from_bytes([0u8; 32]);
        let result = decrypt_from_cloud(&wrong_key, "item-fb-wrong", &blob);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "wrong key via from_bytes must return DecryptFailed, got {result:?}"
        );
    }

    // ── passphrase length enforcement ────────────────────────────────────────

    /// Passphrases shorter than MIN_PASSPHRASE_LEN must be rejected with
    /// `PassphraseTooShort` before Argon2id even runs. Includes an 11-char case
    /// to pin the floor (MIN_PASSPHRASE_LEN == 12).
    #[test]
    fn short_passphrase_returns_passphrase_too_short() {
        for short in &["", "a", "1234567", "12345678901"] {
            assert!(
                short.chars().count() < MIN_PASSPHRASE_LEN,
                "test fixture {short:?} must be shorter than the enforced minimum"
            );
            let result = derive_sync_key(short, ACCOUNT_A);
            assert!(
                matches!(result, Err(SyncKeyError::PassphraseTooShort(_))),
                "passphrase {:?} (len {}) must produce PassphraseTooShort",
                short,
                short.chars().count(),
            );
        }
    }

    /// The enforced minimum is 12 characters.
    #[test]
    fn min_passphrase_len_is_twelve() {
        assert_eq!(MIN_PASSPHRASE_LEN, 12);
    }

    /// A passphrase of exactly MIN_PASSPHRASE_LEN characters must succeed, and
    /// one character shorter must be rejected.
    #[test]
    fn passphrase_at_min_length_succeeds() {
        // "123456789012" is exactly 12 chars — must not return PassphraseTooShort.
        assert!(
            derive_sync_key("123456789012", ACCOUNT_A).is_ok(),
            "passphrase of exactly {MIN_PASSPHRASE_LEN} chars must succeed"
        );
        // 11 chars — one short of the floor — must be rejected.
        assert!(
            matches!(
                derive_sync_key("12345678901", ACCOUNT_A),
                Err(SyncKeyError::PassphraseTooShort(11))
            ),
            "passphrase one char below the floor must be rejected"
        );
    }

    /// The PassphraseTooShort error carries the actual char count.
    #[test]
    fn passphrase_too_short_error_contains_length() {
        match derive_sync_key("abc", ACCOUNT_A) {
            Err(SyncKeyError::PassphraseTooShort(n)) => assert_eq!(n, 3),
            // Note: `SyncKey` has no `Debug`, so don't format the matched value.
            _ => panic!("expected PassphraseTooShort(3), got a different Err variant or Ok"),
        }
    }

    // ── C-P0-4: sync-key rotation framing ────────────────────────────────────

    /// Rotating the sync key is the ONLY real cloud/relay device revocation: a
    /// blob encrypted under key A (held by a now-revoked device) must FAIL to
    /// decrypt under the rotated key B. This proves the revoked device can no
    /// longer read items produced AFTER the rotation, even though it still holds
    /// key A.
    #[test]
    fn rotated_key_cannot_decrypt_pre_rotation_blob() {
        let item_id = "rotation-item-001";
        let plaintext = b"secret produced after rotation";

        // Key A = the pre-rotation key the revoked device still holds.
        let key_a = derive_sync_key("old-shared-passphrase", ACCOUNT_A).expect("derive A");
        // Key B = the rotated key. A different passphrase yields different bytes.
        let key_b = derive_sync_key("new-rotated-passphrase", ACCOUNT_A).expect("derive B");
        assert_ne!(
            key_a.as_bytes(),
            key_b.as_bytes(),
            "rotation must produce a distinct key"
        );

        // A NEW cloud item is encrypted under the rotated key B.
        let blob_b = encrypt_for_cloud(&key_b, item_id, plaintext).expect("encrypt under B");

        // The revoked device, holding only key A, must NOT be able to decrypt it.
        let result = decrypt_from_cloud(&key_a, item_id, &blob_b);
        assert!(
            matches!(result, Err(SyncKeyError::DecryptFailed)),
            "pre-rotation key A must not decrypt a post-rotation blob, got {result:?}"
        );

        // Sanity: the rotated key B still decrypts its own blob.
        let ok = decrypt_from_cloud(&key_b, item_id, &blob_b).expect("B decrypts its own blob");
        assert_eq!(ok, plaintext);
    }

    /// `SyncKey::ct_eq_bytes` is the helper the daemon's provisioning-apply path
    /// uses to distinguish a routine re-provision (identical key → no-op) from a
    /// rotation re-provision (differing key → replace). Verify it matches the
    /// raw-byte equality without leaking via `==`.
    #[test]
    fn ct_eq_bytes_matches_byte_equality() {
        let key = derive_sync_key("ct-eq-passphrase", ACCOUNT_A).expect("derive must succeed");
        let same = *key.as_bytes();
        let mut different = *key.as_bytes();
        different[0] ^= 0xFF;

        assert!(key.ct_eq_bytes(&same), "identical bytes must compare equal");
        assert!(
            !key.ct_eq_bytes(&different),
            "differing bytes must compare unequal"
        );
    }

    // ── per-account salt internals ───────────────────────────────────────────

    /// The per-account salt itself must be deterministic and account-dependent,
    /// and must never collapse to the bare IKM constant.
    #[test]
    fn per_account_salt_is_deterministic_and_unique() {
        let sa1 = derive_per_account_salt(ACCOUNT_A);
        let sa2 = derive_per_account_salt(ACCOUNT_A);
        let sb = derive_per_account_salt(ACCOUNT_B);
        assert_eq!(sa1, sa2, "same account id must yield the same salt");
        assert_ne!(sa1, sb, "different account ids must yield different salts");
        // The per-account salt must not collapse to the raw IKM.
        assert_ne!(&sa1, PER_ACCOUNT_SALT_IKM.as_ref());
    }
}
