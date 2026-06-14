//! Tests for the poison-row guard introduced in CopyPaste-jww / CopyPaste-5y4.
//!
//! A "poison row" is a `clipboard_items` row where:
//!   * `content_type = 'text'`  AND `content IS NOT NULL` AND `content_nonce IS NULL`
//!   * OR `content_type IN ('file','image')` AND `content IS NOT NULL`
//!     AND `content_nonce IS NULL` AND `blob_ref IS NULL`
//!
//! Such rows are created when `rekey_inbound` fails (sync key missing / wrong)
//! and the code falls back to storing the wire item verbatim.  The wire item
//! for a sync-key-wrapped item has `content` (the wrapped blob) but no
//! `content_nonce` (stripped by the sender as the "wrapped" marker) and no
//! `blob_ref` (also stripped for file/image items).  Consumers reject these
//! rows with "missing content_nonce" (text) or "missing blob_ref metadata"
//! (file/image).

use copypaste_core::Database;
use copypaste_daemon::sync_orch::{is_poison_wire, sweep_poison_rows};
use copypaste_sync::protocol::WireItem;

// ── WireItem helpers ──────────────────────────────────────────────────────────

fn base_wire(content_type: &str) -> WireItem {
    WireItem {
        id: uuid::Uuid::new_v4().to_string(),
        item_id: uuid::Uuid::new_v4().to_string(),
        content_type: content_type.to_string(),
        content: Some(vec![0xCA, 0xFE]),
        content_nonce: None,
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 1,
        wall_time: 1_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "test-device".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
        deleted: false,
        pinned: false,
        pin_order: None,
    }
}

// ── Unit: is_poison_wire ──────────────────────────────────────────────────────

#[test]
fn is_poison_wire_text_with_no_nonce_is_poison() {
    // text + content present + no nonce → poison
    let w = base_wire("text");
    assert!(
        is_poison_wire(&w),
        "text with content but no content_nonce must be poison"
    );
}

#[test]
fn is_poison_wire_text_with_nonce_is_clean() {
    // text + content + nonce → clean
    let mut w = base_wire("text");
    w.content_nonce = Some(vec![0u8; 24]);
    assert!(
        !is_poison_wire(&w),
        "text with content AND content_nonce must NOT be poison"
    );
}

#[test]
fn is_poison_wire_file_no_blob_ref_is_poison() {
    // file + content + no nonce + no blob_ref → poison
    let w = base_wire("file");
    assert!(
        is_poison_wire(&w),
        "file with content but no content_nonce and no blob_ref must be poison"
    );
}

#[test]
fn is_poison_wire_file_with_blob_ref_is_clean() {
    // file + content + no nonce + blob_ref present → clean (normal large-file path)
    let mut w = base_wire("file");
    w.blob_ref = Some("bucket/path/to/blob".to_string());
    assert!(
        !is_poison_wire(&w),
        "file with blob_ref must NOT be poison even without content_nonce"
    );
}

// ── Helpers for DB-level tests ────────────────────────────────────────────────

fn open_db() -> Database {
    Database::open_in_memory().expect("in-memory DB must open")
}

/// Insert a poison text row directly via raw SQL (the normal insert API
/// would not produce a row missing `content_nonce`).
fn insert_poison_text(db: &Database, id: &str) {
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, \
              origin_device_id, key_version) \
             VALUES (?1, ?2, 'text', X'CAFE', NULL, NULL, 0, 0, 1, 1000000, 'peer', 2)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert poison text row");
}

/// Insert a poison file row: content present, content_nonce NULL, blob_ref NULL.
fn insert_poison_file(db: &Database, id: &str) {
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, \
              origin_device_id, key_version) \
             VALUES (?1, ?2, 'file', X'CAFE', NULL, NULL, 0, 0, 1, 1000000, 'peer', 2)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert poison file row");
}

/// Insert a healthy text row (content_nonce IS NOT NULL).
fn insert_healthy_text(db: &Database, id: &str) {
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, \
              origin_device_id, key_version) \
             VALUES (?1, ?2, 'text', X'CAFE', X'000000000000000000000000000000000000000000000000', \
                     NULL, 0, 0, 1, 1000000, 'local', 2)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert healthy text row");
}

/// Insert a healthy file row (blob_ref IS NOT NULL).
fn insert_healthy_file(db: &Database, id: &str) {
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, \
              origin_device_id, key_version) \
             VALUES (?1, ?2, 'file', X'CAFE', NULL, 'bucket/some/path', \
                     0, 0, 1, 1000000, 'local', 2)",
            rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert healthy file row");
}

fn row_count(db: &Database) -> i64 {
    db.conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| {
            r.get::<_, i64>(0)
        })
        .expect("count rows")
}

fn row_exists(db: &Database, id: &str) -> bool {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .expect("check row exists")
        > 0
}

// ── Integration: sweep_poison_rows ────────────────────────────────────────────

#[test]
fn sweep_removes_text_poison_row() {
    let db = open_db();
    insert_poison_text(&db, "poison-text-1");
    assert!(row_exists(&db, "poison-text-1"), "row must exist before sweep");

    let swept = sweep_poison_rows(&db).expect("sweep must succeed");

    assert_eq!(swept, 1, "sweep must delete exactly 1 row");
    assert!(
        !row_exists(&db, "poison-text-1"),
        "poison row must be deleted after sweep"
    );
}

#[test]
fn sweep_removes_file_poison_row() {
    let db = open_db();
    insert_poison_file(&db, "poison-file-1");
    assert!(row_exists(&db, "poison-file-1"), "row must exist before sweep");

    let swept = sweep_poison_rows(&db).expect("sweep must succeed");

    assert_eq!(swept, 1, "sweep must delete exactly 1 row");
    assert!(
        !row_exists(&db, "poison-file-1"),
        "poison file row must be deleted after sweep"
    );
}

#[test]
fn sweep_preserves_healthy_text_row() {
    let db = open_db();
    insert_healthy_text(&db, "healthy-text-1");

    let swept = sweep_poison_rows(&db).expect("sweep must succeed");

    assert_eq!(swept, 0, "healthy row must not be swept");
    assert!(
        row_exists(&db, "healthy-text-1"),
        "healthy text row must survive sweep"
    );
}

#[test]
fn sweep_preserves_healthy_file_row() {
    let db = open_db();
    insert_healthy_file(&db, "healthy-file-1");

    let swept = sweep_poison_rows(&db).expect("sweep must succeed");

    assert_eq!(swept, 0, "healthy row must not be swept");
    assert!(
        row_exists(&db, "healthy-file-1"),
        "healthy file row must survive sweep"
    );
}

#[test]
fn sweep_returns_correct_count() {
    let db = open_db();
    insert_poison_text(&db, "p-txt");
    insert_poison_file(&db, "p-file");
    insert_healthy_text(&db, "h-txt");
    assert_eq!(row_count(&db), 3, "setup: 3 rows");

    let swept = sweep_poison_rows(&db).expect("sweep must succeed");

    assert_eq!(swept, 2, "must sweep exactly the 2 poison rows");
    assert_eq!(row_count(&db), 1, "1 healthy row must remain");
    assert!(
        row_exists(&db, "h-txt"),
        "healthy row must survive"
    );
}
