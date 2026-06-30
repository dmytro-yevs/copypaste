//! Cloud sync crypto FFI exports.
//!
//! Covers: Argon2id KDF (`derive_cloud_sync_key`), cloud AEAD
//! (`cloud_encrypt` / `cloud_decrypt`), relay inbox derivation
//! (`relay_inbox_id`, `relay_public_key_b64`, `relay_registration_pop`).

// Brings `Engine::encode` into scope for `relay_public_key_b64` (STANDARD base64).
use base64::Engine as _;
use copypaste_core::{decrypt_from_cloud, derive_sync_key, encrypt_for_cloud, SyncKeyError};
use zeroize::Zeroizing;

use crate::{panic_boundary, CopypasteError};

// ---------------------------------------------------------------------------
// Cloud sync crypto — cross-device SyncKey (Argon2id-derived) + schema v5
//
// These FFI functions expose the SAME crypto used by the macOS daemon's
// cloud.rs so Android can push/pull from the same Supabase table with
// identical encrypted payloads.
//
// Key facts (MUST match cloud.rs):
//   - KDF: Argon2id, 19 MiB / 2 passes / 1 lane, fixed domain salt
//   - AEAD: XChaCha20-Poly1305, 24-byte random nonce prepended to ciphertext
//   - AAD: "{item_id}|5"  (CLOUD_AAD_SCHEMA_VERSION = 5)
//   - Blob wire format: base64(nonce[24] || ciphertext_with_tag)
// ---------------------------------------------------------------------------

/// Minimum accepted passphrase length for cloud-sync key derivation.
///
/// Argon2id accepts any length including empty, but an empty or trivially-short
/// passphrase would produce a weak key that an attacker could brute-force even
/// against a memory-hard KDF. Matches the macOS daemon's UI-side enforcement
/// so both platforms reject the same bad input with an informative error.
///
/// CopyPaste-wg4w: raised 8 -> 12 to match `copypaste_core::MIN_PASSPHRASE_LEN`.
/// If this drifts below the core value, an 8-11 char passphrase would pass this
/// pre-check and then be rejected by core as `PassphraseTooShort` — keep them
/// equal.
pub(crate) const MIN_PASSPHRASE_LEN: usize = 12;

/// Derive a 32-byte sync key from `passphrase` using Argon2id.
///
/// Returns the raw 32-byte key material. The caller (Kotlin) should treat
/// these bytes as a short-lived secret: derive once at passphrase entry,
/// use, then zero the array. Do NOT persist to disk or SharedPreferences.
///
/// # SECURITY NOTE — returned `Vec<u8>` crosses the FFI boundary unzeroized.
/// UniFFI copies the bytes into a Kotlin `ByteArray`; the Kotlin layer MUST
/// zero that array after use. This is a load-bearing contract: failure to do
/// so leaves raw key material on the JVM heap until GC.
///
/// Errors:
///   - `DecryptionFailed { reason }` — passphrase is shorter than
///     `MIN_PASSPHRASE_LEN` bytes; `reason` carries the human-readable cause
///     so the user (and logs) learn why, matching the macOS surface.
///   - `EncryptionFailed` — Argon2 parameter or runtime failure (should not
///     occur with the hardcoded constants; surfaces as a non-panic error).
pub fn derive_cloud_sync_key(passphrase: String) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Guard on char count (Unicode scalar values), not byte length, to match
        // copypaste_core::derive_sync_key which uses passphrase.chars().count().
        // A byte-length guard would silently pass a 2-emoji passphrase (which is
        // ≥8 bytes) while core rejects it as PassphraseTooShort.
        let char_count = passphrase.chars().count();
        if char_count < MIN_PASSPHRASE_LEN {
            return Err(CopypasteError::DecryptionFailed {
                reason: format!(
                    "passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                     (got {char_count})",
                ),
            });
        }
        // CopyPaste-wg4w: Android has no Supabase account id available in the
        // Kotlin layer (see ffi_pairing: supabase_account_id is None), so this is
        // the documented no-account LEGACY (v1) fallback. Threading a per-account
        // salt here would require adding an account-id parameter through the
        // UniFFI surface and the Kotlin callers — tracked with the daemon cutover.
        let key = derive_sync_key(&passphrase).map_err(|e| match e {
            // Propagate any Argon2 runtime message rather than discarding it.
            SyncKeyError::Argon2Params(msg) | SyncKeyError::Argon2Hash(msg) => {
                CopypasteError::DecryptionFailed { reason: msg }
            }
            // Core pre-checked length above, but handle PassphraseTooShort
            // explicitly so the reason is never swallowed into EncryptionFailed.
            SyncKeyError::PassphraseTooShort(n) => CopypasteError::DecryptionFailed {
                reason: format!(
                    "passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                     (got {n})",
                ),
            },
            // These encryption/decryption variants should not arise from key
            // derivation alone; surface them with a reason string.
            SyncKeyError::EncryptFailed(msg) => CopypasteError::DecryptionFailed {
                reason: format!("cloud encrypt failed during key derivation: {msg}"),
            },
            SyncKeyError::DecryptFailed => CopypasteError::DecryptionFailed {
                reason: "cloud decrypt failed during key derivation".into(),
            },
            SyncKeyError::BlobTooShort(n) => CopypasteError::DecryptionFailed {
                reason: format!("blob too short during key derivation: {n} bytes"),
            },
        })?;
        Ok(key.as_bytes().to_vec())
    })
}

/// Encrypt `plaintext` for cloud storage.
///
/// `sync_key_bytes` MUST be the 32 bytes returned by `derive_cloud_sync_key`.
/// `item_id` is the item's UUID string — it is bound into the AEAD AAD so
/// substituting the blob into a different item slot fails authentication.
///
/// Returns `base64(nonce[24] || ciphertext_with_tag)`, matching exactly what
/// the macOS daemon POSTs as `payload_ct`.
///
/// Errors: `EncryptionFailed` on AEAD failure, `InvalidKeyLength` if
/// `sync_key_bytes` is not exactly 32 bytes.
pub fn cloud_encrypt(
    item_id: String,
    plaintext: &[u8],
    sync_key_bytes: &[u8],
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key_bytes
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let sync_key = copypaste_core::SyncKey::from_bytes(*key_arr);
        let blob = encrypt_for_cloud(&sync_key, &item_id, plaintext)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(blob)
    })
}

/// Decrypt a cloud blob produced by `cloud_encrypt` (or the macOS daemon).
///
/// `sync_key_bytes` MUST be the same 32 bytes used during encryption.
/// `item_id` MUST match the value bound into the AAD at encrypt time.
/// `blob` is the raw bytes from base64-decoding the `payload_ct` column.
///
/// Returns the plaintext bytes on success.
///
/// Errors: `DecryptionFailed` if key, item_id, or ciphertext do not match;
/// `InvalidKeyLength` if `sync_key_bytes` is not 32 bytes.
pub fn cloud_decrypt(
    item_id: String,
    blob: &[u8],
    sync_key_bytes: &[u8],
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key_bytes
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let sync_key = copypaste_core::SyncKey::from_bytes(*key_arr);
        decrypt_from_cloud(&sync_key, &item_id, blob).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })
    })
}

// ---------------------------------------------------------------------------
// Shared-account relay inbox derivation (R3b — relay-as-database sync path)
//
// The relay sync path uses a SINGLE inbox per account that every device
// co-registers, pushes to, and subscribes to. Both the inbox `device_id` and
// the registration `public_key_b64` are derived DETERMINISTICALLY from the
// shared sync key so Android shares the macOS daemon's inbox without any
// coordination through the relay. These wrappers expose the EXACT core
// functions (`derive_relay_inbox_id` / `derive_relay_public_key`) so the value
// is byte-identical to the daemon's — Kotlin must NEVER re-derive in-app.
//
// SECURITY: the inbox id is SECRET-derived (anyone who learns it can read/write
// the account's still-E2E-encrypted inbox). Kotlin MUST NOT log it or the
// public key. See crates/copypaste-core/src/relay.rs.
// ---------------------------------------------------------------------------

/// Derive the deterministic shared relay inbox `device_id` from the account's
/// 32-byte sync key (the bytes returned by `derive_cloud_sync_key`).
///
/// Returns a canonical lowercase hyphenated UUID string, byte-identical to the
/// macOS daemon's `copypaste_core::derive_relay_inbox_id`, so Android registers
/// and subscribes to the SAME inbox the daemon uses.
///
/// Errors: `InvalidKeyLength` if `sync_key` is not exactly 32 bytes.
///
/// # Security
/// The returned id is derived from secret key material; Kotlin MUST NOT log it.
pub fn relay_inbox_id(sync_key: &[u8]) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        Ok(copypaste_core::derive_relay_inbox_id(&key_arr))
    })
}

/// Derive the relay registration `public_key_b64` from the account's 32-byte
/// sync key.
///
/// Returns `base64(derive_relay_public_key(sync_key))` using the STANDARD
/// alphabet, byte-identical to what the macOS daemon presents at registration
/// (`base64::engine::general_purpose::STANDARD.encode(pubkey)`), so all of the
/// account's devices co-register with a consistent value.
///
/// Errors: `InvalidKeyLength` if `sync_key` is not exactly 32 bytes.
///
/// # Security
/// Derived from secret key material; Kotlin MUST NOT log it.
pub fn relay_public_key_b64(sync_key: &[u8]) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let pubkey = copypaste_core::derive_relay_public_key(&key_arr);
        Ok(base64::engine::general_purpose::STANDARD.encode(pubkey))
    })
}

// ---------------------------------------------------------------------------
// PG-2 (kmcr): Relay registration Proof-of-Possession (PoP) over Android FFI
//
// The macOS daemon sends HMAC-SHA256(sync_key, "relay-registration-pop-v1:" +
// device_id) at relay registration (relay.rs). Android was missing this export,
// so Kotlin could not compute the PoP and registration silently skipped it.
// This export delegates directly to `copypaste_core::derive_relay_registration_pop`
// — no crypto reimplementation. The result MUST be base64-encoded on the wire.
//
// SECURITY: derived from secret key material; Kotlin MUST NOT log the result.
// ---------------------------------------------------------------------------

/// Compute the relay registration Proof-of-Possession (PoP) for a device.
///
/// Returns `HMAC-SHA256(key=sync_key, msg="relay-registration-pop-v1:" + device_id)`
/// as 32 raw bytes. The caller (Kotlin) MUST base64-encode them for the wire
/// (`pop_b64`) and MUST NOT log the result.
///
/// `sync_key` MUST be the 32 bytes returned by `derive_cloud_sync_key`.
/// `device_id` is the relay inbox id (`relay_inbox_id`), which is also the
/// `device_id` field sent at registration. Using a different value here will
/// produce a PoP that the relay rejects.
///
/// # Security
/// Derived from secret key material; do not log.
pub fn relay_registration_pop(
    sync_key: &[u8],
    device_id: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let pop = copypaste_core::derive_relay_registration_pop(&key_arr, &device_id);
        Ok(pop.to_vec())
    })
}
