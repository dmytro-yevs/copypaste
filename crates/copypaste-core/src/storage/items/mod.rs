mod delete;
mod fts;
mod ids;
mod insert;
mod pinned;
mod query;
mod types;

/// CopyPaste-crh3.94: build a comma-separated `?` placeholder list of length `n`
/// for a dynamic SQL `IN (...)` clause (`n == 3` → `"?,?,?"`). The count is the
/// ONLY thing interpolated into the SQL string — values are always bound as
/// parameters — so this stays injection-safe. Previously this 3-line idiom was
/// copy-pasted across delete.rs (×3) and fts.rs.
pub(crate) fn sql_placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(",")
}

/// Current HKDF key generation written into the `key_version` column for
/// freshly-inserted rows. Pinned here (rather than re-exported from
/// `crypto::keys`) because the storage layer needs an i64 value matching the
/// column type and the on-disk meaning is "which key/AAD format to use at
/// decrypt time" — a storage concern, not a crypto-derivation concern.
///
/// Increase from 2 → N in lockstep with a future HKDF-v3 family + a
/// corresponding migration helper in `super::migration_v4`.
pub const ITEM_KEY_VERSION_CURRENT: i64 = 2;

pub use delete::{
    delete_expired, delete_fts, delete_item, delete_sensitive_expired, has_sensitive_items,
    incremental_vacuum, prune_to_cap, soft_delete_item, soft_delete_item_in_tx,
};
pub use fts::{
    compute_content_hash, fetch_text_preview, fetch_text_previews_batch, get_device_names,
    search_items, search_items_filtered, upsert_fts, MAX_PREVIEW_BYTES,
};
pub use insert::{
    backfill_origin_device_id, get_key_version, insert_item, insert_item_with_fts, insert_tombstone,
};
pub use pinned::{mark_sensitive, pin_item, reorder_pinned, set_thumb, unpin_item};
pub use query::{
    bump_item_recency, count_items, decrypt_page, exists_item_by_item_id, find_recent_by_hash,
    get_item_by_id, get_item_by_item_id, get_page, get_page_meta, get_page_pinned_first,
    get_page_pinned_first_lamport, DecryptedPage,
};
pub use ids::{ItemId, RowId};
pub use types::{next_lamport_ts, ClipboardItem, ItemsError};

// Test-only re-exports of private helpers accessed from tests.rs.
// pub(crate) so the glob `use super::*` in the child module can reach them.
#[cfg(test)]
pub(crate) use fts::{clamp_preview, sanitize_fts5_query};
#[cfg(test)]
pub(crate) use types::now_ms_epoch;

#[cfg(test)]
mod tests;
