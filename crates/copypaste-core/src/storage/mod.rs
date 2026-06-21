pub mod db;
pub mod devices;
pub mod items;
pub mod migration_v4;
pub mod pool;
mod schema;
pub use db::{Database, DbError, MigrationState};
pub use devices::{
    ensure_revoked_devices_table, list_revoked_devices, revoke_device, revoke_devices,
    DevicesError, RevokedDevice,
};
pub use items::{
    bump_item_recency, compute_content_hash, count_items, delete_expired, delete_fts, delete_item,
    find_recent_by_hash, get_item_by_id, get_key_version, get_page, get_page_meta,
    get_page_pinned_first, get_page_pinned_first_lamport, incremental_vacuum, insert_item,
    insert_item_with_fts, insert_tombstone, next_lamport_ts, pin_item, prune_to_cap,
    reorder_pinned, search_items, search_items_filtered, soft_delete_item, upsert_fts,
    ClipboardItem, ItemsError, ITEM_KEY_VERSION_CURRENT,
};
pub use migration_v4::{
    migrate_v1_image_chunks_to_v2, migrate_v1_to_v2_keys, repair_mislabeled_kv2_blob_rows,
    MigrationV4Error,
};
pub use pool::{open_pool, open_pool_with_cache_mb, DbRead, PoolError, ReadHandle, SqlitePool};
