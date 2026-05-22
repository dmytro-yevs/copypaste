use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

const SCHEMA_VERSION: i64 = 2;

pub fn apply_migrations(conn: &Connection) -> Result<(), SchemaError> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(&format!("PRAGMA cache_size=-{};", 8 * 1024))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let current_version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    if current_version >= SCHEMA_VERSION {
        return Ok(());
    }

    if current_version < 1 {
        conn.execute_batch(include_str!("schema_v1.sql"))?;
    }

    if current_version < 2 {
        // Migration v2: add content_hash column for SHA-256-based deduplication.
        // ALTER TABLE is used (not DROP/CREATE) to preserve existing data.
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;
             CREATE INDEX IF NOT EXISTS idx_clipboard_content_hash
                 ON clipboard_items(content_hash) WHERE content_hash IS NOT NULL;",
        )?;
    }

    conn.execute_batch(&format!("PRAGMA user_version={};", SCHEMA_VERSION))?;
    Ok(())
}
