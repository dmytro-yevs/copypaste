mod error;
mod keying;
pub mod migration_state;
mod pragmas;

#[cfg(test)]
mod tests;

pub use error::DbError;
pub use migration_state::MigrationState;
pub(crate) use pragmas::{cache_size_pragma, connection_pragmas, CONNECTION_PRAGMAS};

use super::schema::apply_migrations;
use crate::sensitive::init_patterns;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub struct Database {
    pub(super) conn: Connection,
    /// Filesystem path the connection was opened from. Required so
    /// `rekey` can perform an atomic ATTACH-export-rename rebuild without
    /// asking the caller to re-thread the path through.
    /// `None` for `open_in_memory` connections, where `rekey` falls back
    /// to `PRAGMA rekey` (volatile DB → a crash loses everything anyway).
    path: Option<PathBuf>,
    /// Per-connection page-cache budget in MiB. Re-applied to every fresh
    /// connection this handle creates internally (e.g. the re-open after
    /// `encrypt_existing`, and the rebuilt connection in `rekey`) so the
    /// configured cache survives those operations.
    cache_mb: u32,
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
    ///
    /// Uses the default page-cache size (`SQLITE_CACHE_MB`, 8 MiB). To honour a
    /// user-configured `AppConfig::sqlite_cache_mb`, use
    /// [`Database::open_with_cache_mb`].
    pub fn open(path: impl AsRef<Path>, key: &[u8; 32]) -> Result<Self, DbError> {
        Self::open_with_cache_mb(path, key, crate::config::SQLITE_CACHE_MB)
    }

    /// Like [`Database::open`] but applies `cache_mb` MiB of SQLite page cache
    /// per connection instead of the 8 MiB default. `cache_mb` is clamped to
    /// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` (see `cache_size_pragma`).
    pub fn open_with_cache_mb(
        path: impl AsRef<Path>,
        key: &[u8; 32],
        cache_mb: u32,
    ) -> Result<Self, DbError> {
        // Eagerly compile sensitive-data patterns at first DB open so any
        // invalid regex surfaces as a startup error rather than a panic
        // during the first clipboard scan.
        if let Err(e) = init_patterns() {
            return Err(DbError::Migration(format!("pattern init failed: {e}")));
        }

        let pragmas = connection_pragmas(cache_mb);
        let path = path.as_ref();

        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        // SQLCipher requirement: key pragma MUST be the very first statement.
        conn.execute_batch(&pragmas::key_pragma(key))?;

        // Validate the key by reading the schema table.
        // With a correct key (or a newly-created empty file): returns 0 or N.
        // With a wrong key on an encrypted file: SQLCipher returns SQLITE_NOTADB.
        // With a plaintext file opened under SQLCipher+key: also SQLITE_NOTADB.
        match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        }) {
            Ok(_) => {
                // Key validated; safe to apply per-connection pragmas that
                // touch user data (foreign_keys requires reading the
                // schema). Single-connection callers (e.g. the daemon) now
                // get the same lock / FK behaviour as pooled callers.
                conn.execute_batch(&pragmas)?;
                apply_migrations(&conn)?;
                // apply_migrations re-applies the default cache_size; re-assert
                // the configured value so it wins for this connection.
                conn.execute_batch(&cache_size_pragma(cache_mb))?;
                Ok(Self {
                    conn,
                    path: Some(path.to_path_buf()),
                    cache_mb,
                })
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
                let probe = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE);
                match probe {
                    Ok(plain_conn) => {
                        let schema_ok = plain_conn
                            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                                r.get::<_, i64>(0)
                            })
                            .is_ok();
                        drop(plain_conn);
                        if schema_ok {
                            // Plaintext file confirmed.
                            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                            tracing::warn!(
                                path = %path.display(),
                                size_bytes = size,
                                "plaintext SQLite database detected; \
                                 auto-migrating to SQLCipher in-place. \
                                 Set COPYPASTE_NO_AUTO_MIGRATE=1 to block this."
                            );
                            keying::encrypt_existing(path, key)?;
                            // Re-open encrypted.
                            let enc = Connection::open_with_flags(
                                path,
                                OpenFlags::SQLITE_OPEN_READ_WRITE,
                            )?;
                            enc.execute_batch(&pragmas::key_pragma(key))?;
                            enc.execute_batch(&pragmas)?;
                            apply_migrations(&enc)?;
                            // Re-assert configured cache (apply_migrations sets
                            // the default).
                            enc.execute_batch(&cache_size_pragma(cache_mb))?;
                            Ok(Self {
                                conn: enc,
                                path: Some(path.to_path_buf()),
                                cache_mb,
                            })
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

    /// Like [`Database::open`] but returns [`DbError::PlaintextMigrationBlocked`] instead
    /// of auto-migrating when a plaintext database is found. Use this when the
    /// caller has received `COPYPASTE_NO_AUTO_MIGRATE=1` from the environment.
    ///
    /// Uses the default page-cache size; see
    /// [`Database::open_no_auto_migrate_with_cache_mb`] to tune it.
    pub fn open_no_auto_migrate(path: impl AsRef<Path>, key: &[u8; 32]) -> Result<Self, DbError> {
        Self::open_no_auto_migrate_with_cache_mb(path, key, crate::config::SQLITE_CACHE_MB)
    }

    /// Like [`Database::open_no_auto_migrate`] but applies `cache_mb` MiB of
    /// page cache per connection instead of the 8 MiB default.
    pub fn open_no_auto_migrate_with_cache_mb(
        path: impl AsRef<Path>,
        key: &[u8; 32],
        cache_mb: u32,
    ) -> Result<Self, DbError> {
        let path = path.as_ref();
        if let Err(rusqlite::Error::SqliteFailure(err, msg)) = {
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )?;
            conn.execute_batch(&pragmas::key_pragma(key))?;
            conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                r.get::<_, i64>(0)
            })
        } {
            if err.extended_code == rusqlite::ffi::SQLITE_NOTADB
                || err.code == rusqlite::ErrorCode::DatabaseCorrupt
            {
                let probe = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE);
                if let Ok(plain_conn) = probe {
                    let schema_ok = plain_conn
                        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                            r.get::<_, i64>(0)
                        })
                        .is_ok();
                    if schema_ok {
                        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                        return Err(DbError::PlaintextMigrationBlocked {
                            path: path.display().to_string(),
                            size,
                        });
                    }
                }
                return Err(DbError::Sqlite(rusqlite::Error::SqliteFailure(err, msg)));
            }
        }
        // Plaintext check passed — key is valid; delegate to regular open.
        Self::open_with_cache_mb(path, key, cache_mb)
    }

    /// Open an in-memory (unencrypted) `:memory:` database.
    ///
    /// CopyPaste-9vcn / CopyPaste-crh3.4: gated behind
    /// `#[cfg(any(test, feature = "test-helpers"))]`. The gate keeps this OUT of
    /// arbitrary external builds, but — to be accurate — it IS reachable in the
    /// **copypaste-daemon production binary**, which intentionally enables the
    /// `test-helpers` feature in its `[dependencies]` (not only `[dev-dependencies]`).
    /// Production callers are the in-place DB-recovery quiesce (`reset_database`
    /// and `db_restore`, which swap the live handle to a throwaway `:memory:` DB so
    /// the on-disk files can be replaced) and the relay's transient in-memory store.
    ///
    /// This is SAFE even though the DB is unencrypted: `Connection::open_in_memory`
    /// is a `:memory:` connection that lives only in RAM and is NEVER written to
    /// disk, so it cannot leak clipboard plaintext the way an unkeyed on-disk file
    /// would. Durable, key-protected storage still goes exclusively through
    /// [`Database::open`] (32-byte SQLCipher key). The `test-helpers` feature gates
    /// ONLY these two `:memory:` constructors — no other production-reachable
    /// surface — so enabling it in the daemon exposes nothing beyond this transient
    /// RAM database.
    ///
    /// Uses the default 8 MiB cache; see
    /// [`Database::open_in_memory_with_cache_mb`] to tune it.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::open_in_memory_with_cache_mb(crate::config::SQLITE_CACHE_MB)
    }

    /// Like [`Database::open_in_memory`] but applies `cache_mb` MiB of page
    /// cache instead of the 8 MiB default. Same gating + production-reachability
    /// (transient `:memory:`, no disk leak) as [`Database::open_in_memory`]
    /// (CopyPaste-9vcn / CopyPaste-crh3.4).
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn open_in_memory_with_cache_mb(cache_mb: u32) -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(&connection_pragmas(cache_mb))?;
        apply_migrations(&conn)?;
        // apply_migrations re-applies the default cache_size; re-assert.
        conn.execute_batch(&cache_size_pragma(cache_mb))?;
        Ok(Self {
            conn,
            path: None,
            cache_mb,
        })
    }

    /// Re-encrypt the database with a new key (key rotation).
    ///
    /// Consumes `self` so the caller cannot use a half-rekeyed handle on
    /// failure — the only way to continue is via the returned `Result<Self>`.
    ///
    /// **Why we do NOT use `PRAGMA rekey`**: `rekey` rewrites the database
    /// pages in-place. If the process is interrupted (power cut, panic,
    /// SIGKILL) mid-rewrite, the file ends up with a mix of old-key and
    /// new-key pages — neither key can open it, and there is no automatic
    /// recovery.
    ///
    /// Instead we mirror the `encrypt_existing` migration pattern:
    ///   1. Force `wal_checkpoint(TRUNCATE)` with retry. Silently
    ///      swallowing this would let the WAL diverge from the rebuilt
    ///      file and corrupt the result.
    ///   2. ATTACH a fresh tmp database under the NEW key.
    ///   3. `sqlcipher_export` the live data into the tmp.
    ///   4. Carry the source `user_version` across to the tmp (the export
    ///      copies tables but not pragmas).
    ///   5. DETACH, close the source connection (drop self), fsync tmp,
    ///      atomic `rename` over the original, fsync parent dir.
    ///   6. Re-open under the new key and return the new `Database`.
    ///
    /// Crash-safety: at every point a power-cut leaves *either* the old
    /// file (old key still works) or the new file (new key works), never
    /// a half-rekeyed file.
    pub fn rekey(self, new_key: &[u8; 32]) -> Result<Self, DbError> {
        use std::fmt::Write;

        // Preserve the configured page-cache size across the rebuild. `self`
        // is dropped mid-method (to release the file for rename), so capture
        // it up front.
        let cache_mb = self.cache_mb;

        // In-memory connections have no path to atomically rename onto;
        // fall back to PRAGMA rekey for those. They're crash-safe by
        // virtue of being volatile — a crash loses the whole DB anyway.
        let path = match self.path.clone() {
            Some(p) => p,
            None => {
                keying::checkpoint_with_retry(&self.conn)?;
                // Wrap hex in Zeroizing so key material is scrubbed on drop.
                let mut hex = zeroize::Zeroizing::new(String::with_capacity(64));
                for b in new_key {
                    // Infallible: `fmt::Write for String` only grows a heap
                    // buffer and never returns Err, so this write cannot fail.
                    write!(*hex, "{:02x}", b).unwrap();
                }
                let sql = zeroize::Zeroizing::new(format!("PRAGMA rekey = \"x'{}'\"", *hex));
                self.conn.execute_batch(&sql)?;
                return Ok(Self {
                    conn: self.conn,
                    path: None,
                    cache_mb,
                });
            }
        };

        // Step 1: force the WAL into the main file. Failing this would
        // leave source data split between WAL and main, and the
        // sqlcipher_export below would see only the main-file half.
        keying::checkpoint_with_retry(&self.conn)?;

        // Step 2-3: ATTACH new-key tmp and export.
        let tmp_path = path.with_extension("db.rekey-tmp");
        let _ = std::fs::remove_file(&tmp_path);

        // Wrap hex in Zeroizing so key material is scrubbed from the heap on drop.
        let mut new_hex = zeroize::Zeroizing::new(String::with_capacity(64));
        for b in new_key {
            // Infallible: `fmt::Write for String` only grows a heap buffer and
            // never returns Err, so this formatted write cannot fail.
            write!(*new_hex, "{:02x}", b).unwrap();
        }
        let attach_sql = zeroize::Zeroizing::new(format!(
            "ATTACH DATABASE '{}' AS rekeyed KEY \"x'{}'\"",
            tmp_path.display(),
            *new_hex
        ));
        self.conn
            .execute_batch(&attach_sql)
            .map_err(|e| DbError::Migration(format!("ATTACH rekeyed: {e}")))?;
        self.conn
            .execute_batch("SELECT sqlcipher_export('rekeyed')")
            .map_err(|e| DbError::Migration(format!("sqlcipher_export(rekey): {e}")))?;

        // Step 4: sqlcipher_export copies tables/indexes/triggers but NOT
        // the user_version pragma. Carry it across explicitly so the
        // re-open below doesn't think the rebuilt DB is at v0 and try to
        // re-run every ALTER TABLE (which would fail with "duplicate
        // column").
        let src_version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| DbError::Migration(format!("read user_version: {e}")))?;
        self.conn
            .execute_batch(&format!("PRAGMA rekeyed.user_version = {src_version};"))
            .map_err(|e| DbError::Migration(format!("set rekeyed.user_version: {e}")))?;

        self.conn
            .execute_batch("DETACH DATABASE rekeyed")
            .map_err(|e| DbError::Migration(format!("DETACH rekeyed: {e}")))?;

        // Step 5a: close the live conn by dropping self so the OS will let
        // us rename onto its file (Windows).
        drop(self);

        // Step 5b: fsync tmp → rename → fsync parent dir.
        std::fs::File::open(&tmp_path)
            .and_then(|f| f.sync_all())
            .map_err(|e| DbError::Migration(format!("fsync rekey tmp: {e}")))?;
        std::fs::rename(&tmp_path, &path)
            .map_err(|e| DbError::Migration(format!("rename rekey tmp->original: {e}")))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Ok(dir) = std::fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }

        // Step 6: re-open under the new key. Mirrors the happy-path of
        // `Self::open` but skips the plaintext-detection branch since we
        // just wrote a properly-encrypted file.
        // Embed `path` in every error here so callers can recover or report
        // a meaningful location when the rebuilt file cannot be re-opened
        // (e.g. permissions changed by rename, or disk full after fsync).
        let path_str = path.display().to_string();
        let enc = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|e| DbError::Migration(format!("rekey re-open failed at {path_str}: {e}")))?;
        enc.execute_batch(&pragmas::key_pragma(new_key))
            .map_err(|e| {
                DbError::Migration(format!("rekey key-pragma failed at {path_str}: {e}"))
            })?;
        // Validate the new key actually opens the rebuilt file.
        enc.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| {
            DbError::Migration(format!("rekey key-validation failed at {path_str}: {e}"))
        })?;
        enc.execute_batch(&connection_pragmas(cache_mb))
            .map_err(|e| DbError::Migration(format!("rekey pragmas failed at {path_str}: {e}")))?;
        apply_migrations(&enc).map_err(|e| {
            DbError::Migration(format!("rekey migrations failed at {path_str}: {e}"))
        })?;
        // apply_migrations re-applies the default cache_size; re-assert.
        enc.execute_batch(&cache_size_pragma(cache_mb))
            .map_err(|e| {
                DbError::Migration(format!("rekey cache pragma failed at {path_str}: {e}"))
            })?;

        Ok(Self {
            conn: enc,
            path: Some(path),
            cache_mb,
        })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Return the filesystem path this connection was opened from, if any.
    ///
    /// `None` for in-memory databases (test helpers). Used by the `db_restore`
    /// IPC handler to locate the live DB file for safe file-copy restore.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }
}

/// `Database` implements [`crate::storage::pool::DbRead`] so that the same
/// read-only storage functions (e.g. `get_page`, `search_items`) can accept
/// either the single writer handle or a pooled [`crate::storage::pool::ReadHandle`]
/// without code duplication.
impl crate::storage::pool::DbRead for Database {
    fn conn(&self) -> &Connection {
        &self.conn
    }
}
