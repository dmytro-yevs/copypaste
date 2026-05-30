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
    build_item_aad, build_item_aad_v2, decrypt_item_by_version, decrypt_item_with_aad,
    encrypt_item_with_aad, EncryptError, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
};
#[allow(deprecated)]
pub use crypto::encrypt::{decrypt_item, encrypt_item};
pub use crypto::{
    decrypt_from_cloud, derive_storage_key_v1, derive_storage_key_v2, derive_sync_key,
    derive_sync_key_v2, derive_telemetry_key_v2, derive_v2, encrypt_for_cloud, DeviceKeypair,
    KeyError, PairingPayload, PairingQrError, PairingToken, SyncKey, SyncKeyError,
    ARGON2_M_COST_KIB, ARGON2_P_COST, ARGON2_SYNC_SALT, ARGON2_T_COST, CLOUD_AAD_SCHEMA_VERSION,
    HKDF_VERSION, PAIRING_QR_MAGIC, PAIRING_TOKEN_LEN,
};
pub use image::{
    chunks_from_blob, chunks_to_blob, decode_clipboard_image, decode_image, encode_as_png,
    encode_image, thumbnail, ImageError, ImageMeta, IMAGE_CHUNK_SIZE, MAX_IMAGE_BYTES,
};
pub use sensitive::{
    detect, is_sensitive_app, luhn_valid, redact, PatternMatch, SensitiveCategory,
    SensitiveDetector, SensitiveKind,
};
pub use storage::devices::{
    ensure_revoked_devices_table, list_revoked_devices, revoke_device, revoke_devices,
    DevicesError, RevokedDevice,
};
pub use storage::items::{
    bump_item_recency, count_items, delete_expired, delete_fts, delete_item,
    delete_sensitive_expired, exists_item_by_item_id, fetch_text_preview, find_recent_by_hash,
    get_item_by_id, get_item_by_item_id, get_page, get_page_meta, get_page_pinned_first,
    insert_item, insert_item_with_fts, pin_item, search_items, unpin_item, upsert_fts,
    ClipboardItem, ItemsError, ITEM_KEY_VERSION_CURRENT, MAX_PREVIEW_BYTES,
};
pub use storage::{Database, DbError, MigrationState};
