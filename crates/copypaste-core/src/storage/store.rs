//! The [`Store`] handle: what it takes to get a keyed, schema-validated, pooled
//! connection, and nothing about what is then done with it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use zeroize::Zeroizing;

use super::connection::{build_pool, run_pragma};
use super::model::StoreError;
use super::schema::create;

/// The clipboard store.
///
/// Cheap to clone (the pool is reference-counted) and safe to share across
/// threads.
#[derive(Clone)]
pub struct Store {
    pool: Pool<SqliteConnectionManager>,
    pub(super) path: Option<Arc<PathBuf>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately opaque: the pool's own Debug would print the database
        // path, and a path discloses the local username.
        f.write_str("Store { .. }")
    }
}

impl Store {
    /// Opens the database at `path`, creating the canonical schema only when
    /// the file did not exist. The parent directory must already exist.
    ///
    /// `db_key` is the raw 32-byte SQLCipher key. A key that does not open an
    /// existing file yields [`StoreError::InvalidKey`]; there is no fallback
    /// read and no unkeyed plaintext probe.
    pub fn open(path: &Path, db_key: &[u8; 32]) -> Result<Self, StoreError> {
        let db_key = Zeroizing::new(*db_key);
        let flags =
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = if path.try_exists()? {
            super::dbfile::open_validated(path, &db_key)?
        } else {
            super::creation::create_and_publish(path, &db_key)?;
            super::dbfile::open_validated(path, &db_key)?
        };
        // A restart onto a populated history would otherwise plan every query
        // from default guesses until the pool first retires a connection.
        let _ = run_pragma(&conn, "PRAGMA optimize");
        drop(conn);

        let manager = SqliteConnectionManager::file(path).with_flags(flags);
        let pool = build_pool(manager, db_key, false)?;
        Ok(Self {
            pool,
            path: Some(Arc::new(path.to_owned())),
        })
    }

    /// Opens a private in-memory database. A named shared-cache URI, because
    /// `SqliteConnectionManager::memory()` would give every pooled connection
    /// its *own* empty database; one connection is held permanently idle so the
    /// database is not dropped when the pool goes quiet.
    pub fn open_in_memory(db_key: &[u8; 32]) -> Result<Self, StoreError> {
        let db_key = Zeroizing::new(*db_key);
        let uri = format!(
            "file:copypaste-{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let manager = SqliteConnectionManager::file(&uri).with_flags(
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        );
        let pool = build_pool(manager, db_key, true)?;
        let mut conn = pool.get()?;
        create(&mut conn)?;
        drop(conn);
        Ok(Self { pool, path: None })
    }

    /// Checks a connection out of the pool. `pub(super)` so the query modules
    /// share one pool without exposing it to callers.
    pub(super) fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, StoreError> {
        Ok(self.pool.get()?)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::test_support::{item, store, KEY, OTHER_KEY, T0};
    use super::*;

    #[test]
    fn file_backed_store_round_trips_and_rejects_a_wrong_key() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        let id = {
            let s = Store::open(&path, &KEY).unwrap();
            let stored = s.insert(item("persisted payload", T0)).unwrap();
            assert_eq!(s.count().unwrap(), 1);
            stored.id
        };

        // Re-opening with the right key validates the schema and keeps the data.
        let s = Store::open(&path, &KEY).unwrap();
        assert_eq!(
            s.get(&id).unwrap().unwrap().content_ciphertext,
            b"ct:persisted payload"
        );
        assert_eq!(s.search("persisted", 10).unwrap().len(), 1);
        drop(s);

        // The wrong key must fail closed — never a fallback read.
        let err = Store::open(&path, &OTHER_KEY).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidKey),
            "expected InvalidKey, got {err:?}"
        );

        // And the error must not leak the path (it discloses the username).
        let rendered = err.to_string();
        assert!(!rendered.contains(&*path.to_string_lossy()));
        assert!(!rendered.contains("copypaste-v2.db"));
    }

    #[test]
    fn in_memory_pool_shares_one_database() {
        // Every pooled connection must see the same in-memory database, so hold
        // one while working through another.
        let s = store();
        let held = s.conn().unwrap();
        let stored = s.insert(item("across connections", T0)).unwrap();
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.get(&stored.id).unwrap().is_some());
        drop(held);
    }
}
