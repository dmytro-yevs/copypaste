use super::db::Database;
use rusqlite::params;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ItemsError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub id: String,
    pub item_id: String,
    pub content_type: String,
    pub content: Option<Vec<u8>>,
    pub content_nonce: Option<Vec<u8>>,
    pub blob_ref: Option<String>,
    pub is_sensitive: bool,
    pub is_synced: bool,
    pub lamport_ts: i64,
    pub wall_time: i64,
    pub expires_at: Option<i64>,
    pub app_bundle_id: Option<String>,
    /// SHA-256 hex digest of the raw (pre-encryption) content bytes.
    /// Used for deduplication: skip insert if an identical hash was stored
    /// within the last 60 seconds.
    pub content_hash: Option<String>,
    /// UUID of the device that originated this item. Used as the deterministic
    /// tie-break in the LWW merge (see `copypaste-sync::merge::resolve`).
    /// Empty string for pre-v3 rows until backfilled via
    /// [`backfill_origin_device_id`].
    pub origin_device_id: String,
}

impl ClipboardItem {
    pub fn new_text(encrypted_content: Vec<u8>, nonce: Vec<u8>, lamport_ts: i64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            content_type: "text".to_string(),
            content: Some(encrypted_content),
            content_nonce: Some(nonce),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time: now,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: String::new(),
        }
    }

    /// Create an image item whose content is an encrypted chunk blob.
    ///
    /// `encrypted_blob` is produced by `copypaste_core::chunks_to_blob`.
    /// `image_meta_json` stores width/height/chunk_count/file_id as JSON in `blob_ref`.
    /// The `content_nonce` field is left `None` because XChaCha20 nonces are stored
    /// per-chunk inside the blob itself (no single item-level nonce needed).
    pub fn new_image(encrypted_blob: Vec<u8>, image_meta_json: String, lamport_ts: i64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            content_type: "image".to_string(),
            content: Some(encrypted_blob),
            content_nonce: None,
            blob_ref: Some(image_meta_json),
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time: now,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: String::new(),
        }
    }
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

pub fn insert_item(db: &Database, item: &ClipboardItem) -> Result<(), ItemsError> {
    db.conn().execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            ITEM_KEY_VERSION_CURRENT,
        ],
    )?;
    Ok(())
}

/// Read the `key_version` column for a single item row. Returns `None` if no
/// such row exists. Used by the migration sweep to spot-check that a row
/// landed on `key_version = 2` after re-encryption.
pub fn get_key_version(db: &Database, id: &str) -> Result<Option<i64>, ItemsError> {
    let result = db.conn().query_row(
        "SELECT key_version FROM clipboard_items WHERE id = ?1",
        params![id],
        |r| r.get::<_, i64>(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Atomically insert a clipboard item AND its FTS5 plaintext index
/// inside a single transaction.
///
/// Wraps `insert_item` + `upsert_fts` in `Connection::unchecked_transaction()`
/// so a crash between the two writes can't leave an orphan row with no FTS
/// entry (search would miss it forever).
///
/// Returns the `id` of the inserted row. On SQLITE_CONSTRAINT_UNIQUE from
/// the v5 dedup indexes (`idx_dedup_hash_minute`, `idx_clipboard_item_id`),
/// treats it as successful dedup: re-queries the existing row and returns
/// its id. Caller sees the same id it would have seen had
/// `find_recent_by_hash` won the race.
///
/// `plaintext_for_fts` is the already-decrypted text indexed for search.
/// Pass an empty string to skip FTS indexing (image items).
///
/// TODO(daemon-owner): existing daemon ingest paths still call
/// `insert_item` + `upsert_fts` as two separate steps. Switch to this new
/// fn to close the crash window.
pub fn insert_item_with_fts(
    db: &Database,
    item: &ClipboardItem,
    plaintext_for_fts: &str,
) -> Result<String, ItemsError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let insert_res = tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
        ],
    );

    if let Err(e) = insert_res {
        let is_unique_violation = matches!(
            &e,
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation
        );
        if is_unique_violation {
            drop(tx);
            if let Some(id) = lookup_existing_id(db, item)? {
                return Ok(id);
            }
        }
        return Err(ItemsError::Sqlite(e));
    }

    if !plaintext_for_fts.is_empty() {
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![item.id])?;
        tx.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![item.id, plaintext_for_fts],
        )?;
    }
    tx.commit()?;
    Ok(item.id.clone())
}

/// Find the id of an existing row that conflicts with `item` on one of
/// the v5 UNIQUE indexes. Tries `content_hash` first (the more common
/// race), then falls back to `item_id` (sync replay).
fn lookup_existing_id(db: &Database, item: &ClipboardItem) -> Result<Option<String>, ItemsError> {
    if let Some(hash) = &item.content_hash {
        let minute_bucket = item.wall_time / 60;
        let by_hash = db.conn().query_row(
            "SELECT id FROM clipboard_items
             WHERE content_hash = ?1 AND (wall_time / 60) = ?2
             ORDER BY wall_time DESC LIMIT 1",
            params![hash, minute_bucket],
            |row| row.get::<_, String>(0),
        );
        match by_hash {
            Ok(id) => return Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(ItemsError::Sqlite(e)),
        }
    }
    let by_item_id = db.conn().query_row(
        "SELECT id FROM clipboard_items WHERE item_id = ?1",
        params![item.item_id],
        |row| row.get::<_, String>(0),
    );
    match by_item_id {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Stamp `origin_device_id` on every row that currently carries the empty
/// default (pre-v3 rows, or rows inserted before the daemon knew its device
/// id). Idempotent — rows with a non-empty origin are left alone so items
/// received from peers preserve their original origin.
///
/// Returns the number of rows updated.
pub fn backfill_origin_device_id(
    db: &Database,
    local_device_id: &str,
) -> Result<usize, ItemsError> {
    let changed = db.conn().execute(
        "UPDATE clipboard_items SET origin_device_id = ?1 WHERE origin_device_id = ''",
        params![local_device_id],
    )?;
    Ok(changed)
}

/// Find the id of an item with the given content hash stored within the last
/// `within_ms` milliseconds. Returns `None` if no such item exists.
///
/// Used by the daemon to skip inserting duplicate clipboard content.
pub fn find_recent_by_hash(
    db: &Database,
    hash: &str,
    now_ms: i64,
    within_ms: i64,
) -> Result<Option<String>, ItemsError> {
    let cutoff = now_ms - within_ms;
    let result = db.conn().query_row(
        "SELECT id FROM clipboard_items
         WHERE content_hash = ?1 AND wall_time >= ?2
         ORDER BY wall_time DESC LIMIT 1",
        params![hash, cutoff],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

pub fn get_page(
    db: &Database,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id
         FROM clipboard_items ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit as i64, offset as i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn delete_expired(db: &Database, now_ms: i64) -> Result<usize, ItemsError> {
    let changed = db.conn().execute(
        "DELETE FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1",
        params![now_ms],
    )?;
    Ok(changed)
}

/// Delete sensitive items whose `wall_time` is older than `sensitive_ttl_ms` milliseconds ago.
/// This enforces a local auto-wipe TTL for items marked `is_sensitive = 1`.
pub fn delete_sensitive_expired(
    db: &Database,
    now_ms: i64,
    sensitive_ttl_ms: i64,
) -> Result<usize, ItemsError> {
    let threshold = now_ms - sensitive_ttl_ms;
    let changed = db.conn().execute(
        "DELETE FROM clipboard_items WHERE is_sensitive = 1 AND wall_time < ?1",
        params![threshold],
    )?;
    Ok(changed)
}

pub fn delete_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn()
        .execute("DELETE FROM clipboard_items WHERE id=?1", params![id])?;
    Ok(())
}

/// Remove expiry from an item so it's never auto-deleted.
pub fn pin_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn().execute(
        "UPDATE clipboard_items SET expires_at = NULL WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn count_items(db: &Database) -> Result<i64, ItemsError> {
    Ok(db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))?)
}

/// Insert or replace a plaintext snippet into the FTS5 index.
/// `plaintext` must already be decrypted by the caller.
/// Call this once per item after `insert_item`.
pub fn upsert_fts(db: &Database, id: &str, plaintext: &str) -> Result<(), ItemsError> {
    // FTS5 does not support ON CONFLICT; DELETE + INSERT is the correct upsert pattern.
    db.conn()
        .execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    db.conn().execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
        params![id, plaintext],
    )?;
    Ok(())
}

/// Remove an item's entry from the FTS5 index.
/// Call this after `delete_item` or `delete_expired`.
pub fn delete_fts(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn()
        .execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    Ok(())
}

/// Search clipboard items by full-text query.
/// Returns up to `limit` full `ClipboardItem` rows ordered by FTS5 rank (best match first).
///
/// Two-phase fetch: (1) query FTS5 for matching IDs ordered by rank, (2) fetch full rows from
/// clipboard_items by IN-list, then re-sort in Rust to restore FTS rank order.
pub fn search_items(
    db: &Database,
    query: &str,
    limit: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Phase 1: collect matching IDs ordered by FTS5 rank
    let mut fts_stmt = db.conn().prepare(
        "SELECT id FROM clipboard_fts WHERE content_text MATCH ?1 ORDER BY rank LIMIT ?2",
    )?;
    let ids: Vec<String> = fts_stmt
        .query_map(params![query, limit as i64], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    if ids.is_empty() {
        return Ok(vec![]);
    }

    // Phase 2: fetch full rows from clipboard_items using a dynamic IN-list.
    // Each placeholder is a separate `?` bound via params_from_iter.
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id
         FROM clipboard_items
         WHERE id IN ({})",
        placeholders,
    );

    let mut stmt = db.conn().prepare(&sql)?;
    let mut rows: Vec<ClipboardItem> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;

    // Re-sort to match FTS5 rank order (IN-list returns rows in storage order)
    rows.sort_by_key(|item| {
        ids.iter()
            .position(|id| id == &item.id)
            .unwrap_or(usize::MAX)
    });

    Ok(rows)
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        item_id: row.get(1)?,
        content_type: row.get(2)?,
        content: row.get(3)?,
        content_nonce: row.get(4)?,
        blob_ref: row.get(5)?,
        is_sensitive: row.get::<_, i64>(6)? != 0,
        is_synced: row.get::<_, i64>(7)? != 0,
        lamport_ts: row.get(8)?,
        wall_time: row.get(9)?,
        expires_at: row.get(10)?,
        app_bundle_id: row.get(11)?,
        content_hash: row.get(12)?,
        origin_device_id: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Database;

    fn make_item(lamport: i64) -> ClipboardItem {
        ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 24], lamport)
    }

    #[test]
    fn insert_and_count() {
        let db = Database::open_in_memory().unwrap();
        insert_item(&db, &make_item(1)).unwrap();
        insert_item(&db, &make_item(2)).unwrap();
        assert_eq!(count_items(&db).unwrap(), 2);
    }

    #[test]
    fn pagination_returns_correct_page() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..10 {
            insert_item(&db, &make_item(i)).unwrap();
        }
        let page1 = get_page(&db, 3, 0).unwrap();
        let page2 = get_page(&db, 3, 3).unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page2.len(), 3);
        let ids1: Vec<_> = page1.iter().map(|i| &i.id).collect();
        let ids2: Vec<_> = page2.iter().map(|i| &i.id).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }

    #[test]
    fn delete_expired_removes_old_items() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.expires_at = Some(1000);
        insert_item(&db, &item).unwrap();
        let mut item2 = make_item(2);
        item2.expires_at = None;
        insert_item(&db, &item2).unwrap();
        let removed = delete_expired(&db, 2000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(count_items(&db).unwrap(), 1);
    }

    #[test]
    fn delete_item_removes_specific_row() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();
        delete_item(&db, &id).unwrap();
        assert_eq!(count_items(&db).unwrap(), 0);
    }

    // --- Task 1: upsert_fts ---

    #[test]
    fn upsert_fts_inserts_and_replaces() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();

        upsert_fts(&db, &item.id, "hello world").unwrap();

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Upsert again with different text — must not duplicate
        upsert_fts(&db, &item.id, "updated text").unwrap();
        let count2: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count2, 1);
    }

    // --- Task 2: delete_fts ---

    #[test]
    fn delete_fts_removes_fts_entry() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "some text").unwrap();

        delete_fts(&db, &item.id).unwrap();

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_fts_nonexistent_id_is_ok() {
        let db = Database::open_in_memory().unwrap();
        // Should not error even if id doesn't exist
        delete_fts(&db, "nonexistent-id").unwrap();
    }

    // --- Task 3: search_items ---

    #[test]
    fn search_items_finds_matching_text() {
        let db = Database::open_in_memory().unwrap();
        let item1 = make_item(1);
        let item2 = make_item(2);
        insert_item(&db, &item1).unwrap();
        insert_item(&db, &item2).unwrap();
        upsert_fts(&db, &item1.id, "hello world clipboard").unwrap();
        upsert_fts(&db, &item2.id, "rust programming language").unwrap();

        let results = search_items(&db, "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, item1.id);
    }

    #[test]
    fn search_items_empty_query_returns_empty() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "hello world").unwrap();

        let results = search_items(&db, "", 10).unwrap();
        assert_eq!(results.len(), 0);

        let results2 = search_items(&db, "   ", 10).unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn search_items_no_match_returns_empty() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "hello world").unwrap();

        let results = search_items(&db, "nonexistentword", 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn search_items_respects_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            let item = make_item(i);
            insert_item(&db, &item).unwrap();
            upsert_fts(&db, &item.id, "common search term").unwrap();
        }

        let results = search_items(&db, "common", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn delete_sensitive_expired_removes_old_sensitive_items() {
        let db = Database::open_in_memory().unwrap();

        // Sensitive item with old wall_time (should be deleted)
        let mut old_sensitive = make_item(1);
        old_sensitive.is_sensitive = true;
        old_sensitive.wall_time = 1_000; // very old
        insert_item(&db, &old_sensitive).unwrap();

        // Sensitive item with recent wall_time (should be kept)
        let mut new_sensitive = make_item(2);
        new_sensitive.is_sensitive = true;
        new_sensitive.wall_time = 100_000_000; // very recent relative to now_ms below
        insert_item(&db, &new_sensitive).unwrap();

        // Non-sensitive item with old wall_time (should NOT be deleted)
        let mut old_plain = make_item(3);
        old_plain.is_sensitive = false;
        old_plain.wall_time = 1_000;
        insert_item(&db, &old_plain).unwrap();

        // now_ms = 200_000, ttl = 30_000 → threshold = 170_000
        // old_sensitive.wall_time=1000 < 170_000 → deleted
        // new_sensitive.wall_time=100_000_000 > 170_000 → kept
        // old_plain.wall_time=1000 < 170_000 but not sensitive → kept
        let removed = delete_sensitive_expired(&db, 200_000, 30_000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(count_items(&db).unwrap(), 2);
    }

    #[test]
    fn pin_item_removes_expiry() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.expires_at = Some(9999);
        insert_item(&db, &item).unwrap();
        pin_item(&db, &item.id).unwrap();
        // Verify expired returns 0 (pinned item not deleted)
        let removed = delete_expired(&db, 99999).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn newly_inserted_items_land_on_key_version_2() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();

        let kv = get_key_version(&db, &item.id).unwrap();
        assert_eq!(
            kv,
            Some(ITEM_KEY_VERSION_CURRENT),
            "insert_item must stamp the current key_version on new rows"
        );
        assert_eq!(ITEM_KEY_VERSION_CURRENT, 2);
    }

    #[test]
    fn get_key_version_missing_id_returns_none() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(get_key_version(&db, "nope").unwrap(), None);
    }

    #[test]
    fn insert_item_with_fts_writes_both_atomically() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();

        let returned = insert_item_with_fts(&db, &item, "hello clipboard world").unwrap();
        assert_eq!(returned, id, "fresh insert returns the supplied id");

        let row_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "item row must be present");

        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 1, "FTS row must be present");

        // Search round-trip — confirms the FTS index actually points at
        // the same id and is searchable.
        let results = search_items(&db, "clipboard", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn insert_item_with_fts_skips_fts_on_empty_text() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();

        let returned = insert_item_with_fts(&db, &item, "").unwrap();
        assert_eq!(returned, id);

        let row_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "item row inserted even when FTS skipped");

        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 0, "FTS row skipped for empty plaintext");
    }

    #[test]
    fn insert_item_with_fts_dedup_returns_existing_id_on_hash_race() {
        let db = Database::open_in_memory().unwrap();

        // First insert: stamped with a content_hash.
        let mut first = make_item(1);
        first.content_hash = Some("abc123".to_string());
        first.wall_time = 60_000; // bucket = 60_000 / 60 = 1000
        let first_id = insert_item_with_fts(&db, &first, "hello").unwrap();

        // Second insert: distinct logical id but same hash AND same
        // minute bucket → idx_dedup_hash_minute fires.
        let mut second = make_item(2);
        second.content_hash = Some("abc123".to_string());
        second.wall_time = 60_059; // 60_059 / 60 = 1000 (same bucket)
        let returned = insert_item_with_fts(&db, &second, "hello again").unwrap();

        assert_eq!(
            returned, first_id,
            "dedup race must return the existing row's id, not the new one"
        );
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "second insert must not create a duplicate row");
    }

    #[test]
    fn insert_item_with_fts_dedup_returns_existing_id_on_item_id_race() {
        let db = Database::open_in_memory().unwrap();

        let first = make_item(1);
        let first_id = insert_item_with_fts(&db, &first, "").unwrap();

        // Sync replay: peer re-broadcasts the same item_id with a new
        // logical id. idx_clipboard_item_id fires.
        let mut second = make_item(2);
        second.item_id = first.item_id.clone();
        let returned = insert_item_with_fts(&db, &second, "").unwrap();

        assert_eq!(returned, first_id);
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn backfill_origin_device_id_only_touches_empty_rows() {
        let db = Database::open_in_memory().unwrap();

        // Row A: empty origin (pre-v3 default) → must be backfilled.
        let mut a = make_item(1);
        a.origin_device_id = String::new();
        insert_item(&db, &a).unwrap();

        // Row B: already-set origin (item received from peer "peer-xyz") →
        // must remain untouched so peer-origin items keep their provenance.
        let mut b = make_item(2);
        b.origin_device_id = "peer-xyz".to_string();
        insert_item(&db, &b).unwrap();

        let changed = backfill_origin_device_id(&db, "local-uuid").unwrap();
        assert_eq!(changed, 1, "only the empty-origin row must be updated");

        let got_a: String = db
            .conn()
            .query_row(
                "SELECT origin_device_id FROM clipboard_items WHERE id = ?1",
                params![a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got_a, "local-uuid");

        let got_b: String = db
            .conn()
            .query_row(
                "SELECT origin_device_id FROM clipboard_items WHERE id = ?1",
                params![b.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got_b, "peer-xyz", "peer origin must not be overwritten");
    }
}
