use super::*;
use rusqlite::Connection;

#[test]
fn downgrade_returns_explicit_error() {
    // Open a fresh in-memory DB, run migrations to bring it to SCHEMA_VERSION,
    // then bump user_version past it to simulate a database written by a
    // newer build.
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    conn.execute_batch("PRAGMA user_version = 999;").unwrap();

    let err = apply_migrations(&conn).unwrap_err();
    match err {
        SchemaError::Downgrade { found, expected } => {
            assert_eq!(found, 999);
            assert_eq!(expected, SCHEMA_VERSION);
        }
        other => panic!("expected SchemaError::Downgrade, got {:?}", other),
    }
}

/// CopyPaste-lmlr: `is_duplicate_column_error` must recognise the SQLite
/// "duplicate column name" failure (the one the retry loop in
/// `apply_migrations` is allowed to recover from) and must NOT match an
/// unrelated error, so a genuine schema fault still propagates.
#[test]
fn is_duplicate_column_error_matches_only_duplicate_column() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
    conn.execute_batch("ALTER TABLE t ADD COLUMN b TEXT;")
        .unwrap();
    // Re-adding the same column raises "duplicate column name: b".
    let dup = conn
        .execute_batch("ALTER TABLE t ADD COLUMN b TEXT;")
        .unwrap_err();
    assert!(
        is_duplicate_column_error(&dup),
        "must detect duplicate-column error, got: {dup}"
    );
    // An unrelated error (missing table) must NOT be treated as retryable.
    let other = conn
        .execute_batch("ALTER TABLE nope ADD COLUMN c TEXT;")
        .unwrap_err();
    assert!(
        !is_duplicate_column_error(&other),
        "must not match unrelated error: {other}"
    );
}

/// CopyPaste-m45w: when `content_hash` already exists in the table but
/// `user_version` is still at 1 (WAL-replay-onto-fresh-DB scenario triggered
/// by `reset_database` racing with a concurrent connection), `apply_migrations`
/// must skip the duplicate ALTER, apply all remaining steps, and reach
/// `SCHEMA_VERSION` successfully — NOT fail with "duplicate column name".
#[test]
fn v2_migration_idempotent_when_column_exists() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch("PRAGMA user_version = 1;").unwrap();

    // Pre-add the column that v2 would normally add (simulates WAL replay).
    conn.execute_batch("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;")
        .unwrap();

    // Migration must now SUCCEED (idempotent guard skips the duplicate ALTER).
    let result = apply_migrations(&conn);
    assert!(
        result.is_ok(),
        "migration must succeed when content_hash already exists: {result:?}"
    );

    // Must have reached the current schema version.
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, SCHEMA_VERSION,
        "user_version must reach SCHEMA_VERSION even when content_hash pre-exists"
    );

    // content_hash must appear exactly once.
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let count = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .filter(|name| name == "content_hash")
        .count();
    assert_eq!(count, 1, "content_hash must appear exactly once in schema");
}

/// Verify that the entire migration block is still atomic: when a step that
/// cannot be skipped (v13 purges clipboard_fts) fails because the table was
/// removed from the DB, `user_version` must remain unchanged.
#[test]
fn apply_migrations_is_atomic_on_failure() {
    // Build a v12 state but deliberately DROP `clipboard_fts` so the v13
    // migration (DELETE FROM clipboard_fts …) will fail with "no such table".
    // The BEGIN…COMMIT block must roll back in full, leaving user_version at 12.
    let conn = Connection::open_in_memory().unwrap();

    // Bring the DB up to the v12 schema shape by hand.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
         CREATE TABLE IF NOT EXISTS revoked_devices (\
           fingerprint TEXT PRIMARY KEY NOT NULL,\
           name TEXT NOT NULL DEFAULT '',\
           revoked_at INTEGER NOT NULL);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 12;").unwrap();

    // Drop clipboard_fts so that the v13 DELETE fails.
    conn.execute_batch("DROP TABLE IF EXISTS clipboard_fts;")
        .unwrap();

    // The migration must fail (v13 cannot purge a table that doesn't exist).
    let result = apply_migrations(&conn);
    assert!(
        result.is_err(),
        "migration must fail when clipboard_fts is absent"
    );

    // user_version must NOT have advanced — the transaction was rolled back.
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, 12,
        "user_version must remain at 12 after a rolled-back migration"
    );
}

#[test]
fn fresh_db_reaches_current_schema_version() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn equal_version_is_noop() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    // Second call hits the `current_version == SCHEMA_VERSION` fast path.
    apply_migrations(&conn).unwrap();
}

#[test]
fn fresh_db_has_origin_device_id_column() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        cols.iter().any(|c| c == "origin_device_id"),
        "v3 schema must include origin_device_id column"
    );
}

#[test]
fn fresh_db_has_key_version_column() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let cols: Vec<(String, String, i64)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?, // column name
                r.get::<_, String>(2)?, // declared type
                r.get::<_, i64>(3)?,    // notnull
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let kv = cols
        .iter()
        .find(|c| c.0 == "key_version")
        .expect("v4 schema must include key_version column");
    assert_eq!(kv.1.to_uppercase(), "INTEGER");
    assert_eq!(kv.2, 1, "key_version must be NOT NULL");
}

#[test]
fn fresh_db_has_migration_state_table() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='migration_state'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "migration_state table must be created by v6 migration"
    );

    // The seed row must be present.
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM migration_state WHERE key = 'v4-key-version-sweep'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(row_count, 1, "seed row must be inserted by v6 migration");
}

#[test]
fn v3_to_v4_migration_marks_existing_rows_as_v1_key() {
    // Bring a fresh DB only up to v3 by short-circuiting the v4 step,
    // then re-run apply_migrations and assert existing rows landed on
    // key_version=1 (the DEFAULT in V4_ALTER_SQL).
    let conn = Connection::open_in_memory().unwrap();

    // Hand-build v3 state.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 3;").unwrap();

    // Insert a v3-era row.
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, content_hash, origin_device_id) \
         VALUES ('id-1', 'item-1', 'text', 1, 1000, NULL, '')",
        [],
    )
    .unwrap();

    // Run apply_migrations → must add key_version column and DEFAULT 1
    // backfills the pre-existing row.
    apply_migrations(&conn).unwrap();

    let kv: i64 = conn
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = 'id-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        kv, 1,
        "pre-v4 rows must land on key_version=1 so the v1→v2 sweep can find them"
    );

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

#[test]
fn fresh_db_has_pinned_column() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let cols: Vec<(String, String, i64)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?, // column name
                r.get::<_, String>(2)?, // declared type
                r.get::<_, i64>(3)?,    // notnull
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let pinned_col = cols
        .iter()
        .find(|c| c.0 == "pinned")
        .expect("v7 schema must include pinned column");
    assert_eq!(pinned_col.1.to_uppercase(), "INTEGER");
    assert_eq!(pinned_col.2, 1, "pinned must be NOT NULL");
}

#[test]
fn fresh_db_has_thumb_column() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let cols: Vec<(String, String)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?, // column name
                r.get::<_, String>(2)?, // declared type
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let thumb = cols
        .iter()
        .find(|c| c.0 == "thumb")
        .expect("v9 schema must include thumb column");
    assert_eq!(thumb.1.to_uppercase(), "BLOB");
}

#[test]
fn v8_to_v9_migration_backfills_existing_rows_with_null_thumb() {
    // Simulate a v8 database (no thumb column), run migrations, and verify
    // existing rows land on thumb = NULL (the DEFAULT in V9_ALTER) and the
    // user_version reaches the current SCHEMA_VERSION.
    let conn = Connection::open_in_memory().unwrap();

    // Bring a fresh DB fully up to v8 by short-circuiting the v9 step: run
    // the real migrator (it will go straight to 9), then we can't easily
    // stop at 8 — so hand-build the v8 shape instead.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 8;").unwrap();

    // Insert a v8-era row (no thumb column yet).
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned) \
         VALUES ('id-v8', 'item-v8', 'image', 1, 1000, '', 2, 0)",
        [],
    )
    .unwrap();

    // Run apply_migrations → must add thumb column, DEFAULT NULL backfills.
    apply_migrations(&conn).unwrap();

    let thumb: Option<Vec<u8>> = conn
        .query_row(
            "SELECT thumb FROM clipboard_items WHERE id = 'id-v8'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(thumb.is_none(), "pre-v9 rows must land on thumb = NULL");

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

#[test]
fn v6_to_v7_migration_backfills_existing_rows_as_unpinned() {
    // Simulate a v6 database (no pinned column), run migrations, and
    // verify existing rows land on pinned=0 (the DEFAULT in V7_ALTER_SQL).
    let conn = Connection::open_in_memory().unwrap();

    // Hand-build v6 state.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 6;").unwrap();

    // Insert a v6-era row (no pinned column yet).
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version) \
         VALUES ('id-v6', 'item-v6', 'text', 1, 1000, '', 2)",
        [],
    )
    .unwrap();

    // Run apply_migrations → must add pinned column, DEFAULT 0 backfills.
    apply_migrations(&conn).unwrap();

    let pinned: i64 = conn
        .query_row(
            "SELECT pinned FROM clipboard_items WHERE id = 'id-v6'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pinned, 0, "pre-v7 rows must land on pinned=0");

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

#[test]
fn fresh_db_has_deleted_column() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
    let cols: Vec<(String, String, i64)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?, // column name
                r.get::<_, String>(2)?, // declared type
                r.get::<_, i64>(3)?,    // notnull
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let deleted_col = cols
        .iter()
        .find(|c| c.0 == "deleted")
        .expect("v10 schema must include deleted column");
    assert_eq!(deleted_col.1.to_uppercase(), "INTEGER");
    assert_eq!(deleted_col.2, 1, "deleted must be NOT NULL");
}

#[test]
fn v9_to_v10_migration_backfills_existing_rows_as_not_deleted() {
    // Simulate a v9 database (no deleted column), run migrations, and verify
    // existing rows land on deleted=0 (the DEFAULT in V10_ALTER) and the
    // user_version reaches the current SCHEMA_VERSION.
    let conn = Connection::open_in_memory().unwrap();

    // Hand-build v9 state.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 9;").unwrap();

    // Insert a v9-era row (no deleted column yet).
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned) \
         VALUES ('id-v9', 'item-v9', 'text', 1, 1000, '', 2, 0)",
        [],
    )
    .unwrap();

    // Run apply_migrations → must add deleted column, DEFAULT 0 backfills.
    apply_migrations(&conn).unwrap();

    let deleted: i64 = conn
        .query_row(
            "SELECT deleted FROM clipboard_items WHERE id = 'id-v9'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deleted, 0, "pre-v10 rows must land on deleted=0");

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

/// CopyPaste-61fu: migration v12 must create the `revoked_devices` table and
/// its index as part of the standard migration chain so that the table exists
/// on every properly-initialised DB without requiring an explicit
/// `ensure_revoked_devices_table` call.
#[test]
fn fresh_db_has_revoked_devices_table() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();

    // Table must exist.
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='revoked_devices'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "revoked_devices table must be created by v12 migration"
    );

    // Index must exist.
    let idx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='index' AND name='idx_revoked_devices_revoked_at'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        idx_count, 1,
        "idx_revoked_devices_revoked_at must be created by v12 migration"
    );
}

/// CopyPaste-61fu: a v11 database (no revoked_devices table) upgraded via
/// apply_migrations must end up with the table and user_version == SCHEMA_VERSION.
#[test]
fn v11_to_v12_migration_creates_revoked_devices_table() {
    let conn = Connection::open_in_memory().unwrap();

    // Hand-build a v11 state: all v1–v11 changes, no revoked_devices table.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
         CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
           ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 11;").unwrap();

    // Sanity: table must not exist before migration.
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='revoked_devices'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        before, 0,
        "revoked_devices must not exist before v12 migration"
    );

    // Run the migration.
    apply_migrations(&conn).unwrap();

    // Table must now exist.
    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='revoked_devices'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(after, 1, "revoked_devices must be created by v12 migration");

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

/// CopyPaste-61fu: a DB that already has the revoked_devices table (created
/// by the old ad-hoc path) must survive the v12 migration without error
/// (CREATE TABLE IF NOT EXISTS is idempotent).
#[test]
fn v12_migration_is_idempotent_when_table_already_exists() {
    let conn = Connection::open_in_memory().unwrap();

    // Build a v11 state that already has revoked_devices (the old ad-hoc path).
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
         CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
           ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
         CREATE TABLE IF NOT EXISTS revoked_devices (\
           fingerprint TEXT PRIMARY KEY NOT NULL,\
           name TEXT NOT NULL DEFAULT '',\
           revoked_at INTEGER NOT NULL);\n\
         CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
           ON revoked_devices(revoked_at DESC);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 11;").unwrap();

    // Migration must succeed without error even though the table already exists.
    apply_migrations(&conn).unwrap();

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

/// CopyPaste-i6pp: migration v13 must delete clipboard_fts rows that
/// belong to sensitive items, and leave non-sensitive FTS rows intact.
#[test]
fn v13_migration_purges_sensitive_fts_rows() {
    let conn = Connection::open_in_memory().unwrap();

    // Build a v12 state with clipboard_items + clipboard_fts.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
         CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
           ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
         CREATE TABLE IF NOT EXISTS revoked_devices (\
           fingerprint TEXT PRIMARY KEY NOT NULL,\
           name TEXT NOT NULL DEFAULT '',\
           revoked_at INTEGER NOT NULL);\n\
         CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
           ON revoked_devices(revoked_at DESC);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 12;").unwrap();

    // Insert one sensitive and one non-sensitive item.
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
         VALUES ('id-secret', 'iid-s', 'text', 1, 1000, '', 2, 0, 1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
         VALUES ('id-normal', 'iid-n', 'text', 2, 2000, '', 2, 0, 0)",
        [],
    )
    .unwrap();

    // Simulate the pre-fix bug: both items have FTS rows.
    conn.execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-secret', 'my super secret password')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-normal', 'ordinary clipboard text')",
        [],
    )
    .unwrap();

    // Sanity: both FTS rows exist before migration.
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, 2, "both FTS rows must exist before v13 migration");

    // Run migration.
    apply_migrations(&conn).unwrap();

    // Sensitive FTS row must be gone.
    let secret_fts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = 'id-secret'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        secret_fts, 0,
        "v13 migration must remove FTS row for sensitive item (CopyPaste-i6pp)"
    );

    // Non-sensitive FTS row must survive.
    let normal_fts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = 'id-normal'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        normal_fts, 1,
        "v13 migration must preserve FTS row for non-sensitive item"
    );

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

/// CopyPaste-i6pp: v13 migration is idempotent — running it on a DB that
/// has no sensitive FTS rows must succeed without error.
#[test]
fn v13_migration_is_noop_when_no_sensitive_fts_rows_exist() {
    let conn = Connection::open_in_memory().unwrap();

    // Build a v12 state with only a non-sensitive item.
    conn.execute_batch(V1_SCHEMA_SQL).unwrap();
    conn.execute_batch(
        "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
         ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
         ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
         ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
         ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
         ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
         CREATE TABLE IF NOT EXISTS migration_state (\
           key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
           last_processed_id INTEGER NOT NULL DEFAULT 0,\
           started_at INTEGER, completed_at INTEGER);\n\
         INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
         CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
           ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
         CREATE TABLE IF NOT EXISTS revoked_devices (\
           fingerprint TEXT PRIMARY KEY NOT NULL,\
           name TEXT NOT NULL DEFAULT '',\
           revoked_at INTEGER NOT NULL);\n\
         CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
           ON revoked_devices(revoked_at DESC);",
    )
    .unwrap();
    conn.execute_batch("PRAGMA user_version = 12;").unwrap();

    conn.execute(
        "INSERT INTO clipboard_items \
         (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
         VALUES ('id-n', 'iid-n', 'text', 1, 1000, '', 2, 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-n', 'hello world')",
        [],
    )
    .unwrap();

    // Must succeed without error.
    apply_migrations(&conn).unwrap();

    let fts: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        fts, 1,
        "non-sensitive FTS row must survive a no-op v13 migration"
    );

    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(uv, SCHEMA_VERSION);
}

/// CopyPaste-89rd: migration v14 must create `idx_clipboard_history_page` so
/// `get_page_pinned_first`'s `WHERE deleted=0 ORDER BY pinned DESC ...` query
/// uses an index range scan instead of a full-table scan + filesort.
///
/// Verified via EXPLAIN QUERY PLAN: the plan detail must contain "USING INDEX"
/// referencing `idx_clipboard_history_page` rather than "SCAN clipboard_items".
#[test]
fn v14_migration_creates_history_page_index() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();

    // Index must exist in sqlite_master.
    let idx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='index' AND name='idx_clipboard_history_page'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        idx_count, 1,
        "idx_clipboard_history_page must be created by v14 migration (CopyPaste-89rd)"
    );
}

/// CopyPaste-89rd: EXPLAIN QUERY PLAN confirms the history-page query uses the
/// new index rather than a full-table scan.
///
/// This is the acceptance criterion from the issue: the plan detail must contain
/// "USING INDEX idx_clipboard_history_page" (or equivalent "idx_clipboard_history_page"
/// substring), meaning SQLite chose the partial covering index over a table scan.
#[test]
fn history_page_query_uses_index_not_full_scan() {
    let conn = Connection::open_in_memory().unwrap();
    apply_migrations(&conn).unwrap();

    // The exact SQL used by get_page_pinned_first (wall-time variant).
    // We run EXPLAIN QUERY PLAN and assert the plan contains the index name.
    let plan_rows: Vec<String> = conn
        .prepare(
            "EXPLAIN QUERY PLAN \
             SELECT id FROM clipboard_items \
             WHERE deleted = 0 \
             ORDER BY \
               CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC, \
               pin_order IS NULL ASC, \
               pin_order ASC, \
               wall_time DESC \
             LIMIT 50 OFFSET 0",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let plan = plan_rows.join(" ");
    // The index must be referenced in the query plan — this is the primary
    // correctness signal. SQLite uses the index for the WHERE deleted=0 filter
    // even when the CASE expression in ORDER BY still requires a temp B-tree
    // for final sort ordering; the plan reads "SCAN ... USING INDEX ..." rather
    // than a bare "SCAN clipboard_items" (no index).
    assert!(
        plan.contains("idx_clipboard_history_page"),
        "EXPLAIN QUERY PLAN must reference idx_clipboard_history_page, \
         got plan: {plan:?} (CopyPaste-89rd)"
    );
    // Without the index SQLite emits "SCAN clipboard_items" with no "USING INDEX"
    // suffix. With the index the plan says "SCAN/SEARCH ... USING INDEX
    // idx_clipboard_history_page". Assert there is no bare unindexed scan.
    assert!(
        !plan.eq("SCAN clipboard_items"),
        "EXPLAIN QUERY PLAN must not be a bare unindexed full-table scan, \
         got plan: {plan:?} (CopyPaste-89rd)"
    );
}

/// CopyPaste-2lc9: regression for the WAL-replay duplicate-column race.
///
/// Scenario: Connection A writes a v1 schema with `content_hash` already
/// added to a REAL FILE database (simulating the WAL state left by a
/// previous migration or crash) and drops WITHOUT checkpointing. Connection
/// B opens the same file and calls `apply_migrations`.
///
/// Before the fix: if the WAL was lazily applied between `column_exists`
/// returning false and `execute_batch` running the BEGIN…COMMIT script, the
/// ALTER TABLE would fail with "duplicate column name: content_hash".
///
/// After the fix: `PRAGMA wal_checkpoint(TRUNCATE)` at the top of
/// `apply_migrations` flushes any outstanding WAL frames into the main
/// database file BEFORE `column_exists` runs, making the guard
/// authoritative. Regardless of WAL state, `column_exists` always sees the
/// complete post-checkpoint schema.
///
/// Note: the true concurrent race (another writer commits `content_hash` to
/// the WAL between `column_exists` and `execute_batch`) cannot be triggered
/// deterministically in a single-threaded test. This test validates the
/// file-DB code path of the guard and documents the scenario; the
/// wal_checkpoint fix is the authoritative defence against the CI-observed
/// intermittent failure.
#[test]
fn wal_replay_does_not_cause_duplicate_column() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("wal_race.db");

    // ── Connection A: write v1 schema + content_hash to a WAL-mode
    // file DB, then drop WITHOUT checkpointing. This leaves committed WAL
    // frames containing the schema on disk, visible to the next reader.
    {
        let conn_a = Connection::open(&path).expect("open conn_a");
        conn_a
            .execute_batch("PRAGMA journal_mode=WAL;")
            .expect("WAL mode");
        conn_a.execute_batch(V1_SCHEMA_SQL).expect("v1 schema");
        conn_a
            .execute_batch("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;")
            .expect("pre-add content_hash");
        conn_a
            .execute_batch("PRAGMA user_version = 1;")
            .expect("set user_version=1");
        // conn_a is dropped here WITHOUT calling wal_checkpoint — the WAL
        // frames remain on disk exactly as a background writer would leave
        // them in the reset_database race scenario.
    }

    // ── Connection B: opens the same file. The WAL file is still present
    // (not checkpointed). `apply_migrations` must succeed:
    //   1. `PRAGMA wal_checkpoint(TRUNCATE)` at the top flushes the WAL.
    //   2. `column_exists` now sees `content_hash` → skips the ALTER.
    //   3. The migration script runs without "duplicate column name".
    {
        let conn_b = Connection::open(&path).expect("open conn_b");
        let result = apply_migrations(&conn_b);
        assert!(
            result.is_ok(),
            "apply_migrations must succeed when WAL contains pre-existing \
             content_hash (CopyPaste-2lc9): {result:?}"
        );

        // Must reach the current schema version.
        let version: i64 = conn_b
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, SCHEMA_VERSION,
            "must reach SCHEMA_VERSION after migration on file DB with WAL"
        );

        // content_hash must appear exactly once — no duplicate from the race.
        let mut stmt = conn_b
            .prepare("PRAGMA table_info(clipboard_items)")
            .unwrap();
        let count = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .filter(|name| name == "content_hash")
            .count();
        assert_eq!(
            count, 1,
            "content_hash must appear exactly once (no duplicate from WAL race)"
        );
    }
}
