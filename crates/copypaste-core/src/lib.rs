pub mod config;
pub mod crypto;
pub mod sensitive;
pub mod storage;

// Top-level re-exports
pub use config::AppConfig;
pub use crypto::{DeviceKeypair, KeyError};
pub use crypto::encrypt::{encrypt_item, decrypt_item, EncryptError, NONCE_SIZE};
pub use crypto::chunks::{encrypt_chunks, decrypt_chunks, EncryptedChunk, ChunkError};
pub use sensitive::{detect, SensitiveKind};
pub use storage::{Database, DbError};
pub use storage::items::{
    ClipboardItem, ItemsError,
    insert_item, get_page, delete_expired, delete_item, count_items,
    upsert_fts, delete_fts, search_items, pin_item,
};
