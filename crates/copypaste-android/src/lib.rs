uniffi::include_scaffolding!("copypaste_android");

use copypaste_core::{encrypt_item, decrypt_item, detect, NONCE_SIZE};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
}

#[derive(uniffi::Record)]
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
        .map_err(|_| CopypasteError::DecryptionFailed("wrong nonce length".into()))?;
    decrypt_item(ciphertext, &nonce_arr, &key_arr)
        .map_err(|e| CopypasteError::DecryptionFailed(e.to_string()))
}

pub fn is_sensitive(text: String) -> bool {
    detect(&text).is_some()
}

pub fn sensitive_kind(text: String) -> Option<String> {
    detect(&text).map(|k| format!("{:?}", k))
}

// Database handle table (non-Send Database wrapped in Mutex)
static DB_HANDLES: Mutex<HashMap<u64, copypaste_core::Database>> =
    Mutex::new(HashMap::new());
static NEXT_HANDLE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub fn open_database(path: String) -> Result<u64, CopypasteError> {
    let db = copypaste_core::Database::open(std::path::Path::new(&path))
        .map_err(|e| CopypasteError::DatabaseError(e.to_string()))?;
    let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    DB_HANDLES.lock().unwrap().insert(handle, db);
    Ok(handle)
}

pub fn close_database(handle: u64) {
    DB_HANDLES.lock().unwrap().remove(&handle);
}
