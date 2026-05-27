#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

pub mod panic_boundary;
pub mod version;
pub use panic_boundary::PanicError;
pub use version::{
    check_compatibility, core_version, uniffi_abi_version, VersionError, UNIFFI_ABI_VERSION,
};

use copypaste_core::{
    build_item_aad, decrypt_item_with_aad, detect, encrypt_item_with_aad, AAD_SCHEMA_VERSION,
    NONCE_SIZE,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// When using UDL-based scaffolding, uniffi::Error and uniffi::Record proc-macro
// derives conflict with the generated scaffolding. Only thiserror is needed here.
#[derive(Debug, thiserror::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {message}")]
    DecryptionFailed { message: String },
    #[error("Database error: {message}")]
    DatabaseError { message: String },
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
    /// v0.3 (OI-7): a Rust panic was caught at the FFI boundary by
    /// [`panic_boundary::catch_result`]. Carries the panic message so Kotlin
    /// can log/surface it instead of seeing a JVM-killing abort.
    #[error("Panicked: {message}")]
    Panicked { message: String },
}

impl From<PanicError> for CopypasteError {
    fn from(p: PanicError) -> Self {
        match p {
            PanicError::Panicked(message) => CopypasteError::Panicked { message },
        }
    }
}

pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// v0.3: `item_id` is bound into the AEAD AAD alongside `AAD_SCHEMA_VERSION`.
/// Kotlin callers MUST persist the same item_id alongside the ciphertext and
/// pass it back to `decrypt_text` verbatim — a mismatch will fail decryption
/// with `EncryptionFailed`. (Legacy empty-AAD fallback was removed in 1c55e57.)
pub fn encrypt_text(
    item_id: String,
    bytes: &[u8],
    key: &[u8],
) -> Result<EncryptedBlob, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) = encrypt_item_with_aad(bytes, &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(EncryptedBlob {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    })
}

/// v0.3: `item_id` MUST match the value used during `encrypt_text` — see the
/// docstring on `encrypt_text` for the AAD binding rationale.
pub fn decrypt_text(
    item_id: String,
    ciphertext: &[u8],
    nonce: &[u8],
    key: &[u8],
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let nonce_arr: [u8; NONCE_SIZE] =
            nonce
                .try_into()
                .map_err(|_| CopypasteError::DecryptionFailed {
                    message: "wrong nonce length".into(),
                })?;
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        decrypt_item_with_aad(ciphertext, &nonce_arr, &key_arr, &aad).map_err(|e| {
            CopypasteError::DecryptionFailed {
                message: e.to_string(),
            }
        })
    })
}

pub fn is_sensitive(text: String) -> bool {
    detect(&text).is_some()
}

pub fn sensitive_kind(text: String) -> Option<String> {
    detect(&text).map(|k| format!("{:?}", k))
}

// Database handle table. OnceLock is stable on Rust 1.70+ (our MSRV is 1.75).
static DB_HANDLES: OnceLock<Mutex<HashMap<u64, copypaste_core::Database>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn db_handles() -> &'static Mutex<HashMap<u64, copypaste_core::Database>> {
    DB_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Open (or create) an encrypted SQLite database at `path` using the 32-byte `key`.
/// Returns an opaque handle for subsequent calls.
pub fn open_database(path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let db =
            copypaste_core::Database::open(std::path::Path::new(&path), &key_arr).map_err(|e| {
                CopypasteError::DatabaseError {
                    message: e.to_string(),
                }
            })?;
        let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // recover from mutex poison instead of panicking across FFI boundary
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle, db);
        Ok(handle)
    })
}

pub fn close_database(handle: u64) {
    // A poisoned mutex on the global handle table would otherwise abort the
    // JVM via `unwrap()`. Wrapping in `catch_result` converts that into a
    // `Result::Err(PanicError)` we then deliberately discard — `close_database`
    // is declared as void in the UDL and Kotlin callers cannot signal a
    // failure path, but at minimum we keep the process alive.
    let _ = panic_boundary::catch_result(|| {
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle); // recover from mutex poison instead of panicking across FFI boundary
        Ok::<(), CopypasteError>(())
    });
}

// ---------------------------------------------------------------------------
// Live binding for Android end-to-end clipboard flow.
//
// Behaviour:
//   * Feature `android-uniffi-live` ON  → open DB at `db_path`, encrypt the
//     text via `copypaste_core::encrypt_item`, build a `ClipboardItem`, and
//     persist via `copypaste_core::insert_item`. Returns the new row id, or
//     an empty string if the text was flagged as sensitive.
//   * Feature OFF (default)            → no DB I/O. Returns a deterministic
//     stub id so callers can still exercise the binding in CI without bundling
//     the storage stack.
//
// `key` must be the 32-byte device key (derived from Android Keystore by the
// caller; that derivation lives in Kotlin).
// ---------------------------------------------------------------------------

#[cfg(feature = "android-uniffi-live")]
pub fn add_clipboard_item(
    db_path: String,
    key: &[u8],
    text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;

        // Skip sensitive content (caller-visible: empty string return).
        if detect(&text).is_some() {
            return Ok(String::new());
        }

        let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr).map_err(
            |e| CopypasteError::DatabaseError {
                message: e.to_string(),
            },
        )?;

        // v0.3: pre-generate item_id so the AAD baked into the ciphertext matches
        // the value persisted in the row — decryption later rebuilds the AAD from
        // the stored item_id (AAD_SCHEMA_VERSION = 3). Legacy empty-AAD fallback
        // was removed in 1c55e57.
        let item_id = uuid::Uuid::new_v4().to_string();
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) = encrypt_item_with_aad(text.as_bytes(), &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;

        let lamport_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut item =
            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport_ts);
        item.item_id = item_id;
        let id = item.id.clone();

        copypaste_core::insert_item(&db, &item).map_err(|e| CopypasteError::DatabaseError {
            message: e.to_string(),
        })?;

        Ok(id)
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn add_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Validate key shape to mirror the live path's error surface.
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok("stub-uniffi-not-live".to_string())
    })
}

#[cfg(feature = "android-uniffi-live")]
pub fn get_history_count(db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr).map_err(
            |e| CopypasteError::DatabaseError {
                message: e.to_string(),
            },
        )?;
        let n = copypaste_core::count_items(&db).map_err(|e| CopypasteError::DatabaseError {
            message: e.to_string(),
        })?;
        Ok(n.max(0) as u64)
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_count(_db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(0)
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![7u8; 32]
    }

    #[test]
    fn encrypt_then_decrypt_roundtrips() {
        let key = test_key();
        let item_id = "test-android-item".to_string();
        let blob = encrypt_text(item_id.clone(), b"hello android", &key).expect("encrypt");
        let plaintext =
            decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key).expect("decrypt");
        assert_eq!(plaintext, b"hello android");
    }

    /// v0.3 regression: ciphertext is bound to item_id via AAD — decrypting
    /// with a different item_id must fail with `DecryptionFailed` rather than
    /// silently returning plaintext (legacy empty-AAD fallback removed in
    /// 1c55e57).
    #[test]
    fn decrypt_rejects_mismatched_item_id() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"secret", &key).expect("encrypt");
        let err = decrypt_text("item-B".into(), &blob.ciphertext, &blob.nonce, &key)
            .expect_err("mismatched item_id must reject");
        assert!(
            matches!(err, CopypasteError::DecryptionFailed { .. }),
            "expected DecryptionFailed, got {err:?}"
        );
    }

    /// v0.3 OI-7: a panic raised inside a wrapped UniFFI body must surface
    /// as `CopypasteError::Panicked` (via the `From<PanicError>` impl) rather
    /// than aborting the process.
    #[test]
    fn panic_boundary_converts_to_copypaste_panicked() {
        let result: Result<(), CopypasteError> = panic_boundary::catch_result(|| {
            panic!("synthetic panic inside FFI body");
        });
        match result {
            Err(CopypasteError::Panicked { message }) => {
                assert!(
                    message.contains("synthetic panic inside FFI body"),
                    "expected panic message in error, got: {message}"
                );
            }
            other => panic!("expected CopypasteError::Panicked, got {other:?}"),
        }
    }

    #[test]
    fn add_clipboard_item_rejects_bad_key() {
        let err = add_clipboard_item("/tmp/copypaste-test.db".into(), &[0u8; 16], "x".into())
            .expect_err("16-byte key must error");
        assert!(matches!(err, CopypasteError::InvalidKeyLength));
    }

    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn add_clipboard_item_returns_stub_when_feature_off() {
        let id =
            add_clipboard_item("/dev/null".into(), &test_key(), "hello".into()).expect("stub path");
        assert_eq!(id, "stub-uniffi-not-live");
        let n = get_history_count("/dev/null".into(), &test_key()).expect("stub count");
        assert_eq!(n, 0);
    }

    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_persists_when_feature_on() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        let id = add_clipboard_item(
            path.to_string_lossy().into_owned(),
            &key,
            "live android body".into(),
        )
        .expect("insert");
        assert!(!id.is_empty(), "real insert returns a uuid");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1);
    }

    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_skips_sensitive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        // GitHub PAT pattern is detected by copypaste_core::detect.
        let pat = format!("ghp_{}", "A".repeat(36));
        let id = add_clipboard_item(path.to_string_lossy().into_owned(), &key, pat)
            .expect("sensitive returns Ok empty");
        assert!(id.is_empty(), "sensitive content yields empty id");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 0, "no row inserted for sensitive content");
    }
}
