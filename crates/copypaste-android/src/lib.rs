#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

use copypaste_core::{encrypt_item, decrypt_item, detect, NONCE_SIZE};
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
}

pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub fn encrypt_text(bytes: &[u8], key: &[u8]) -> Result<EncryptedBlob, CopypasteError> {
    let key_arr: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    let (nonce, ciphertext) =
        encrypt_item(bytes, &key_arr).map_err(|_| CopypasteError::EncryptionFailed)?;
    Ok(EncryptedBlob { nonce: nonce.to_vec(), ciphertext })
}

pub fn decrypt_text(ciphertext: &[u8], nonce: &[u8], key: &[u8]) -> Result<Vec<u8>, CopypasteError> {
    let key_arr: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    let nonce_arr: [u8; NONCE_SIZE] = nonce.try_into()
        .map_err(|_| CopypasteError::DecryptionFailed { message: "wrong nonce length".into() })?;
    decrypt_item(ciphertext, &nonce_arr, &key_arr)
        .map_err(|e| CopypasteError::DecryptionFailed { message: e.to_string() })
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
    let key_arr: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    let db = copypaste_core::Database::open(std::path::Path::new(&path), &key_arr)
        .map_err(|e| CopypasteError::DatabaseError { message: e.to_string() })?;
    let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    db_handles().lock().unwrap().insert(handle, db);
    Ok(handle)
}

pub fn close_database(handle: u64) {
    db_handles().lock().unwrap().remove(&handle);
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
    let key_arr: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;

    // Skip sensitive content (caller-visible: empty string return).
    if detect(&text).is_some() {
        return Ok(String::new());
    }

    let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr)
        .map_err(|e| CopypasteError::DatabaseError { message: e.to_string() })?;

    let (nonce, ciphertext) = encrypt_item(text.as_bytes(), &key_arr)
        .map_err(|_| CopypasteError::EncryptionFailed)?;

    let lamport_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let item = copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport_ts);
    let id = item.id.clone();

    copypaste_core::insert_item(&db, &item)
        .map_err(|e| CopypasteError::DatabaseError { message: e.to_string() })?;

    Ok(id)
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn add_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
) -> Result<String, CopypasteError> {
    // Validate key shape to mirror the live path's error surface.
    let _: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    Ok("stub-uniffi-not-live".to_string())
}

#[cfg(feature = "android-uniffi-live")]
pub fn get_history_count(db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    let key_arr: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr)
        .map_err(|e| CopypasteError::DatabaseError { message: e.to_string() })?;
    let n = copypaste_core::count_items(&db)
        .map_err(|e| CopypasteError::DatabaseError { message: e.to_string() })?;
    Ok(n.max(0) as u64)
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_count(_db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    let _: [u8; 32] = key.try_into().map_err(|_| CopypasteError::InvalidKeyLength)?;
    Ok(0)
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
        let blob = encrypt_text(b"hello android", &key).expect("encrypt");
        let plaintext = decrypt_text(&blob.ciphertext, &blob.nonce, &key).expect("decrypt");
        assert_eq!(plaintext, b"hello android");
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
        let id = add_clipboard_item("/dev/null".into(), &test_key(), "hello".into())
            .expect("stub path");
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
