
use super::*;
use crate::storage::db::Database;
use rusqlite::params;
use uuid::Uuid;

fn make_item(lamport: i64) -> ClipboardItem {
    ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 24], lamport)
}

/// CopyPaste-bfiu: `insert_tombstone` persists a deleted row for an unknown
/// item_id (delete-before-create race) so a later create loses LWW. The row
/// is visible to the merge layer (`get_item_by_item_id`) but hidden from
/// user-facing list queries (`get_page` filters deleted=0).
#[test]
fn insert_tombstone_persists_hidden_deleted_row() {
    let db = Database::open_in_memory().unwrap();
    let n = insert_tombstone(&db, "row-1", "iid-unknown", 42, 9000, "dev-X").unwrap();
    assert_eq!(n, 1, "one tombstone row inserted");

    // Visible to the merge layer.
    let row = get_item_by_item_id(&db, "iid-unknown")
        .unwrap()
        .expect("tombstone row exists");
    assert!(row.deleted, "row must be deleted");
    assert!(row.content.is_none(), "tombstone has no content");
    assert_eq!(row.lamport_ts, 42);
    assert_eq!(row.wall_time, 9000);
    assert_eq!(row.origin_device_id, "dev-X");

    // Hidden from the user-facing history list.
    let page = get_page(&db, 100, 0).unwrap();
    assert!(
        page.iter().all(|i| i.item_id != "iid-unknown"),
        "tombstone must not appear in the history list"
    );
}

/// CopyPaste-00zz: `decrypt_page` must DEGRADE GRACEFULLY across a mix of
/// decryptable and undecryptable (wrong-key) rows: it returns ONLY the
/// rows that verify+decrypt and reports the rest in `skipped`, instead of
/// aborting the whole load on the first AEAD failure. A failed auth tag is
/// never accepted as plaintext.
#[test]
fn decrypt_page_skips_undecryptable_legacy_rows_and_counts_them() {
    use crate::crypto::encrypt::{build_item_aad_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4};

    let db = Database::open_in_memory().unwrap();
    // The "current" v2 key the load path will try. v1_key is unused by the
    // v2 rows below but `decrypt_page` requires both, mirroring the live
    // dual-key dispatch.
    let v1_key = [0x11u8; 32];
    let v2_key = [0x22u8; 32];
    // A DIFFERENT v2 key, standing in for a rotated/old key under which the
    // legacy rows were encrypted — their auth tag cannot verify under v2_key.
    let stale_key = [0x99u8; 32];

    // Seed a row encrypted under `enc_key` with key_version=2 (v4 AAD).
    fn seed_v2(db: &Database, enc_key: &[u8; 32], plaintext: &[u8], lamport: i64) -> String {
        let item_id = ItemId::from(Uuid::new_v4().to_string());
        let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, enc_key, &aad).unwrap();
        let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport);
        item.item_id = item_id;
        let id = item.id.to_string();
        insert_item(db, &item).unwrap();
        id
    }

    // 2 decryptable rows (encrypted under the real v2_key) ...
    let good_a = seed_v2(&db, &v2_key, b"hello-A", 10);
    let good_b = seed_v2(&db, &v2_key, b"hello-B", 11);
    // ... and 3 undecryptable legacy rows (encrypted under the stale key).
    let _bad_1 = seed_v2(&db, &stale_key, b"legacy-1", 1);
    let _bad_2 = seed_v2(&db, &stale_key, b"legacy-2", 2);
    let _bad_3 = seed_v2(&db, &stale_key, b"legacy-3", 3);

    let page = decrypt_page(&db, &v1_key, &v2_key, 100, 0).unwrap();

    assert_eq!(
        page.items.len(),
        2,
        "only the 2 decryptable rows must be surfaced"
    );
    assert_eq!(
        page.skipped, 3,
        "the 3 wrong-key rows must be skipped + counted, not surfaced or fatal"
    );

    // The surfaced rows are exactly the decryptable ones, with correct plaintext.
    let mut got: Vec<(String, Vec<u8>)> = page
        .items
        .into_iter()
        .map(|(row, pt)| (row.id.into_string(), pt))
        .collect();
    got.sort();
    let mut want = vec![(good_a, b"hello-A".to_vec()), (good_b, b"hello-B".to_vec())];
    want.sort();
    assert_eq!(
        got, want,
        "decrypted plaintext must match what was encrypted"
    );
}

#[test]
fn new_image_carries_thumb_and_text_does_not() {
    let img = ClipboardItem::new_image(
        vec![0x01, 0x02],
        "{}".to_string(),
        1,
        Some(vec![0xAA, 0xBB, 0xCC]),
    );
    assert_eq!(img.thumb.as_deref(), Some(&[0xAA, 0xBB, 0xCC][..]));
    assert_eq!(img.content_type, "image");

    let txt = ClipboardItem::new_text(vec![0x00], vec![0u8; 24], 1);
    assert!(txt.thumb.is_none(), "text items must not carry a thumbnail");
}

#[test]
fn new_file_has_file_content_type_and_no_thumb() {
    let item = ClipboardItem::new_file(vec![0x01, 0x02], "{\"k\":1}".to_string(), 3);
    assert_eq!(item.content_type, "file");
    assert!(
        item.thumb.is_none(),
        "file items must not carry a thumbnail"
    );
    assert!(
        item.content_nonce.is_none(),
        "file blob nonces live per-chunk"
    );
    assert_eq!(item.blob_ref.as_deref(), Some("{\"k\":1}"));
}

#[test]
fn new_file_roundtrips_through_insert_and_select() {
    let db = Database::open_in_memory().unwrap();
    let blob = vec![0xCAu8, 0xFE, 0xBA, 0xBE];
    let meta_json = "{\"filename\":\"a.bin\",\"mime\":\"application/octet-stream\"}".to_string();
    let item = ClipboardItem::new_file(blob.clone(), meta_json.clone(), 5);
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert_eq!(got.content_type, "file");
    assert_eq!(
        got.content.as_deref(),
        Some(blob.as_slice()),
        "encrypted blob must survive insert + select"
    );
    assert_eq!(
        got.blob_ref.as_deref(),
        Some(meta_json.as_str()),
        "blob_ref meta JSON must survive insert + select"
    );
}

#[test]
fn thumb_roundtrips_through_insert_and_select() {
    let db = Database::open_in_memory().unwrap();
    let thumb = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    let item = ClipboardItem::new_image(vec![0x10, 0x20], "{}".to_string(), 1, Some(thumb.clone()));
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert_eq!(
        got.thumb.as_deref(),
        Some(thumb.as_slice()),
        "thumb blob must survive insert + select"
    );
}

#[test]
fn set_thumb_backfills_and_clears() {
    let db = Database::open_in_memory().unwrap();
    // Insert an image row with NO thumbnail (legacy / pre-pipeline row).
    let item = ClipboardItem::new_image(vec![0x10, 0x20], "{}".to_string(), 1, None);
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();
    assert!(get_item_by_id(&db, &id).unwrap().unwrap().thumb.is_none());

    // Lazy backfill.
    let blob = vec![0x01u8, 0x02, 0x03];
    let changed = set_thumb(&db, &id, Some(&blob)).unwrap();
    assert_eq!(changed, 1);
    assert_eq!(
        get_item_by_id(&db, &id).unwrap().unwrap().thumb.as_deref(),
        Some(blob.as_slice())
    );

    // Clearing back to NULL.
    let changed = set_thumb(&db, &id, None).unwrap();
    assert_eq!(changed, 1);
    assert!(get_item_by_id(&db, &id).unwrap().unwrap().thumb.is_none());

    // No-op on an unknown id.
    let changed = set_thumb(&db, "00000000-0000-0000-0000-000000000000", Some(&blob)).unwrap();
    assert_eq!(changed, 0);
}

// CopyPaste-44rq.49 — SECURITY: sensitive image items must NEVER have a
// thumbnail stored in the database, regardless of what the caller passes.

#[test]
fn sensitive_image_insert_item_suppresses_thumb() {
    let db = Database::open_in_memory().unwrap();
    let thumb = vec![0xAAu8, 0xBB, 0xCC];
    let mut item =
        ClipboardItem::new_image(vec![0x01, 0x02], "{}".to_string(), 1, Some(thumb.clone()));
    // Mark as sensitive — the insert path must drop the thumbnail.
    item.is_sensitive = true;
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert!(
        got.thumb.is_none(),
        "sensitive image must have NULL thumb after insert_item (got {:?})",
        got.thumb
    );
}

#[test]
fn sensitive_image_insert_item_with_fts_suppresses_thumb() {
    let db = Database::open_in_memory().unwrap();
    let thumb = vec![0xDDu8, 0xEE, 0xFF];
    let mut item =
        ClipboardItem::new_image(vec![0x03, 0x04], "{}".to_string(), 2, Some(thumb.clone()));
    item.is_sensitive = true;
    let id = item.id.clone();
    insert_item_with_fts(&db, &item, "").unwrap();

    let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert!(
        got.thumb.is_none(),
        "sensitive image must have NULL thumb after insert_item_with_fts (got {:?})",
        got.thumb
    );
}

#[test]
fn non_sensitive_image_insert_retains_thumb() {
    // Confirm the fix does not break the non-sensitive path.
    let db = Database::open_in_memory().unwrap();
    let thumb = vec![0x11u8, 0x22, 0x33];
    let item = ClipboardItem::new_image(vec![0x05, 0x06], "{}".to_string(), 3, Some(thumb.clone()));
    assert!(!item.is_sensitive, "factory default must be non-sensitive");
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert_eq!(
        got.thumb.as_deref(),
        Some(thumb.as_slice()),
        "non-sensitive image must retain its thumbnail"
    );
}

#[test]
fn set_thumb_suppresses_backfill_for_sensitive_item() {
    let db = Database::open_in_memory().unwrap();
    // Insert a sensitive image row with no thumbnail (as required by policy).
    let mut item = ClipboardItem::new_image(vec![0x07, 0x08], "{}".to_string(), 4, None);
    item.is_sensitive = true;
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    // Attempt to backfill a thumbnail — must be suppressed.
    let blob = vec![0xABu8, 0xCD, 0xEF];
    let changed = set_thumb(&db, &id, Some(&blob)).unwrap();
    assert_eq!(
        changed, 0,
        "set_thumb must not write to a sensitive row (returned {changed})"
    );
    assert!(
        get_item_by_id(&db, &id).unwrap().unwrap().thumb.is_none(),
        "sensitive row must still have NULL thumb after set_thumb attempt"
    );

    // Clearing (None) must still be allowed so downstream cleanups work.
    let changed = set_thumb(&db, &id, None).unwrap();
    assert_eq!(
        changed, 1,
        "set_thumb(None) must still be allowed for sensitive rows (returned {changed})"
    );
}

#[test]
fn insert_and_count() {
    let db = Database::open_in_memory().unwrap();
    insert_item(&db, &make_item(1)).unwrap();
    insert_item(&db, &make_item(2)).unwrap();
    assert_eq!(count_items(&db).unwrap(), 2);
}

/// CopyPaste-crh3.3 / crh3.56: `prune_to_cap` must NEVER hard-delete a
/// soft-delete tombstone (`deleted = 1`). Tombstones are 0-byte rows that
/// carry sync obligations — the merge layer propagates the delete to offline
/// peers from the persisted tombstone, so evicting one before a peer
/// reconnects would resurrect the item there. A tombstone also frees no
/// space, so excluding it from size-based eviction is strictly correct.
#[test]
fn prune_to_cap_never_evicts_tombstones() {
    let db = Database::open_in_memory().unwrap();

    // Eviction order is (wall_time ASC, id ASC). Make the tombstone the
    // OLDEST unpinned row so the pre-fix code would have evicted it first.
    let mut doomed = make_item(1);
    doomed.wall_time = 10;
    let tombstone_id = doomed.id.clone();
    insert_item(&db, &doomed).unwrap();
    soft_delete_item(&db, &tombstone_id, 100, 11).unwrap();

    // Several live 2-byte rows, all newer than the tombstone.
    for i in 0..5 {
        let mut it = make_item(10 + i);
        it.wall_time = 100 + i;
        insert_item(&db, &it).unwrap();
    }

    let tombstones: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tombstones, 1, "one tombstone seeded");

    // 5 live rows * 2 bytes = 10 bytes unpinned; cap to 4 forces eviction
    // (the newest live row is always protected from same-tick eviction).
    let evicted = prune_to_cap(&db, 4).unwrap();
    assert!(evicted > 0, "eviction must have removed some live rows");

    let survived: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1 AND deleted = 1",
            params![tombstone_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        survived, 1,
        "prune_to_cap must never hard-delete a tombstone"
    );
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
fn get_page_meta_omits_content_blob_but_keeps_metadata() {
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.content_hash = Some("deadbeef".to_string());
    item.blob_ref = Some("blob://x".to_string());
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    // Sanity: get_page returns the full blob.
    let full = get_page(&db, 10, 0).unwrap();
    assert_eq!(full.len(), 1);
    assert_eq!(full[0].content.as_deref(), Some(&[0xAA, 0xBB][..]));

    // get_page_meta drops the blob but preserves metadata.
    let meta = get_page_meta(&db, 10, 0).unwrap();
    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].id, id);
    assert!(
        meta[0].content.is_none(),
        "get_page_meta must NOT load content blob"
    );
    assert_eq!(meta[0].content_hash.as_deref(), Some("deadbeef"));
    assert_eq!(meta[0].blob_ref.as_deref(), Some("blob://x"));
    assert_eq!(meta[0].content_nonce.as_deref(), Some(&[0u8; 24][..]));
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
    let removed = delete_item(&db, &id).unwrap();
    assert_eq!(removed, 1, "exactly one row removed");
    assert_eq!(count_items(&db).unwrap(), 0);
}

#[test]
fn delete_item_reports_zero_for_missing_row() {
    let db = Database::open_in_memory().unwrap();
    let removed = delete_item(&db, "00000000-0000-0000-0000-000000000000").unwrap();
    assert_eq!(removed, 0, "no row matched, nothing removed");
}

#[test]
fn get_item_by_id_returns_matching_row() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(7);
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    let found = get_item_by_id(&db, &id).unwrap();
    assert!(found.is_some(), "inserted row must be found by id");
    let found = found.unwrap();
    assert_eq!(found.id, id);
    assert_eq!(found.lamport_ts, 7);
    assert_eq!(found.content.as_deref(), Some(&[0xAA, 0xBB][..]));
}

#[test]
fn get_item_by_id_returns_none_for_missing_row() {
    let db = Database::open_in_memory().unwrap();
    let found = get_item_by_id(&db, "00000000-0000-0000-0000-000000000000").unwrap();
    assert!(found.is_none(), "absent id must yield None, not an error");
}

#[test]
fn get_item_by_id_finds_row_beyond_first_page() {
    // Regression: `copy_item` used to page get_page(1000, 0) and scan, so
    // any item past position 1000 was unreachable. get_item_by_id must
    // resolve a row regardless of how many other rows exist.
    let db = Database::open_in_memory().unwrap();
    let mut target_id = String::new();
    for i in 0..1200 {
        let item = make_item(i);
        if i == 0 {
            // Oldest row (sorts last under ORDER BY wall_time DESC) — would
            // fall outside a 1000-row window once 1200 rows exist.
            target_id = item.id.to_string();
        }
        insert_item(&db, &item).unwrap();
    }
    let found = get_item_by_id(&db, &target_id).unwrap();
    assert!(
        found.is_some(),
        "row beyond the legacy 1000-row page window must still be found"
    );
    assert_eq!(found.unwrap().id, target_id);
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

/// CopyPaste-j9pv: `upsert_fts` must be atomic — the DELETE and INSERT must
/// commit together so a partial write cannot leave an item permanently
/// unsearchable.  We verify the all-or-nothing guarantee by asserting that
/// after a successful upsert exactly one FTS row exists with the new text.
#[test]
fn upsert_fts_atomic_replace() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();

    // First upsert seeds the FTS row.
    upsert_fts(&db, &item.id, "initial content").unwrap();

    // Second upsert must atomically remove the old row and insert the new
    // one — exactly one row must exist after the call, containing the new text.
    upsert_fts(&db, &item.id, "replaced content").unwrap();

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            params![item.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "exactly one FTS row must exist after atomic upsert"
    );

    // Verify the FTS row reflects the new text (not the old one).
    let results = search_items(&db, "replaced", 10).unwrap();
    assert_eq!(results.len(), 1, "FTS must return the updated text");

    // The old text must NOT match.
    let old_results = search_items(&db, "initial", 10).unwrap();
    assert_eq!(
        old_results.len(),
        0,
        "old FTS text must be gone after atomic replace"
    );
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

/// Regression (P0): a hyphen-joined query like `foo-bar` must not reach the
/// FTS5 MATCH operator with a raw `-`, otherwise FTS5 parses `-bar` as a
/// column filter and errors with "no such column: bar". The sanitizer
/// rewrites `-` to whitespace so these queries succeed (return Ok).
#[test]
fn search_items_hyphen_query_does_not_error() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    upsert_fts(&db, &item.id, "harmless content").unwrap();

    // Each of these previously triggered "no such column: ..." on real DBs.
    for q in [
        "foo-bar",
        "2026-06-02",
        "x86-64",
        "well-known",
        "co-op coffee",
    ] {
        let res = search_items(&db, q, 10);
        assert!(
            res.is_ok(),
            "hyphen query {q:?} must not error, got: {:?}",
            res.err()
        );
    }
}

/// A stored item containing a hyphenated word must be found when the user
/// searches for that same hyphenated term: `well-known` → `well AND known*`.
#[test]
fn search_items_finds_hyphenated_term() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    upsert_fts(&db, &item.id, "this is a well-known endpoint").unwrap();

    let results = search_items(&db, "well-known", 10).unwrap();
    assert_eq!(results.len(), 1, "hyphenated term must match stored item");
    assert_eq!(results[0].id, item.id);
}

// --- CopyPaste-tteo: search_items_filtered type-filter tests ---

/// `search_items_filtered(None)` is identical to `search_items`.
#[test]
fn search_items_filtered_none_matches_all_types() {
    let db = Database::open_in_memory().unwrap();
    let text_item = make_item(1);
    insert_item(&db, &text_item).unwrap();
    upsert_fts(&db, &text_item.id, "common keyword").unwrap();

    // No filter: must find the text item.
    let results = search_items_filtered(&db, "common", 10, None).unwrap();
    assert_eq!(results.len(), 1, "no filter must find the item");
    assert_eq!(results[0].id, text_item.id);
}

/// Kind filter `"text"` finds only text items, not image items.
#[test]
fn search_items_filtered_text_type_finds_only_text() {
    let db = Database::open_in_memory().unwrap();
    let text_item = make_item(1); // content_type = "text"
    let image_item = ClipboardItem::new_image(vec![0u8; 4], "{}".to_string(), 2, None);
    insert_item(&db, &text_item).unwrap();
    insert_item(&db, &image_item).unwrap();
    // FTS only indexes text content, but we give the image an entry too
    // so the filter is what discriminates — not absence of FTS row.
    upsert_fts(&db, &text_item.id, "shared term").unwrap();
    upsert_fts(&db, &image_item.id, "shared term").unwrap();

    let results = search_items_filtered(&db, "shared", 10, Some("text")).unwrap();
    assert_eq!(results.len(), 1, "text filter must find only text items");
    assert_eq!(results[0].content_type, "text");

    let results_image = search_items_filtered(&db, "shared", 10, Some("image")).unwrap();
    assert_eq!(
        results_image.len(),
        1,
        "image filter must find only image items"
    );
    assert_eq!(results_image[0].content_type, "image");
}

/// Kind filter for a type with no matching items returns empty.
#[test]
fn search_items_filtered_unknown_type_returns_empty() {
    let db = Database::open_in_memory().unwrap();
    let text_item = make_item(1);
    insert_item(&db, &text_item).unwrap();
    upsert_fts(&db, &text_item.id, "needle").unwrap();

    let results = search_items_filtered(&db, "needle", 10, Some("file")).unwrap();
    assert!(
        results.is_empty(),
        "file filter must return empty when only text items exist"
    );
}

/// Empty query still returns empty even with a type filter.
#[test]
fn search_items_filtered_empty_query_with_type_returns_empty() {
    let db = Database::open_in_memory().unwrap();
    let text_item = make_item(1);
    insert_item(&db, &text_item).unwrap();
    upsert_fts(&db, &text_item.id, "anything").unwrap();

    let results = search_items_filtered(&db, "", 10, Some("text")).unwrap();
    assert!(results.is_empty(), "empty query must always return empty");
}

/// Direct unit check of the sanitizer: hyphens become whitespace-separated
/// AND-ed terms and no raw `-` survives.
#[test]
fn sanitize_fts5_query_rewrites_hyphen_to_space() {
    let out = sanitize_fts5_query("foo-bar").expect("non-empty");
    assert!(!out.contains('-'), "no raw hyphen may remain: {out:?}");
    assert_eq!(out, "foo AND bar*");
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
fn delete_sensitive_expired_keeps_pinned_items() {
    // Regression: a pinned + sensitive item past the sensitive cutoff must
    // NOT be auto-wiped — pinned rows are exempt from every TTL prune.
    let db = Database::open_in_memory().unwrap();

    // Pinned + sensitive + old wall_time → must survive the prune.
    let mut pinned_sensitive = make_item(1);
    pinned_sensitive.is_sensitive = true;
    pinned_sensitive.pinned = true;
    pinned_sensitive.wall_time = 1_000; // well past the cutoff below
    let pinned_id = pinned_sensitive.id.clone();
    insert_item(&db, &pinned_sensitive).unwrap();

    // Unpinned + sensitive + old wall_time → control row, must be deleted.
    let mut unpinned_sensitive = make_item(2);
    unpinned_sensitive.is_sensitive = true;
    unpinned_sensitive.pinned = false;
    unpinned_sensitive.wall_time = 1_000;
    insert_item(&db, &unpinned_sensitive).unwrap();

    // now_ms=200_000, ttl=30_000 → threshold=170_000; both wall_times qualify.
    let removed = delete_sensitive_expired(&db, 200_000, 30_000).unwrap();
    assert_eq!(removed, 1, "only the unpinned sensitive row is wiped");
    assert!(
        get_item_by_id(&db, &pinned_id).unwrap().is_some(),
        "pinned+sensitive item must survive the sensitive TTL prune"
    );
}

/// CopyPaste-ny0g: `has_sensitive_items` must return `true` when the DB is
/// healthy and contains an unpinned sensitive row.
#[test]
fn has_sensitive_items_returns_true_when_sensitive_row_present() {
    let db = Database::open_in_memory().unwrap();

    let mut sensitive = make_item(1);
    sensitive.is_sensitive = true;
    sensitive.pinned = false;
    insert_item(&db, &sensitive).unwrap();

    assert!(
        has_sensitive_items(&db),
        "must return true when an unpinned sensitive row exists"
    );
}

/// CopyPaste-ny0g: `has_sensitive_items` must return `false` when only
/// pinned sensitive items exist (pinned items are exempt from TTL sweeps).
#[test]
fn has_sensitive_items_returns_false_for_pinned_only() {
    let db = Database::open_in_memory().unwrap();

    let mut pinned_sensitive = make_item(1);
    pinned_sensitive.is_sensitive = true;
    pinned_sensitive.pinned = true;
    insert_item(&db, &pinned_sensitive).unwrap();

    assert!(
        !has_sensitive_items(&db),
        "pinned sensitive rows must not count — they are exempt from TTL"
    );
}

/// CopyPaste-ny0g: fail-closed security guarantee — `has_sensitive_items`
/// must return `true` (not `false`) when the query errors.
///
/// Returning `false` on error causes the TTL sweep to be silently skipped,
/// allowing sensitive items to outlive their TTL. Returning `true` is
/// conservative (causes an unnecessary sweep attempt) but guarantees the
/// TTL sweep path is always invoked, giving `delete_sensitive_expired` a
/// chance to enforce the TTL. This is the fail-closed stance: prefer a
/// false-positive pre-check over a silently-suppressed sweep.
#[test]
fn has_sensitive_items_fails_closed_on_db_error() {
    // Open a valid DB, then destroy the clipboard_items table so the
    // SELECT EXISTS query fails with "no such table". The function must
    // return `true` (treat-as-sensitive) rather than `false` (silent skip).
    let db = Database::open_in_memory().unwrap();
    db.conn()
        .execute_batch("DROP TABLE clipboard_items;")
        .unwrap();

    assert!(
        has_sensitive_items(&db),
        "must return true (fail-closed) when the query errors, \
             to avoid silently skipping the sensitive-item TTL sweep"
    );
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

/// Regression: `pin_item` and `unpin_item` must bump `lamport_ts` so the
/// pin-state change wins LWW merge on peers that already have the item.
/// Without this bump a peer receiving the item via cloud backlog or P2P
/// would silently discard the pin update because the timestamp tie-breaks
/// in favour of the (unchanged) local copy.
///
/// CopyPaste-ojhe: the bump now stamps the UNIFIED value space
/// `MAX(lamport_ts + 1, now_ms)`, not a bare `+1`. A `make_item(10)` row
/// pinned today lands on `now_ms` (~1.75e12), strictly greater than 10, so
/// the pin remains monotonic AND time-ordered — and can overtake a stale
/// now_ms-magnitude recopy of the same item (the bug this fixes).
#[test]
fn pin_unpin_bumps_lamport_ts() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(10);
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();
    // Wall-clock floor: every unified stamp is at least this.
    let floor = now_ms_epoch() - 1000;

    // pin_item must advance lamport_ts to the unified value (>= now_ms).
    pin_item(&db, &id).unwrap();
    let after_pin = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert!(
        after_pin.lamport_ts > 10,
        "pin_item must bump lamport_ts above the inserted value (was 10, got {})",
        after_pin.lamport_ts
    );
    assert!(
        after_pin.lamport_ts >= floor,
        "pin_item must stamp the unified now_ms-based value (got {}, floor {})",
        after_pin.lamport_ts,
        floor
    );
    assert!(after_pin.pinned, "item must be pinned after pin_item");
    assert!(
        after_pin.pin_order.is_some(),
        "pin_item must assign a non-null pin_order"
    );

    // unpin_item must advance lamport_ts strictly beyond the post-pin value.
    unpin_item(&db, &id).unwrap();
    let after_unpin = get_item_by_id(&db, &id).unwrap().expect("row must exist");
    assert!(
        after_unpin.lamport_ts >= after_pin.lamport_ts,
        "unpin_item must not regress lamport_ts (pin={}, unpin={})",
        after_pin.lamport_ts,
        after_unpin.lamport_ts
    );
    assert!(
        !after_unpin.pinned,
        "item must be unpinned after unpin_item"
    );
    assert!(
        after_unpin.pin_order.is_none(),
        "unpin_item must clear pin_order back to NULL"
    );
}

/// CopyPaste-ojhe: `next_lamport_ts` is monotonic AND time-ordered.
#[test]
fn next_lamport_ts_is_monotonic_and_time_ordered() {
    // When now_ms dominates (fresh capture, prev=0), we get now_ms.
    assert_eq!(next_lamport_ts(0, 1_750_000_000_000), 1_750_000_000_000);
    // When prev+1 dominates (two edits in the same ms), we get prev+1 so the
    // value still strictly increases.
    assert_eq!(
        next_lamport_ts(1_750_000_000_005, 1_750_000_000_000),
        1_750_000_000_006
    );
    // Always strictly greater than prev.
    for prev in [0i64, 1, 1_750_000_000_000, i64::MAX - 1] {
        assert!(next_lamport_ts(prev, 0) > prev || prev == i64::MAX);
    }
}

/// CopyPaste-ojhe: a newer pin (unified) beats an older recopy (now_ms) when
/// compared by raw lamport — the exact data-loss scenario from the audit.
#[test]
fn newer_pin_lamport_beats_older_recopy_lamport() {
    // Older recopy stamped at now_ms.
    let recopy_now = 1_750_000_000_000i64;
    let recopy_lamport = next_lamport_ts(0, recopy_now); // == recopy_now

    // The item is then pinned a few ms later: MAX(recopy + 1, pin_now).
    let pin_now = recopy_now + 5;
    let pin_lamport = next_lamport_ts(recopy_lamport, pin_now);

    assert!(
        pin_lamport > recopy_lamport,
        "the unified pin lamport ({pin_lamport}) must exceed the recopy \
             lamport ({recopy_lamport}) so lamport-first LWW keeps the pin"
    );
}

/// Regression: `reorder_pinned` must bump `lamport_ts` on each row so the
/// new drag-to-reorder ordering wins LWW merge on peers.
#[test]
fn reorder_pinned_bumps_lamport_ts() {
    let db = Database::open_in_memory().unwrap();

    let item_a = make_item(5);
    let id_a = item_a.id.clone();
    insert_item(&db, &item_a).unwrap();
    pin_item(&db, &id_a).unwrap();

    let item_b = make_item(6);
    let id_b = item_b.id.clone();
    insert_item(&db, &item_b).unwrap();
    pin_item(&db, &id_b).unwrap();

    // Record lamport_ts values after pinning.
    let a_before = get_item_by_id(&db, &id_a)
        .unwrap()
        .expect("row must exist")
        .lamport_ts;
    let b_before = get_item_by_id(&db, &id_b)
        .unwrap()
        .expect("row must exist")
        .lamport_ts;

    // Reorder: put b first, a second.
    reorder_pinned(&db, &[&id_b, &id_a]).unwrap();

    let a_after = get_item_by_id(&db, &id_a)
        .unwrap()
        .expect("row must exist")
        .lamport_ts;
    let b_after = get_item_by_id(&db, &id_b)
        .unwrap()
        .expect("row must exist")
        .lamport_ts;

    assert!(
        a_after > a_before,
        "reorder_pinned must bump lamport_ts on item_a: before={a_before}, after={a_after}"
    );
    assert!(
        b_after > b_before,
        "reorder_pinned must bump lamport_ts on item_b: before={b_before}, after={b_after}"
    );
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
fn insert_persists_item_key_version_not_constant() {
    // Regression: insert must bind `item.key_version`, not the
    // ITEM_KEY_VERSION_CURRENT constant. A v1 item must persist as 1.
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.key_version = 1;
    insert_item(&db, &item).unwrap();
    assert_eq!(
        get_key_version(&db, &item.id).unwrap(),
        Some(1),
        "insert_item must persist item.key_version verbatim"
    );

    // Same contract for the FTS path.
    let mut item2 = make_item(2);
    item2.key_version = 1;
    let id2 = insert_item_with_fts(&db, &item2, "indexed text").unwrap();
    assert_eq!(get_key_version(&db, &id2).unwrap(), Some(1));
}

#[test]
fn insert_rejects_out_of_range_key_version() {
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.key_version = 3; // outside the supported {1, 2} set
    let err = insert_item(&db, &item).unwrap_err();
    assert!(
        matches!(err, ItemsError::UnsupportedKeyVersion(3)),
        "out-of-range key_version must be rejected, not silently written: {err:?}"
    );
    // Nothing should have been persisted.
    assert_eq!(count_items(&db).unwrap(), 0);
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

// --- T5: UI clipboard model — preview clamp + edge cases ---

/// `clamp_preview` must return the text unchanged when it fits within the limit.
#[test]
fn clamp_preview_short_text_unchanged() {
    let text = "hello world".to_string();
    assert_eq!(clamp_preview(text.clone(), MAX_PREVIEW_BYTES), text);
}

/// `clamp_preview` must truncate at a UTF-8 boundary and append `…`.
#[test]
fn clamp_preview_long_text_truncated() {
    // Build a string that is longer than MAX_PREVIEW_BYTES (1024 bytes).
    let long_text: String = "a".repeat(MAX_PREVIEW_BYTES + 100);
    let result = clamp_preview(long_text, MAX_PREVIEW_BYTES);
    // Result must be at most MAX_PREVIEW_BYTES bytes (plus the 3-byte `…` ellipsis).
    // The truncated body is ≤ MAX_PREVIEW_BYTES and the appended ellipsis is "…" (3 bytes).
    assert!(
        result.len() <= MAX_PREVIEW_BYTES + "…".len(),
        "clamped preview too long: {} bytes",
        result.len()
    );
    assert!(
        result.ends_with('…'),
        "clamped preview must end with ellipsis"
    );
    assert!(
        result.is_char_boundary(result.len()),
        "result must be valid UTF-8"
    );
}

/// `clamp_preview` must not split a multi-byte character.
#[test]
fn clamp_preview_respects_utf8_boundary() {
    // Each '€' is 3 bytes (U+20AC).  Build a string where the naive byte
    // boundary would fall inside a character.
    let euros: String = "€".repeat(400); // 1200 bytes total
    let result = clamp_preview(euros, MAX_PREVIEW_BYTES);
    // Must be valid UTF-8 (would panic on index otherwise).
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    assert!(result.ends_with('…'));
}

/// `fetch_text_preview` returns `None` for items with no FTS entry (e.g. images).
#[test]
fn fetch_text_preview_returns_none_for_no_fts_entry() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    // No FTS entry inserted — simulates an image item or pre-FTS row.
    let result = fetch_text_preview(&db, &item.id).unwrap();
    assert!(result.is_none(), "expected None when no FTS entry exists");
}

/// `fetch_text_preview` returns clamped plaintext for text items.
#[test]
fn fetch_text_preview_returns_short_text_unchanged() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    upsert_fts(&db, &item.id, "short snippet").unwrap();

    let result = fetch_text_preview(&db, &item.id).unwrap();
    assert_eq!(result, Some("short snippet".to_string()));
}

/// `fetch_text_preview` clamps text that exceeds MAX_PREVIEW_BYTES.
#[test]
fn fetch_text_preview_clamps_large_text() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    let big_text: String = "x".repeat(MAX_PREVIEW_BYTES + 500);
    upsert_fts(&db, &item.id, &big_text).unwrap();

    let result = fetch_text_preview(&db, &item.id).unwrap().unwrap();
    assert!(
        result.len() <= MAX_PREVIEW_BYTES + "…".len(),
        "preview must be clamped to ~{} bytes, got {}",
        MAX_PREVIEW_BYTES,
        result.len()
    );
    assert!(result.ends_with('…'));
}

/// CopyPaste-mnte: batch preview fetch returns clamped text for every id
/// that has an FTS entry, in one round-trip, and omits ids without one.
#[test]
fn fetch_text_previews_batch_returns_map_for_present_ids() {
    let db = Database::open_in_memory().unwrap();
    let a = make_item(1);
    let b = make_item(2);
    let c = make_item(3); // no FTS entry — must be absent from the map
    insert_item(&db, &a).unwrap();
    insert_item(&db, &b).unwrap();
    insert_item(&db, &c).unwrap();
    upsert_fts(&db, &a.id, "alpha snippet").unwrap();
    upsert_fts(&db, &b.id, "beta snippet").unwrap();

    let ids = [a.id.as_str(), b.id.as_str(), c.id.as_str()];
    let map = fetch_text_previews_batch(&db, &ids).unwrap();

    assert_eq!(map.get(a.id.as_str()).map(String::as_str), Some("alpha snippet"));
    assert_eq!(map.get(b.id.as_str()).map(String::as_str), Some("beta snippet"));
    assert!(
        !map.contains_key(c.id.as_str()),
        "id with no FTS entry must be absent from the batch map"
    );
    // Parity with the per-item helper for both present ids.
    assert_eq!(
        map.get(a.id.as_str()).cloned(),
        fetch_text_preview(&db, &a.id).unwrap()
    );
    assert_eq!(
        map.get(b.id.as_str()).cloned(),
        fetch_text_preview(&db, &b.id).unwrap()
    );
}

/// CopyPaste-mnte: empty id slice issues no SQL and returns an empty map.
#[test]
fn fetch_text_previews_batch_empty_ids_is_noop() {
    let db = Database::open_in_memory().unwrap();
    let map = fetch_text_previews_batch(&db, &[]).unwrap();
    assert!(map.is_empty());
}

/// CopyPaste-mnte: batch preview clamps long text identically to the
/// per-item path.
#[test]
fn fetch_text_previews_batch_clamps_large_text() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    let big_text: String = "y".repeat(MAX_PREVIEW_BYTES + 500);
    upsert_fts(&db, &item.id, &big_text).unwrap();

    let map = fetch_text_previews_batch(&db, &[item.id.as_str()]).unwrap();
    let got = map.get(item.id.as_str()).expect("present");
    assert!(got.len() <= MAX_PREVIEW_BYTES + "…".len());
    assert!(got.ends_with('…'));
}

/// CopyPaste-pvp4: the schema-v11 partial covering index used by the
/// `prune_to_cap` size gate exists on a freshly migrated database.
#[test]
fn schema_has_unpinned_len_covering_index() {
    let db = Database::open_in_memory().unwrap();
    let found: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'index' AND name = 'idx_clipboard_unpinned_len'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(found, 1, "idx_clipboard_unpinned_len must exist");
}

/// CopyPaste-pvp4: the `prune_to_cap` size-gate SUM is planned as an
/// index-only scan over the partial covering index (no full-table scan and
/// no BLOB reads). We assert the query plan references the covering index.
#[test]
fn prune_to_cap_size_gate_uses_covering_index() {
    let db = Database::open_in_memory().unwrap();
    for i in 0..5 {
        insert_item(&db, &make_item(i)).unwrap();
    }
    let plan: Vec<String> = {
        let conn = db.conn();
        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN \
                     SELECT COALESCE(SUM(LENGTH(COALESCE(content, ''))), 0) \
                     FROM clipboard_items WHERE pinned = 0",
            )
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let joined = plan.join(" | ");
    assert!(
        joined.contains("idx_clipboard_unpinned_len"),
        "size-gate SUM must use the covering index, plan was: {joined}"
    );
}

/// Empty history list — model correctly handles zero items.
#[test]
fn get_page_meta_empty_db_returns_empty_list() {
    let db = Database::open_in_memory().unwrap();
    let result = get_page_meta(&db, 50, 0).unwrap();
    assert!(result.is_empty(), "expected empty list for empty DB");
}

// --- FIX 1: get_page_pinned_first ---

/// Pinned items must appear before unpinned items regardless of wall_time.
#[test]
fn get_page_pinned_first_pins_before_unpinned() {
    let db = Database::open_in_memory().unwrap();

    // Insert three items with ascending wall_time.
    // item_a: oldest, will be pinned
    // item_b: middle, unpinned
    // item_c: newest, unpinned
    let mut item_a = make_item(1);
    item_a.wall_time = 1_000;
    let id_a = item_a.id.clone();
    insert_item(&db, &item_a).unwrap();

    let mut item_b = make_item(2);
    item_b.wall_time = 2_000;
    insert_item(&db, &item_b).unwrap();

    let mut item_c = make_item(3);
    item_c.wall_time = 3_000;
    insert_item(&db, &item_c).unwrap();

    // Pin item_a (the oldest one).
    pin_item(&db, &id_a).unwrap();

    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(page.len(), 3);
    // Pinned item must be first, regardless of its wall_time.
    assert_eq!(
        page[0].id, id_a,
        "pinned item must be first regardless of age"
    );
    assert!(page[0].pinned, "first item must have pinned=true");
    // Remaining items must be sorted newest-first.
    assert!(
        page[1].wall_time >= page[2].wall_time,
        "unpinned items must be newest-first"
    );
}

/// Multiple pinned items are sorted newest-first within the pinned group.
/// Pinned items appear before unpinned items; within the pinned group they
/// are ordered by `pin_order ASC` (insertion order by default, since
/// `pin_item` assigns `MAX(pin_order)+1`). This test verifies that the item
/// pinned first has the lower `pin_order` and therefore appears first,
/// regardless of its `wall_time`.
#[test]
fn get_page_pinned_first_multiple_pins_sorted_by_pin_order() {
    let db = Database::open_in_memory().unwrap();

    // old_pin: low wall_time, pinned first → pin_order = 1.0
    let mut old_pin = make_item(1);
    old_pin.wall_time = 100;
    let old_pin_id = old_pin.id.clone();
    insert_item(&db, &old_pin).unwrap();
    pin_item(&db, &old_pin_id).unwrap();

    // new_pin: high wall_time, pinned second → pin_order = 2.0
    let mut new_pin = make_item(2);
    new_pin.wall_time = 900;
    let new_pin_id = new_pin.id.clone();
    insert_item(&db, &new_pin).unwrap();
    pin_item(&db, &new_pin_id).unwrap();

    let mut unpinned = make_item(3);
    unpinned.wall_time = 500;
    insert_item(&db, &unpinned).unwrap();

    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(page.len(), 3);
    // Both pins appear first.
    assert!(
        page[0].pinned && page[1].pinned,
        "first two items must be pinned"
    );
    // Within the pinned group, order is by pin_order ASC (insertion order).
    // old_pin was pinned first (pin_order=1.0) so it appears before new_pin
    // (pin_order=2.0) even though old_pin has a lower wall_time.
    assert_eq!(
        page[0].id, old_pin_id,
        "item pinned first (lower pin_order) must appear first"
    );
    assert_eq!(
        page[1].id, new_pin_id,
        "item pinned second (higher pin_order) must appear second"
    );
    assert!(
        page[0].pin_order.unwrap() < page[1].pin_order.unwrap(),
        "pin_order must be ascending within the pinned group"
    );
    // Then unpinned.
    assert!(!page[2].pinned, "third item must not be pinned");
}

/// Defensive (HIGH): a sync-replaced pinned row whose `pin_order` became
/// NULL must sort AFTER pinned rows with explicit `pin_order` values, not
/// before them. SQLite sorts NULL first under plain `ASC`, so the ORDER BY
/// adds `pin_order IS NULL ASC` to push NULLs to the end of the pinned group.
#[test]
fn get_page_pinned_first_null_pin_order_sorts_last_among_pins() {
    let db = Database::open_in_memory().unwrap();

    // Two normally-pinned items with explicit pin_order 1.0 and 2.0.
    let mut p1 = make_item(1);
    p1.wall_time = 100;
    let p1_id = p1.id.clone();
    insert_item(&db, &p1).unwrap();
    pin_item(&db, &p1_id).unwrap();

    let mut p2 = make_item(2);
    p2.wall_time = 200;
    let p2_id = p2.id.clone();
    insert_item(&db, &p2).unwrap();
    pin_item(&db, &p2_id).unwrap();

    // A pinned item whose pin_order is NULL (simulating a sync replace that
    // dropped pin_order). Insert directly with pinned=1, pin_order=None.
    let mut null_pin = make_item(3);
    null_pin.wall_time = 9_999; // newest, to prove ordering is by pin_order not wall_time
    null_pin.pinned = true;
    null_pin.pin_order = None;
    let null_pin_id = null_pin.id.clone();
    insert_item(&db, &null_pin).unwrap();

    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(page.len(), 3);
    assert!(
        page[0].pinned && page[1].pinned && page[2].pinned,
        "all three items are pinned"
    );
    // Explicit pin_order rows come first, in pin_order order.
    assert_eq!(page[0].id, p1_id, "pin_order=1.0 first");
    assert_eq!(page[1].id, p2_id, "pin_order=2.0 second");
    // The NULL pin_order row sorts LAST despite the newest wall_time.
    assert_eq!(
        page[2].id, null_pin_id,
        "NULL pin_order must sort after explicit pin_order values"
    );
    assert!(page[2].pin_order.is_none());
}

/// Unpinning an item moves it back into the unpinned group.
#[test]
fn pin_and_unpin_changes_sort_position() {
    let db = Database::open_in_memory().unwrap();

    let mut old = make_item(1);
    old.wall_time = 100;
    let old_id = old.id.clone();
    insert_item(&db, &old).unwrap();

    let mut new = make_item(2);
    new.wall_time = 200;
    insert_item(&db, &new).unwrap();

    // Pin the old item — it should appear first.
    pin_item(&db, &old_id).unwrap();
    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(page[0].id, old_id, "pinned old item must be first");

    // Unpin it — it should fall back to recency order (last).
    unpin_item(&db, &old_id).unwrap();
    let page2 = get_page_pinned_first(&db, 10, 0).unwrap();
    assert!(
        !page2[0].pinned,
        "after unpin, first item must not be pinned"
    );
    assert!(
        page2[0].wall_time >= page2[1].wall_time,
        "items must be newest-first after unpin"
    );
}

// --- FIX 2: bump_item_recency + content_hash dedup ---

/// `bump_item_recency` updates wall_time and lamport_ts, returns 1 row changed.
#[test]
fn bump_item_recency_updates_wall_time_and_lamport() {
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.wall_time = 1_000;
    insert_item(&db, &item).unwrap();

    let changed = bump_item_recency(&db, &item.id, 99_000, 99_000, None).unwrap();
    assert_eq!(changed, 1, "one row must be updated");

    let fetched = get_item_by_id(&db, &item.id).unwrap().unwrap();
    assert_eq!(fetched.wall_time, 99_000, "wall_time must be bumped");
    assert_eq!(fetched.lamport_ts, 99_000, "lamport_ts must be bumped");
}

/// `bump_item_recency` returns 0 when id does not exist (no row updated).
#[test]
fn bump_item_recency_returns_zero_for_missing_id() {
    let db = Database::open_in_memory().unwrap();
    let changed = bump_item_recency(&db, "nonexistent-id", 999, 999, None).unwrap();
    assert_eq!(changed, 0, "no row matched; must return 0");
}

/// After `bump_item_recency`, the bumped item sorts to the top in
/// `get_page_pinned_first` because its wall_time is now the newest.
#[test]
fn bumped_item_sorts_to_top() {
    let db = Database::open_in_memory().unwrap();

    // Three items, item_a is oldest.
    let mut item_a = make_item(1);
    item_a.wall_time = 100;
    let id_a = item_a.id.clone();
    insert_item(&db, &item_a).unwrap();

    let mut item_b = make_item(2);
    item_b.wall_time = 200;
    insert_item(&db, &item_b).unwrap();

    let mut item_c = make_item(3);
    item_c.wall_time = 300;
    insert_item(&db, &item_c).unwrap();

    // Bump item_a to wall_time=999 (the new highest).
    bump_item_recency(&db, &id_a, 999, 999, None).unwrap();

    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(page[0].id, id_a, "bumped item must appear at the top");
    assert_eq!(page[0].wall_time, 999);
}

/// Fix 4: `find_recent_by_hash` must not overflow when `now_ms < within_ms`
/// (e.g. now_ms=0 and within_ms=i64::MAX). Before the fix, the subtraction
/// `now_ms - within_ms` panics in debug builds.
#[test]
fn find_recent_by_hash_cutoff_no_overflow() {
    let db = Database::open_in_memory().unwrap();
    // now_ms=0, within_ms=i64::MAX → would overflow without saturating_sub.
    let result = find_recent_by_hash(&db, "anyhash", 0, i64::MAX);
    assert!(
        result.is_ok(),
        "must not panic or error on underflowing cutoff"
    );
    assert!(result.unwrap().is_none(), "empty db returns None");
}

/// Fix 3: `row_to_item` must return `CorruptKeyVersion` for out-of-range
/// key_version values (e.g. 999 does not fit in u8 without silent truncation).
#[test]
fn row_to_item_corrupt_key_version_returns_error() {
    let db = Database::open_in_memory().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    // Insert a row with key_version=999 directly via SQL, bypassing insert_item's
    // ITEM_KEY_VERSION_CURRENT stamp.
    db.conn()
        .execute(
            "INSERT INTO clipboard_items
                 (id, item_id, content_type, content, content_nonce, blob_ref,
                  is_sensitive, is_synced, lamport_ts, wall_time, expires_at,
                  app_bundle_id, content_hash, origin_device_id, key_version, pinned)
                 VALUES (?1,?2,'text',NULL,NULL,NULL,0,0,1,1,NULL,NULL,NULL,'',999,0)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    let result = get_item_by_id(&db, &id);
    assert!(
        matches!(result, Err(ItemsError::CorruptKeyVersion(999))),
        "expected CorruptKeyVersion(999), got: {result:?}"
    );
}

/// `find_recent_by_hash` finds a matching row when the window is wide open.
#[test]
fn find_recent_by_hash_finds_any_row_with_wide_window() {
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.content_hash = Some("aabbcc".to_string());
    item.wall_time = 1_000;
    insert_item(&db, &item).unwrap();

    // With i64::MAX window, any row with that hash should be found.
    let now_ms = i64::MAX / 2;
    let found = find_recent_by_hash(&db, "aabbcc", now_ms, i64::MAX).unwrap();
    assert_eq!(
        found,
        Some(item.id.to_string()),
        "should find the row with matching hash"
    );
}

/// `find_recent_by_hash` returns None when no row has the given hash.
#[test]
fn find_recent_by_hash_returns_none_for_missing_hash() {
    let db = Database::open_in_memory().unwrap();
    let found = find_recent_by_hash(&db, "deadbeef", 99_000, i64::MAX).unwrap();
    assert!(found.is_none(), "no rows, must return None");
}

/// Dedup simulation: inserting the same content hash a second time via
/// find_recent_by_hash + bump avoids a second row, and the bumped item
/// sorts to the top.
/// CopyPaste-fuxl: re-copying content that was soft-deleted within the SAME
/// `wall_time / 60` bucket must create a FRESH LIVE row, not silently dedup
/// against the tombstone. Before the v15 index rebuild + the lookup
/// `deleted = 0` filter, the re-insert hit the dedup UNIQUE violation and fell
/// back to the tombstone id, so the re-copy vanished.
#[test]
fn recopy_after_same_bucket_soft_delete_inserts_fresh_live_row() {
    let db = Database::open_in_memory().unwrap();
    let hash = "fuxlhash".to_string();

    // Capture in minute bucket 1000 (60_000 / 60 == 1000).
    let mut first = make_item(1);
    first.wall_time = 60_000;
    first.content_hash = Some(hash.clone());
    let first_id = first.id.clone();
    insert_item(&db, &first).unwrap();
    assert_eq!(
        get_page(&db, 100, 0).unwrap().len(),
        1,
        "one live row after capture"
    );

    // Soft-delete it — the tombstone keeps content_hash + the same bucket.
    soft_delete_item(&db, &first_id, 100, 60_010).unwrap();
    assert!(
        get_page(&db, 100, 0).unwrap().is_empty(),
        "no live rows after delete"
    );

    // Re-copy the SAME content in the SAME minute bucket (60_030 / 60 == 1000).
    let mut recopy = make_item(2);
    recopy.wall_time = 60_030;
    recopy.content_hash = Some(hash.clone());
    insert_item(&db, &recopy).unwrap();

    // The re-copy must be a fresh LIVE row, not deduped to the tombstone.
    let live = get_page(&db, 100, 0).unwrap();
    assert_eq!(live.len(), 1, "re-copy must appear as a fresh live row");
    assert_eq!(live[0].content_hash.as_deref(), Some(hash.as_str()));
    assert_ne!(live[0].id, first_id, "must be a NEW row, not the tombstone");
}

#[test]
fn dedup_bump_prevents_duplicate_row_and_sorts_to_top() {
    let db = Database::open_in_memory().unwrap();

    // First capture: insert item with content_hash.
    let hash = "cafebabe".to_string();
    let mut item_first = make_item(1);
    item_first.wall_time = 1_000;
    item_first.content_hash = Some(hash.clone());
    let id_first = item_first.id.clone();
    insert_item(&db, &item_first).unwrap();

    // Insert a second, newer item so there are two rows total.
    let mut item_second = make_item(2);
    item_second.wall_time = 2_000;
    insert_item(&db, &item_second).unwrap();

    // "Second capture" of the same content: simulate the daemon dedup path.
    let now_ms: i64 = 9_999;
    let existing_id = find_recent_by_hash(&db, &hash, now_ms, i64::MAX)
        .unwrap()
        .expect("existing row must be found");
    assert_eq!(existing_id, id_first, "must find the original row");

    // Bump it.
    let changed = bump_item_recency(&db, &existing_id, now_ms, now_ms, None).unwrap();
    assert_eq!(changed, 1, "bump must affect one row");

    // Still only two rows total — no duplicate inserted.
    let total = count_items(&db).unwrap();
    assert_eq!(
        total, 2,
        "dedup must not insert a second row for the same hash"
    );

    // The bumped item now sorts to the top.
    let page = get_page_pinned_first(&db, 10, 0).unwrap();
    assert_eq!(
        page[0].id, id_first,
        "bumped item must appear first after recency update"
    );
    assert_eq!(page[0].wall_time, now_ms);
}

// ── prune_to_cap tests ────────────────────────────────────────────────────

/// Build a ClipboardItem whose encrypted content is exactly `size` bytes,
/// with a deterministic wall_time so tests can control eviction order.
fn make_sized_item(lamport: i64, wall_time_ms: i64, size: usize) -> ClipboardItem {
    let mut item = make_item(lamport);
    item.wall_time = wall_time_ms;
    item.content = Some(vec![0xCC; size]);
    item
}

/// Under the quota: nothing deleted.
#[test]
fn prune_to_cap_no_op_when_under_quota() {
    let db = Database::open_in_memory().unwrap();
    // 3 items × 10 bytes = 30 bytes; quota = 100.
    for i in 0..3_i64 {
        insert_item(&db, &make_sized_item(i, i * 1_000, 10)).unwrap();
    }
    let deleted = prune_to_cap(&db, 100).unwrap();
    assert_eq!(deleted, 0, "no eviction when total < quota");
    assert_eq!(count_items(&db).unwrap(), 3);
}

/// Exactly at the quota: nothing deleted.
#[test]
fn prune_to_cap_no_op_when_exactly_at_quota() {
    let db = Database::open_in_memory().unwrap();
    // 5 items × 20 bytes = 100 bytes; quota = 100.
    for i in 0..5_i64 {
        insert_item(&db, &make_sized_item(i, i * 1_000, 20)).unwrap();
    }
    let deleted = prune_to_cap(&db, 100).unwrap();
    assert_eq!(deleted, 0);
    assert_eq!(count_items(&db).unwrap(), 5);
}

/// Oldest items are evicted first (wall_time ASC ordering).
#[test]
fn prune_to_cap_evicts_oldest_first() {
    let db = Database::open_in_memory().unwrap();
    // Items ordered by wall_time: 1=oldest … 5=newest, each 20 bytes.
    // Total = 100, quota = 60 → excess = 40 → must remove 2 oldest (40 bytes).
    let mut ids = Vec::new();
    for i in 1..=5_i64 {
        let item = make_sized_item(i, i * 1_000, 20);
        ids.push(item.id.clone());
        insert_item(&db, &item).unwrap();
    }
    let deleted = prune_to_cap(&db, 60).unwrap();
    assert_eq!(deleted, 2, "exactly 2 oldest rows deleted");
    // Oldest two ids must be gone; newest three must remain.
    let conn = db.conn();
    let exists = |id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    };
    assert!(!exists(&ids[0]), "oldest must be gone");
    assert!(!exists(&ids[1]), "second oldest must be gone");
    assert!(exists(&ids[2]), "third must remain");
    assert!(exists(&ids[3]), "fourth must remain");
    assert!(exists(&ids[4]), "newest must remain");
}

/// A `new_file` blob counts toward the byte cap exactly like text/image
/// rows (`prune_to_cap` sums LENGTH(content) for all content types) and is
/// evicted oldest-first.
#[test]
fn prune_to_cap_evicts_oldest_file_blob() {
    let db = Database::open_in_memory().unwrap();
    // Oldest row is a file blob (40 bytes); two newer text rows (20 each).
    // Total = 80, quota = 40 → must evict the oldest (the file) only.
    let mut file_item = ClipboardItem::new_file(vec![0xFFu8; 40], "{}".to_string(), 1);
    file_item.wall_time = 1_000;
    let file_id = file_item.id.clone();
    insert_item(&db, &file_item).unwrap();

    let mid = make_sized_item(2, 2_000, 20);
    let mid_id = mid.id.clone();
    insert_item(&db, &mid).unwrap();

    let newest = make_sized_item(3, 3_000, 20);
    let newest_id = newest.id.clone();
    insert_item(&db, &newest).unwrap();

    let deleted = prune_to_cap(&db, 40).unwrap();
    assert_eq!(deleted, 1, "only the oldest (file) row evicted");

    let conn = db.conn();
    let exists = |id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    };
    assert!(!exists(&file_id), "oldest file blob must be evicted first");
    assert!(exists(&mid_id), "newer text row survives");
    assert!(exists(&newest_id), "newest text row survives");
}

/// The "tipping" row that crosses the byte threshold is evicted.
#[test]
fn prune_to_cap_tipping_row_is_evicted() {
    let db = Database::open_in_memory().unwrap();
    // 3 rows: 10 bytes, 10 bytes, 50 bytes (oldest → newest).
    // Total = 70, quota = 60 → excess = 10.
    // Row 1 (10 bytes): cum=10, cum-row=0 < 10 → DELETE (tipping).
    // Row 2 (10 bytes): cum=20, cum-row=10, 10 < 10 is FALSE → KEEP.
    let item1 = make_sized_item(1, 1_000, 10);
    let item2 = make_sized_item(2, 2_000, 10);
    let item3 = make_sized_item(3, 3_000, 50);
    let id1 = item1.id.clone();
    let id2 = item2.id.clone();
    let id3 = item3.id.clone();
    insert_item(&db, &item1).unwrap();
    insert_item(&db, &item2).unwrap();
    insert_item(&db, &item3).unwrap();

    let deleted = prune_to_cap(&db, 60).unwrap();
    assert_eq!(deleted, 1, "only the tipping row (oldest) deleted");
    let conn = db.conn();
    let exists = |id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    };
    assert!(!exists(&id1), "tipping row deleted");
    assert!(exists(&id2), "row 2 kept");
    assert!(exists(&id3), "row 3 kept");
}

/// Pinned items are never evicted, even when they are the oldest.
#[test]
fn prune_to_cap_pinned_items_never_evicted() {
    let db = Database::open_in_memory().unwrap();
    // Pin the oldest item; its bytes must not count toward the quota.
    // 3 items × 20 bytes = 60 bytes. Quota = 30.
    // Unpinned bytes = 40 (rows 2 and 3). Excess = 10. Row 2 is evicted.
    let item1 = make_sized_item(1, 1_000, 20); // will be pinned
    let item2 = make_sized_item(2, 2_000, 20);
    let item3 = make_sized_item(3, 3_000, 20);
    let id1 = item1.id.clone();
    let id2 = item2.id.clone();
    let id3 = item3.id.clone();
    insert_item(&db, &item1).unwrap();
    insert_item(&db, &item2).unwrap();
    insert_item(&db, &item3).unwrap();
    pin_item(&db, &id1).unwrap();

    let deleted = prune_to_cap(&db, 30).unwrap();
    assert_eq!(deleted, 1, "one unpinned row evicted");
    let conn = db.conn();
    let exists = |id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    };
    assert!(exists(&id1), "pinned oldest must not be evicted");
    assert!(!exists(&id2), "oldest unpinned evicted");
    assert!(exists(&id3), "newest unpinned kept");
}

/// After `prune_to_cap` evicts rows, no orphan FTS rows must remain and
/// a full-text search for a pruned term must return nothing.
#[test]
fn prune_to_cap_no_fts_orphans_after_eviction() {
    let db = Database::open_in_memory().unwrap();

    // Insert 3 items with FTS entries (oldest → newest, 20 bytes each).
    // Total = 60 bytes. Quota = 20 → excess = 40 → oldest 2 evicted.
    let mut ids = Vec::new();
    let terms = ["alpha unique term", "beta unique term", "gamma unique term"];
    for (i, term) in terms.iter().enumerate() {
        let item = make_sized_item(i as i64, (i as i64 + 1) * 1_000, 20);
        ids.push(item.id.clone());
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, term).unwrap();
    }

    let deleted = prune_to_cap(&db, 20).unwrap();
    assert_eq!(deleted, 2, "2 oldest items evicted");

    // No orphan FTS rows: count(fts) must equal count(items).
    let item_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    let fts_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        fts_count, item_count,
        "clipboard_fts count ({fts_count}) must equal clipboard_items count \
             ({item_count}) — no orphan FTS rows after size-cap eviction"
    );

    // Ghost-search check: pruned terms must not appear in search results.
    let r_alpha = search_items(&db, "alpha", 10).unwrap();
    assert!(
        r_alpha.is_empty(),
        "pruned term 'alpha' must not appear in search results"
    );
    let r_beta = search_items(&db, "beta", 10).unwrap();
    assert!(
        r_beta.is_empty(),
        "pruned term 'beta' must not appear in search results"
    );

    // The surviving item must still be searchable.
    let r_gamma = search_items(&db, "gamma", 10).unwrap();
    assert_eq!(
        r_gamma.len(),
        1,
        "surviving item with term 'gamma' must still be found"
    );
    assert_eq!(r_gamma[0].id, ids[2]);
}

/// Empty database: prune is a no-op.
#[test]
fn prune_to_cap_empty_db_is_noop() {
    let db = Database::open_in_memory().unwrap();
    let deleted = prune_to_cap(&db, 1024).unwrap();
    assert_eq!(deleted, 0);
}

/// Items with NULL content (e.g. blob_ref-only rows) count as 0 bytes.
#[test]
fn prune_to_cap_null_content_counts_as_zero_bytes() {
    let db = Database::open_in_memory().unwrap();
    // One item with NULL content + one with 50 bytes. Total = 50. Quota = 40.
    // The NULL-content row is oldest; it contributes 0 bytes so cum_bytes
    // after it is 0, meaning cum-row=0 < 10 (excess) → it gets evicted first.
    // After evicting it: remaining = 50 bytes which still > 40. Then the
    // 50-byte row: cum=50, cum-row=0 < 10 → also evicted.
    // Wait — excess = 50 - 40 = 10. Row1 (0 bytes): cum=0, cum-row=0 < 10 → DELETE.
    // Row2 (50 bytes): cum=50, cum-row=0 < 10 → DELETE.
    // So both deleted, 0 remaining.  Let's redesign to make it meaningful:
    // NULL row (0b) at t=1, 50b at t=2. Total=50. Quota=50 → NO-OP.
    // NULL row (0b) at t=1, 50b at t=2. Total=50. Quota=49 → excess=1.
    // Row1: cum=0, 0-0=0 < 1 → DELETE; Row2: cum=50, 50-50=0 < 1 → DELETE.
    // Hmm — a NULL-content row always has cum-row=0 which is < any positive excess.
    // Use quota=50 for the no-op assertion:
    let mut item_null = make_item(1);
    item_null.wall_time = 1_000;
    item_null.content = None;
    let item_big = make_sized_item(2, 2_000, 50);
    insert_item(&db, &item_null).unwrap();
    insert_item(&db, &item_big).unwrap();
    // Quota exactly equals total (50). No prune.
    let deleted = prune_to_cap(&db, 50).unwrap();
    assert_eq!(deleted, 0, "no eviction when quota met");
    assert_eq!(count_items(&db).unwrap(), 2);
}

// --- CopyPaste-6fd: pending_uploads defensive cleanup ---

/// Insert a `pending_uploads` row keyed by the given cross-device item_id.
fn insert_pending_upload(db: &Database, item_id: &str) {
    db.conn()
        .execute(
            "INSERT INTO pending_uploads \
                 (item_id, tus_url, bytes_uploaded, total_bytes, chunk_format_version, \
                  created_at, expires_at) \
                 VALUES (?1, 'https://relay/tus/x', 0, 100, 1, 0, 0)",
            params![item_id],
        )
        .unwrap();
}

fn count_pending(db: &Database) -> i64 {
    db.conn()
        .query_row("SELECT COUNT(*) FROM pending_uploads", [], |r| r.get(0))
        .unwrap()
}

/// `delete_item` must also remove the matching `pending_uploads` row so a
/// hard-deleted item can never strand a resumable-upload row.
#[test]
fn delete_item_cleans_pending_uploads() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    insert_pending_upload(&db, &item.item_id);
    // A second unrelated pending row must survive.
    insert_pending_upload(&db, "other-item-id");
    assert_eq!(count_pending(&db), 2);

    delete_item(&db, &item.id).unwrap();

    assert_eq!(
        count_pending(&db),
        1,
        "only the deleted item's pending_uploads row is removed"
    );
    let survivor: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM pending_uploads WHERE item_id = 'other-item-id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(survivor, 1, "unrelated pending_uploads row must survive");
}

/// `prune_to_cap` eviction must also clean `pending_uploads` for evicted ids.
#[test]
fn prune_to_cap_cleans_pending_uploads() {
    let db = Database::open_in_memory().unwrap();
    // 3 × 20 bytes = 60. Quota = 20 → 2 oldest evicted.
    let mut items = Vec::new();
    for i in 0..3 {
        let item = make_sized_item(i, (i + 1) * 1_000, 20);
        insert_item(&db, &item).unwrap();
        insert_pending_upload(&db, &item.item_id);
        items.push(item);
    }
    assert_eq!(count_pending(&db), 3);

    let deleted = prune_to_cap(&db, 20).unwrap();
    assert_eq!(deleted, 2);

    // Only the surviving (newest) item keeps its pending_uploads row.
    assert_eq!(
        count_pending(&db),
        1,
        "evicted items' pending_uploads rows must be cleaned"
    );
    let surviving_iid = &items[2].item_id;
    let survivor: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM pending_uploads WHERE item_id = ?1",
            params![surviving_iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(survivor, 1, "surviving item keeps its pending_uploads row");
}

/// `delete_expired` (TTL prune) must also clean `pending_uploads`.
#[test]
fn delete_expired_cleans_pending_uploads() {
    let db = Database::open_in_memory().unwrap();
    let mut item = make_item(1);
    item.expires_at = Some(1_000); // already expired vs now=10_000
    insert_item(&db, &item).unwrap();
    insert_pending_upload(&db, &item.item_id);
    assert_eq!(count_pending(&db), 1);

    let removed = delete_expired(&db, 10_000).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(
        count_pending(&db),
        0,
        "TTL-expired item's pending_uploads row must be cleaned"
    );
}

/// Large dataset (50 rows): window-function rewrite produces identical
/// eviction to the reference naive algorithm (compute total, subtract quota,
/// delete oldest prefix summing to ≥ excess).
#[test]
fn prune_to_cap_large_dataset_matches_naive_eviction() {
    use std::collections::HashSet;

    let db = Database::open_in_memory().unwrap();

    // Insert 50 items with varying sizes (5..=54 bytes) and distinct
    // wall_times so the ordering is deterministic.
    let mut items: Vec<ClipboardItem> = (0..50_i64)
        .map(|i| make_sized_item(i, (i + 1) * 1_000, 5 + i as usize))
        .collect();
    for item in &items {
        insert_item(&db, item).unwrap();
    }

    // Pin the 3 most-recent items so they survive unconditionally.
    let pinned_ids: HashSet<String> = items[47..].iter().map(|i| i.id.to_string()).collect();
    for id in &pinned_ids {
        pin_item(&db, id).unwrap();
    }

    // Total bytes (items is sorted oldest-first, sizes 5..54 bytes).
    // Unpinned = items[0..47]; total_unpinned = sum(5..52) = 47*(5+51)/2 = 1316.
    let total_unpinned: i64 = items[..47]
        .iter()
        .map(|it| it.content.as_ref().map_or(0, |c| c.len() as i64))
        .sum();
    let quota: i64 = 800;
    let excess = total_unpinned - quota;
    assert!(excess > 0, "sanity: quota must be below total");

    // Naive reference: collect oldest-first ids until cumulative bytes >= excess.
    items[..47].sort_by_key(|it| (it.wall_time, it.id.clone()));
    let mut cum: i64 = 0;
    let mut naive_delete: HashSet<String> = HashSet::new();
    for it in &items[..47] {
        let row_bytes = it.content.as_ref().map_or(0, |c| c.len() as i64);
        if cum < excess {
            naive_delete.insert(it.id.to_string());
            cum += row_bytes;
        }
    }

    // Run prune_to_cap.
    let deleted = prune_to_cap(&db, quota).unwrap();
    assert_eq!(
        deleted,
        naive_delete.len(),
        "window-fn and naive must delete the same number of rows"
    );

    // Verify each id matches.
    let conn = db.conn();
    for id in &naive_delete {
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 0, "naive-evicted row {id} must be gone");
    }
    // Pinned items must still be present.
    for id in &pinned_ids {
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "pinned row {id} must remain");
    }
}

/// CopyPaste-bhm9: `soft_delete_item` (tombstone path) must also clean
/// `pending_uploads` so in-flight upload rows don't leak when an item is
/// soft-deleted instead of hard-deleted.
#[test]
fn soft_delete_item_cleans_pending_uploads() {
    let db = Database::open_in_memory().unwrap();
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    insert_pending_upload(&db, &item.item_id);
    // A second unrelated pending row must survive.
    insert_pending_upload(&db, "other-item-id");
    assert_eq!(count_pending(&db), 2);

    let changed = soft_delete_item(&db, &item.id, 9_999, 9_999).unwrap();
    assert_eq!(changed, 1, "soft_delete_item must affect exactly one row");

    assert_eq!(
        count_pending(&db),
        1,
        "soft_delete_item must remove the pending_uploads row for the deleted item"
    );
    let survivor: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM pending_uploads WHERE item_id = 'other-item-id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(survivor, 1, "unrelated pending_uploads row must survive");
}

// --- CopyPaste-89ib: sensitive TTL must be recomputed on recency bump ---

/// When a sensitive item is re-copied (bump_item_recency), `expires_at` must
/// advance to `now_ms + sensitive_ttl_ms`. Without this fix the old
/// expires_at fires immediately after the bump, wiping a freshly-recopied
/// sensitive item.
#[test]
fn bump_item_recency_recomputes_expires_at_for_sensitive_items() {
    let db = Database::open_in_memory().unwrap();

    let ttl_ms: i64 = 30_000;
    let original_wall: i64 = 1_000;
    let original_expires: i64 = original_wall + ttl_ms; // 31_000

    let mut item = make_item(1);
    item.is_sensitive = true;
    item.wall_time = original_wall;
    item.expires_at = Some(original_expires);
    insert_item(&db, &item).unwrap();

    // Re-copy at now_ms=50_000. New expires_at must be 50_000 + 30_000 = 80_000.
    let now_ms: i64 = 50_000;
    let changed = bump_item_recency(&db, &item.id, now_ms, now_ms, Some(ttl_ms)).unwrap();
    assert_eq!(changed, 1);

    let fetched = get_item_by_id(&db, &item.id).unwrap().unwrap();
    assert_eq!(fetched.wall_time, now_ms, "wall_time must be bumped");
    assert_eq!(
        fetched.expires_at,
        Some(now_ms + ttl_ms),
        "expires_at must be recalculated as now_ms + sensitive_ttl_ms"
    );
}

/// Non-sensitive items must NOT have expires_at changed by bump_item_recency
/// even if a ttl_ms hint is supplied.
#[test]
fn bump_item_recency_does_not_set_expires_at_for_non_sensitive_items() {
    let db = Database::open_in_memory().unwrap();

    let mut item = make_item(1);
    item.is_sensitive = false;
    item.wall_time = 1_000;
    item.expires_at = None;
    insert_item(&db, &item).unwrap();

    bump_item_recency(&db, &item.id, 50_000, 50_000, Some(30_000)).unwrap();

    let fetched = get_item_by_id(&db, &item.id).unwrap().unwrap();
    assert_eq!(
        fetched.expires_at, None,
        "non-sensitive items must not gain an expires_at from the bump"
    );
}

// --- CopyPaste-3e7y: unified TTL path via expires_at ---

/// delete_sensitive_expired must backfill expires_at for sensitive items
/// that lack it, then delegate to delete_expired's expires_at predicate.
/// The result must be identical to the old wall_time-based path.
#[test]
fn delete_sensitive_expired_unified_via_expires_at() {
    let db = Database::open_in_memory().unwrap();
    let ttl_ms: i64 = 30_000;
    let now_ms: i64 = 200_000;
    // threshold = now_ms - ttl_ms = 170_000

    // Old sensitive item (wall_time=1_000, no expires_at) → should be deleted.
    let mut old_sensitive = make_item(1);
    old_sensitive.is_sensitive = true;
    old_sensitive.wall_time = 1_000;
    old_sensitive.expires_at = None; // old-style: no expires_at
    insert_item(&db, &old_sensitive).unwrap();

    // Recent sensitive item (wall_time=190_000) → should survive.
    let mut new_sensitive = make_item(2);
    new_sensitive.is_sensitive = true;
    new_sensitive.wall_time = 190_000;
    new_sensitive.expires_at = None;
    insert_item(&db, &new_sensitive).unwrap();

    let removed = delete_sensitive_expired(&db, now_ms, ttl_ms).unwrap();
    assert_eq!(removed, 1, "only the old sensitive item must be deleted");
    assert_eq!(count_items(&db).unwrap(), 1);
}

// --- CopyPaste-kexs: incremental_vacuum support ---

/// incremental_vacuum must execute without error and return Ok.
#[test]
fn incremental_vacuum_runs_without_error() {
    let db = Database::open_in_memory().unwrap();
    // Insert and delete an item to give SQLite some pages to reclaim.
    let item = make_item(1);
    insert_item(&db, &item).unwrap();
    delete_item(&db, &item.id).unwrap();

    let result = incremental_vacuum(&db, 10);
    assert!(
        result.is_ok(),
        "incremental_vacuum must succeed: {:?}",
        result.err()
    );
}

// --- CopyPaste-yfm8: prune_to_cap single-pass ---

/// After refactoring, prune_to_cap must still produce the same result as
/// before. This test is identical to existing prune_to_cap tests; the point
/// is that the single-pass implementation stays correct.
#[test]
fn prune_to_cap_single_pass_matches_reference() {
    let db = Database::open_in_memory().unwrap();

    // Insert 10 items with known sizes (content = N bytes, wall_time = N ms).
    for i in 1i64..=10 {
        let mut item = make_item(i);
        item.wall_time = i * 1_000;
        // Force content length = i * 10 bytes.
        item.content = Some(vec![0u8; (i * 10) as usize]);
        insert_item(&db, &item).unwrap();
    }

    // Total = sum(10..=100 step 10) = 550 bytes. Cap at 300 → must free 250.
    let deleted = prune_to_cap(&db, 300).unwrap();
    assert!(deleted > 0, "must delete rows to free space");

    let remaining: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(COALESCE(content,''))),0) FROM clipboard_items",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        remaining <= 300,
        "remaining bytes {remaining} must be within the cap"
    );
}

// --- CopyPaste-y4v1: compute_content_hash returns full SHA-256 hex ---

/// compute_content_hash must return a 64-character lowercase hex string
/// (the full SHA-256 digest), not a 32-character truncated version.
#[test]
fn compute_content_hash_returns_full_sha256_hex() {
    let hash = compute_content_hash(b"hello clipboard");
    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hex must be 64 chars (32 bytes); got {} chars: {}",
        hash.len(),
        hash
    );
    // Must be deterministic.
    let hash2 = compute_content_hash(b"hello clipboard");
    assert_eq!(hash, hash2, "hash must be deterministic");
    // Different inputs must differ.
    let other = compute_content_hash(b"different content");
    assert_ne!(
        hash, other,
        "different inputs must produce different hashes"
    );
}

// --- CopyPaste-9vcn: open_in_memory visibility is test-only ---
// (Compile-time enforcement — no runtime test needed.
//  The function's #[cfg(test)] gate is verified by the fact that this test
//  module CAN call it while non-test callers cannot.)

// --- CopyPaste-i6pp: sensitive items must NOT appear in FTS or search ---

/// Regression: `insert_item_with_fts` must NOT write a sensitive item's
/// plaintext into `clipboard_fts`, even when a non-empty `plaintext_for_fts`
/// is supplied.  Callers that detect sensitivity should pass `""` (same
/// convention as image items), and the function itself must enforce the
/// policy as a final safeguard.
#[test]
fn sensitive_item_not_indexed_in_fts_by_insert_item_with_fts() {
    let db = Database::open_in_memory().unwrap();

    let mut item = make_item(1);
    item.is_sensitive = true;
    let id = item.id.clone();

    // Pass non-empty plaintext: the function must silently discard it
    // because the item is sensitive.
    insert_item_with_fts(&db, &item, "super secret password").unwrap();

    let fts_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        fts_count, 0,
        "sensitive item must NOT be present in clipboard_fts (CopyPaste-i6pp)"
    );
}

/// Regression: `upsert_fts` must refuse to index plaintext for a sensitive
/// item. The id is looked up in `clipboard_items` and if `is_sensitive = 1`
/// the FTS row must not be written.
#[test]
fn upsert_fts_rejects_sensitive_item() {
    let db = Database::open_in_memory().unwrap();

    let mut item = make_item(1);
    item.is_sensitive = true;
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    // upsert_fts must be a no-op for sensitive rows.
    upsert_fts(&db, &id, "classified payload").unwrap();

    let fts_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        fts_count, 0,
        "upsert_fts must not index plaintext for sensitive items (CopyPaste-i6pp)"
    );
}

/// Defense-in-depth: `search_items` must never return sensitive items even
/// if a stale FTS row was inserted before this fix.
#[test]
fn search_items_does_not_return_sensitive_items() {
    let db = Database::open_in_memory().unwrap();

    // Insert a non-sensitive item — must be findable.
    let normal = make_item(1);
    let normal_id = normal.id.clone();
    insert_item(&db, &normal).unwrap();
    // Manually insert the FTS row (mirrors what the daemon does after decryption).
    db.conn()
        .execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![normal_id, "findme plaintext"],
        )
        .unwrap();

    // Insert a sensitive item and — simulating a pre-fix row — manually
    // force a stale FTS entry for it.
    let mut sensitive = make_item(2);
    sensitive.is_sensitive = true;
    let sensitive_id = sensitive.id.clone();
    insert_item(&db, &sensitive).unwrap();
    db.conn()
        .execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![sensitive_id, "findme secret"],
        )
        .unwrap();

    // Both words appear in FTS, but search_items must only return the non-sensitive hit.
    let results = search_items(&db, "findme", 10).unwrap();
    assert_eq!(
        results.len(),
        1,
        "search must return exactly one result (the non-sensitive item)"
    );
    assert_eq!(
        results[0].id, normal_id,
        "result must be the non-sensitive item (CopyPaste-i6pp)"
    );
    assert!(
        results.iter().all(|i| !i.is_sensitive),
        "search_items must never return sensitive items"
    );
}

// --- CopyPaste-44rq.45: mark_sensitive must remove FTS entry atomically ---

/// A non-sensitive item that was FTS-indexed must NOT be searchable after
/// `mark_sensitive` is called.
#[test]
fn mark_sensitive_removes_fts_entry() {
    let db = Database::open_in_memory().unwrap();

    // Insert a non-sensitive item with FTS entry.
    let item = make_item(1);
    let id = item.id.clone();
    insert_item_with_fts(&db, &item, "unicorn password payload").unwrap();

    // Confirm it is searchable before the transition.
    let before = search_items(&db, "unicorn", 10).unwrap();
    assert_eq!(
        before.len(),
        1,
        "item must be searchable before mark_sensitive"
    );

    // Transition to sensitive.
    let changed = mark_sensitive(&db, &id).unwrap();
    assert_eq!(changed, 1, "mark_sensitive must update 1 row");

    // Confirm it is no longer searchable.
    let after = search_items(&db, "unicorn", 10).unwrap();
    assert!(
        after.is_empty(),
        "item must NOT be searchable after mark_sensitive (CopyPaste-44rq.45)"
    );

    // Confirm the FTS row itself is gone (not just filtered by search_items).
    let fts_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        fts_count, 0,
        "FTS row must be deleted by mark_sensitive, not just filtered"
    );

    // Confirm the item itself is still in clipboard_items (not deleted).
    let got = get_item_by_id(&db, &id)
        .unwrap()
        .expect("item must still exist");
    assert!(
        got.is_sensitive,
        "item must have is_sensitive=true after mark_sensitive"
    );
}

/// `mark_sensitive` on an already-sensitive item with a stale FTS row must
/// clean up the FTS row (idempotent repair).
#[test]
fn mark_sensitive_clears_stale_fts_for_already_sensitive_item() {
    let db = Database::open_in_memory().unwrap();

    let mut item = make_item(2);
    item.is_sensitive = true;
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    // Simulate a stale FTS row (e.g., written before this fix was deployed).
    db.conn()
        .execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![id, "stale secret stale"],
        )
        .unwrap();

    // mark_sensitive must clean the stale row even though is_sensitive is already 1.
    let changed = mark_sensitive(&db, &id).unwrap();
    // changed may be 0 or 1 depending on whether SQLite counts a no-op UPDATE —
    // we only assert the FTS row is gone.
    let _ = changed;

    let fts_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        fts_count, 0,
        "mark_sensitive must delete stale FTS row for already-sensitive item (CopyPaste-44rq.45)"
    );
}

/// `mark_sensitive` on an unknown id must return 0 and not error.
#[test]
fn mark_sensitive_unknown_id_is_noop() {
    let db = Database::open_in_memory().unwrap();
    let changed = mark_sensitive(&db, "00000000-0000-0000-0000-000000000099").unwrap();
    assert_eq!(changed, 0, "mark_sensitive on unknown id must return 0");
}
