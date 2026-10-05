//! The FTS5 layer for live text items.

use rusqlite::{params, Connection};

use super::model::{item_columns_ci, row_to_item, ItemColumns, StoreError, StoredItem};
use super::store::Store;

const SEARCH_SQL: &str = concat!(
    "SELECT ",
    item_columns_ci!(),
    " FROM clipboard_fts fts \
              JOIN clipboard_items ci ON ci.id = fts.id \
              WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0 \
                AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%') \
              ORDER BY rank LIMIT ?2"
);

impl Store {
    /// Full-text search over live items, best match first.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<StoredItem>, StoreError> {
        self.search_with_budget(query, limit, usize::MAX)
    }

    /// Full-text search that retains a ranked prefix within `max_bytes` of
    /// ciphertext, except that the first item is always retained for progress.
    pub fn search_bounded(
        &self,
        query: &str,
        limit: u32,
        max_bytes: usize,
    ) -> Result<Vec<StoredItem>, StoreError> {
        self.search_with_budget(query, limit, max_bytes)
    }

    fn search_with_budget(
        &self,
        query: &str,
        limit: u32,
        max_bytes: usize,
    ) -> Result<Vec<StoredItem>, StoreError> {
        let Some(match_expr) = sanitize_fts5_query(query) else {
            return Ok(Vec::new());
        };
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(SEARCH_SQL)?;
        let columns = ItemColumns::resolve(&stmt)?;
        let mut rows = stmt.query(params![match_expr, i64::from(limit)])?;
        let mut found = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = rows.next()? {
            let item = row_to_item(row, &columns)?;
            let item_bytes = item.content_ciphertext.len();
            if !found.is_empty() && bytes.saturating_add(item_bytes) > max_bytes {
                break;
            }
            bytes = bytes.saturating_add(item_bytes);
            found.push(item);
        }
        Ok(found)
    }
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
    content_type: &str,
) -> rusqlite::Result<Option<i64>> {
    if !copypaste_ipc::content_type::is_text(content_type) {
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
/// * Punctuation becomes a space instead of being dropped. FTS5 tokenizes
///   URLs, email addresses, and paths at punctuation, so dropping it would
///   join adjacent terms into a token the index does not contain.
/// * Only alphanumerics (Unicode, so Cyrillic/CJK survive), `_`, `"`, `*` and
///   whitespace are otherwise kept.
/// * An odd number of quotes is an unclosed phrase — an FTS5 syntax error — so
///   all quotes are dropped.
/// * `*` is appended to *every* token, not just the last: search-as-you-type
///   means any token can be mid-word, and last-token-only made `"priv key"`
///   match nothing.
pub(super) fn sanitize_fts5_query(raw: &str) -> Option<String> {
    const RESERVED: [&str; 4] = ["NOT", "OR", "AND", "NEAR"];

    let mut cleaned = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '-' => cleaned.push(' '),
            c if c.is_alphanumeric() || matches!(c, '_' | '"' | '*' | ' ' | '\t') => {
                cleaned.push(c)
            }
            _ => cleaned.push(' '),
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
    use super::super::test_support::{fts_row_count, item, plan_of, plant_fts_row, store, T0};
    use super::*;

    #[test]
    fn search_finds_a_text_item() {
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
    fn search_uses_fts_boundaries_for_punctuation_in_copied_prose() {
        let s = store();
        let copied = s
            .insert(item(
                "See https://example.test/help. Contact person@example.test or /Users/person/notes.",
                T0,
            ))
            .unwrap();
        s.insert(item("unrelated payload", T0 + 60_000)).unwrap();

        for query in [
            " See https://example.test/help ",
            "person",
            "\"See https://example.test/help\"",
            "\"person",
            "exam",
        ] {
            assert_eq!(
                s.search(query, 10)
                    .unwrap()
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                vec![copied.id.as_str()],
                "query: {query}",
            );
        }

        assert!(s.search("^:;", 10).unwrap().is_empty());
        assert!(s.search("person OR unrelated", 10).unwrap().is_empty());
    }

    #[test]
    fn bounded_search_stops_before_mapping_a_later_malformed_match() {
        let s = store();
        for text in [
            "bounded token token token",
            "bounded token token",
            "bounded token",
        ] {
            s.insert(item(text, T0)).unwrap();
        }
        let ranked = s.search("bounded token", 3).unwrap();
        let ids: Vec<&str> = ranked.iter().map(|item| item.id.as_str()).collect();
        let conn = s.conn().unwrap();
        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
            params![ids[0], vec![0u8; 8]],
        )
        .unwrap();
        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
            params![ids[1], vec![0u8; 1]],
        )
        .unwrap();
        let bad_app_name = [0xffu8];
        conn.execute(
            "UPDATE clipboard_items SET app_name = CAST(?2 AS TEXT) WHERE id = ?1",
            params![ids[2], &bad_app_name[..]],
        )
        .unwrap();

        let bounded = s.search_bounded("bounded token", 3, 8).unwrap();
        assert_eq!(bounded.len(), 1);
        assert_eq!(bounded[0].id, ids[0]);

        conn.execute(
            "UPDATE clipboard_items SET app_name = CAST(?2 AS TEXT) WHERE id = ?1",
            params![ids[1], &bad_app_name[..]],
        )
        .unwrap();
        assert!(s.search_bounded("bounded token", 3, 8).is_err());
    }

    #[test]
    fn bounded_search_keeps_one_oversize_ranked_item() {
        let s = store();
        for text in ["oversize token token", "oversize token"] {
            s.insert(item(text, T0)).unwrap();
        }
        let ranked = s.search("oversize token", 2).unwrap();
        let ids: Vec<&str> = ranked.iter().map(|item| item.id.as_str()).collect();
        let conn = s.conn().unwrap();
        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
            params![ids[0], vec![0u8; 9]],
        )
        .unwrap();
        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
            params![ids[1], vec![0u8; 1]],
        )
        .unwrap();

        let bounded = s.search_bounded("oversize token", 2, 8).unwrap();
        assert_eq!(
            bounded
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![ids[0]]
        );
    }

    #[test]
    fn bounded_search_returns_the_ranked_prefix_at_and_over_its_budget() {
        let s = store();
        for text in [
            "budget token token token",
            "budget token token",
            "budget token",
        ] {
            s.insert(item(text, T0)).unwrap();
        }
        let ranked = s.search("budget token", 3).unwrap();
        let ids: Vec<&str> = ranked.iter().map(|item| item.id.as_str()).collect();
        let conn = s.conn().unwrap();
        for (id, bytes) in [(ids[0], 4usize), (ids[1], 4), (ids[2], 1)] {
            conn.execute(
                "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
                params![id, vec![0u8; bytes]],
            )
            .unwrap();
        }

        let exact = s.search_bounded("budget token", 3, 8).unwrap();
        assert_eq!(
            exact
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ids[..2]
        );

        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext = ?2 WHERE id = ?1",
            params![ids[1], vec![0u8; 5]],
        )
        .unwrap();
        let over = s.search_bounded("budget token", 3, 8).unwrap();
        assert_eq!(
            over.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            ids[..1]
        );
    }

    #[test]
    fn bounded_search_preserves_filters_and_never_mutates_search_state() {
        let s = store();
        let visible = s.insert(item("bounded visible token", T0)).unwrap();
        let non_text = s
            .insert(NewItem {
                content_type: "image/png".to_string(),
                search_text: None,
                ..item("bounded non-text token", T0 + 2)
            })
            .unwrap();
        let tombstone = s.insert(item("bounded tombstone token", T0 + 3)).unwrap();
        s.delete(&tombstone.id).unwrap();
        plant_fts_row(&s, &non_text.id, "bounded non-text token");
        plant_fts_row(&s, &tombstone.id, "bounded tombstone token");
        let fts_before = s
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();

        assert!(s.search_bounded("", 10, 8).unwrap().is_empty());
        assert!(s.search_bounded("^:;", 10, 8).unwrap().is_empty());
        assert!(s.search_bounded("bounded", 0, 8).unwrap().is_empty());
        let found = s.search_bounded("bounded", 10, usize::MAX).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![visible.id.as_str()]
        );
        let fts_after = s
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(fts_after, fts_before);
    }

    #[test]
    fn search_rank_order_needs_no_payload_sort() {
        let s = store();
        s.insert(item("plan token", T0)).unwrap();
        let plan = plan_of(&s, SEARCH_SQL);
        assert!(
            !plan.iter().any(|detail| detail.contains("TEMP B-TREE")),
            "rank search must not materialize a payload sort, got {plan:?}"
        );
    }

    #[test]
    fn all_text_is_indexed_without_content_classification() {
        let s = store();
        let row = s.insert(item("AKIAIOSFODNN7EXAMPLE", T0)).unwrap();
        assert_eq!(fts_row_count(&s, &row.id), 1);
        assert_eq!(s.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap()[0].id, row.id);
    }

    #[test]
    fn stale_non_text_rows_do_not_surface() {
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
        assert_eq!(
            fts_row_count(&s, &image.id),
            1,
            "search must not mutate index state"
        );
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
            let rowid = insert_fts_in_tx(&tx, &a.id, "rewritten text", "text").unwrap();
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
            Some("col* AND val*")
        );
        assert_eq!(sanitize_fts5_query("привет").as_deref(), Some("привет*"));
        assert!(sanitize_fts5_query("").is_none());
        assert!(sanitize_fts5_query("   ").is_none());
        assert!(sanitize_fts5_query("^^^").is_none());
        assert!(sanitize_fts5_query("AND OR").is_none());
    }
}
