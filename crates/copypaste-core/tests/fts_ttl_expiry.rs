//! Regression test for CopyPaste-ekzn.
//!
//! Assert that when a clipboard item expires via TTL (`delete_expired`), its
//! corresponding FTS5 row is also removed — so a subsequent `search_items` call
//! cannot surface the expired item. This guards against a "secret leak via
//! search after expiry" class of bugs: an item whose content was sensitive but
//! whose TTL has elapsed must be fully invisible, including from FTS queries.

use copypaste_core::{delete_expired, insert_item, search_items, upsert_fts, ClipboardItem, Database};
use tempfile::tempdir;

fn fresh_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("fts_ttl_expiry_test.db");
    let key = [0xABu8; 32];
    let db = Database::open(&path, &key).expect("open encrypted db");
    (dir, db)
}

/// Insert a text item with an explicit `expires_at` and an FTS shadow row,
/// then run `delete_expired` and confirm FTS no longer matches the text.
#[test]
fn fts_row_removed_when_item_expires_by_ttl() {
    let (_dir, db) = fresh_db();

    // Build a minimal encrypted text item with a TTL already in the past.
    let mut item = ClipboardItem::new_text(vec![0u8; 4], vec![0u8; 24], 1);
    // expires_at < now_ms: this item is already expired.
    item.expires_at = Some(1_000); // Unix epoch millisecond 1 — always in the past

    let id = item.id.clone();
    insert_item(&db, &item).expect("insert_item");

    // Index the secret plaintext in FTS.
    let secret_word = "supersecretpassword";
    upsert_fts(&db, &id, secret_word).expect("upsert_fts");

    // Pre-condition: FTS returns the item before expiry cleanup.
    let before = search_items(&db, secret_word, 10).expect("search before expiry");
    assert_eq!(
        before.len(),
        1,
        "pre-condition: item must be searchable before delete_expired"
    );

    // Trigger TTL expiry with a timestamp far in the future — the item's
    // `expires_at = 1` is less than `now_ms = i64::MAX / 2`, so it should be pruned.
    let pruned = delete_expired(&db, i64::MAX / 2).expect("delete_expired");
    assert_eq!(pruned, 1, "delete_expired must remove exactly one expired item");

    // Post-condition: the FTS row must also be gone — the secret is no longer
    // surfaced by search.
    let after = search_items(&db, secret_word, 10).expect("search after expiry");
    assert!(
        after.is_empty(),
        "FTS must not surface expired item after delete_expired (CopyPaste-ekzn: secret leak via search)"
    );
}

/// Pinned items are exempt from TTL prune — FTS row must SURVIVE `delete_expired`.
#[test]
fn pinned_expired_item_fts_row_is_retained() {
    let (_dir, db) = fresh_db();

    let mut item = ClipboardItem::new_text(vec![0u8; 4], vec![0u8; 24], 1);
    item.expires_at = Some(1_000); // would expire...
    item.pinned = true; // ...but is pinned: exempt from TTL prune

    let id = item.id.clone();
    insert_item(&db, &item).expect("insert_item");
    upsert_fts(&db, &id, "pinnedcontent").expect("upsert_fts");

    let pruned = delete_expired(&db, i64::MAX / 2).expect("delete_expired");
    assert_eq!(pruned, 0, "pinned item must not be pruned by delete_expired");

    let results = search_items(&db, "pinnedcontent", 10).expect("search");
    assert_eq!(
        results.len(),
        1,
        "FTS row for a pinned item must survive TTL cleanup"
    );
}

/// Non-expired item must not have its FTS row removed.
#[test]
fn non_expired_item_fts_row_is_retained() {
    let (_dir, db) = fresh_db();

    let mut item = ClipboardItem::new_text(vec![0u8; 4], vec![0u8; 24], 1);
    // expires_at is 1 year from now in milliseconds — well in the future.
    let far_future_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64)
        + 365 * 24 * 3_600_000;
    item.expires_at = Some(far_future_ms);

    let id = item.id.clone();
    insert_item(&db, &item).expect("insert_item");
    upsert_fts(&db, &id, "futurecontent").expect("upsert_fts");

    // Run prune with current time — should not touch this item.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let pruned = delete_expired(&db, now_ms).expect("delete_expired");
    assert_eq!(pruned, 0, "non-expired item must not be pruned");

    let results = search_items(&db, "futurecontent", 10).expect("search");
    assert_eq!(
        results.len(),
        1,
        "FTS row for a non-expired item must be retained"
    );
}
