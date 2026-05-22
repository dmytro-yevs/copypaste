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
    #[error("Plaintext-to-encrypted migration failed: {0}")]
    Migration(String),
}

pub struct Database {
    conn: Connection,
}

/// Format a 32-byte key as the hex string SQLCipher expects:
///   PRAGMA key = "x'<64 hex chars>'"
fn key_pragma(key: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for b in key {
        write!(hex, "{:02x}", b).unwrap();
    }
    format!("PRAGMA key = \"x'{}'\"", hex)
}

impl Database {
    /// Open (or create) an encrypted database at `path`.
    ///
    /// `key` is a 32-byte AES-256 key (typically `DeviceKeypair::local_enc_key()`).
    /// The PRAGMA key statement is applied before any other statement, as required
    /// by SQLCipher.
    ///
    /// If the file exists but is plaintext (written before this change), the function
    /// automatically re-encrypts it in-place using the rusqlite Backup API before
    /// returning.
    ///
    /// Returns `Err(DbError::Sqlite(...))` if `key` is wrong for an existing
    /// encrypted database.
    pub fn open(path: impl AsRef<Path>, key: &[u8; 32]) -> Result<Self, DbError> {
        let path = path.as_ref();

        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        // SQLCipher requirement: key pragma MUST be the very first statement.
        conn.execute_batch(&key_pragma(key))?;

        // Validate the key by reading the schema table.
        // With a correct key (or a newly-created empty file): returns 0 or N.
        // With a wrong key on an encrypted file: SQLCipher returns SQLITE_NOTADB.
        // With a plaintext file opened under SQLCipher+key: also SQLITE_NOTADB.
        match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        }) {
            Ok(_) => {
                apply_migrations(&conn)?;
                Ok(Self { conn })
            }
            Err(rusqlite::Error::SqliteFailure(err, msg))
                if err.extended_code == rusqlite::ffi::SQLITE_NOTADB
                    || err.code == rusqlite::ErrorCode::DatabaseCorrupt =>
            {
                // Could be (a) wrong key or (b) plaintext file.
                // Distinguish: open WITHOUT key and probe schema.
                // On success → plaintext → migrate.
                // On failure → wrong key → propagate original error.
                drop(conn);
                let probe = Connection::open_with_flags(
                    path,
                    OpenFlags::SQLITE_OPEN_READ_WRITE,
                );
                match probe {
                    Ok(plain_conn) => {
                        let schema_ok = plain_conn
                            .query_row(
                                "SELECT COUNT(*) FROM sqlite_master",
                                [],
                                |r| r.get::<_, i64>(0),
                            )
                            .is_ok();
                        drop(plain_conn);
                        if schema_ok {
                            // Plaintext file confirmed. Migrate in-place.
                            Self::encrypt_existing(path, key)?;
                            // Re-open encrypted.
                            let enc = Connection::open_with_flags(
                                path,
                                OpenFlags::SQLITE_OPEN_READ_WRITE,
                            )?;
                            enc.execute_batch(&key_pragma(key))?;
                            apply_migrations(&enc)?;
                            Ok(Self { conn: enc })
                        } else {
                            // Both keyed and unkeyed probes fail → wrong key.
                            Err(DbError::Sqlite(rusqlite::Error::SqliteFailure(err, msg)))
                        }
                    }
                    Err(_) => {
                        // Cannot open at all → wrong key.
                        Err(DbError::Sqlite(rusqlite::Error::SqliteFailure(err, msg)))
                    }
                }
            }
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Migrate an unencrypted file to SQLCipher in-place.
    ///
    /// Strategy: open the plaintext source, ATTACH a new encrypted destination,
    /// use `sqlcipher_export()` to copy all content, DETACH, then atomically
    /// replace the original file. This is the SQLCipher-recommended migration path.
    ///
    /// `sqlcipher_export()` is available on any connection compiled with the
    /// `bundled-sqlcipher` feature.
    fn encrypt_existing(path: &Path, key: &[u8; 32]) -> Result<(), DbError> {
        use std::fmt::Write as FmtWrite;

        let tmp_path = path.with_extension("db.tmp");
        // Remove any leftover tmp from a previous crashed migration.
        let _ = std::fs::remove_file(&tmp_path);

        // Build the hex key for the ATTACH statement.
        let mut hex = String::with_capacity(64);
        for b in key {
            write!(hex, "{:02x}", b).unwrap();
        }
        let key_hex = hex;

        // Open the plaintext source (no key pragma needed).
        let plaintext_conn = Connection::open(path)
            .map_err(|e| DbError::Migration(format!("open plaintext: {e}")))?;

        // ATTACH a new encrypted DB as 'encrypted'.
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\"",
            tmp_path.display(),
            key_hex
        );
        plaintext_conn
            .execute_batch(&attach_sql)
            .map_err(|e| DbError::Migration(format!("ATTACH encrypted: {e}")))?;

        // Copy everything using SQLCipher's built-in export function.
        plaintext_conn
            .execute_batch("SELECT sqlcipher_export('encrypted')")
            .map_err(|e| DbError::Migration(format!("sqlcipher_export: {e}")))?;

        plaintext_conn
            .execute_batch("DETACH DATABASE encrypted")
            .map_err(|e| DbError::Migration(format!("DETACH: {e}")))?;

        drop(plaintext_conn);

        // Atomically replace the plaintext file with the encrypted copy.
        std::fs::rename(&tmp_path, path)
            .map_err(|e| DbError::Migration(format!("rename tmp->original: {e}")))?;

        Ok(())
    }

    /// Open an in-memory (unencrypted) database.
    ///
    /// Used exclusively in tests. Signature is unchanged so all existing test
    /// callers compile without modification.
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    /// Re-encrypt the database with a new key (key rotation).
    ///
    /// Checkpoints the WAL journal first (required by SQLCipher when WAL mode is
    /// active), then uses `PRAGMA rekey` to rewrite all pages in-place.
    /// The new key is active for all subsequent connections.
    pub fn rekey(&mut self, new_key: &[u8; 32]) -> Result<(), DbError> {
        use std::fmt::Write;

        // Checkpoint WAL so all pages are in the main file before rekey.
        // Ignore errors — if WAL isn't active this is a no-op.
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");

        let mut hex = String::with_capacity(64);
        for b in new_key {
            write!(hex, "{:02x}", b).unwrap();
        }
        let sql = format!("PRAGMA rekey = \"x'{}'\"", hex);
        self.conn.execute_batch(&sql)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn database_opens_with_wal_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal_test.db");
        let key = [0x01u8; 32];
        let db = Database::open(&path, &key).unwrap();
        let mode: String = db
            .conn()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn schema_creates_all_tables() {
        let db = Database::open_in_memory().unwrap();
        for table in &["clipboard_items", "devices", "settings", "pending_uploads"] {
            let count: i64 = db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table: {}", table);
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let key = [0x02u8; 32];
        Database::open(&path, &key).unwrap();
        Database::open(&path, &key).unwrap();
    }

    // --- SQLCipher tests ---

    #[test]
    fn encrypted_db_rejects_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("enc.db");
        let key_a = [0xAAu8; 32];
        let key_b = [0xBBu8; 32];
        Database::open(&path, &key_a).unwrap();
        let result = Database::open(&path, &key_b);
        assert!(result.is_err(), "wrong key should not open encrypted DB");
    }

    #[test]
    fn encrypted_db_round_trips_with_correct_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("enc2.db");
        let key = [0xCCu8; 32];
        {
            let db = Database::open(&path, &key).unwrap();
            db.conn()
                .execute(
                    "INSERT INTO clipboard_items \
                     (id, item_id, content_type, content, content_nonce, \
                      is_sensitive, is_synced, lamport_ts, wall_time) \
                     VALUES (?1,?2,?3,?4,?5,0,0,1,1000)",
                    rusqlite::params![
                        "test-id-1",
                        "item-id-1",
                        "text/plain",
                        b"payload" as &[u8],
                        b"nonce123456789012345678901" as &[u8],
                    ],
                )
                .unwrap();
        }
        let db2 = Database::open(&path, &key).unwrap();
        let count: i64 = db2
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn rekey_changes_encryption_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rekey.db");
        let old_key = [0x11u8; 32];
        let new_key = [0x22u8; 32];
        {
            let mut db = Database::open(&path, &old_key).unwrap();
            db.rekey(&new_key).unwrap();
        }
        assert!(Database::open(&path, &old_key).is_err());
        assert!(Database::open(&path, &new_key).is_ok());
    }

    #[test]
    fn plaintext_db_is_migrated_on_first_encrypted_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("migrate.db");
        // Create plaintext DB (simulates pre-Phase-2c database on disk)
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
            conn.execute_batch(include_str!("schema_v1.sql")).unwrap();
            conn.execute_batch("PRAGMA user_version=1;").unwrap();
        }
        let key = [0x55u8; 32];
        let db = Database::open(&path, &key).expect("migration should succeed");
        let _count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        drop(db);
        assert!(Database::open(&path, &[0x66u8; 32]).is_err());
    }

    #[test]
    fn open_in_memory_still_works_without_key() {
        let db = Database::open_in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
