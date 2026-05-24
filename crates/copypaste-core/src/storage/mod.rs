pub mod db;
pub mod devices;
pub mod items;
pub mod migration_v4;
pub mod pool;
mod schema;
pub use db::{Database, DbError, MigrationState};
pub use devices::{
    ensure_revoked_devices_table, list_revoked_devices, revoke_device, DevicesError, RevokedDevice,
};
pub use items::{
    count_items, delete_expired, delete_fts, delete_item, find_recent_by_hash, get_key_version,
    get_page, get_page_meta, insert_item, insert_item_with_fts, pin_item, search_items, upsert_fts,
    ClipboardItem, ItemsError, ITEM_KEY_VERSION_CURRENT,
};
pub use migration_v4::{migrate_v1_to_v2_keys, MigrationV4Error};
pub use pool::{open_pool, PoolError, SqlitePool};
