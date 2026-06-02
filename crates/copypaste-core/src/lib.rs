pub mod config;
pub mod crypto;
pub mod file;
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
// `encrypt_item` / `decrypt_item` are deprecated (empty-AAD, replay-vulnerable)
// but still re-exported because `copypaste-daemon` and integration tests rely on
// them for legacy v1-key ciphertext handling during the migration sweep. The
// `#[allow(deprecated)]` suppresses the lint at the `pub use` site; callers that
// import these functions from the crate root will still see the deprecation warning
// and must migrate to `encrypt_item_with_aad` / `decrypt_item_with_aad`. This
// re-export will be removed once the v4 migration sweep is complete and no
// callers remain.
#[allow(deprecated)]
pub use crypto::encrypt::{decrypt_item, encrypt_item};
pub use crypto::{
    decrypt_from_cloud, derive_storage_key_v1, derive_storage_key_v2, derive_sync_key,
    derive_sync_key_v2, derive_telemetry_key_v2, derive_v2, encrypt_for_cloud, strip_deeplink,
    DeviceKeypair, KeyError, PairingPayload, PairingQrError, PairingToken, SyncKey, SyncKeyError,
    ARGON2_M_COST_KIB, ARGON2_P_COST, ARGON2_SYNC_SALT, ARGON2_T_COST, CLOUD_AAD_SCHEMA_VERSION,
    HKDF_VERSION, PAIRING_DEEPLINK_PREFIX, PAIRING_QR_MAGIC, PAIRING_TOKEN_LEN,
};
pub use file::{decode_file, encode_file, FileError, FileMeta, FILE_CHUNK_SIZE, MAX_FILE_BYTES};
pub use image::{
    chunks_from_blob, chunks_to_blob, decode_clipboard_image, decode_clipboard_image_limited,
    decode_image, decode_thumbnail, encode_as_png, encode_image, encode_image_full,
    encode_image_with_limit, encode_thumbnail, thumbnail, ImageError, ImageMeta, IMAGE_CHUNK_SIZE,
    MAX_IMAGE_BYTES, THUMBNAIL_MAX_DIM,
};
pub use sensitive::{
    detect, is_sensitive_app, is_sensitive_for_autowipe, luhn_valid, redact, PatternMatch,
    SensitiveCategory, SensitiveDetector, SensitiveKind,
};
pub use storage::devices::{
    ensure_revoked_devices_table, list_revoked_devices, revoke_device, revoke_devices,
    DevicesError, RevokedDevice,
};
pub use storage::items::{
    bump_item_recency, count_items, delete_expired, delete_fts, delete_item,
    delete_sensitive_expired, exists_item_by_item_id, fetch_text_preview, find_recent_by_hash,
    get_item_by_id, get_item_by_item_id, get_page, get_page_meta, get_page_pinned_first,
    insert_item, insert_item_with_fts, pin_item, prune_to_cap, reorder_pinned, search_items,
    set_thumb, unpin_item, upsert_fts, ClipboardItem, ItemsError, ITEM_KEY_VERSION_CURRENT,
    MAX_PREVIEW_BYTES,
};
pub use storage::{Database, DbError, MigrationState};
