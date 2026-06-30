pub mod config;
pub mod crypto;
pub mod file;
pub mod filename_security;
pub mod image;
pub mod logging;
pub mod relay;
pub mod sensitive;
pub mod storage;
pub mod text_kind;

// Top-level re-exports
pub use config::AppConfig;
pub use crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};
pub use crypto::encrypt::{
    build_item_aad, build_item_aad_v2, decrypt_item_by_version, decrypt_item_with_aad,
    encrypt_item_with_aad, EncryptError, V1Key, V2Key, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4,
    NONCE_SIZE,
};
pub use relay::{derive_relay_inbox_id, derive_relay_public_key, derive_relay_registration_pop};
// `encrypt_item` / `decrypt_item` were deprecated empty-AAD wrappers.
// Audit confirmed no production callers — only bench group-name strings and
// comments referenced them. Re-export removed; use `encrypt_item_with_aad` /
// `decrypt_item_with_aad` for all new call sites.
pub use crypto::{
    decrypt_from_cloud, derive_storage_key_v1, derive_storage_key_v2, derive_sync_key,
    derive_sync_key_v2, derive_telemetry_key_v2, derive_v2, encrypt_for_cloud, strip_deeplink,
    DeviceKeypair, KeyError, PairingPayload, PairingQrError, PairingToken, QrProvisioning, SyncKey,
    SyncKeyError, ARGON2_M_COST_KIB, ARGON2_P_COST, ARGON2_T_COST, CLOUD_AAD_SCHEMA_VERSION,
    HKDF_VERSION, MIN_PASSPHRASE_LEN, PAIRING_DEEPLINK_PREFIX, PAIRING_QR_MAGIC, PAIRING_TOKEN_LEN,
    PER_ACCOUNT_SALT_IKM,
};
pub use file::{
    decode_file, decode_file_zeroizing, encode_file, FileError, FileMeta, FILE_CHUNK_SIZE,
    MAX_FILE_BYTES,
};
pub use filename_security::{is_dangerous_extension, sanitize_filename};
pub use image::{
    chunks_from_blob, chunks_to_blob, decode_clipboard_image, decode_clipboard_image_limited,
    decode_image, decode_image_zeroizing, decode_thumbnail, decode_thumbnail_zeroizing,
    encode_as_png, encode_image, encode_image_full, encode_image_with_limit, encode_thumbnail,
    encode_thumbnail_from_png, thumb_dims_exceed_cap, thumbnail, ImageError, ImageMeta,
    IMAGE_CHUNK_SIZE, MAX_IMAGE_BYTES, THUMBNAIL_MAX_DIM,
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
    bump_item_recency, compute_content_hash, count_items, decrypt_page, delete_expired, delete_fts,
    delete_item, delete_sensitive_expired, exists_item_by_item_id, fetch_text_preview,
    fetch_text_previews_batch, find_recent_by_hash, get_device_names, get_item_by_id,
    get_item_by_item_id, get_page, get_page_meta, get_page_pinned_first,
    get_page_pinned_first_lamport, has_sensitive_items, incremental_vacuum, insert_item,
    insert_item_with_fts, insert_tombstone, next_lamport_ts, pin_item, prune_to_cap,
    reorder_pinned, search_items, search_items_filtered, set_thumb, soft_delete_item, unpin_item,
    upsert_fts, ClipboardItem, DecryptedPage, ItemId, ItemsError, RowId, ITEM_KEY_VERSION_CURRENT,
    MAX_PREVIEW_BYTES,
};
pub use storage::{open_pool, open_pool_with_cache_mb, PoolError, SqlitePool};
pub use storage::{Database, DbError, DbRead, MigrationState, ReadHandle};
