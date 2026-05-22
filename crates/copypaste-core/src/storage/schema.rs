use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

const SCHEMA_VERSION: i64 = 1;

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

    conn.execute_batch(include_str!("schema_v1.sql"))?;
    conn.execute_batch(&format!("PRAGMA user_version={};", SCHEMA_VERSION))?;
    Ok(())
}
