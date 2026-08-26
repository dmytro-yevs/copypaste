//! The FTS5 layer and the `CopyPaste-i6pp` enforcement that keeps sensitive content out
//! of it. Counted in one place:
//!
//! 1. **Write guard** — [`super::Store::insert`] drops any `search_text` on an
//!    item marked sensitive, whatever the caller passed.
//! 2. **In-transaction re-read** — [`upsert_fts_in_tx`] re-checks
//!    `is_sensitive` inside the transaction that writes the index row.
//! 3. **Read predicate** — [`super::Store::search`] joins on a live,
//!    non-sensitive text item, so even a row planted directly in the FTS table
//!    can never surface.
//!
//! All three are required. They are decided *at capture*, which is
//! what [`crate::sensitive::purge_indexed_secrets`] exists to correct — it reads
//! the index through [`Store::indexed_texts`] and drops what the current ruleset
//! calls a secret, whatever the row was flagged as when it arrived.

use rusqlite::{params, Connection};

use super::connection::write_tx;
use super::model::{item_columns_ci, row_to_item, ItemColumns, StoreError, StoredItem};
use super::store::Store;

/// One row of the search index, as stored. `text` is plaintext: `clipboard_fts`
/// is the one table not under the item AEAD, which is exactly why anything
/// sensitive reaching it matters.
pub struct IndexedText {
    /// Resume point for the next page. Stable under the deletes this feeds,
    /// which only ever remove rows already passed.
    pub rowid: i64,
    pub id: String,
    /// Lossily decoded, because a row that will not decode is the one row that
    /// must not stop the pass. See [`indexed_texts_in`].
    pub text: String,
}

impl Store {
    /// Full-text search over non-sensitive items, best match first. Layer 3:
    /// the JOIN filters `is_sensitive = 0` even though the write path already
    /// refuses to index one, so a stale FTS row can never surface.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<StoredItem>, StoreError> {
        let Some(match_expr) = sanitize_fts5_query(query) else {
            return Ok(Vec::new());
        };
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns_ci!(),
            " FROM clipboard_fts fts \
              JOIN clipboard_items ci ON ci.id = fts.id \
              WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0 AND ci.is_sensitive = 0 \
                AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%') \
              ORDER BY rank LIMIT ?2"
        ))?;
        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map(params![match_expr, i64::from(limit)], |row| {
            row_to_item(row, &columns)
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// A page of the index in `rowid` order, after `after_rowid` exclusive,
    /// bounded by both `max_rows` and `max_bytes`.
    ///
    /// Paged rather than collected: the index holds every searchable clipboard
    /// item's plaintext, and a rescan that materialised all of it at once would
    /// hold the whole history in memory to look at one row at a time.
    pub fn indexed_texts(
        &self,
        after_rowid: i64,
        max_rows: u32,
        max_bytes: usize,
    ) -> Result<Vec<IndexedText>, StoreError> {
        let conn = self.conn()?;
        Ok(indexed_texts_in(&conn, after_rowid, max_rows, max_bytes)?)
    }

    /// Removes every index row without a live, non-sensitive text item,
    /// returning how many went.
    ///
    /// Manifest 03 S4: an index row for a
    /// flagged item is one the write guard should never have allowed, and it
    /// holds plaintext whatever the current ruleset would say about that
    /// plaintext now. Costs one indexed lookup per FTS row and no decryption, so it is the
    /// cheap half of [`crate::sensitive::purge_indexed_secrets`] and runs first.
    pub fn purge_index_of_unsearchable(&self) -> Result<u64, StoreError> {
        let conn = self.conn()?;
        Ok(purge_index_of_unsearchable_in(&conn)?)
    }

    /// Removes index rows by `rowid`, returning how many went.
    ///
    /// **Only `clipboard_fts` is touched.** No history row is deleted,
    /// tombstoned or reflagged, so the worst outcome of a wrong verdict here is
    /// an item the user cannot find by searching — never one they cannot find at
    /// all (AGENTS.md rule 4). Flipping `is_sensitive` instead would arm
    /// [`crate::sensitive::sweep_sensitive`], which hard-deletes.
    pub fn purge_from_index(&self, rowids: &[i64]) -> Result<u64, StoreError> {
        if rowids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let removed = purge_from_index_in(&tx, rowids)?;
        tx.commit()?;
        Ok(removed)
    }
}

/// Two bounds, because a row cap alone is not one. The item cap is 4 MiB, so
/// `max_rows` of 512 permits a 2 GiB page; the byte budget is what makes peak
/// memory a number rather than a function of what the user copied.
///
/// The budget is applied while streaming rather than as a SQL running sum —
/// `retention`'s `BYTE_VICTIMS_SQL` shape does not transfer, because there is no
/// indexed length on an FTS5 content column and summing one would read every
/// row's text to decide not to read it. The row that would cross the budget ends
/// the page instead of joining it, unless the page is empty: a single row larger
/// than the whole budget must still make progress or the cursor never advances.
///
/// **The text is decoded lossily.** `content_text` is written as UTF-8 by every
/// path in this crate, but the column is `TEXT` in a file the process does not
/// exclusively own, and typing it as `String` made one undecodable row abort the
/// read — taking every *later* page with it, so a genuine secret behind it stayed
/// indexed indefinitely. Replacement characters can only ever make the detector
/// see more, never split an ASCII credential, and the consequence of a match here
/// is an index row removed rather than an item deleted (see [`crate::sensitive`]).
pub(crate) fn indexed_texts_in(
    conn: &Connection,
    after_rowid: i64,
    max_rows: u32,
    max_bytes: usize,
) -> rusqlite::Result<Vec<IndexedText>> {
    let mut stmt = conn.prepare_cached(
        "SELECT fts.rowid, fts.id, fts.content_text FROM clipboard_fts fts \
          JOIN clipboard_items ci ON ci.id = fts.id \
          WHERE fts.rowid > ?1 AND ci.deleted = 0 AND ci.is_sensitive = 0 \
            AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%') \
          ORDER BY fts.rowid LIMIT ?2",
    )?;
    let mut rows = stmt.query(params![after_rowid, i64::from(max_rows)])?;
    let mut page: Vec<IndexedText> = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next()? {
        // `get::<String>` rejects both of these; `get::<Vec<u8>>` rejects TEXT.
        // Anything else stored in the column carries no text a rule could match,
        // so there is nothing to judge and the row keeps its index entry.
        let raw = match row.get_ref(2)? {
            rusqlite::types::ValueRef::Text(raw) | rusqlite::types::ValueRef::Blob(raw) => raw,
            _ => b"",
        };
        if !page.is_empty() && bytes.saturating_add(raw.len()) > max_bytes {
            break;
        }
        bytes = bytes.saturating_add(raw.len());
        page.push(IndexedText {
            rowid: row.get(0)?,
            id: row.get(1)?,
            text: String::from_utf8_lossy(raw).into_owned(),
        });
    }
    Ok(page)
}

pub(crate) fn purge_index_of_unsearchable_in(conn: &Connection) -> rusqlite::Result<u64> {
    let removed = conn.execute(
        "DELETE FROM clipboard_fts \
          WHERE NOT EXISTS \
            (SELECT 1 FROM clipboard_items ci \
             WHERE ci.id = clipboard_fts.id AND ci.deleted = 0 AND ci.is_sensitive = 0 \
               AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%'))",
        [],
    )?;
    Ok(removed as u64)
}

pub(crate) fn purge_from_index_in(conn: &Connection, rowids: &[i64]) -> rusqlite::Result<u64> {
    let mut removed = 0u64;
    // Both seeks. Clearing the back-pointer first, while the index row still
    // names its item: a `fts_rowid` left behind would later name whichever row
    // FTS5 hands that rowid to next, and the delete would take the wrong one.
    let mut clear = conn.prepare(
        "UPDATE clipboard_items SET fts_rowid = NULL \
          WHERE id = (SELECT id FROM clipboard_fts WHERE rowid = ?1)",
    )?;
    let mut stmt = conn.prepare("DELETE FROM clipboard_fts WHERE rowid = ?1")?;
    for rowid in rowids {
        clear.execute([rowid])?;
        removed += stmt.execute([rowid])? as u64;
    }
    Ok(removed)
}

/// Removes an item's index row, by the one key FTS5 can seek on.
///
/// The caller must clear `fts_rowid` in the same transaction — normally by
/// folding it into the row update it is already making.
pub(super) fn delete_fts_row_in_tx(tx: &Connection, id: &str) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM clipboard_fts \
          WHERE rowid = (SELECT fts_rowid FROM clipboard_items WHERE id = ?1)",
        [id],
    )?;
    Ok(())
}

pub(super) fn insert_fts_in_tx(
    tx: &Connection,
    id: &str,
    text: &str,
    is_sensitive: bool,
    content_type: &str,
) -> rusqlite::Result<Option<i64>> {
    if is_sensitive || !copypaste_ipc::content_type::is_text(content_type) {
        return Ok(None);
    }
    tx.execute(
        "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
        params![id, text],
    )?;
    Ok(Some(tx.last_insert_rowid()))
}

/// Turns arbitrary user input into an FTS5 MATCH expression, or `None` when
/// there is nothing left to search for.
///
/// A whitelist tokenizer, not an escaper. Each rule records a reported bug:
///
/// * `-` becomes a space *first*: FTS5 reads `-bar` as a column filter and
///   errors with "no such column: bar", so `foo-bar` must become
///   `foo* AND bar*`.
/// * Only alphanumerics (Unicode, so Cyrillic/CJK survive), `_`, `"`, `*` and
///   whitespace are kept.
/// * An odd number of quotes is an unclosed phrase — an FTS5 syntax error — so
///   all quotes are dropped.
/// * `*` is appended to *every* token, not just the last: search-as-you-type
///   means any token can be mid-word, and last-token-only made `"priv key"`
///   match nothing.
fn sanitize_fts5_query(raw: &str) -> Option<String> {
    const RESERVED: [&str; 4] = ["NOT", "OR", "AND", "NEAR"];

    let mut cleaned = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '-' => cleaned.push(' '),
            c if c.is_alphanumeric() || matches!(c, '_' | '"' | '*' | ' ' | '\t') => {
                cleaned.push(c)
            }
            _ => {}
        }
    }

    let mut cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.matches('"').count() % 2 == 1 {
        cleaned = cleaned.replace('"', "").trim().to_string();
        if cleaned.is_empty() {
            return None;
        }
    }
    if (cleaned.len() > 1 && cleaned.starts_with('"') && cleaned.ends_with('"'))
        || cleaned.ends_with('*')
    {
        return Some(cleaned);
    }

    let tokens: Vec<String> = cleaned
        .split_whitespace()
        .filter(|t| t.chars().any(|c| c.is_alphanumeric() || c == '_'))
        .filter(|t| !RESERVED.iter().any(|r| r.eq_ignore_ascii_case(t)))
        .map(|t| {
            if t.ends_with('*') {
                t.to_string()
            } else {
                format!("{t}*")
            }
        })
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" AND "))
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::super::model::NewItem;
    use super::super::test_support::{
        fts_dump, fts_row_count, item, plan_of, plant_fts_row, sensitive_item, store, T0,
    };
    use super::*;

    #[test]
    fn search_finds_a_non_sensitive_item() {
        let s = store();
        let hit = s.insert(item("meeting notes for tuesday", T0)).unwrap();
        s.insert(item("unrelated payload", T0 + 60_000)).unwrap();

        let found = s.search("tuesday", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, hit.id);

        assert_eq!(s.search("meet", 10).unwrap().len(), 1);
        assert_eq!(s.search("meeting notes", 10).unwrap().len(), 1);
        // A hyphenated query must not error (FTS5 would read `-notes` as a
        // column filter).
        assert_eq!(s.search("meeting-notes", 10).unwrap().len(), 1);
        assert!(s.search("zzzznotpresent", 10).unwrap().is_empty());
        assert!(s.search("   ", 10).unwrap().is_empty());
        assert!(s.search("^:;", 10).unwrap().is_empty());
    }

    #[test]
    fn sensitive_items_never_reach_the_search_index() {
        let s = store();
        let secret = s
            .insert(sensitive_item("hunter2 super secret token", T0))
            .unwrap();
        let normal = s
            .insert(item("ordinary shopping list", T0 + 60_000))
            .unwrap();

        // Layer 1: the write guard ignores what the caller passed. A sensitive
        // item that arrives *with* search_text is still not indexed.
        let leaky = s
            .insert(NewItem {
                search_text: Some("leaked passphrase correcthorse".to_string()),
                ..sensitive_item("another secret", T0 + 120_000)
            })
            .unwrap();

        assert_eq!(fts_row_count(&s, &secret.id), 0);
        assert_eq!(fts_row_count(&s, &leaky.id), 0);
        assert_eq!(fts_row_count(&s, &normal.id), 1);

        // No sensitive plaintext is anywhere in the FTS table.
        let dump = fts_dump(&s);
        assert!(!dump.contains("hunter2"));
        assert!(!dump.contains("secret"));
        assert!(!dump.contains("passphrase"));
        assert!(!dump.contains("correcthorse"));
        assert!(dump.contains("shopping"));

        assert!(s.search("hunter2", 10).unwrap().is_empty());
        assert!(s.search("passphrase", 10).unwrap().is_empty());

        // Layer 3: even a stale FTS row planted directly cannot surface.
        {
            let conn = s.conn().unwrap();
            conn.execute(
                "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
                params![&secret.id, "hunter2 super secret token"],
            )
            .unwrap();
        }
        assert_eq!(fts_row_count(&s, &secret.id), 1);
        assert!(s.search("hunter2", 10).unwrap().is_empty());
        assert!(s.search("token", 10).unwrap().is_empty());

        // Layer 2: the in-transaction re-read refuses to index a sensitive row.
        {
            let mut conn = s.conn().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [&secret.id])
                .unwrap();
            assert!(
                insert_fts_in_tx(&tx, &secret.id, "hunter2 again", true, "text")
                    .unwrap()
                    .is_none()
            );
            assert!(
                insert_fts_in_tx(&tx, &secret.id, "hunter2 again", false, "image/png")
                    .unwrap()
                    .is_none()
            );
            tx.commit().unwrap();
        }
        assert_eq!(fts_row_count(&s, &secret.id), 0);
    }

    #[test]
    fn stale_non_text_rows_never_surface_or_reach_the_rescan() {
        let s = store();
        let image = s
            .insert(NewItem {
                content_type: "image/png".to_string(),
                search_text: None,
                ..item("image bytes", T0)
            })
            .unwrap();
        plant_fts_row(&s, &image.id, "private image caption");

        assert!(s.search("caption", 10).unwrap().is_empty());
        assert!(s.indexed_texts(0, 10, 4096).unwrap().is_empty());
        assert_eq!(s.purge_index_of_unsearchable().unwrap(), 1);
        assert_eq!(fts_row_count(&s, &image.id), 0);
    }

    /// F-STOR-1. `clipboard_fts.id` is UNINDEXED, so FTS5's `xBestIndex`
    /// declines a constraint on it and SQLite filters after a full scan of the
    /// table holding every searchable item's plaintext — 278 µs at 2 000 rows,
    /// 1.32 ms at 8 000, once per victim inside an eviction. Every delete must
    /// therefore name the row by `rowid`.
    #[test]
    fn every_index_delete_seeks_on_rowid_rather_than_scanning_the_index() {
        let s = store();
        for sql in [
            "DELETE FROM clipboard_fts WHERE rowid = ?1",
            "DELETE FROM clipboard_fts \
              WHERE rowid = (SELECT fts_rowid FROM clipboard_items WHERE id = ?1)",
        ] {
            let plan = plan_of(&s, sql);
            assert!(
                plan.iter()
                    .any(|d| d.contains("clipboard_fts VIRTUAL TABLE INDEX 0:=")),
                "an index delete must seek, got {plan:?}"
            );
        }

        // The form this replaced still demonstrates the scan, so the assertion
        // above is pinning a real difference.
        let scanning = plan_of(&s, "DELETE FROM clipboard_fts WHERE id = ?1");
        assert!(
            scanning
                .iter()
                .any(|d| d.contains("SCAN clipboard_fts VIRTUAL TABLE INDEX 0:")
                    && !d.contains("INDEX 0:=")),
            "delete by id must still be the scan this fix removed, got {scanning:?}"
        );
    }

    /// The hazard the `fts_rowid` back-pointer introduces: FTS5 hands a freed
    /// rowid to the next insert, so a pointer left behind by a delete would
    /// name somebody else's index row. Deleting one item must never unindex
    /// another.
    #[test]
    fn a_reused_index_rowid_is_never_deleted_out_from_under_its_new_owner() {
        let s = store();
        let keep = s.insert(item("alpha alpha", T0)).unwrap();
        let doomed = s.insert(item("bravo bravo", T0 + 60_000)).unwrap();
        assert!(s.delete(&doomed.id).unwrap());

        // Takes the rowid the delete above freed.
        let reuser = s.insert(item("charlie charlie", T0 + 120_000)).unwrap();
        assert_eq!(fts_row_count(&s, &reuser.id), 1);

        assert!(s.delete(&keep.id).unwrap());
        assert_eq!(
            fts_row_count(&s, &reuser.id),
            1,
            "deleting one item unindexed another"
        );
        assert_eq!(s.search("charlie", 10).unwrap().len(), 1);
    }

    /// An eviction is the compounding case: it was one scan of the plaintext
    /// index per victim inside a single write transaction.
    #[test]
    fn a_hard_delete_leaves_no_index_row_and_no_dangling_pointer() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..4 {
            ids.push(
                s.insert(item(&format!("victim {n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        assert_eq!(s.evict_over_cap(1).unwrap(), 3);

        for id in &ids[..3] {
            assert_eq!(fts_row_count(&s, id), 0);
        }
        assert_eq!(fts_row_count(&s, &ids[3]), 1);
        assert!(!s.search("victim", 10).unwrap().is_empty());
        assert_eq!(dangling_fts_pointers(&s), 0);
    }

    /// Every live index row is named by exactly the item it belongs to.
    fn dangling_fts_pointers(store: &Store) -> i64 {
        let conn = store.conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items ci \
              WHERE ci.fts_rowid IS NOT NULL \
                AND ci.id IS NOT (SELECT f.id FROM clipboard_fts f WHERE f.rowid = ci.fts_rowid)",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn a_reindex_repoints_the_row_at_its_new_index_entry() {
        let s = store();
        let a = s.insert(item("first text", T0)).unwrap();
        s.insert(item("second text", T0 + 60_000)).unwrap();
        {
            let mut conn = s.conn().unwrap();
            let tx = conn.transaction().unwrap();
            delete_fts_row_in_tx(&tx, &a.id).unwrap();
            let rowid = insert_fts_in_tx(&tx, &a.id, "rewritten text", false, "text").unwrap();
            tx.execute(
                "UPDATE clipboard_items SET fts_rowid = ?2 WHERE id = ?1",
                params![&a.id, rowid],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(fts_row_count(&s, &a.id), 1, "the upsert must not duplicate");
        assert_eq!(dangling_fts_pointers(&s), 0);
        assert_eq!(s.search("rewritten", 10).unwrap().len(), 1);
        assert!(s.search("first", 10).unwrap().is_empty());
    }

    #[test]
    fn fts5_query_sanitizer() {
        assert_eq!(
            sanitize_fts5_query("foo-bar").as_deref(),
            Some("foo* AND bar*")
        );
        assert_eq!(
            sanitize_fts5_query("priv key").as_deref(),
            Some("priv* AND key*")
        );
        assert_eq!(sanitize_fts5_query("foo*").as_deref(), Some("foo*"));
        assert_eq!(
            sanitize_fts5_query("\"exact phrase\"").as_deref(),
            Some("\"exact phrase\"")
        );
        // Unbalanced quote: strip rather than hand FTS5 a syntax error.
        assert_eq!(sanitize_fts5_query("\"oops").as_deref(), Some("oops*"));
        assert_eq!(
            sanitize_fts5_query("foo OR bar").as_deref(),
            Some("foo* AND bar*")
        );
        assert_eq!(
            sanitize_fts5_query("col:val;--").as_deref(),
            Some("colval*")
        );
        assert_eq!(sanitize_fts5_query("привет").as_deref(), Some("привет*"));
        assert!(sanitize_fts5_query("").is_none());
        assert!(sanitize_fts5_query("   ").is_none());
        assert!(sanitize_fts5_query("^^^").is_none());
        assert!(sanitize_fts5_query("AND OR").is_none());
    }
}
