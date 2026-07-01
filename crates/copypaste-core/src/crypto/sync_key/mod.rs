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

#[cfg(test)]
mod tests;

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
