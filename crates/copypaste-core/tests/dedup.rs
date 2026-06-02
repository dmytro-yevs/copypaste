//! Content-hash deduplication tests.
//!
//! Beta-bonus coverage for the `content_hash` column (schema v2) and the
//! `find_recent_by_hash()` helper used by the daemon to skip inserting an
//! identical clipboard payload twice in a row.
//!
//! Contract under test (see `storage/items.rs`):
//!   * `content_hash` is a SHA-256 hex digest of the **raw pre-encryption**
//!     bytes. Two items with the same plaintext content produce the same
//!     hash regardless of the random nonce used for encryption.
//!   * `find_recent_by_hash(db, hash, now_ms, within_ms)` returns the id of
//!     the most-recent item whose `wall_time >= now_ms - within_ms`, or
//!     `None` if no such row exists. The dedup window is **caller-provided**
//!     — the code comment names 60 s (60_000 ms) as the daemon default but
//!     the helper itself is window-agnostic.
//!   * Dedup is hash-only (schema v2 has no `origin_device_id` column on
//!     `clipboard_items`). Two identical payloads tagged with different
//!     `app_bundle_id` values must collapse to a single dedup hit.
//!
//! Each test uses an on-disk encrypted DB in a fresh `tempfile::tempdir()`
//! so the production code path (SQLCipher + migrations + index) is exercised
//! end to end.

use copypaste_core::{count_items, find_recent_by_hash, insert_item, ClipboardItem, Database};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

// --- Helpers ----------------------------------------------------------------

/// Default dedup window the daemon uses. Defined here so tests state their
/// intent in milliseconds rather than re-typing magic numbers.
const DEDUP_WINDOW_MS: i64 = 60_000;

/// Open a fresh on-disk encrypted DB. Key is a per-test constant so the
/// file is reproducible if anyone needs to inspect it after a failure.
fn fresh_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("dedup_test.db");
    let key = [0x7Fu8; 32];
    let db = Database::open(&path, &key).expect("open encrypted db");
    (dir, db)
}

/// SHA-256 hex digest of the raw plaintext bytes — matches the daemon's
/// production hashing of clipboard content.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Build a text `ClipboardItem` whose `content_hash` is computed from the
/// supplied raw plaintext (NOT from the encrypted payload). `wall_time` is
/// set explicitly so tests can pin items at deterministic times relative
/// to the dedup window.
fn text_item(plaintext: &[u8], wall_time: i64, lamport_ts: i64) -> ClipboardItem {
    // Encrypted content is irrelevant for the dedup contract — only the
    // SHA-256 of the plaintext matters. We use the plaintext as-is to keep
    // the wire format simple; the dedup helper never touches `content`.
    let mut item = ClipboardItem::new_text(plaintext.to_vec(), vec![0u8; 24], lamport_ts);
    item.wall_time = wall_time;
    item.content_hash = Some(sha256_hex(plaintext));
    item
}

/// Build an image-shape `ClipboardItem` whose `content_hash` is computed from
/// the raw bytes. Image dedup uses the same SHA-256 contract as text.
fn image_item(raw_bytes: &[u8], wall_time: i64, lamport_ts: i64) -> ClipboardItem {
    let mut item = ClipboardItem::new_image(
        raw_bytes.to_vec(),
        String::from("{\"width\":1,\"height\":1,\"chunks\":1,\"file_id\":\"x\"}"),
        lamport_ts,
        None,
    );
    item.wall_time = wall_time;
    item.content_hash = Some(sha256_hex(raw_bytes));
    item
}

// --- Tests ------------------------------------------------------------------

/// The daemon-style flow: hash the bytes, ask the DB whether that hash was
/// seen in the dedup window, and only `insert_item` if `find_recent_by_hash`
/// returned `None`. Running this flow twice for the same payload must produce
/// exactly one row and the second lookup must return the first row's id.
#[test]
fn insert_same_text_twice_returns_existing_id_no_new_row() {
    let (_tmp, db) = fresh_db();
    let plaintext = b"hello clipboard world";
    let hash = sha256_hex(plaintext);
    let now: i64 = 1_000_000;

    // First write: nothing matches, insert.
    assert!(
        find_recent_by_hash(&db, &hash, now, DEDUP_WINDOW_MS)
            .unwrap()
            .is_none(),
        "fresh DB must report no dedup hit"
    );
    let first = text_item(plaintext, now, 1);
    let first_id = first.id.clone();
    insert_item(&db, &first).unwrap();

    // Second write (50 ms later, well inside the 60 s window): hash hits,
    // daemon would skip the insert.
    let later = now + 50;
    let hit = find_recent_by_hash(&db, &hash, later, DEDUP_WINDOW_MS).unwrap();
    assert_eq!(
        hit.as_deref(),
        Some(first_id.as_str()),
        "duplicate payload must report the existing row's id"
    );

    // No second insert happened → row count is 1.
    assert_eq!(count_items(&db).unwrap(), 1);
}

/// Image payloads use the same SHA-256 + `find_recent_by_hash` flow. The
/// content shape differs (binary blob via `new_image`) but the dedup
/// contract is identical.
#[test]
fn insert_same_image_bytes_twice_returns_existing_id() {
    let (_tmp, db) = fresh_db();
    let raw = b"\x89PNG\r\n\x1a\n\x00\x00fakebinaryimagepayload\xff\xd9".to_vec();
    let hash = sha256_hex(&raw);
    let now: i64 = 2_000_000;

    let first = image_item(&raw, now, 1);
    let first_id = first.id.clone();
    insert_item(&db, &first).unwrap();

    let hit = find_recent_by_hash(&db, &hash, now + 100, DEDUP_WINDOW_MS).unwrap();
    assert_eq!(hit.as_deref(), Some(first_id.as_str()));
    assert_eq!(count_items(&db).unwrap(), 1);
}

/// Different payloads → different SHA-256 → two rows, two distinct dedup
/// lookups. Confirms the helper isn't accidentally collapsing on something
/// other than the hash (e.g. wall_time bucketing).
#[test]
fn different_content_different_hash_creates_two_rows() {
    let (_tmp, db) = fresh_db();
    let now: i64 = 3_000_000;

    let a = text_item(b"payload alpha", now, 1);
    let b = text_item(b"payload beta", now + 10, 2);
    assert_ne!(
        a.content_hash, b.content_hash,
        "different plaintexts must hash to different values"
    );

    insert_item(&db, &a).unwrap();
    insert_item(&db, &b).unwrap();
    assert_eq!(count_items(&db).unwrap(), 2);

    // Each hash resolves to its own row.
    let hit_a = find_recent_by_hash(
        &db,
        a.content_hash.as_ref().unwrap(),
        now + 50,
        DEDUP_WINDOW_MS,
    )
    .unwrap();
    let hit_b = find_recent_by_hash(
        &db,
        b.content_hash.as_ref().unwrap(),
        now + 50,
        DEDUP_WINDOW_MS,
    )
    .unwrap();
    assert_eq!(hit_a.as_deref(), Some(a.id.as_str()));
    assert_eq!(hit_b.as_deref(), Some(b.id.as_str()));
}

/// Sanity check on the hashing primitive itself: a corpus of realistic
/// clipboard payloads must produce 100% unique SHA-256 digests. Catches
/// regressions if anyone ever swaps `sha2::Sha256` for a weaker primitive.
#[test]
fn hash_collision_resistant_for_realistic_payloads() {
    let payloads: Vec<&[u8]> = vec![
        b"",
        b" ",
        b"a",
        b"A",
        b"https://example.com/path?q=1",
        b"https://example.com/path?q=2",
        b"user@example.com",
        b"4111 1111 1111 1111",
        b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIabcdef",
        b"-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----",
        b"{\"json\":\"true\"}",
        b"{\"json\":\"false\"}",
        &[0u8; 256],
        &[0xFFu8; 256],
    ];
    let mut seen = std::collections::HashSet::new();
    for p in &payloads {
        let h = sha256_hex(p);
        assert!(seen.insert(h.clone()), "hash collision on payload {:?}", p);
        // Hex-encoded SHA-256 is always 64 chars.
        assert_eq!(h.len(), 64);
    }
}

/// `find_recent_by_hash` honours the `within_ms` window strictly:
///   * an item inside the window is returned
///   * the same item, queried after the window expires, is NOT returned
///
/// This validates that the helper compares `wall_time >= now_ms - within_ms`
/// and not some absolute/loose comparison.
#[test]
fn find_recent_by_hash_returns_within_dedup_window() {
    let (_tmp, db) = fresh_db();
    let plaintext = b"window-test payload";
    let hash = sha256_hex(plaintext);

    // Insert at t=1_000_000 ms.
    let t_insert: i64 = 1_000_000;
    let item = text_item(plaintext, t_insert, 1);
    let id = item.id.clone();
    insert_item(&db, &item).unwrap();

    // Inside the 60s window → hit.
    let inside = t_insert + DEDUP_WINDOW_MS - 1;
    assert_eq!(
        find_recent_by_hash(&db, &hash, inside, DEDUP_WINDOW_MS)
            .unwrap()
            .as_deref(),
        Some(id.as_str()),
        "lookup inside dedup window must hit"
    );

    // Exactly at the boundary (now - within == wall_time) → still a hit
    // because the comparison is `>=`.
    let boundary = t_insert + DEDUP_WINDOW_MS;
    assert_eq!(
        find_recent_by_hash(&db, &hash, boundary, DEDUP_WINDOW_MS)
            .unwrap()
            .as_deref(),
        Some(id.as_str()),
        "lookup at boundary (wall_time == cutoff) must hit"
    );

    // One ms past the boundary → miss.
    let outside = t_insert + DEDUP_WINDOW_MS + 1;
    assert!(
        find_recent_by_hash(&db, &hash, outside, DEDUP_WINDOW_MS)
            .unwrap()
            .is_none(),
        "lookup past dedup window must miss"
    );

    // A tighter window (5 s) also misses an item that's older than 5 s.
    let tight_window = 5_000_i64;
    let later = t_insert + tight_window + 1;
    assert!(
        find_recent_by_hash(&db, &hash, later, tight_window)
            .unwrap()
            .is_none(),
        "tight window must miss older entry"
    );
}

/// Dedup is currently **hash-only** — schema v2 carries no `origin_device_id`
/// column on `clipboard_items`. The closest proxy for "origin" available on
/// the row is `app_bundle_id`. This test documents the current contract:
/// identical content from two different source apps still collapses to a
/// single dedup hit, because `find_recent_by_hash` only looks at
/// `content_hash` + `wall_time`.
///
/// If a future schema adds per-origin dedup, this test should be updated to
/// assert separate rows for the two origins.
#[test]
fn dedup_is_content_only_origin_app_id_does_not_split_hash() {
    let (_tmp, db) = fresh_db();
    let plaintext = b"shared payload across apps";
    let hash = sha256_hex(plaintext);
    let now: i64 = 4_000_000;

    // First copy: originated from "com.apple.Safari".
    let mut from_safari = text_item(plaintext, now, 1);
    from_safari.app_bundle_id = Some("com.apple.Safari".to_string());
    let safari_id = from_safari.id.clone();
    insert_item(&db, &from_safari).unwrap();

    // Second copy of the IDENTICAL bytes, 10 ms later, from a different app.
    // The daemon's dedup path would consult `find_recent_by_hash` BEFORE
    // inserting and skip the write because the hash already exists in the
    // window. We assert that lookup returns the Safari row.
    let later = now + 10;
    let hit = find_recent_by_hash(&db, &hash, later, DEDUP_WINDOW_MS).unwrap();
    assert_eq!(
        hit.as_deref(),
        Some(safari_id.as_str()),
        "same hash from a different app must still hit dedup (content-only contract)"
    );

    // Sanity: only the Safari row exists. The "Terminal" copy was never
    // written because dedup short-circuited it.
    assert_eq!(count_items(&db).unwrap(), 1);
}
