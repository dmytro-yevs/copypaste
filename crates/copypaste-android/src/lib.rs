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
    let (nonce, ciphertext) = encrypt_item(bytes, &key_arr);
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
