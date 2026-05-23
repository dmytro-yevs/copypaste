pub mod config;
pub mod crypto;
pub mod image;
pub mod logging;
pub mod sensitive;
pub mod storage;

// Top-level re-exports
pub use config::AppConfig;
pub use crypto::{DeviceKeypair, KeyError};
pub use crypto::encrypt::{
    build_item_aad, decrypt_item, decrypt_item_with_aad, encrypt_item, encrypt_item_with_aad,
    EncryptError, AAD_SCHEMA_VERSION, NONCE_SIZE,
};
pub use crypto::chunks::{encrypt_chunks, decrypt_chunks, EncryptedChunk, ChunkError};
pub use image::{
    encode_image, decode_image, chunks_to_blob, chunks_from_blob, thumbnail,
    ImageError, ImageMeta, IMAGE_CHUNK_SIZE, MAX_IMAGE_BYTES,
};
pub use sensitive::{
    detect, redact, luhn_valid, is_sensitive_app,
    SensitiveKind, SensitiveCategory, SensitiveDetector, PatternMatch,
};
pub use storage::{Database, DbError};
pub use storage::items::{
    ClipboardItem, ItemsError,
    insert_item, get_page, delete_expired, delete_sensitive_expired, delete_item, count_items,
    upsert_fts, delete_fts, search_items, pin_item, find_recent_by_hash,
};
