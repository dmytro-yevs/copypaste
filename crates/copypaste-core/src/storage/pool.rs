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
//!
//! # Read-pool concurrency (CopyPaste-j8p)
//!
//! SQLite WAL mode supports multiple simultaneous readers on the same database
//! file, even while a writer holds the write lock. The `DbRead` trait and
//! `ReadHandle` type expose this: read-only storage functions accept
//! `&impl DbRead`, so callers can supply either the single `Database` writer
//! (backward compatible) **or** a pooled `ReadHandle` that does not compete
//! with writes. The daemon routes `list`/`count`/`search`/`history_page`/
//! `stats` through a 4-connection `SqlitePool`, eliminating mutex contention
//! for the hot read path.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::fmt::Write as FmtWrite;
use std::path::Path;
use thiserror::Error;
use zeroize::Zeroizing;

/// A pool of SQLCipher-encrypted SQLite connections.
pub type SqlitePool = Pool<SqliteConnectionManager>;

/// Shared interface for anything that can supply a `&rusqlite::Connection`.
///
/// Implemented by:
/// * [`super::db::Database`] — the single write connection (backward compat).
/// * [`ReadHandle`] — a connection drawn from an [`SqlitePool`].
///
/// Read-only storage functions accept `&impl DbRead` so they can serve
/// requests from either path without code duplication.
pub trait DbRead {
    /// Return a shared reference to the underlying SQLite connection.
    fn conn(&self) -> &rusqlite::Connection;
}

/// A pooled, read-only database connection.
///
/// Wraps an [`r2d2::PooledConnection`] obtained from [`SqlitePool::get()`].
/// Implements [`DbRead`] so read-only storage functions accept it directly.
/// The connection is returned to the pool when this value is dropped.
///
/// In WAL mode (which every connection in the pool enables via its `with_init`
/// hook) multiple `ReadHandle`s can coexist simultaneously — they do not block
/// each other or the single writer.
pub struct ReadHandle(pub r2d2::PooledConnection<SqliteConnectionManager>);

impl DbRead for ReadHandle {
    fn conn(&self) -> &rusqlite::Connection {
        // `r2d2::PooledConnection<SqliteConnectionManager>` implements
        // `Deref<Target = rusqlite::Connection>`, so `&self.0` coerces
        // directly to `&rusqlite::Connection`.
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("failed to build r2d2 pool: {0}")]
    Build(#[from] r2d2::Error),
    /// A rusqlite error occurred during the schema-version pre-flight check
    /// (before the pool was built).
    #[error("pool pre-flight SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The database at `path` has not been migrated yet (`user_version = 0`).
    ///
    /// Call `Database::open()` on the same path first to apply schema
    /// migrations, then open the pool.  This guard prevents silent
    /// "no such table" errors when a pool connection is used on a brand-new
    /// or uninitialized SQLCipher file.
    #[error(
        "database schema not initialized (user_version = 0); \
         call Database::open() to run migrations before opening a pool"
    )]
    SchemaNotInitialized,
}

/// Format a 32-byte key as the hex string SQLCipher expects in a `PRAGMA key`
/// statement: `x'<64 hex chars>'`.
///
/// Returns a [`Zeroizing<String>`] so the hex key material is scrubbed from
/// the heap on drop (mirrors `db::key_pragma`).
fn key_hex(key: &[u8; 32]) -> Zeroizing<String> {
    let mut hex = String::with_capacity(64);
    for b in key {
        // Infallible: `fmt::Write for String` never returns Err (it only grows
        // the heap buffer), so writing a formatted byte cannot fail here.
        write!(hex, "{:02x}", b).unwrap();
    }
    Zeroizing::new(hex)
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
///
/// Uses the default page-cache size (`SQLITE_CACHE_MB`, 8 MiB per connection).
/// To honour a configured `AppConfig::sqlite_cache_mb`, use
/// [`open_pool_with_cache_mb`].
pub fn open_pool(path: &Path, key: &[u8; 32], max_size: u32) -> Result<SqlitePool, PoolError> {
    open_pool_with_cache_mb(path, key, max_size, crate::config::SQLITE_CACHE_MB)
}

/// Like [`open_pool`] but applies `cache_mb` MiB of page cache per connection
/// instead of the 8 MiB default. `cache_mb` is clamped to
/// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` (via
/// `crate::storage::db::cache_size_pragma`).
pub fn open_pool_with_cache_mb(
    path: &Path,
    key: &[u8; 32],
    max_size: u32,
    cache_mb: u32,
) -> Result<SqlitePool, PoolError> {
    let key_hex = key_hex(key);
    // `cache_size_pragma` clamps and formats the negative-KiB cache_size
    // statement, keeping the value in sync with the single-connection path.
    let cache_pragma = crate::storage::db::cache_size_pragma(cache_mb);
    // Build the full PRAGMA string inside a Zeroizing buffer so the key hex
    // material in the PRAGMA string is also scrubbed from the heap on drop.
    // The per-connection pragmas are sourced from `db::CONNECTION_PRAGMAS` so
    // both the single-connection path and the pool path stay in sync — a diff
    // to one will be visible in the other.
    let pragma_str: Zeroizing<String> = Zeroizing::new(format!(
        "PRAGMA key = \"x'{}'\";\nPRAGMA journal_mode = WAL;\n{}{}",
        key_hex.as_str(),
        crate::storage::db::CONNECTION_PRAGMAS,
        cache_pragma.trim_end()
    ));

    // Schema-version pre-flight: open a single throw-away connection to check
    // `PRAGMA user_version` before building the pool.  This catches the common
    // mistake of calling `open_pool()` before `Database::open()` has been used
    // to apply migrations, turning a silent "no such table: clipboard_items"
    // deep inside the application into an immediate, descriptive error here.
    //
    // We read user_version on a fresh connection (applying the key + WAL
    // pragmas first, same as the pool's init hook) so that the check works on
    // an encrypted SQLCipher file.  The connection is dropped before the pool
    // is built; no resources are leaked.
    {
        let probe = rusqlite::Connection::open(path)?;
        probe.execute_batch(pragma_str.as_str())?;
        let user_version: i64 = probe.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if user_version == 0 {
            return Err(PoolError::SchemaNotInitialized);
        }
    }

    let manager = SqliteConnectionManager::file(path).with_init(move |conn| {
        // SQLCipher requirement: key pragma MUST be the very first statement
        // executed on a fresh connection. WAL mode applies to the database
        // file (not the connection) but is idempotent and safe to set here.
        //
        // The remaining pragmas are per-connection (not persisted in the
        // file) and MUST be re-applied each time the pool hands out a fresh
        // connection — otherwise UI reader / daemon writer races surface as
        // silent `SQLITE_BUSY` and foreign-key checks silently no-op.
        // Built from `db::CONNECTION_PRAGMAS` above — single source of truth.
        conn.execute_batch(pragma_str.as_str())
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

    /// Fix 2: `key_hex` must return a `Zeroizing<String>` so the hex key
    /// material is scrubbed from heap on drop. This test asserts the return
    /// type is `Zeroizing<String>` (compile-time check) and that the content
    /// is the expected hex string (runtime check).
    #[test]
    fn key_hex_returns_zeroizing_string() {
        let key = [0xABu8; 32];
        let hex: zeroize::Zeroizing<String> = key_hex(&key);
        assert_eq!(hex.len(), 64, "key_hex must produce 64 hex chars");
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "key_hex output must be all hex digits"
        );
        assert_eq!(&hex[..4], "abab", "first bytes must be lower-case hex 'ab'");
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

    /// CopyPaste-44rq.63: opening a pool on a SQLCipher file that has not had
    /// `Database::open()` run yet (i.e. `user_version = 0`) must return
    /// `PoolError::SchemaNotInitialized` rather than silently succeeding and
    /// failing later with "no such table: clipboard_items".
    #[test]
    fn pool_rejects_uninitialized_schema() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("uninit.db");
        let key = [0x11u8; 32];

        // Create a raw encrypted SQLCipher file with the key applied but
        // WITHOUT running any migrations (user_version stays at 0).
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let kh = key_hex(&key);
            conn.execute_batch(&format!(
                "PRAGMA key = \"x'{}'\";\nPRAGMA journal_mode = WAL;",
                kh.as_str()
            ))
            .unwrap();
            // Do not call Database::open() — leave user_version = 0.
        }

        let result = open_pool(&path, &key, 2);
        assert!(
            matches!(result, Err(PoolError::SchemaNotInitialized)),
            "expected SchemaNotInitialized, got {:?}",
            result
        );
    }

    /// CopyPaste-j8p: 8 threads each acquire a `ReadHandle` concurrently and
    /// run a SELECT.  In WAL mode all reads should proceed in parallel without
    /// deadlock.  Correctness: every thread must get a valid count (≥ 0).
    ///
    /// This test also exercises the `DbRead` trait path: `ReadHandle::conn()`
    /// is called via the trait through `count_items`, which accepts `&impl DbRead`.
    #[test]
    fn read_handle_concurrent_reads_dont_deadlock() {
        use crate::storage::items::count_items;

        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent_reads.db");
        let key = [0x55u8; 32];
        bootstrap_db(&path, &key);

        // Open the pool with 8 slots so all 8 threads can hold a connection
        // simultaneously.  `pool_supports_concurrent_connections` already
        // tested the 4-thread case; here we verify the DbRead trait path.
        let pool = Arc::new(open_pool(&path, &key, 8).unwrap());

        let mut handles = Vec::new();
        for i in 0..8 {
            let pool = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                let conn = pool.get().expect("acquire pooled conn");
                // Wrap in ReadHandle and go through the DbRead trait.
                let handle = ReadHandle(conn);
                // count_items accepts &impl DbRead — uses ReadHandle::conn()
                let count = count_items(&handle).unwrap_or(-1);
                assert!(
                    count >= 0,
                    "thread {i}: count_items through ReadHandle returned negative ({count})"
                );
            }));
        }
        for h in handles {
            h.join().expect("concurrent read thread must not panic");
        }
    }
}
