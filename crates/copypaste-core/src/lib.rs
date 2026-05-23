pub mod config;
pub mod crypto;
pub mod image;
pub mod logging;
pub mod sensitive;
pub mod storage;

// Top-level re-exports
pub use config::AppConfig;
pub use crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};
pub use crypto::encrypt::{
    build_item_aad, decrypt_item_with_aad, encrypt_item_with_aad, EncryptError, AAD_SCHEMA_VERSION,
    NONCE_SIZE,
};
pub use crypto::{DeviceKeypair, KeyError};
pub use image::{
    chunks_from_blob, chunks_to_blob, decode_image, encode_image, thumbnail, ImageError, ImageMeta,
    IMAGE_CHUNK_SIZE, MAX_IMAGE_BYTES,
};
pub use sensitive::{
    detect, is_sensitive_app, luhn_valid, redact, PatternMatch, SensitiveCategory,
    SensitiveDetector, SensitiveKind,
};
pub use storage::items::{
    count_items, delete_expired, delete_fts, delete_item, delete_sensitive_expired,
    find_recent_by_hash, get_page, insert_item, pin_item, search_items, upsert_fts, ClipboardItem,
    ItemsError,
};
pub use storage::{Database, DbError};
