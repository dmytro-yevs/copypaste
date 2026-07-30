//! Keying a connection, the per-connection pragmas, and the pool that has to
//! re-apply both to every connection it opens.
//!
//! Everything here is *per connection*, which is the whole reason it is one
//! module: none of these settings persist in the file, so the pool's `with_init`
//! and the one-off probe connection in [`super::store`] must run the same
//! sequence in the same order.

use std::time::Duration;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use zeroize::Zeroizing;

use super::model::{is_not_a_database, StoreError};

/// WAL gives many concurrent readers and one writer; four connections is what
/// the daemon needed in v1 (`CopyPaste-j8p`).
const POOL_SIZE: u32 = 4;

/// Per-connection pragmas, applied in order *after* `PRAGMA key`.
///
/// These are connection-scoped and are not persisted, so they must be
/// re-applied to every connection the pool creates.
const CONNECTION_PRAGMAS: &[&str] = &[
    // Without it a reader and the writer race instantly and surface a silent
    // SQLITE_BUSY.
    "PRAGMA busy_timeout = 5000",
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA foreign_keys = ON",
    // Keeps temp B-trees (which hold decrypted intermediates) off the disk.
    "PRAGMA temp_store = MEMORY",
    "PRAGMA wal_autocheckpoint = 1000",
    "PRAGMA journal_size_limit = 67108864",
    "PRAGMA cache_size = -8192",
];

pub(super) fn build_pool(
    manager: SqliteConnectionManager,
    db_key: [u8; 32],
    in_memory: bool,
) -> Result<Pool<SqliteConnectionManager>, StoreError> {
    // The key has to live as long as the pool: every connection the pool opens
    // later needs it. That is inherent to pooling an encrypted database.
    let manager = manager.with_init(move |conn| {
        apply_key(conn, &db_key)?;
        apply_connection_pragmas(conn)
    });

    let mut builder = Pool::builder()
        .max_size(POOL_SIZE)
        .connection_timeout(Duration::from_secs(10));
    if in_memory {
        // A shared-cache in-memory database exists only while a connection to
        // it is open, so keep one open for the life of the pool.
        builder = builder
            .min_idle(Some(1))
            .idle_timeout(None)
            .max_lifetime(None);
    }
    Ok(builder.build(manager)?)
}

/// `PRAGMA key` in SQLCipher raw-key form. **Must be the first statement on
/// every connection**, before any other pragma or query.
///
/// The `x'<64 lowercase hex>'` form means SQLCipher takes the 32 bytes as the
/// page key directly and skips PBKDF2 over a passphrase. No other cipher
/// parameter is set: the SQLCipher 4 defaults are what we want, and setting
/// `cipher_page_size` / `kdf_iter` / `cipher_hmac_algorithm` would change the
/// derived key or the page layout.
pub(super) fn apply_key(conn: &Connection, db_key: &[u8; 32]) -> rusqlite::Result<()> {
    let key_hex = Zeroizing::new(hex::encode(db_key));
    let stmt = Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", key_hex.as_str()));
    run_pragma(conn, &stmt)
}

pub(super) fn apply_connection_pragmas(conn: &Connection) -> rusqlite::Result<()> {
    for pragma in CONNECTION_PRAGMAS {
        run_pragma(conn, pragma)?;
    }
    Ok(())
}

/// Runs a pragma, tolerating the ones that return a row (`journal_mode`,
/// `busy_timeout`, …). `Connection::execute` rejects those.
pub(super) fn run_pragma(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

/// Proves the key is right before anything else touches the file. Fails closed:
/// a wrong key is an error, never a fallback to an unkeyed read.
pub(super) fn validate_key(conn: &Connection) -> Result<(), StoreError> {
    match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(e) if is_not_a_database(&e) => Err(StoreError::InvalidKey),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::test_support::{item, KEY, T0};
    use super::super::Store;
    use super::*;

    #[test]
    fn database_is_encrypted_on_disk_and_in_wal_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("clipboard.db");
        let s = Store::open(&path, &KEY).unwrap();
        s.insert(item("plaintext marker chinchilla", T0)).unwrap();
        {
            let conn = s.conn().unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "wal");
            run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        }
        drop(s);

        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
        assert!(!bytes
            .windows(b"chinchilla".len())
            .any(|w| w == b"chinchilla"));
    }
}
