//! r2d2 connection pool for SQLCipher-encrypted SQLite databases.
//!
//! Foundation for beta-arch-4: Phase 1 of the daemon migration from a single
//! `Mutex<Connection>` to a pooled architecture. This module introduces the
//! `SqlitePool` type and `open_pool()` constructor; the existing `Database`
//! API in `db.rs` is intentionally left unchanged so current callsites
//! continue to compile. Daemon wiring will migrate in Wave 3.1.
//!
//! Each connection drawn from the pool has the SQLCipher key applied and
//! `journal_mode=WAL` enabled before it can be used, via r2d2_sqlite's
//! `with_init` hook.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::fmt::Write as FmtWrite;
use std::path::Path;
use thiserror::Error;

/// A pool of SQLCipher-encrypted SQLite connections.
pub type SqlitePool = Pool<SqliteConnectionManager>;

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("failed to build r2d2 pool: {0}")]
    Build(#[from] r2d2::Error),
}

/// Format a 32-byte key as the hex string SQLCipher expects in a `PRAGMA key`
/// statement: `x'<64 hex chars>'`.
fn key_hex(key: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(64);
    for b in key {
        write!(hex, "{:02x}", b).unwrap();
    }
    hex
}

/// Open (or create) an r2d2 pool of SQLCipher connections at `path`.
///
/// `key` is the 32-byte AES-256 SQLCipher key (typically the device-local
/// enc key). `max_size` is the maximum number of connections the pool will
/// hold open.
///
/// Each connection acquired from the pool will have these PRAGMAs applied
/// before it is handed back to the caller:
///   * `PRAGMA key = "x'<hex>'"` — required FIRST statement for SQLCipher
///   * `PRAGMA journal_mode = WAL` — concurrent readers + single writer
///   * `PRAGMA busy_timeout = 5000` — wait up to 5 s on lock contention
///     instead of returning `SQLITE_BUSY` immediately. Without this, the UI
///     reader and the daemon writer race the first time they touch the file
///     and either side may see silent `SQLITE_BUSY` errors.
///   * `PRAGMA synchronous = NORMAL` — WAL-safe durability with better
///     write throughput than the default `FULL`.
///   * `PRAGMA foreign_keys = ON` — must be re-enabled per connection;
///     SQLite defaults to OFF and the setting is NOT persisted to the file.
///   * `PRAGMA temp_store = MEMORY` — keep temp B-trees off disk so
///     plaintext intermediates never hit the filesystem.
///
/// Note: this constructor does **not** run schema migrations. Either run
/// `Database::open()` once first on the same path (it will apply migrations),
/// or call the schema module directly on a borrowed connection.
pub fn open_pool(path: &Path, key: &[u8; 32], max_size: u32) -> Result<SqlitePool, PoolError> {
    let key_hex = key_hex(key);
    let manager = SqliteConnectionManager::file(path).with_init(move |conn| {
        // SQLCipher requirement: key pragma MUST be the very first statement
        // executed on a fresh connection. WAL mode applies to the database
        // file (not the connection) but is idempotent and safe to set here.
        //
        // The remaining pragmas are per-connection (not persisted in the
        // file) and MUST be re-applied each time the pool hands out a fresh
        // connection — otherwise UI reader / daemon writer races surface as
        // silent `SQLITE_BUSY` and foreign-key checks silently no-op.
        // Kept in sync with `db::CONNECTION_PRAGMAS`.
        conn.execute_batch(&format!(
            "PRAGMA key = \"x'{key_hex}'\";\n\
             PRAGMA journal_mode = WAL;\n\
             PRAGMA busy_timeout = 5000;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA foreign_keys = ON;\n\
             PRAGMA temp_store = MEMORY;"
        ))
    });

    let pool = Pool::builder()
        .max_size(max_size)
        .build(manager)
        .map_err(PoolError::Build)?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    /// Create a SQLCipher DB at `path` with schema applied, using the
    /// existing `Database` ctor. Returns once the file is closed.
    fn bootstrap_db(path: &Path, key: &[u8; 32]) {
        let _db = Database::open(path, key).expect("bootstrap open");
        // dropped here; file persists with schema + encryption
    }

    #[test]
    fn pool_opens_with_correct_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pool_ok.db");
        let key = [0x42u8; 32];
        bootstrap_db(&path, &key);

        let pool = open_pool(&path, &key, 4).expect("pool builds");
        let conn = pool.get().expect("acquire conn");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
            .expect("read sqlite_master with correct key");
        assert!(count > 0, "schema should expose at least one master row");
    }

    #[test]
    fn pool_rejects_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pool_wrong.db");
        let key_a = [0xAAu8; 32];
        let key_b = [0xBBu8; 32];
        bootstrap_db(&path, &key_a);

        // Pool builder eagerly opens one connection to validate; with a wrong
        // key SQLCipher will return SQLITE_NOTADB during the validation read.
        let result = open_pool(&path, &key_b, 2);
        if let Ok(pool) = result {
            // Some r2d2 versions defer validation until first get(); in that
            // case the query must fail.
            let conn = pool.get();
            match conn {
                Ok(c) => {
                    let res: Result<i64, _> =
                        c.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0));
                    assert!(
                        res.is_err(),
                        "wrong key must not be able to read sqlite_master"
                    );
                }
                Err(_) => {
                    // Pool refused to hand out a working connection — acceptable.
                }
            }
        }
        // If `open_pool` returned Err immediately that's also a pass.
    }

    #[test]
    fn pool_supports_concurrent_connections() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pool_concurrent.db");
        let key = [0x77u8; 32];
        bootstrap_db(&path, &key);

        let pool = Arc::new(open_pool(&path, &key, 4).unwrap());

        // Spawn 4 threads, each grabs a connection and runs a read.
        let mut handles = Vec::new();
        for i in 0..4 {
            let pool = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                let conn = pool.get().expect("acquire conn");
                let mode: String = conn
                    .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                    .expect("journal_mode read");
                assert_eq!(mode, "wal", "thread {i} expected wal mode");
            }));
        }
        for h in handles {
            h.join().expect("thread join");
        }

        // Sanity: state() reports >0 connections after concurrent use.
        let state = pool.state();
        assert!(
            state.connections >= 1,
            "pool should have warmed >=1 connection, got {}",
            state.connections
        );
    }

    #[test]
    fn pool_pragmas_applied() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pool_pragmas.db");
        let key = [0x33u8; 32];
        bootstrap_db(&path, &key);

        let pool = open_pool(&path, &key, 2).unwrap();
        let conn = pool.get().unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            mode, "wal",
            "WAL journal mode must be applied via with_init"
        );

        // SQLCipher version pragma round-trips only on encrypted connections.
        let cipher_version: Result<String, _> =
            conn.query_row("PRAGMA cipher_version", [], |r| r.get(0));
        assert!(
            cipher_version.is_ok() && !cipher_version.unwrap().is_empty(),
            "cipher_version pragma should return non-empty on a SQLCipher build"
        );
    }
}
