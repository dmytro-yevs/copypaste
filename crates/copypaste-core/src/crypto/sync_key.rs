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
//! ## Why the salt is per-account, not one global constant
//!
//! Argon2id's work factor raises the cost of *each individual* guess, but it
//! does **not** stop an attacker from *amortising* one precomputed dictionary
//! across many victims. With a single global salt (the legacy v1 scheme) an
//! attacker who scrapes the untrusted cloud's E2E ciphertext corpus can run a
//! passphrase dictionary through Argon2id **once** and try every resulting key
//! against **every** user's blobs — a classic precompute / rainbow-table
//! amortisation that a unique salt is specifically designed to defeat.
//!
//! A random per-user salt would defeat precompute but breaks cloud sync: two
//! devices that only share a passphrase could not agree on the same key without
//! transmitting and storing the salt. The resolution is a **deterministic
//! per-account salt** (the v2 scheme): we mix a stable account identifier (the
//! Supabase `user_id`, which both devices of an account already know) into the
//! Argon2id salt via HKDF. This keeps derivation **deterministic across the two
//! devices of the same account** (cloud sync still works) while making the salt
//! **unique per account**, so an attacker must restart the entire memory-hard
//! dictionary for every account — cross-user amortisation is gone.
//!
//! ## Versioning (back-compat)
//!
//! - **v1** — `Argon2id(passphrase, ARGON2_SYNC_SALT)` with the fixed global
//!   salt. Retained as the documented fallback for code paths that have **no**
//!   account id (relay-only / P2P / local) and to keep existing ciphertexts
//!   decryptable. Exposed as [`derive_sync_key`].
//! - **v2** — `Argon2id(passphrase, HKDF(account_id))` with a deterministic
//!   per-account salt. Exposed as [`derive_sync_key_for_account`]. Defeats the
//!   cross-user precompute above.
//!
//! [`derive_sync_key_versioned`] dispatches on `Option<account_id>`: `Some`
//! selects v2, `None` falls back to the legacy v1 derivation. The legacy salt is
//! still domain separation (a different application/context yields a different
//! key) — but it is no longer claimed to substitute for a unique salt.
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

/// Global domain-separation salt for the **v1 (legacy)** sync-key derivation.
///
/// This salt is IDENTICAL across all devices and is NOT secret. In the v1
/// scheme it is the *only* salt, which means it provides domain separation but
/// does NOT defeat cross-user precompute (see the module-level "Why the salt is
/// per-account" section). v1 is retained for two reasons:
///   1. Code paths with no account id (relay-only / P2P / local) need a
///      deterministic, account-free salt — they fall back to this constant.
///   2. Existing v1 ciphertexts must remain decryptable.
///
/// In the **v2** scheme this constant is reused as the HKDF input keying
/// material (see [`derive_per_account_salt`]) so the per-account salt remains
/// domain-separated from any other use of the same byte string.
///
/// Changing this constant is a hard-fork of all existing v1 cloud ciphertexts —
/// every passphrase will derive a different key and existing blobs will
/// become unreadable. This test pins the value:
/// `ARGON2_SYNC_SALT == SHA-256(b"copypaste/cloud-sync-key/v1/argon2id-salt")`
pub const ARGON2_SYNC_SALT: &[u8; 32] = &[
    // SHA-256("copypaste/cloud-sync-key/v1/argon2id-salt")
    0xe1, 0xb5, 0x69, 0xc8, 0xe9, 0xd3, 0xc8, 0x22, 0xdf, 0x1c, 0xdf, 0x05, 0x09, 0x90, 0x4c, 0x07,
    0xe6, 0x13, 0x10, 0x60, 0x81, 0x43, 0x5d, 0x43, 0xa8, 0xd3, 0x3b, 0x63, 0x08, 0x00, 0x85, 0xbd,
];

/// Sync-key derivation scheme versions.
///
/// These tag *which salt* fed Argon2id. They are NOT stored inside the cloud
/// blob (the blob format is `nonce || ciphertext` and is byte-identical for
/// both versions); they exist so callers and docs can name the scheme
/// unambiguously and so a future migration can dispatch on them.
///
/// | version | salt | when used |
/// |---------|------|-----------|
/// | 1 | global `ARGON2_SYNC_SALT` | no account id available (relay/P2P/local) and all pre-migration data |
/// | 2 | `HKDF(ARGON2_SYNC_SALT, account_id)` | a Supabase account id is available |
pub const SYNC_KEY_DERIVATION_VERSION_V1: u32 = 1;

/// See [`SYNC_KEY_DERIVATION_VERSION_V1`].
pub const SYNC_KEY_DERIVATION_VERSION_V2: u32 = 2;

/// HKDF `info` string for the v2 per-account Argon2id salt.
///
/// Purpose-separated (per the project's HKDF-info convention) so this salt can
/// never collide with the relay-inbox / storage / telemetry HKDF derivations
/// that share `sha2`/`hkdf`. The account id is appended to this prefix.
const SYNC_SALT_INFO_V2: &[u8] = b"copypaste/cloud-sync-key/v2/per-account-salt|";

/// Derive the **deterministic** per-account Argon2id salt for the v2 scheme.
///
/// `salt = HKDF-SHA256(ikm = ARGON2_SYNC_SALT, salt = None, info =
/// SYNC_SALT_INFO_V2 || account_id)`.
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

    let hk = Hkdf::<Sha256>::new(None, ARGON2_SYNC_SALT);
    let mut info = Vec::with_capacity(SYNC_SALT_INFO_V2.len() + account_id.len());
    info.extend_from_slice(SYNC_SALT_INFO_V2);
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

    /// Generate a fresh 32-byte key from the OS CSPRNG.
    ///
    /// Unlike `derive_sync_key`, this key is NOT reproducible from a
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

/// Minimum passphrase length enforced by all sync-key derivation entry points.
///
/// A passphrase shorter than this is brute-forceable even with Argon2id's
/// memory-hard work factor, and a global salt previously let that brute force
/// be amortised across users (see the module-level security section). Twelve
/// characters raises the dictionary-attack floor materially (the OWASP/NIST
/// guidance for human-chosen secrets) while staying enterable for a
/// passphrase-style secret. Raising this only validates NEW passphrase entry;
/// it does not invalidate already-derived stored keys.
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Run Argon2id over `passphrase` with an explicit 32-byte `salt`.
///
/// Shared core of every public derivation entry point so the Argon2id
/// parameters, the length guard, and the zeroize-on-drop handling are
/// byte-identical regardless of which salt (v1 global or v2 per-account) is
/// supplied. The salt is the ONLY difference between the schemes.
fn derive_sync_key_with_salt(passphrase: &str, salt: &[u8; 32]) -> Result<SyncKey, SyncKeyError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    // Fix [HIGH]: reject trivially short passphrases before hashing so the
    // IPC layer can surface a user-actionable error rather than silently
    // accepting a short, brute-forceable passphrase.
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

/// Derive the **v1 (legacy)** 32-byte shared sync key from `passphrase`.
///
/// Uses the fixed global [`ARGON2_SYNC_SALT`]. The derivation is
/// **deterministic**: the same passphrase always produces the same key on every
/// device. This is the cross-device agreement property required for cloud sync.
///
/// This is the documented fallback for code paths that have **no** account id
/// (relay-only, P2P, or local) and the scheme under which all pre-migration
/// ciphertexts were produced — so its output is intentionally byte-identical to
/// the original implementation and MUST NOT change. When a Supabase account id
/// is available, prefer [`derive_sync_key_for_account`] /
/// [`derive_sync_key_versioned`], which defeat cross-user precompute.
///
/// # Errors
/// Returns `SyncKeyError::PassphraseTooShort` if `passphrase` is shorter than
/// `MIN_PASSPHRASE_LEN` characters (prevents trivial brute force).
/// Returns `SyncKeyError::Argon2Params` if the Argon2id parameter struct
/// cannot be built (only possible if the hardcoded constants are invalid),
/// or `SyncKeyError::Argon2Hash` if the hashing operation fails at runtime.
pub fn derive_sync_key(passphrase: &str) -> Result<SyncKey, SyncKeyError> {
    derive_sync_key_with_salt(passphrase, ARGON2_SYNC_SALT)
}

/// Derive the **v2 (per-account)** 32-byte shared sync key from `passphrase`.
///
/// Feeds Argon2id a deterministic per-account salt derived from `account_id`
/// (see [`derive_per_account_salt`]). Because the salt is unique per account,
/// an attacker who scrapes the untrusted cloud's E2E ciphertext corpus cannot
/// reuse a single Argon2id dictionary precompute across accounts. Because the
/// salt is deterministic, both devices of the same account derive the identical
/// key, so cloud sync still works.
///
/// `account_id` must be a STABLE identifier that BOTH devices of the account
/// agree on (the canonical `copypaste_supabase::supabase_account_id`, i.e.
/// `"<project_ref>|<user_id>"`). It is not secret — it is mixed in only as a
/// salt — but it must be identical on both devices or they will not agree on a
/// key.
///
/// # Errors
/// Same as [`derive_sync_key`].
pub fn derive_sync_key_for_account(
    passphrase: &str,
    account_id: &str,
) -> Result<SyncKey, SyncKeyError> {
    let salt = derive_per_account_salt(account_id);
    derive_sync_key_with_salt(passphrase, &salt)
}

/// Derive a sync key, selecting the scheme by whether an account id is present.
///
/// This is the threading entry point for callers: pass the Supabase account id
/// when one is configured, or `None` for relay-only / P2P / local / no-cloud
/// paths.
///
/// - `Some(account_id)` → **v2** per-account salt ([`derive_sync_key_for_account`]).
/// - `None` → **v1** legacy global salt ([`derive_sync_key`]). This fallback is
///   explicit and deliberate, not accidental: a path with no account id has no
///   stable per-account value to mix in, and must stay byte-compatible with
///   relay/P2P peers and existing data.
///
/// # Errors
/// Same as [`derive_sync_key`].
pub fn derive_sync_key_versioned(
    passphrase: &str,
    account_id: Option<&str>,
) -> Result<SyncKey, SyncKeyError> {
    match account_id {
        Some(id) => derive_sync_key_for_account(passphrase, id),
        None => derive_sync_key(passphrase),
    }
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

/// Decrypt a cloud blob by trying each candidate key in order, returning the
/// plaintext from the FIRST key that authenticates.
///
/// This is the restart-surviving **dual-key read dispatch** primitive for the
/// cloud (Supabase) download path: after the v2 per-account-salt cutover, a row
/// may have been written under either the **v2** key (new writes) or the legacy
/// **v1** key (pre-cutover data). The reader cannot tell which from the blob
/// (the wire format `nonce || ciphertext` is byte-identical for both schemes and
/// carries no version discriminator), so it trial-decrypts: pass the candidates
/// in `[v2, v1]` order and the XChaCha20-Poly1305 auth tag rejects every wrong
/// key, so only the key the row was actually encrypted under succeeds.
///
/// Trial decryption is sound because the AEAD tag makes a wrong-key attempt
/// indistinguishable from a tampered blob — there is no padding/format oracle to
/// leak which key matched. The cost is one extra (failed) AEAD verification per
/// pre-cutover row, which is negligible next to the network round-trip.
///
/// Returns `BlobTooShort` if the blob is shorter than the nonce (checked once,
/// before any key is tried), `DecryptFailed` if `keys` is empty or NONE of the
/// candidates authenticate, or the recovered plaintext on the first success.
///
/// # Security
/// - Never reveals WHICH key matched via the return type — callers that need to
///   know (e.g. to decide whether to re-write under v2) must observe it
///   out-of-band; the public contract is plaintext-or-`DecryptFailed`.
/// - Each candidate is tried with the identical AAD `(item_id, schema v5)`.
pub fn decrypt_from_cloud_trying(
    keys: &[&SyncKey],
    item_id: &str,
    blob: &[u8],
) -> Result<Vec<u8>, SyncKeyError> {
    // Length is independent of the key, so check it once up front rather than
    // per-candidate; a too-short blob is a structural error, not a wrong key.
    if blob.len() < NONCE_SIZE {
        return Err(SyncKeyError::BlobTooShort(blob.len()));
    }
    for key in keys {
        match decrypt_from_cloud(key, item_id, blob) {
            Ok(plaintext) => return Ok(plaintext),
            // Wrong key for THIS candidate — try the next. We deliberately do not
            // short-circuit on the first failure: a v1 row fails the v2 candidate
            // and must still be offered the v1 candidate.
            Err(SyncKeyError::DecryptFailed) => continue,
            // BlobTooShort cannot occur here (length checked above); propagate any
            // other (currently unreachable) error variant rather than masking it.
            Err(other) => return Err(other),
        }
    }
    Err(SyncKeyError::DecryptFailed)
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
        let key = make_key("empty-test-pad");
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
        let key = make_key("tamper-test-pad");
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
        let key = make_key("aad-test-pass");
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
        let key = make_key("nonce-test-pad");
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
        let key = make_key("format-test-pad");
        let plaintext = b"format check";
        let blob = encrypt_for_cloud(&key, "item-fmt", plaintext).unwrap();
        // blob length must be nonce(24) + plaintext(N) + tag(16)
        assert_eq!(blob.len(), NONCE_SIZE + plaintext.len() + 16);
    }

    // ── blob too short ───────────────────────────────────────────────────────

    /// A blob shorter than NONCE_SIZE must return BlobTooShort, not panic.
    #[test]
    fn blob_too_short_returns_error_not_panic() {
        let key = make_key("short-test-pad");
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
    /// with `PassphraseTooShort` before Argon2id even runs. Includes an 11-char
    /// case to pin the raised floor (MIN_PASSPHRASE_LEN == 12).
    #[test]
    fn short_passphrase_returns_passphrase_too_short() {
        for short in &["", "a", "1234567", "12345678901"] {
            assert!(
                short.chars().count() < MIN_PASSPHRASE_LEN,
                "test fixture {short:?} must be shorter than the enforced minimum"
            );
            let result = derive_sync_key(short);
            assert!(
                matches!(result, Err(SyncKeyError::PassphraseTooShort(_))),
                "passphrase {:?} (len {}) must produce PassphraseTooShort",
                short,
                short.chars().count(),
            );
        }
    }

    /// The enforced minimum is 12 characters (raised from 8).
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
            derive_sync_key("123456789012").is_ok(),
            "passphrase of exactly {MIN_PASSPHRASE_LEN} chars must succeed"
        );
        // 11 chars — one short of the floor — must be rejected.
        assert!(
            matches!(
                derive_sync_key("12345678901"),
                Err(SyncKeyError::PassphraseTooShort(11))
            ),
            "passphrase one char below the floor must be rejected"
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
        let key_a = derive_sync_key("old-shared-passphrase").expect("derive A must succeed");
        // Key B = the rotated key. A different passphrase yields different bytes.
        let key_b = derive_sync_key("new-rotated-passphrase").expect("derive B must succeed");
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
            "pre-rotation key A must not decrypt a post-rotation blob, got {:?}",
            result
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
        let key = derive_sync_key("ct-eq-passphrase").expect("derive must succeed");
        let same = *key.as_bytes();
        let mut different = *key.as_bytes();
        different[0] ^= 0xFF;

        assert!(key.ct_eq_bytes(&same), "identical bytes must compare equal");
        assert!(
            !key.ct_eq_bytes(&different),
            "differing bytes must compare unequal"
        );
    }

    // ── CopyPaste-wg4w: v2 per-account salt (cross-user precompute defence) ───

    const ACCOUNT_A: &str = "proj_abc|00000000-0000-0000-0000-0000000000aa";
    const ACCOUNT_B: &str = "proj_abc|00000000-0000-0000-0000-0000000000bb";
    const PASS: &str = "correct horse battery staple";

    /// A v2 (per-account) key must round-trip through the cloud AEAD.
    #[test]
    fn v2_account_key_roundtrips() {
        let key = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("v2 derive");
        let item_id = "item-v2-roundtrip";
        let plaintext = b"v2 per-account ciphertext";
        let blob = encrypt_for_cloud(&key, item_id, plaintext).expect("encrypt");
        let recovered = decrypt_from_cloud(&key, item_id, &blob).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// A LEGACY (v1) blob must still decrypt with the v1 key after the migration
    /// — back-compat for existing cloud data.
    #[test]
    fn legacy_v1_blob_still_decrypts() {
        let key = derive_sync_key(PASS).expect("v1 derive");
        let item_id = "item-v1-legacy";
        let plaintext = b"blob written under the old global-salt scheme";
        let blob = encrypt_for_cloud(&key, item_id, plaintext).expect("encrypt");
        // Re-derive the v1 key independently (as a fresh daemon would) and decrypt.
        let key_again = derive_sync_key(PASS).expect("v1 re-derive");
        let recovered = decrypt_from_cloud(&key_again, item_id, &blob).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// Two DIFFERENT account ids with the SAME passphrase must derive DIFFERENT
    /// keys — this is the property that defeats cross-user precompute.
    #[test]
    fn different_accounts_same_passphrase_derive_different_keys() {
        let key_a = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("derive A");
        let key_b = derive_sync_key_for_account(PASS, ACCOUNT_B).expect("derive B");
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

    /// The SAME account id + SAME passphrase must derive the SAME key on every
    /// call — cross-device determinism for cloud sync.
    #[test]
    fn same_account_same_passphrase_is_deterministic() {
        let k1 = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("derive 1");
        let k2 = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("derive 2");
        assert_eq!(
            k1.as_bytes(),
            k2.as_bytes(),
            "same account + passphrase must be deterministic across devices"
        );
    }

    /// The no-account fallback must equal the legacy v1 key byte-for-byte, so
    /// relay/P2P/local paths and existing data are completely unchanged.
    #[test]
    fn versioned_none_equals_legacy_v1() {
        let legacy = derive_sync_key(PASS).expect("v1");
        let fallback = derive_sync_key_versioned(PASS, None).expect("versioned None");
        assert_eq!(
            legacy.as_bytes(),
            fallback.as_bytes(),
            "derive_sync_key_versioned(_, None) must be byte-identical to derive_sync_key"
        );
    }

    /// `derive_sync_key_versioned(_, Some(id))` must equal
    /// `derive_sync_key_for_account(_, id)` (dispatcher selects v2).
    #[test]
    fn versioned_some_equals_for_account() {
        let direct = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("for_account");
        let via = derive_sync_key_versioned(PASS, Some(ACCOUNT_A)).expect("versioned Some");
        assert_eq!(direct.as_bytes(), via.as_bytes());
    }

    /// The v2 (per-account) key must DIFFER from the v1 (global-salt) key for the
    /// same passphrase — proving the salt actually changed the derivation.
    #[test]
    fn v2_key_differs_from_v1_key() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let v2 = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("v2");
        assert_ne!(v1.as_bytes(), v2.as_bytes());
    }

    /// The per-account salt itself must be deterministic and account-dependent.
    #[test]
    fn per_account_salt_is_deterministic_and_unique() {
        let sa1 = derive_per_account_salt(ACCOUNT_A);
        let sa2 = derive_per_account_salt(ACCOUNT_A);
        let sb = derive_per_account_salt(ACCOUNT_B);
        assert_eq!(sa1, sa2, "same account id must yield the same salt");
        assert_ne!(sa1, sb, "different account ids must yield different salts");
        // The per-account salt must not collapse to the global v1 salt.
        assert_ne!(&sa1, ARGON2_SYNC_SALT.as_ref());
    }

    /// v2 derivation enforces the same passphrase-length floor as v1.
    #[test]
    fn v2_rejects_short_passphrase() {
        assert!(matches!(
            derive_sync_key_for_account("short", ACCOUNT_A),
            Err(SyncKeyError::PassphraseTooShort(_))
        ));
    }

    /// Derivation-version constants are stable (1 and 2).
    #[test]
    fn derivation_version_constants() {
        assert_eq!(SYNC_KEY_DERIVATION_VERSION_V1, 1);
        assert_eq!(SYNC_KEY_DERIVATION_VERSION_V2, 2);
    }

    // ── CopyPaste-jdq5: dual-key read dispatch (decrypt_from_cloud_trying) ─────

    /// A blob written under the v2 key must be recovered when the candidate list
    /// is `[v2, v1]` — the post-cutover happy path.
    #[test]
    fn trying_recovers_v2_blob_with_v2_then_v1() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let v2 = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("v2");
        let item_id = "item-trying-v2";
        let plaintext = b"written under v2 per-account salt";
        let blob = encrypt_for_cloud(&v2, item_id, plaintext).expect("encrypt v2");
        let recovered =
            decrypt_from_cloud_trying(&[&v2, &v1], item_id, &blob).expect("v2 candidate decrypts");
        assert_eq!(recovered, plaintext);
    }

    /// A LEGACY v1 blob must still be recovered when the candidate list is
    /// `[v2, v1]` — this is the zero-data-loss back-compat guarantee: existing
    /// pre-cutover cloud rows keep decrypting after v2 is introduced. The v2
    /// candidate is tried first and fails (auth-tag mismatch); the v1 candidate
    /// then succeeds.
    #[test]
    fn trying_recovers_v1_blob_via_fallback() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let v2 = derive_sync_key_for_account(PASS, ACCOUNT_A).expect("v2");
        let item_id = "item-trying-v1-legacy";
        let plaintext = b"legacy row written before the v2 cutover";
        // Encrypt under v1 (as a pre-cutover daemon would have).
        let blob = encrypt_for_cloud(&v1, item_id, plaintext).expect("encrypt v1");
        // Reader offers [v2, v1]: v2 fails, v1 wins.
        let recovered =
            decrypt_from_cloud_trying(&[&v2, &v1], item_id, &blob).expect("v1 fallback decrypts");
        assert_eq!(recovered, plaintext);
    }

    /// A single-candidate list `[v1]` (the un-cutover default: no v2 key present)
    /// behaves exactly like `decrypt_from_cloud` for a v1 blob.
    #[test]
    fn trying_single_v1_candidate_matches_plain_decrypt() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let item_id = "item-trying-single";
        let plaintext = b"only the v1 key is configured";
        let blob = encrypt_for_cloud(&v1, item_id, plaintext).expect("encrypt v1");
        let recovered =
            decrypt_from_cloud_trying(&[&v1], item_id, &blob).expect("single candidate decrypts");
        assert_eq!(recovered, plaintext);
    }

    /// When NONE of the candidates match (wrong passphrase entirely), the result
    /// is `DecryptFailed` — never a panic, never a partial plaintext.
    #[test]
    fn trying_all_wrong_keys_returns_decrypt_failed() {
        let real = derive_sync_key(PASS).expect("real");
        let wrong_a = derive_sync_key("totally-different-pass-a").expect("wrong a");
        let wrong_b = derive_sync_key_for_account("totally-different-pass-b", ACCOUNT_B)
            .expect("wrong b");
        let item_id = "item-trying-none";
        let blob = encrypt_for_cloud(&real, item_id, b"secret").expect("encrypt");
        assert!(matches!(
            decrypt_from_cloud_trying(&[&wrong_a, &wrong_b], item_id, &blob),
            Err(SyncKeyError::DecryptFailed)
        ));
    }

    /// An empty candidate list yields `DecryptFailed` (nothing to try), not a
    /// panic.
    #[test]
    fn trying_empty_candidates_returns_decrypt_failed() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let blob = encrypt_for_cloud(&v1, "x", b"data").expect("encrypt");
        let no_keys: [&SyncKey; 0] = [];
        assert!(matches!(
            decrypt_from_cloud_trying(&no_keys, "x", &blob),
            Err(SyncKeyError::DecryptFailed)
        ));
    }

    /// A too-short blob is reported as `BlobTooShort` (checked once, before any
    /// candidate is tried) rather than collapsing into `DecryptFailed`.
    #[test]
    fn trying_short_blob_returns_blob_too_short() {
        let v1 = derive_sync_key(PASS).expect("v1");
        let short = [0u8; 5];
        assert!(matches!(
            decrypt_from_cloud_trying(&[&v1], "x", &short),
            Err(SyncKeyError::BlobTooShort(5))
        ));
    }

    /// Restart simulation: a v1 blob written by a "previous run" must still
    /// decrypt when BOTH key slots are reloaded from persisted bytes (via
    /// `from_bytes`, the exact path the daemon uses to restore Keychain/file-store
    /// key material across a restart) and offered as `[v2, v1]`.
    #[test]
    fn trying_survives_restart_reload_of_both_slots() {
        let item_id = "item-restart-v1";
        let plaintext = b"persisted under v1 before the daemon restarted";
        // "Before restart": derive v1, encrypt a row.
        let blob = {
            let v1 = derive_sync_key(PASS).expect("v1 pre-restart");
            encrypt_for_cloud(&v1, item_id, plaintext).expect("encrypt v1")
        };
        // "After restart": the daemon reloads the persisted key BYTES (no
        // passphrase available) for both the v1 and v2 slots and reconstructs
        // SyncKeys via from_bytes.
        let v1_bytes = *derive_sync_key(PASS).expect("v1 bytes").as_bytes();
        let v2_bytes = *derive_sync_key_for_account(PASS, ACCOUNT_A)
            .expect("v2 bytes")
            .as_bytes();
        let v1_reloaded = SyncKey::from_bytes(v1_bytes);
        let v2_reloaded = SyncKey::from_bytes(v2_bytes);
        let recovered =
            decrypt_from_cloud_trying(&[&v2_reloaded, &v1_reloaded], item_id, &blob)
                .expect("reloaded slots decrypt the legacy v1 row");
        assert_eq!(recovered, plaintext);
    }
}
