//! The FTS5 layer, and the ADR-015 enforcement that keeps sensitive content out
//! of it. Counted in one place:
//!
//! 1. **Write guard** — [`super::Store::insert`] drops any `search_text` on an
//!    item marked sensitive, whatever the caller passed.
//! 2. **In-transaction re-read** — [`upsert_fts_in_tx`] re-checks
//!    `is_sensitive` inside the transaction that writes the index row.
//! 3. **Read predicate** — [`super::Store::search`] joins on
//!    `ci.is_sensitive = 0`, so even a row planted directly in the FTS table
//!    can never surface.
//!
//! All three are required: v1 shipped databases with plaintext passwords in FTS
//! because one was missing. All three are also decided *at capture*, which is
//! what [`crate::sensitive::purge_indexed_secrets`] exists to correct — it reads
//! the index through [`Store::indexed_texts`] and drops what the current ruleset
//! calls a secret, whatever the row was flagged as when it arrived.

use rusqlite::{params, OptionalExtension, Transaction};

use super::connection::write_tx;
use super::model::{item_columns_ci, row_to_item, StoreError, StoredItem};
use super::store::Store;

/// One row of the search index, as stored. `text` is plaintext: `clipboard_fts`
/// is the one table not under the item AEAD, which is exactly why anything
/// sensitive reaching it matters.
pub struct IndexedText {
    /// Resume point for the next page. Stable under the deletes this feeds,
    /// which only ever remove rows already passed.
    pub rowid: i64,
    pub id: String,
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
              ORDER BY rank LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![match_expr, i64::from(limit)], row_to_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// A page of the index in `rowid` order, after `after_rowid` exclusive.
    ///
    /// Paged rather than collected: the index holds every searchable clipboard
    /// item's plaintext, and a rescan that materialised all of it at once would
    /// hold the whole history in memory to look at one row at a time.
    pub fn indexed_texts(
        &self,
        after_rowid: i64,
        limit: u32,
    ) -> Result<Vec<IndexedText>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT rowid, id, content_text FROM clipboard_fts \
              WHERE rowid > ?1 ORDER BY rowid LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_rowid, i64::from(limit)], |row| {
            Ok(IndexedText {
                rowid: row.get(0)?,
                id: row.get(1)?,
                text: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Removes every index row belonging to an item already flagged
    /// `is_sensitive`, returning how many went.
    ///
    /// Manifest 03 S4, which v1 discharged as migration v13: an index row for a
    /// flagged item is one the write guard should never have allowed, and it
    /// holds plaintext whatever the current ruleset would say about that
    /// plaintext now. Costs one indexed sub-select and no scanning, so it is the
    /// cheap half of [`crate::sensitive::purge_indexed_secrets`] and runs first.
    pub fn purge_index_of_flagged(&self) -> Result<u64, StoreError> {
        let conn = self.conn()?;
        let removed = conn.execute(
            "DELETE FROM clipboard_fts WHERE id IN \
                 (SELECT id FROM clipboard_items WHERE is_sensitive = 1)",
            [],
        )?;
        Ok(removed as u64)
    }

    /// Removes index rows by `rowid`, returning how many went.
    ///
    /// **Only `clipboard_fts` is touched.** No history row is deleted,
    /// tombstoned or reflagged, so the worst outcome of a wrong verdict here is
    /// an item the user cannot find by searching — never one they cannot find at
    /// all (CLAUDE.md rule 4). Flipping `is_sensitive` instead would arm
    /// [`crate::sensitive::sweep_sensitive`], which hard-deletes.
    pub fn purge_from_index(&self, rowids: &[i64]) -> Result<u64, StoreError> {
        if rowids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let mut removed = 0u64;
        {
            let mut stmt = tx.prepare("DELETE FROM clipboard_fts WHERE rowid = ?1")?;
            for rowid in rowids {
                removed += stmt.execute([rowid])? as u64;
            }
        }
        tx.commit()?;
        Ok(removed)
    }
}

/// Layer 2 of ADR-015: re-read `is_sensitive` **inside the same transaction** as
/// the FTS write, so a concurrent update that flips an item to sensitive cannot
/// slip its plaintext into the index. A missing row is also a no-op.
///
/// FTS5 has no `ON CONFLICT`, so the upsert idiom is DELETE + INSERT; both run
/// in this transaction, so a crash cannot leave an item permanently
/// unsearchable.
pub(super) fn upsert_fts_in_tx(tx: &Transaction<'_>, id: &str, text: &str) -> rusqlite::Result<()> {
    let searchable: Option<bool> = tx
        .query_row(
            "SELECT is_sensitive = 0 AND (content_type = 'text' OR content_type LIKE 'text/%') \
             FROM clipboard_items WHERE id = ?1 AND deleted = 0",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if searchable != Some(true) {
        return Ok(());
    }
    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
    tx.execute(
        "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
        params![id, text],
    )?;
    Ok(())
}

/// Turns arbitrary user input into an FTS5 MATCH expression, or `None` when
/// there is nothing left to search for.
///
/// A whitelist tokenizer, not an escaper. Each rule was a reported v1 bug:
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
    use super::super::test_support::{fts_dump, fts_row_count, item, sensitive_item, store, T0};
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
            upsert_fts_in_tx(&tx, &secret.id, "hunter2 again").unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(fts_row_count(&s, &secret.id), 0);
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
