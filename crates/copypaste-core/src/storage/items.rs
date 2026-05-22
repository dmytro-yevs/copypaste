use rusqlite::params;
use uuid::Uuid;
use thiserror::Error;
use super::db::Database;

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
        }
    }
}

pub fn insert_item(db: &Database, item: &ClipboardItem) -> Result<(), ItemsError> {
    db.conn().execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            item.id, item.item_id, item.content_type,
            item.content, item.content_nonce, item.blob_ref,
            item.is_sensitive as i64, item.is_synced as i64,
            item.lamport_ts, item.wall_time, item.expires_at,
            item.app_bundle_id,
        ],
    )?;
    Ok(())
}

pub fn get_page(db: &Database, limit: usize, offset: usize) -> Result<Vec<ClipboardItem>, ItemsError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id
         FROM clipboard_items ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt.query_map(params![limit as i64, offset as i64], row_to_item)?
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

pub fn delete_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn().execute("DELETE FROM clipboard_items WHERE id=?1", params![id])?;
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
    Ok(db.conn().query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))?)
}

/// Insert or replace a plaintext snippet into the FTS5 index.
/// `plaintext` must already be decrypted by the caller.
/// Call this once per item after `insert_item`.
pub fn upsert_fts(db: &Database, id: &str, plaintext: &str) -> Result<(), ItemsError> {
    // FTS5 does not support ON CONFLICT; DELETE + INSERT is the correct upsert pattern.
    db.conn().execute(
        "DELETE FROM clipboard_fts WHERE id = ?1",
        params![id],
    )?;
    db.conn().execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
        params![id, plaintext],
    )?;
    Ok(())
}

/// Remove an item's entry from the FTS5 index.
/// Call this after `delete_item` or `delete_expired`.
pub fn delete_fts(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn().execute(
        "DELETE FROM clipboard_fts WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Search clipboard items by full-text query.
/// Returns up to `limit` full `ClipboardItem` rows ordered by FTS5 rank (best match first).
///
/// Two-phase fetch: (1) query FTS5 for matching IDs ordered by rank, (2) fetch full rows from
/// clipboard_items by IN-list, then re-sort in Rust to restore FTS rank order.
pub fn search_items(db: &Database, query: &str, limit: usize) -> Result<Vec<ClipboardItem>, ItemsError> {
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
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id
         FROM clipboard_items
         WHERE id IN ({})",
        placeholders,
    );

    let mut stmt = db.conn().prepare(&sql)?;
    let mut rows: Vec<ClipboardItem> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;

    // Re-sort to match FTS5 rank order (IN-list returns rows in storage order)
    rows.sort_by_key(|item| ids.iter().position(|id| id == &item.id).unwrap_or(usize::MAX));

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
        for i in 0..10 { insert_item(&db, &make_item(i)).unwrap(); }
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

        let count: i64 = db.conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Upsert again with different text — must not duplicate
        upsert_fts(&db, &item.id, "updated text").unwrap();
        let count2: i64 = db.conn()
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

        let count: i64 = db.conn()
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
}
