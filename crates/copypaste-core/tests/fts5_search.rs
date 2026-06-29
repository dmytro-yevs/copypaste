//! FTS5 search query + tokenizer integration tests.
//!
//! Beta-bonus coverage for `search_items()` against the `clipboard_fts`
//! virtual table (default FTS5 unicode61 tokenizer, configured in
//! `storage/schema_v1.sql`).
//!
//! Each test opens a fresh on-disk encrypted database via `tempfile::tempdir()`
//! and exercises the public `insert_item` + `upsert_fts` + `search_items`
//! surface — matching how the daemon writes the index in production.

use copypaste_core::{insert_item, search_items, upsert_fts, ClipboardItem, Database, RowId};
use tempfile::tempdir;

/// Open a fresh encrypted DB in a temp dir. The key is deterministic per-test
/// so the file is reproducible if anyone needs to inspect it.
fn fresh_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("fts5_search_test.db");
    let key = [0x42u8; 32];
    let db = Database::open(&path, &key).expect("open encrypted db");
    (dir, db)
}

/// Insert one clipboard row + its FTS5 plaintext shadow.
/// `lamport` keeps rows distinguishable without affecting search behaviour.
fn insert_with_text(db: &Database, lamport: i64, plaintext: &str) -> RowId {
    // Real content bytes are irrelevant for FTS — we only index `plaintext`.
    let item = ClipboardItem::new_text(vec![0u8; 4], vec![0u8; 24], lamport);
    let id = item.id.clone();
    insert_item(db, &item).expect("insert_item");
    upsert_fts(db, &id, plaintext).expect("upsert_fts");
    id
}

#[test]
fn search_exact_word_match() {
    let (_dir, db) = fresh_db();
    let id = insert_with_text(&db, 1, "hello world");

    let results = search_items(&db, "hello", 10).expect("search");

    assert_eq!(results.len(), 1, "expected exactly one match for 'hello'");
    assert_eq!(results[0].id, id);
}

#[test]
fn search_prefix_match_with_asterisk() {
    let (_dir, db) = fresh_db();
    let id = insert_with_text(&db, 1, "developer documentation");
    // Decoy that must NOT match — confirms the prefix is anchored.
    let _other = insert_with_text(&db, 2, "rust language");

    let results = search_items(&db, "dev*", 10).expect("search");

    assert_eq!(results.len(), 1, "prefix 'dev*' should match 'developer'");
    assert_eq!(results[0].id, id);
}

#[test]
fn search_phrase_match_with_quotes() {
    let (_dir, db) = fresh_db();
    let id_match = insert_with_text(&db, 1, "foo bar baz qux");
    // Same words, wrong adjacency — must be rejected by a quoted phrase query.
    let _id_decoy = insert_with_text(&db, 2, "bar foo baz");

    let results = search_items(&db, "\"bar baz\"", 10).expect("search");

    assert_eq!(
        results.len(),
        1,
        "phrase '\"bar baz\"' must match only adjacent occurrence"
    );
    assert_eq!(results[0].id, id_match);
}

#[test]
fn search_no_match_returns_empty() {
    let (_dir, db) = fresh_db();
    insert_with_text(&db, 1, "alpha beta gamma");
    insert_with_text(&db, 2, "delta epsilon zeta");

    let results = search_items(&db, "omicron", 10).expect("search");

    assert!(results.is_empty(), "expected zero hits for absent token");
}

#[test]
fn search_unicode_text() {
    // unicode61 tokenizer (FTS5 default) should split on whitespace and treat
    // Cyrillic glyphs as ordinary tokens.
    let (_dir, db) = fresh_db();
    let id = insert_with_text(&db, 1, "Привіт світ");
    let _decoy = insert_with_text(&db, 2, "hello world");

    let results = search_items(&db, "Привіт", 10).expect("search");

    assert_eq!(results.len(), 1, "unicode61 must tokenize Cyrillic text");
    assert_eq!(results[0].id, id);
}

#[test]
fn search_rank_ordering_bm25() {
    // FTS5 default `ORDER BY rank` is BM25 ascending (smaller = better match).
    // The document with the most frequent occurrence of "rust" relative to its
    // length should outrank the others.
    let (_dir, db) = fresh_db();
    let id_strong = insert_with_text(&db, 1, "rust rust rust rust"); // highest relevance
    let id_medium = insert_with_text(&db, 2, "rust rust language"); // medium
    let id_weak = insert_with_text(&db, 3, "rust language tutorial guide"); // weakest

    let results = search_items(&db, "rust", 10).expect("search");

    assert_eq!(results.len(), 3, "all three docs contain 'rust'");
    // Strongest match must be first; weakest must be last.
    assert_eq!(results[0].id, id_strong, "highest-density doc must rank #1");
    assert_eq!(results[2].id, id_weak, "lowest-density doc must rank #3");
    // Sanity: medium doc sits between the two extremes.
    assert_eq!(results[1].id, id_medium);
}

#[test]
fn search_case_insensitive() {
    // unicode61 default tokenizer folds case (remove_diacritics defaults to 1,
    // case folding always on).
    let (_dir, db) = fresh_db();
    let id = insert_with_text(&db, 1, "Hello World");

    let results = search_items(&db, "hello", 10).expect("search");

    assert_eq!(results.len(), 1, "search must be case-insensitive");
    assert_eq!(results[0].id, id);
}

#[test]
fn search_special_chars_escaped_safely() {
    // Adversarial query: SQL-injection-shaped string. FTS5's MATCH operator
    // tokenizes the input; bind parameters protect against SQL injection at
    // the SQLite layer. Goal: no panic, no error (or a clean FTS5 syntax
    // error surfaced as Err), and the underlying table is intact afterwards.
    let (_dir, db) = fresh_db();
    insert_with_text(&db, 1, "benign clipboard content");

    let malicious = "a;DROP TABLE clipboard_items--";
    // We accept either Ok(empty) OR a clean rusqlite error (FTS5 syntax).
    // The MUST-NOT is: panic, or successful destruction of the table.
    let _ = search_items(&db, malicious, 10);

    // Table must still exist with its single row.
    let still_there: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .expect("clipboard_items must still be queryable");
    assert_eq!(
        still_there, 1,
        "adversarial query must not affect base table"
    );
}
