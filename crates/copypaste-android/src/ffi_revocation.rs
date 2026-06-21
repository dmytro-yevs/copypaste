//! Key rotation and peer revocation FFI exports.
//!
//! Covers: `revoke_device_and_rotate_key`, `rotate_sync_key`,
//! `derive_new_sync_key_from_passphrase` (shared internal helper),
//! `RevokedPeer`, `revoke_device_audit`, `list_revoked_fingerprints`,
//! `list_revoked_peers`.

use copypaste_core::{derive_sync_key, SyncKeyError};

use crate::{ffi_cloud_sync::MIN_PASSPHRASE_LEN, panic_boundary, CopypasteError};

// with_cached_db and Zeroizing are only used by the live DB path.
#[cfg(feature = "android-uniffi-live")]
use crate::ffi_db::with_cached_db;
#[cfg(feature = "android-uniffi-live")]
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// PG-12 (8qcm): Revoke peer + sync-key rotation over Android FFI
//
// macOS exposes `revoke_and_rotate` (ipc.rs:4882): revoke a peer's DB row +
// rotate the cloud sync key under a new passphrase in one atomic step. Android
// DevicesActivity.kt:577 only calls `revoke_device_audit` (DB revoke) — it never
// rotates the sync key. That means the revoked peer still holds the old key and
// can continue decrypting any blobs in the shared relay/cloud inbox.
//
// This FFI adds `revoke_device_and_rotate_key` which:
//   1. Derives the new sync key from the provided passphrase (FAIL FAST if bad).
//   2. Calls `revoke_device_audit` for the audit-table write (db side-effect).
//   3. Returns the new 32-byte derived sync key so Kotlin can:
//      a. Store it in AndroidKeystore (replacing the old key).
//      b. Re-encrypt any locally-cached blobs that must survive (optional, same
//         as macOS wave: remaining devices re-provision).
//      c. Re-derive the relay inbox id + PoP for re-registration under the new key.
//
// SECURITY INVARIANTS (load-bearing — do NOT relax):
//   - Key derivation MUST fail before any revocation mutation so a bad passphrase
//     does not leave the DB in a half-revoked state.
//   - The returned key bytes MUST be stored in AndroidKeystore by Kotlin. The
//     ByteArray MUST be zeroed after persisting (identical contract to
//     `derive_cloud_sync_key`).
//   - Kotlin MUST also call `update_p2p_listener_peers` / `sync_with_peer` with
//     the revoked fingerprint in `revoked_fingerprints` so the mTLS denylist is
//     updated at the transport layer.
//
// RUNTIME VERIFICATION REQUIRED before trusting in production: the full
// round-trip (revoke + re-register with new key + confirm old key rejected) can
// only be tested with a live relay and the Android Gradle build. Flag this as
// GRADLE-REQUIRED for integration test coverage.
// ---------------------------------------------------------------------------

/// Revoke a peer and rotate the cloud sync key to a new passphrase (live build).
///
/// # Steps (in order — FAIL FAST before any mutation)
///
/// 1. Derive `new_sync_key = Argon2id(new_passphrase)`. Returns
///    `DecryptionFailed` if the passphrase is too short or derivation fails —
///    this happens BEFORE any DB write so no revocation occurs on a bad passphrase.
/// 2. Write the revocation audit row via `revoke_device` (DB I/O). Returns
///    `DatabaseError` on failure.
/// 3. Return `new_sync_key` (32 raw bytes) so Kotlin can store it in
///    AndroidKeystore and re-register with the relay under the new key.
///
/// # SECURITY NOTE
/// The returned `Vec<u8>` crosses the FFI boundary unzeroized. UniFFI copies it
/// into a Kotlin `ByteArray`. The Kotlin layer MUST zero that array after
/// persisting the key to AndroidKeystore — this is a load-bearing contract.
/// Kotlin MUST also remove the peer from its P2P roster and call
/// `update_p2p_listener_peers` with the revoked fingerprint in the denylist.
///
/// # GRADLE-REQUIRED
/// Full end-to-end verification (relay re-registration under new key, old-key
/// rejection) requires a live relay and can only be tested via the Android
/// Gradle/instrumented-test pipeline — not host `cargo check`.
#[cfg(feature = "android-uniffi-live")]
pub fn revoke_device_and_rotate_key(
    db_path: String,
    key: &[u8],
    fingerprint: String,
    name: String,
    new_passphrase: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // STEP 1: Derive the new key FIRST so a bad passphrase fails before any
        // revocation mutation (mirrors ipc.rs:4910-4918 "Derive the new key FIRST").
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;

        // STEP 2: Revoke the peer audit row. `key` is the 32-byte device storage
        // key (distinct from the cloud sync key being rotated).
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::revoke_device(db.conn(), &fingerprint, &name).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })
        })?;

        // STEP 3: Return the new key bytes. Kotlin stores them in AndroidKeystore.
        // SECURITY: ByteArray crosses FFI unzeroized — Kotlin MUST zero after storing.
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Stub (feature off): derives and returns the new key WITHOUT the DB revocation
/// write. Kotlin gets the new key bytes so the rotation path can be exercised
/// even without the live DB; the DB revocation must be done separately by the
/// Kotlin layer via `revoke_device_audit` when the live feature is not compiled in.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn revoke_device_and_rotate_key(
    _db_path: String,
    key: &[u8],
    _fingerprint: String,
    _name: String,
    new_passphrase: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Validate the DB key shape (mirrors the live path's key check).
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        // Derive + return the new key; no DB I/O in stub mode.
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Rotate the cloud sync key to a new passphrase WITHOUT revoking a peer.
///
/// Use this when the user changes their sync passphrase independently of a
/// revocation event. Mirrors the macOS `rotate_sync_key` IPC handler path
/// (ipc.rs:5099-5105) but without the revocation audit write.
///
/// Returns the new 32-byte derived sync key. Kotlin MUST store it in
/// AndroidKeystore and zero the ByteArray after persisting.
///
/// # GRADLE-REQUIRED
/// Full verification requires a live relay — see `revoke_device_and_rotate_key`.
pub fn rotate_sync_key(new_passphrase: String) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Internal helper: validate a passphrase length and derive a new SyncKey.
///
/// Shared by `revoke_device_and_rotate_key` and `rotate_sync_key` so the
/// validation/error-mapping path is byte-for-byte identical on both call sites —
/// the same pattern `derive_cloud_sync_key` uses. Mirrors the macOS
/// `ipc.rs:4910-4918` "derive FIRST so a bad passphrase fails before mutation".
pub fn derive_new_sync_key_from_passphrase(
    passphrase: &str,
) -> Result<copypaste_core::SyncKey, CopypasteError> {
    let char_count = passphrase.chars().count();
    if char_count < MIN_PASSPHRASE_LEN {
        return Err(CopypasteError::DecryptionFailed {
            reason: format!(
                "new passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                 (got {char_count})",
            ),
        });
    }
    derive_sync_key(passphrase).map_err(|e| match e {
        SyncKeyError::PassphraseTooShort(n) => CopypasteError::DecryptionFailed {
            reason: format!(
                "new passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                 (got {n})",
            ),
        },
        SyncKeyError::Argon2Params(msg) | SyncKeyError::Argon2Hash(msg) => {
            CopypasteError::DecryptionFailed { reason: msg }
        }
        SyncKeyError::EncryptFailed(msg) => CopypasteError::DecryptionFailed {
            reason: format!("key derivation encrypt step failed: {msg}"),
        },
        SyncKeyError::DecryptFailed => CopypasteError::DecryptionFailed {
            reason: "key derivation decrypt step failed".into(),
        },
        SyncKeyError::BlobTooShort(n) => CopypasteError::DecryptionFailed {
            reason: format!("key derivation blob too short: {n} bytes"),
        },
    })
}

// ── Device-management parity (W7 — revoke / audit) ───────────────────────────
//
// `RevokedPeer` mirrors `copypaste_core::RevokedDevice`. The audit table lives
// in the SQLCipher `copypaste.db` under the same key as the rest of the Android
// store, so these calls are feature-gated exactly like `add_clipboard_item` /
// `get_history_count`: with `android-uniffi-live` off they are pure stubs.

/// One revoked-device audit row (mirror of `copypaste_core::RevokedDevice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedPeer {
    pub fingerprint: String,
    pub name: String,
    pub revoked_at: i64,
}

/// Record a manual peer revocation in the local `revoked_devices` audit table
/// (and remove the matching `devices` row), returning the `revoked_at`
/// timestamp. Live build: writes through `with_cached_db` →
/// `copypaste_core::revoke_device`.
#[cfg(feature = "android-uniffi-live")]
pub fn revoke_device_audit(
    db_path: String,
    key: &[u8],
    fingerprint: String,
    name: String,
) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::revoke_device(db.conn(), &fingerprint, &name).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })
        })
    })
}

/// Stub revoke (feature off): no DB I/O; returns a current unix-seconds
/// timestamp so the Kotlin caller's UI can still echo a revoke time. The peer
/// removal from the roster and the denylist enforcement happen Kotlin-side.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn revoke_device_audit(
    _db_path: String,
    key: &[u8],
    _fingerprint: String,
    _name: String,
) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(now)
    })
}

/// List the fingerprints of all revoked devices, newest first (live build).
#[cfg(feature = "android-uniffi-live")]
pub fn list_revoked_fingerprints(
    db_path: String,
    key: &[u8],
) -> Result<Vec<String>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let rows = copypaste_core::list_revoked_devices(db.conn()).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(rows.into_iter().map(|r| r.fingerprint).collect())
        })
    })
}

/// Stub (feature off): no revoked devices to report.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn list_revoked_fingerprints(
    _db_path: String,
    key: &[u8],
) -> Result<Vec<String>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

/// Richer revoked-device listing (fingerprint + name + revoked_at), newest
/// first (live build).
#[cfg(feature = "android-uniffi-live")]
pub fn list_revoked_peers(db_path: String, key: &[u8]) -> Result<Vec<RevokedPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let rows = copypaste_core::list_revoked_devices(db.conn()).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(rows
                .into_iter()
                .map(|r| RevokedPeer {
                    fingerprint: r.fingerprint,
                    name: r.name,
                    revoked_at: r.revoked_at,
                })
                .collect())
        })
    })
}

/// Stub (feature off): no revoked devices to report.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn list_revoked_peers(
    _db_path: String,
    key: &[u8],
) -> Result<Vec<RevokedPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}
