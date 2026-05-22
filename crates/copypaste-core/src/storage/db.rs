use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use thiserror::Error;
use super::schema::{apply_migrations, SchemaError};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Schema migration error: {0}")]
    Schema(#[from] SchemaError),
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection { &self.conn }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn database_opens_with_wal_mode() {
        // WAL mode requires a file-backed database; in-memory always reports "memory".
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal_test.db");
        let db = Database::open(&path).unwrap();
        let mode: String = db.conn().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn schema_creates_all_tables() {
        let db = Database::open_in_memory().unwrap();
        for table in &["clipboard_items", "devices", "settings", "pending_uploads"] {
            let count: i64 = db.conn()
                .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?", [table], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1, "missing table: {}", table);
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        Database::open(&path).unwrap();
        Database::open(&path).unwrap();
    }
}
