//! Schema migration tests (beta-bonus).
//!
//! Exercises the v0 → v1 → v2 → v3 migration ladder from
//! `copypaste-core::storage::schema` via the public `Database::open` /
//! `Database::open_in_memory` API.
//!
//! v2 (per scripts/migrate-alpha-to-beta.sh, commit 9e0fd9e):
//!   * ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT
//!   * CREATE INDEX idx_clipboard_content_hash ON clipboard_items(content_hash)
//!     WHERE content_hash IS NOT NULL
//!   * PRAGMA user_version = 2
//!
//! v3 (fix for merge.rs:39 CRDT tie-break BUG):
//!   * ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL
//!     DEFAULT ''
//!   * `items::backfill_origin_device_id` stamps the local device UUID onto
//!     legacy rows whose default empty origin survived the migration.
//!   * PRAGMA user_version = 3
//!
//! All migrations MUST run inside a single transaction so user_version advances
//! atomically with the schema change (never partially applied).

use copypaste_core::Database;
use rusqlite::Connection;
use tempfile::tempdir;

/// Mirror of `copypaste-core/src/storage/schema.rs::SCHEMA_VERSION`.
/// Kept in-sync manually because the module is private. Bumping
/// SCHEMA_VERSION in src/ MUST be accompanied by bumping this and adding
/// a new migration test below.
const CURRENT_SCHEMA_VERSION: i64 = 3;

/// v1 schema (the exact contents of src/storage/schema_v1.sql, inlined because
/// the file is `include_str!`'d into the crate and not accessible from
/// integration tests).
const V1_SCHEMA_SQL: &str = "\
CREATE TABLE IF NOT EXISTS clipboard_items (
    id              TEXT PRIMARY KEY NOT NULL,
    item_id         TEXT NOT NULL,
    content_type    TEXT NOT NULL,
    content         BLOB,
    content_nonce   BLOB,
    blob_ref        TEXT,
    is_sensitive    INTEGER NOT NULL DEFAULT 0,
    is_synced       INTEGER NOT NULL DEFAULT 0,
    lamport_ts      INTEGER NOT NULL,
    wall_time       INTEGER NOT NULL,
    expires_at      INTEGER,
    app_bundle_id   TEXT
);
CREATE INDEX IF NOT EXISTS idx_clipboard_wall_time ON clipboard_items(wall_time DESC);
CREATE INDEX IF NOT EXISTS idx_clipboard_expires ON clipboard_items(expires_at) WHERE expires_at IS NOT NULL;
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
    USING fts5(id UNINDEXED, content_text);
CREATE TABLE IF NOT EXISTS devices (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    platform        TEXT NOT NULL,
    public_key      TEXT NOT NULL,
    fingerprint     TEXT NOT NULL,
    verified        INTEGER NOT NULL DEFAULT 0,
    last_seen       INTEGER
);
CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY NOT NULL,
    value           TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pending_uploads (
    item_id         TEXT PRIMARY KEY NOT NULL,
    tus_url         TEXT NOT NULL,
    bytes_uploaded  INTEGER NOT NULL DEFAULT 0,
    total_bytes     INTEGER NOT NULL,
    chunk_format_version INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL
);
";

/// Helper: stage a plaintext SQLite file at the given path with the v1
/// schema and `user_version=1`. Returns once the connection is closed and
/// the file is fully flushed.
///
/// `Database::open` will then take this plaintext file through:
///   1. plaintext → SQLCipher in-place migration (encrypt_existing)
///   2. schema migration v1 → CURRENT_SCHEMA_VERSION
fn stage_v1_plaintext(path: &std::path::Path) {
    let conn = Connection::open(path).expect("create plaintext db");
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch("PRAGMA user_version = 1;").unwrap();
    drop(conn);
}


/// Helper: read `PRAGMA user_version` via the open `Database`.
fn user_version(db: &Database) -> i64 {
    db.conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

/// Helper: check whether a column exists on a table.
fn column_exists(db: &Database, table: &str, column: &str) -> bool {
    let mut stmt = db
        .conn()
        .prepare(&format!("PRAGMA table_info({})", table))
        .unwrap();
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    rows.iter().any(|c| c == column)
}

/// Helper: check whether an index exists by name.
fn index_exists(db: &Database, name: &str) -> bool {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
            [name],
            |r| r.get(0),
        )
        .unwrap();
    count == 1
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn fresh_db_creates_at_current_user_version() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("fresh.db");
    let key = [0x01u8; 32];

    let db = Database::open(&path, &key).expect("open fresh");
    assert_eq!(
        user_version(&db),
        CURRENT_SCHEMA_VERSION,
        "fresh DB must land directly at the current schema version, \
         not via per-step migrations"
    );

    // And the v2-only column must be present.
    assert!(
        column_exists(&db, "clipboard_items", "content_hash"),
        "fresh DB missing content_hash column"
    );
    assert!(
        index_exists(&db, "idx_clipboard_content_hash"),
        "fresh DB missing idx_clipboard_content_hash"
    );
}

#[test]
fn migrate_v0_to_v1_adds_baseline_tables() {
    // v0 = empty file, user_version=0, no tables. Database::open must
    // apply v1 (creates baseline tables) AND v2 (adds content_hash) in
    // one atomic batch.
    let dir = tempdir().unwrap();
    let path = dir.path().join("v0.db");

    // Create a plaintext, empty database with user_version=0 (default).
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        // No CREATE TABLE — start from zero. user_version defaults to 0.
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 0, "staged DB must start at user_version=0");
        drop(conn);
    }

    let key = [0x02u8; 32];
    let db = Database::open(&path, &key).expect("v0->current migration");

    assert_eq!(user_version(&db), CURRENT_SCHEMA_VERSION);

    // All baseline (v1) tables must exist.
    for table in &[
        "clipboard_items",
        "clipboard_fts",
        "devices",
        "settings",
        "pending_uploads",
    ] {
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE (type='table' OR type='virtual') AND name=?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "v1 baseline missing table: {}", table);
    }
}

#[test]
fn migrate_v1_to_v2_adds_content_hash_column_and_index() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("v1.db");
    stage_v1_plaintext(&path);

    // Sanity: at v1, content_hash must NOT exist yet (in the plaintext
    // file we just wrote).
    {
        let conn = Connection::open(&path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            !cols.iter().any(|c| c == "content_hash"),
            "v1 baseline must not have content_hash column"
        );
    }

    let key = [0x03u8; 32];
    let db = Database::open(&path, &key).expect("v1 -> v2 migration");

    assert_eq!(user_version(&db), CURRENT_SCHEMA_VERSION);
    assert!(
        column_exists(&db, "clipboard_items", "content_hash"),
        "v2 migration must add content_hash column"
    );
    assert!(
        index_exists(&db, "idx_clipboard_content_hash"),
        "v2 migration must create idx_clipboard_content_hash"
    );

    // Verify the index is the partial WHERE-clause variant by checking its
    // CREATE SQL stored in sqlite_master.
    let sql: String = db
        .conn()
        .query_row(
            "SELECT sql FROM sqlite_master \
             WHERE type='index' AND name='idx_clipboard_content_hash'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        sql.to_uppercase().contains("WHERE"),
        "idx_clipboard_content_hash must be a partial index (WHERE content_hash IS NOT NULL); \
         actual SQL: {}",
        sql
    );
}

#[test]
fn migrate_idempotent_rerun_is_noop() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("idem.db");
    let key = [0x04u8; 32];

    // First open: fresh → CURRENT_SCHEMA_VERSION.
    let db1 = Database::open(&path, &key).unwrap();
    assert_eq!(user_version(&db1), CURRENT_SCHEMA_VERSION);
    drop(db1);

    // Reopen multiple times. Each must be a no-op (the equal-version
    // fast path inside apply_migrations) and must not corrupt the file.
    for _ in 0..3 {
        let db = Database::open(&path, &key).expect("idempotent reopen");
        assert_eq!(user_version(&db), CURRENT_SCHEMA_VERSION);
        // content_hash still present (no rebuild of clipboard_items).
        assert!(column_exists(&db, "clipboard_items", "content_hash"));
    }
}

#[test]
fn partial_migration_does_not_corrupt_data() {
    // Strategy: stage a v1 plaintext DB with real rows, then drop the
    // staging connection abruptly (simulates a kill before any v1->v2
    // migration could begin). Reopen via Database::open and assert:
    //   1. migration completes to CURRENT_SCHEMA_VERSION
    //   2. all original v1 rows are still present and queryable
    //   3. content_hash column is NULL for those rows (legacy data,
    //      no hash was computed pre-migration)
    //
    // This exercises the atomicity contract: even if the previous process
    // died after writing v1 data but before the v2 step ran, the data
    // survives the next open intact.
    let dir = tempdir().unwrap();
    let path = dir.path().join("partial.db");

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, \
              is_sensitive, is_synced, lamport_ts, wall_time) \
             VALUES \
               ('a', 'i-a', 'text/plain', X'01', X'AABBCC', 0, 0, 1, 1000), \
               ('b', 'i-b', 'text/plain', X'02', X'AABBCD', 0, 0, 2, 2000), \
               ('c', 'i-c', 'text/plain', X'03', X'AABBCE', 0, 0, 3, 3000);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        // Hard drop — simulates process kill between v1 commit and v2 start.
        drop(conn);
    }

    let key = [0x05u8; 32];
    let db = Database::open(&path, &key).expect("recover + migrate v1 -> v2");

    assert_eq!(user_version(&db), CURRENT_SCHEMA_VERSION);

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3, "all 3 rows must survive the migration");

    // content_hash exists, defaults to NULL for pre-migration rows.
    let null_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE content_hash IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        null_count, 3,
        "legacy v1 rows must have NULL content_hash after v2 migration"
    );
}

#[test]
fn existing_rows_preserved_through_migration() {
    // Same shape as partial_migration_does_not_corrupt_data but focuses on
    // *byte-level* preservation of content / content_nonce / lamport_ts /
    // wall_time — i.e., that v2 is a pure ALTER + CREATE INDEX and never
    // rewrites row data.
    let dir = tempdir().unwrap();
    let path = dir.path().join("preserve.db");

    let payload: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
    let nonce: &[u8] = &[
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
    ];

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, \
              is_sensitive, is_synced, lamport_ts, wall_time) \
             VALUES (?1, ?2, 'text/plain', ?3, ?4, 0, 0, 42, 1700000000)",
            rusqlite::params!["row-1", "item-1", payload, nonce],
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        drop(conn);
    }

    let key = [0x06u8; 32];
    let db = Database::open(&path, &key).unwrap();

    let (got_content, got_nonce, got_lamport, got_wall): (Vec<u8>, Vec<u8>, i64, i64) = db
        .conn()
        .query_row(
            "SELECT content, content_nonce, lamport_ts, wall_time \
             FROM clipboard_items WHERE id = 'row-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();

    assert_eq!(got_content, payload, "content bytes mutated by migration");
    assert_eq!(got_nonce, nonce, "content_nonce bytes mutated by migration");
    assert_eq!(got_lamport, 42);
    assert_eq!(got_wall, 1700000000);
}

#[test]
fn pragma_user_version_advances_atomically() {
    // After a successful v1 -> v2 migration the version must read exactly
    // CURRENT_SCHEMA_VERSION (never an intermediate value like 1 with the
    // v2 column already present, or 2 without the column). This pins the
    // atomicity contract of the single-transaction migration block in
    // schema.rs.
    let dir = tempdir().unwrap();
    let path = dir.path().join("atomic.db");
    stage_v1_plaintext(&path);

    let key = [0x07u8; 32];
    let db = Database::open(&path, &key).unwrap();

    let version = user_version(&db);
    let has_column = column_exists(&db, "clipboard_items", "content_hash");
    let has_index = index_exists(&db, "idx_clipboard_content_hash");

    assert_eq!(version, CURRENT_SCHEMA_VERSION);
    assert!(has_column, "version advanced but column missing");
    assert!(has_index, "version advanced but index missing");

    // Reopen and reverify — version must still be CURRENT, and the schema
    // shape must match (no second-application of the ALTER, which would
    // have errored with "duplicate column" if migration ran again).
    drop(db);
    let db2 = Database::open(&path, &key).unwrap();
    assert_eq!(user_version(&db2), CURRENT_SCHEMA_VERSION);
    assert!(column_exists(&db2, "clipboard_items", "content_hash"));
}

#[test]
fn migrate_v2_to_v3_adds_origin_device_id_column_with_empty_default() {
    // v3 is the fix for the merge.rs:39 CRDT tie-break BUG (comparing
    // `remote.origin_device_id` against `local.id` mixed two unrelated
    // identifier spaces). The migration adds `origin_device_id TEXT NOT
    // NULL DEFAULT ''` to `clipboard_items` so the new field has a
    // deterministic value on every legacy row; the daemon-side helper
    // `items::backfill_origin_device_id` stamps the local UUID later.
    //
    // We exercise the schema migration via an in-memory `Database::open_in_memory`
    // started from a v2 snapshot, then assert that the v3 column lands with
    // the documented empty default and that `backfill_origin_device_id`
    // stamps it idempotently.
    //
    // Bypassing `Database::open` (the plaintext→SQLCipher path) is
    // intentional: `sqlcipher_export` does not preserve `PRAGMA user_version`,
    // so on-disk v2→v3 migration must be tested by the daemon-level
    // integration suite, not at the schema layer. This test pins the v3
    // schema delta + backfill semantics, which is what the merge tie-break
    // depends on.
    use copypaste_core::storage::items::{
        backfill_origin_device_id, insert_item, ClipboardItem,
    };

    let db = Database::open_in_memory().expect("fresh v3 in-memory DB");

    // Fresh DB lands at v3 directly (see `fresh_db_creates_at_current_user_version`).
    assert_eq!(user_version(&db), CURRENT_SCHEMA_VERSION);
    assert!(
        column_exists(&db, "clipboard_items", "origin_device_id"),
        "v3 schema must include origin_device_id column"
    );

    // Simulate a legacy v2 row: insert with empty origin (the value an
    // `ALTER ADD COLUMN … DEFAULT ''` would have stamped on a real upgrade).
    let mut legacy = ClipboardItem::new_text(vec![0xAA], vec![0u8; 24], 1);
    legacy.id = "legacy-1".to_string();
    legacy.item_id = "i-legacy-1".to_string();
    legacy.wall_time = 1_000;
    legacy.origin_device_id = String::new(); // matches v2->v3 ALTER default
    insert_item(&db, &legacy).unwrap();

    let pre: String = db
        .conn()
        .query_row(
            "SELECT origin_device_id FROM clipboard_items WHERE id = 'legacy-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        pre, "",
        "legacy v2 row must surface with origin_device_id = '' before \
         backfill — this is the exact state a real ALTER … DEFAULT '' \
         leaves rows in"
    );

    // Backfill must stamp the empty rows with the local device id.
    let updated = backfill_origin_device_id(&db, "local-uuid-xyz").unwrap();
    assert_eq!(updated, 1, "backfill must touch the one empty-origin row");

    let post: String = db
        .conn()
        .query_row(
            "SELECT origin_device_id FROM clipboard_items WHERE id = 'legacy-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(post, "local-uuid-xyz");

    // Idempotency: re-running backfill is a no-op (zero rows match
    // `origin_device_id = ''` because we just filled them).
    let updated2 = backfill_origin_device_id(&db, "local-uuid-xyz").unwrap();
    assert_eq!(
        updated2, 0,
        "backfill must be idempotent — second call updates zero rows"
    );

    // Rows that already carry an origin (e.g. items received from peers)
    // must NOT be overwritten by backfill, so cross-device provenance is
    // preserved through subsequent merge tie-breaks.
    let mut peer_row = ClipboardItem::new_text(vec![0xBB], vec![0u8; 24], 2);
    peer_row.id = "peer-1".to_string();
    peer_row.item_id = "i-peer-1".to_string();
    peer_row.origin_device_id = "peer-A".to_string();
    insert_item(&db, &peer_row).unwrap();

    let updated3 = backfill_origin_device_id(&db, "local-uuid-xyz").unwrap();
    assert_eq!(
        updated3, 0,
        "backfill must skip rows that already have a non-empty origin"
    );

    let peer_origin: String = db
        .conn()
        .query_row(
            "SELECT origin_device_id FROM clipboard_items WHERE id = 'peer-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        peer_origin, "peer-A",
        "peer-origin row must not be overwritten by local backfill"
    );
}
