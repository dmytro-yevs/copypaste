//! The read side: what a session may advertise, what it may compare against,
//! and what it may serve.

use copypaste_p2p::protocol::ItemSummary;
use rusqlite::{params, OptionalExtension};

use super::{LocalVersion, Meta, MetaError, StoredVersion};

/// Ceiling on how many summaries one session advertises.
///
/// `sync::advertise` truncates to `MAX_SUMMARIES_PER_MESSAGE` itself, so this
/// exists to bound the *query*, not the message: without it a history at the
/// 10 000-item cap plus its tombstones would be read out of SQLite in full only
/// to be thrown away.
const SUMMARY_LIMIT: i64 = copypaste_p2p::protocol::MAX_SUMMARIES_PER_MESSAGE as i64;

impl Meta {
    /// Everything eligible to sync, newest first, tombstones included.
    ///
    /// **Sensitive items are excluded here and nowhere else matters more.**
    /// This is the query that decides what leaves the device: `sync::advertise`
    /// turns the result into the session's `advertised` set, and `serve_items`
    /// refuses to send anything outside it. A sensitive item that slipped into
    /// this list would be served on request.
    pub fn summaries(&self) -> Result<Vec<ItemSummary>, MetaError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(
            "SELECT id, created_at, deleted, content_hash \
               FROM clipboard_items \
              WHERE is_sensitive = 0 \
              ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([SUMMARY_LIMIT], |row| {
            Ok(ItemSummary {
                item_id: row.get(0)?,
                created_at: row.get(1)?,
                deleted: row.get::<_, i64>(2)? != 0,
                content_hash: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The local version of one item — live, tombstoned, or sensitive.
    ///
    /// Sensitive rows are *included*: `apply` has to compare against them, or a
    /// peer's copy of something this device flagged would be stored a second
    /// time under the same id.
    pub fn local_version(&self, item_id: &str) -> Result<Option<LocalVersion>, MetaError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(
            "SELECT ci.created_at, ci.deleted, ci.content_hash, ci.is_sensitive, \
                    o.origin_device_id, ci.pinned \
               FROM clipboard_items ci \
               LEFT JOIN sync_item_origin o ON o.item_id = ci.id \
              WHERE ci.id = ?1",
        )?;
        let found = stmt
            .query_row([item_id], |row| {
                let origin: Option<String> = row.get(4)?;
                Ok(LocalVersion {
                    summary: ItemSummary {
                        item_id: item_id.to_string(),
                        created_at: row.get(0)?,
                        deleted: row.get::<_, i64>(1)? != 0,
                        content_hash: row.get(2)?,
                    },
                    // An item with no recorded origin was captured here: the
                    // origin table only gains a row for something that arrived
                    // from a peer or was ingested after this feature existed.
                    origin_device_id: origin.unwrap_or_else(|| self.device_id().to_string()),
                    is_sensitive: row.get::<_, i64>(3)? != 0,
                    pinned: row.get::<_, i64>(5)? != 0,
                })
            })
            .optional()?;
        Ok(found)
    }

    /// Every version stamped at or after `since_ms`, oldest first, still
    /// encrypted and with sensitive items excluded.
    ///
    /// This is the cloud transport's outbound query, and the two filters are
    /// the same two [`Meta::summaries`] applies for a peer session: sensitive
    /// items never leave the device, and a tombstone is a version like any
    /// other. `>=` rather than `>` because the cursor is inclusive — re-sending
    /// the boundary row costs one idempotent upsert, and excluding it loses
    /// every row that shares the boundary millisecond.
    ///
    /// Note that a tombstone keeps the *item's* `created_at`
    /// (`Store::delete` does not restamp), so deleting an item older than the
    /// cursor produces a version this query cannot see. See
    /// `crate::cloud::source` for what that costs and why it is not repaired
    /// here.
    pub fn versions_since(
        &self,
        since_ms: i64,
        limit: i64,
    ) -> Result<Vec<StoredVersion>, MetaError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(
            "SELECT ci.id, ci.content_ciphertext, ci.nonce, ci.content_type, ci.content_hash, \
                    ci.created_at, ci.deleted, o.origin_device_id \
               FROM clipboard_items ci \
               LEFT JOIN sync_item_origin o ON o.item_id = ci.id \
              WHERE ci.is_sensitive = 0 AND ci.created_at >= ?1 \
              ORDER BY ci.created_at ASC, ci.id ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![since_ms, limit], |row| self.row_to_version(row))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The stamp of the oldest version this device would ever offer, if any.
    ///
    /// Used to reset a sync cursor to "everything": a bulk delete has to
    /// re-offer versions the cursor has already passed, and this is the bound
    /// on how far back that goes.
    pub fn oldest_version_ms(&self) -> Result<Option<i64>, MetaError> {
        let conn = self.lock()?;
        let oldest: Option<i64> = conn.query_row(
            "SELECT MIN(created_at) FROM clipboard_items WHERE is_sensitive = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(oldest)
    }

    /// The rows behind a request, still encrypted. Sensitive items are omitted.
    ///
    /// Unknown ids are omitted rather than erroring, which is what
    /// `SyncSource::fetch` promises.
    pub fn fetch(&self, ids: &[String]) -> Result<Vec<StoredVersion>, MetaError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.lock()?;
        // One statement per batch size rather than a bound-parameter loop: the
        // batch is at most `MAX_ITEMS_PER_MESSAGE`.
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT ci.id, ci.content_ciphertext, ci.nonce, ci.content_type, ci.content_hash, \
                    ci.created_at, ci.deleted, o.origin_device_id \
               FROM clipboard_items ci \
               LEFT JOIN sync_item_origin o ON o.item_id = ci.id \
              WHERE ci.is_sensitive = 0 AND ci.id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(ids.iter());
        let rows = stmt.query_map(params, |row| self.row_to_version(row))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The shared projection behind [`Meta::fetch`] and [`Meta::versions_since`].
    ///
    /// One mapper for one column list: the two queries differ in their `WHERE`
    /// clause and in nothing else, and a second copy of this is how the origin
    /// fallback below ends up applied on one path and not the other.
    fn row_to_version(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVersion> {
        let origin: Option<String> = row.get(7)?;
        Ok(StoredVersion {
            item_id: row.get(0)?,
            content_ciphertext: row.get(1)?,
            nonce: row.get(2)?,
            content_type: row.get(3)?,
            content_hash: row.get(4)?,
            created_at: row.get(5)?,
            deleted: row.get::<_, i64>(6)? != 0,
            // An item with no recorded origin was captured here.
            origin_device_id: origin.unwrap_or_else(|| self.device_id().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::meta::testutil::{fixture, insert};

    #[test]
    fn summaries_exclude_sensitive_items_and_include_tombstones() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        insert(&f.store, "secret", "hash-secret", 2_000, true);
        insert(&f.store, "gone", "hash-gone", 3_000, false);
        assert!(f.store.delete("gone").unwrap());

        let summaries = f.meta.summaries().unwrap();
        let ids: Vec<&str> = summaries.iter().map(|s| s.item_id.as_str()).collect();
        assert!(ids.contains(&"plain"), "{ids:?}");
        assert!(
            ids.contains(&"gone"),
            "a tombstone is a version, not an absence"
        );
        assert!(
            !ids.contains(&"secret"),
            "a sensitive item must never be advertised"
        );

        let tombstone = summaries.iter().find(|s| s.item_id == "gone").unwrap();
        assert!(tombstone.deleted);
        assert_eq!(tombstone.content_hash, "hash-gone");
    }

    #[test]
    fn fetch_omits_sensitive_and_unknown_ids() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        insert(&f.store, "secret", "hash-secret", 2_000, true);

        let rows = f
            .meta
            .fetch(&[
                "plain".to_string(),
                "secret".to_string(),
                "never-existed".to_string(),
            ])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item_id, "plain");
        assert_eq!(rows[0].content_ciphertext.as_deref(), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn an_item_with_no_recorded_origin_belongs_to_this_device() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        let local = f.meta.local_version("plain").unwrap().expect("known item");
        assert_eq!(local.origin_device_id, f.meta.device_id());
    }
}
