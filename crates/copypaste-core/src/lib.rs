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
    build_item_aad, build_item_aad_v2, decrypt_item, decrypt_item_by_version, decrypt_item_with_aad,
    encrypt_item, encrypt_item_with_aad, EncryptError, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4,
    NONCE_SIZE,
};
pub use crypto::{
    derive_storage_key_v1, derive_storage_key_v2, derive_sync_key_v2, derive_telemetry_key_v2,
    derive_v2, DeviceKeypair, KeyError, HKDF_VERSION,
};
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
    fetch_text_preview, find_recent_by_hash, get_page, get_page_meta, insert_item,
    insert_item_with_fts, pin_item, search_items, upsert_fts, ClipboardItem, ItemsError,
    MAX_PREVIEW_BYTES,
};
pub use storage::{Database, DbError, MigrationState};
pub use storage::devices::{
    ensure_revoked_devices_table, list_revoked_devices, revoke_device, DevicesError, RevokedDevice,
};
