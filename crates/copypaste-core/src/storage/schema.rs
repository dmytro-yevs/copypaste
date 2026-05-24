use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The on-disk database was created by a *newer* version of the application
    /// than the one currently running. Downgrading the schema would silently
    /// drop forward-compatible columns / tables, so we refuse to open the file.
    #[error(
        "Database schema downgrade detected (on-disk version {found}, \
         binary expects {expected}). Refusing to open to avoid data loss."
    )]
    Downgrade { found: i64, expected: i64 },
}

/// Current on-disk schema version.
///
/// Bumps:
///   * 2 → 3: added `origin_device_id` for the LWW merge tie-break
///     (see `copypaste-sync::merge::resolve`).
///   * 3 → 4 (v0.3 T5): added `key_version` column to `clipboard_items` to
///     track which HKDF key generation (v1 or v2) encrypted each row's
///     ciphertext. See [`V4_ALTER_SQL`] and `super::migration_v4`.
///   * 4 → 5 (beta.6 merge): added two UNIQUE INDEXes — `content_hash`+minute
///     bucket for TOCTOU dedup, `item_id` for sync replay protection.
///     See [`V5_INDEXES_SQL`] / `schema_v2.sql`.
///   * 5 → 6 (wave1a-atomic): added `migration_state` table for resumable
///     v4 key-rotation sweep tracking. Seeds the initial row so
///     `Database::migration_state()` always returns a valid state.
///   * 6 → 7 (v0.3 pinned-fix): added `pinned` column to `clipboard_items`
///     so explicitly pinned items are distinguishable from normal rows with
///     `expires_at = NULL`. The TTL prune and history-limit prune both
///     filter `WHERE pinned = 0` to guarantee pinned items are never deleted.
pub const SCHEMA_VERSION: i64 = 7;

/// Baseline (v1) schema as a single SQL script. Made `pub(crate)` so the
/// crate-internal `db` and `schema` tests can stage a legacy plaintext DB
/// without duplicating the SQL. Integration tests still inline a copy because
/// `include_str!` paths are crate-relative and not visible from `tests/`.
pub(crate) const V1_SCHEMA_SQL: &str = include_str!("schema_v1.sql");

/// v3 ALTER step — add `origin_device_id` to `clipboard_items`. SQLite
/// requires a literal constant default for `ALTER TABLE ADD COLUMN`, so we
/// use the empty string and let `items::backfill_origin_device_id` stamp the
/// real local UUID at daemon startup.
pub(crate) const V3_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n";

/// v4 ALTER step — add `key_version` to `clipboard_items`.
///
/// Default `1` ensures every existing row is marked as v1-key-encrypted, so
/// `super::migration_v4::migrate_v1_to_v2_keys` can find them via the
/// straightforward `WHERE key_version = 1` predicate. New `insert_item`
/// calls write the current key version (`2`) explicitly — the `DEFAULT 1`
/// here is exclusively for the existing-row backfill case.
pub(crate) const V4_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
CREATE INDEX IF NOT EXISTS idx_clipboard_key_version \
    ON clipboard_items(key_version) WHERE key_version < 2;\n";

/// v5 step — add two UNIQUE INDEXes (`content_hash`+minute-bucket for TOCTOU
/// dedup, `item_id` for sync replay protection). Originally landed in beta
/// as user_version=4 (V4_INDEXES_SQL) but v3 already claimed v4 for
/// key_version. Bumped to v5 on merge into v0.3.
///
/// SQL file kept as `schema_v2.sql` for historical reasons.
pub(crate) const V5_INDEXES_SQL: &str = include_str!("schema_v2.sql");

/// v7 ALTER step — add `pinned` column to `clipboard_items`.
///
/// `DEFAULT 0` means all existing rows are treated as unpinned, which is
/// correct: items pinned under the old scheme (where `pin_item` only cleared
/// `expires_at`) become re-pinnable via the updated `pin_item` call that now
/// also sets `pinned = 1`. The `DEFAULT 0` here is exclusively for the
/// existing-row backfill case.
pub(crate) const V7_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
CREATE INDEX IF NOT EXISTS idx_clipboard_pinned \
    ON clipboard_items(pinned) WHERE pinned = 1;\n";

/// Apply pending schema migrations atomically inside a single transaction.
///
/// Behavior contract:
///   * `current_version == SCHEMA_VERSION` → no-op, `Ok(())`.
///   * `current_version <  SCHEMA_VERSION` → run migrations inside a
///     transaction. If any step fails, the transaction is rolled back and
///     `user_version` remains untouched.
///   * `current_version >  SCHEMA_VERSION` → return `SchemaError::Downgrade`.
///     Previously this branch fell through to `Ok(())` and silently masked
///     the version mismatch (CRITICAL edge-case #2).
pub fn apply_migrations(conn: &Connection) -> Result<(), SchemaError> {
    // Connection-level pragmas that MUST run before BEGIN (PRAGMA journal_mode
    // is a no-op inside a transaction). Only the pragmas NOT already applied by
    // `db::CONNECTION_PRAGMAS` live here — callers that go through
    // `Database::open` / `Database::open_in_memory` apply CONNECTION_PRAGMAS
    // separately, so we keep only the ones unique to the migration path
    // (journal_mode and cache_size) to avoid redundant double-application.
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(&format!("PRAGMA cache_size=-{};", 8 * 1024))?;

    let current_version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    if current_version == SCHEMA_VERSION {
        return Ok(());
    }
    if current_version > SCHEMA_VERSION {
        return Err(SchemaError::Downgrade {
            found: current_version,
            expected: SCHEMA_VERSION,
        });
    }

    // --- Atomic migration block (architecture MEDIUM #15) ---
    //
    // We build one SQL script that contains BEGIN, every needed step, the
    // user_version bump, and COMMIT. SQLite will roll back automatically if
    // any statement inside fails, leaving `user_version` at its previous
    // value (verified by `apply_migrations_is_atomic_on_failure`).
    let mut script = String::with_capacity(2048);
    script.push_str("BEGIN;\n");

    if current_version < 1 {
        script.push_str(V1_SCHEMA_SQL);
        script.push('\n');
    }

    if current_version < 2 {
        // Migration v2: add content_hash column for SHA-256-based deduplication.
        // ALTER TABLE is used (not DROP/CREATE) to preserve existing data.
        script.push_str(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             CREATE INDEX IF NOT EXISTS idx_clipboard_content_hash\n\
                 ON clipboard_items(content_hash) WHERE content_hash IS NOT NULL;\n",
        );
    }

    if current_version < 3 {
        // Migration v3: add origin_device_id column used by the LWW merge
        // tie-break (see `copypaste-sync::merge::resolve`). Defaults to the
        // empty string for legacy rows; the daemon calls
        // `items::backfill_origin_device_id` after open to stamp the local
        // device UUID onto any rows still carrying the empty default.
        script.push_str(V3_ALTER_SQL);
    }

    if current_version < 4 {
        // Migration v4 (T5): add `key_version` column so the re-encrypt sweep
        // can identify rows still encrypted under the v1 HKDF key family.
        // The actual decrypt-with-v1 + re-encrypt-with-v2 work is performed
        // by `super::migration_v4::migrate_v1_to_v2_keys`, invoked by the
        // daemon at startup after the schema migration commits.
        script.push_str(V4_ALTER_SQL);
    }

    if current_version < 5 {
        // Migration v5 (beta.6 merge): two UNIQUE INDEXes. CREATE INDEX IF
        // NOT EXISTS is idempotent so safe to re-run during partial-rollout.
        // See schema_v2.sql for per-index rationale.
        script.push_str(V5_INDEXES_SQL);
        script.push('\n');
    }

    if current_version < 6 {
        // Migration v6 (wave1a-atomic): create `migration_state` table for
        // resumable v4 key-rotation sweep tracking.
        //
        // Seed the row with completed_at already set when there are no
        // key_version=1 rows (fresh install or database already clean).
        // This prevents the gate in insert_item from blocking writes on a
        // brand-new database that has nothing to sweep.
        //
        // For upgrades from an earlier schema that may have key_version=1
        // rows, completed_at is left NULL so the daemon startup sweep runs.
        script.push_str(
            "CREATE TABLE IF NOT EXISTS migration_state (\n\
             key                     TEXT PRIMARY KEY,\n\
             key_version_in_progress INTEGER,\n\
             last_processed_id       INTEGER NOT NULL DEFAULT 0,\n\
             started_at              INTEGER,\n\
             completed_at            INTEGER\n\
             );\n\
             INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at, completed_at) \
             VALUES (\n\
               'v4-key-version-sweep', 2, 0, strftime('%s','now'),\n\
               CASE WHEN (SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1) = 0\n\
                    THEN strftime('%s','now') ELSE NULL END\n\
             );\n",
        );
    }

    if current_version < 7 {
        // Migration v7 (v0.3 pinned-fix): add `pinned` column so explicitly
        // pinned items survive both the TTL prune and the history-limit prune.
        // `DEFAULT 0` backfills all existing rows as unpinned, which is safe:
        // items were pinned only by clearing `expires_at`, so no data is lost.
        script.push_str(V7_ALTER_SQL);
    }

    script.push_str(&format!("PRAGMA user_version={};\n", SCHEMA_VERSION));
    script.push_str("COMMIT;\n");

    // execute_batch runs everything; on error SQLite implicitly rolls back the
    // open transaction, so we don't need an explicit ROLLBACK statement.
    conn.execute_batch(&script)?;

    Ok(())
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn apply_migrations_is_atomic_on_failure() {
        // Pre-create `clipboard_items` with ONLY the legacy v1 shape, then set
        // user_version to 1 so the migrator believes v2 must run. We then
        // pre-add the `content_hash` column ourselves so the v2 ALTER TABLE
        // step fails with "duplicate column name". Because the entire
        // migration runs inside a single transaction, user_version must
        // remain at 1 after the failure (NOT be updated to SCHEMA_VERSION).
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        // Pre-add the column the v2 step would add → guarantees ALTER fails.
        conn.execute_batch("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;")
            .unwrap();

        let result = apply_migrations(&conn);
        assert!(result.is_err(), "migration should fail on duplicate column");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, 1,
            "user_version must remain at 1 after rolled-back migration"
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
}
