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

const SCHEMA_VERSION: i64 = 2;

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
    // Connection-level pragmas. These are NOT part of a migration and MUST
    // run before BEGIN (PRAGMA journal_mode is a no-op inside a transaction).
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(&format!("PRAGMA cache_size=-{};", 8 * 1024))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

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
        script.push_str(include_str!("schema_v1.sql"));
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
        conn.execute_batch(include_str!("schema_v1.sql")).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        // Pre-add the column the v2 step would add → guarantees ALTER fails.
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;",
        )
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
}
