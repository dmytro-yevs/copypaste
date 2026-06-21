use super::schema::{apply_migrations, SchemaError};
use crate::sensitive::init_patterns;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Migration state
// ---------------------------------------------------------------------------

/// Tracks the progress of the v4 key-version sweep through `migration_state`.
///
/// The row is keyed on `'v4-key-version-sweep'` and persists across restarts
/// so a mid-sweep crash picks up from `InProgress.last_id` rather than
/// restarting from the beginning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    /// The sweep row does not exist — schema migration ran but the sweep has
    /// never been triggered. This happens on a fresh install where every new
    /// row lands at `key_version = 2` from the start; no sweep is needed.
    NotStarted,
    /// Sweep is in progress. `last_id` is the row-id high-water mark: all
    /// rows with `rowid <= last_id` that were at `key_version = 1` have been
    /// processed (either rotated to v2 or logged as undecryptable).
    InProgress { last_id: i64 },
    /// Every `key_version = 1` row has been processed. Daemon ingest paths
    /// check for this state before inserting; while `InProgress` they return
    /// `IpcError::MigrationInProgress` instead of writing.
    Complete,
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("Schema migration error: {0}")]
    Schema(#[from] SchemaError),
    #[error("Plaintext-to-encrypted migration failed: {0}")]
    Migration(String),
    /// `PRAGMA wal_checkpoint(TRUNCATE)` could not flush the WAL within the
    /// retry budget. Surfaced as a hard error before destructive operations
    /// (e.g. `rekey`) so we never run them on a database whose WAL state
    /// disagrees with the main file.
    #[error("WAL checkpoint failed after retries: {0}")]
    CheckpointFailed(String),
    /// Underlying filesystem reported `SQLITE_FULL` (out of disk). Mapped
    /// here so callers can surface a user-actionable message instead of an
    /// opaque "sqlite error".
    #[error("Disk full")]
    DiskFull,
    /// Underlying filesystem reported `SQLITE_READONLY` (e.g. APFS snapshot,
    /// chmod 400, EROFS mount).
    #[error("Database is read-only")]
    ReadOnly,
    /// `SQLITE_BUSY` / `SQLITE_LOCKED` after the per-connection
    /// `busy_timeout` expired. Means real lock contention, not the silent
    /// instant-failure mode that the missing-pragma bug used to surface.
    #[error("Database is locked")]
    Locked,
    /// A plaintext database was found but auto-migration is disabled
    /// (`COPYPASTE_NO_AUTO_MIGRATE=1`). Run `copypaste migrate` or unset
    /// the flag to allow in-place encryption.
    #[error(
        "plaintext database found at {path} ({size} bytes) — \
         auto-migration is disabled (COPYPASTE_NO_AUTO_MIGRATE=1). \
         Back up the file and re-run the daemon without that flag to encrypt it."
    )]
    PlaintextMigrationBlocked { path: String, size: u64 },
}

/// Promote well-known operational SQLite failures (`SQLITE_FULL`,
/// `SQLITE_READONLY`, `SQLITE_BUSY`, `SQLITE_LOCKED`) to dedicated
/// `DbError` variants so callers can surface user-actionable messages
/// instead of an opaque "sqlite error". Anything else falls through to
/// the generic `Sqlite` variant.
///
/// Implemented via `From` (rather than a free function) so existing call
/// sites that use `?` on a `rusqlite::Result` keep compiling unchanged
/// while now benefiting from the richer classification.
impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            use rusqlite::ErrorCode;
            match err.code {
                ErrorCode::DiskFull => return DbError::DiskFull,
                ErrorCode::ReadOnly => return DbError::ReadOnly,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => return DbError::Locked,
                _ => {}
            }
        }
        DbError::Sqlite(e)
    }
}

/// Run `PRAGMA wal_checkpoint(TRUNCATE)` with bounded retry. A non-OK
/// checkpoint result means the WAL still contains frames that were not
/// merged into the main DB. For destructive operations (`rekey`) that's
/// not acceptable, because the source data is then split between WAL and
/// main file and `sqlcipher_export` would see only the main-file half.
///
/// We retry up to 3 times with 100 ms backoff. The per-connection
/// `busy_timeout=5000` already handles SQLITE_BUSY at the FFI layer; this
/// retry covers the case where the checkpoint *returns* OK at the FFI
/// layer but reports `busy=1` in its result row (uncommitted writer).
fn checkpoint_with_retry(conn: &Connection) -> Result<(), DbError> {
    const MAX_ATTEMPTS: u32 = 3;
    const BACKOFF: std::time::Duration = std::time::Duration::from_millis(100);

    let mut last_err: Option<String> = None;
    for attempt in 0..MAX_ATTEMPTS {
        // `PRAGMA wal_checkpoint(TRUNCATE)` returns one row:
        //   (busy, log_pages, checkpointed_pages)
        // busy = 0 means the checkpoint completed cleanly.
        let res = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        });
        match res {
            Ok((0, _, _)) => return Ok(()),
            Ok((_busy, log, ckpt)) => {
                // busy != 0 → WAL still has unmerged frames.
                last_err = Some(format!(
                    "checkpoint busy=1 (log_pages={log}, checkpointed={ckpt}) on attempt {}",
                    attempt + 1
                ));
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // WAL not active — nothing to do.
                return Ok(());
            }
            Err(e) => {
                last_err = Some(format!(
                    "checkpoint sqlite error on attempt {}: {e}",
                    attempt + 1
                ));
            }
        }
        if attempt + 1 < MAX_ATTEMPTS {
            std::thread::sleep(BACKOFF);
        }
    }
    Err(DbError::CheckpointFailed(last_err.unwrap_or_else(|| {
        "unknown checkpoint failure".to_string()
    })))
}

pub struct Database {
    conn: Connection,
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

/// Format a 32-byte key as the hex string SQLCipher expects:
///   PRAGMA key = "x'<64 hex chars>'"
///
/// Returns a `Zeroizing<String>` so the key hex is scrubbed from the heap
/// as soon as the returned value is dropped, limiting the window during
/// which plaintext key material appears in a heap dump.
fn key_pragma(key: &[u8; 32]) -> zeroize::Zeroizing<String> {
    use std::fmt::Write;
    let mut hex = zeroize::Zeroizing::new(String::with_capacity(64));
    for b in key {
        // Infallible: `fmt::Write for String` only grows a heap buffer and
        // never returns Err, so this formatted write cannot fail.
        write!(*hex, "{:02x}", b).unwrap();
    }
    zeroize::Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", *hex))
}

/// Per-connection PRAGMAs that must follow `PRAGMA key`. These are NOT
/// persisted to the database file — every fresh `Connection` must apply them
/// again. Skipping these is the root cause of two production issues:
///   * Missing `busy_timeout` ⇒ UI reader and daemon writer race instantly,
///     surfacing as silent `SQLITE_BUSY`.
///   * Missing `foreign_keys=ON` ⇒ any `ON DELETE CASCADE` FK silently no-ops.
///
/// NOTE (CopyPaste-6fd): the schema currently declares NO `ON DELETE CASCADE`
/// foreign keys. In particular `pending_uploads(item_id)` is a bare PK with no
/// FK back to `clipboard_items`, so this pragma does NOT cascade-clean it when
/// an item is hard-deleted. That cleanup is done explicitly in code by
/// `storage::items::delete_pending_uploads_for_ids`, called from every
/// hard-delete / prune / evict path. Keep `foreign_keys=ON` set anyway so any
/// future cascading FK behaves; do not rely on it for `pending_uploads`.
///
/// Keep this in sync with `pool::open_pool` and `schema::apply_migrations`
/// — every code path that opens a SQLCipher connection must apply the same
/// set so behaviour is uniform across UI reader, daemon writer, and the
/// migration pass.
///
/// The `cache_size` pragma is NOT included here because it is configurable:
/// it is applied separately via [`cache_size_pragma`] so a caller's
/// `AppConfig::sqlite_cache_mb` can be honoured. Every open path applies both
/// this static set and a `cache_size_pragma(..)` (see [`connection_pragmas`]).
pub(crate) const CONNECTION_PRAGMAS: &str = "\
PRAGMA busy_timeout = 5000;\n\
PRAGMA synchronous = NORMAL;\n\
PRAGMA foreign_keys = ON;\n\
PRAGMA temp_store = MEMORY;\n\
PRAGMA wal_autocheckpoint = 1000;\n\
PRAGMA journal_size_limit = 67108864;\n";
// wal_autocheckpoint=1000: trigger a passive checkpoint after every 1000
// WAL pages (~4 MiB at the 4 KiB default page size). Without this the WAL
// file grows without bound during the v4 migration sweep, which can write
// tens of thousands of rows in a single session. The default is 1000 pages
// (SQLite default), but explicitly setting it here ensures the pool and the
// single-connection path both see the same value (CopyPaste-ayg).
//
// journal_size_limit=64 MiB: cap the WAL file size so even if the
// checkpoint cannot shrink the file immediately (e.g. active reader holding
// a snapshot), it is truncated back to at most 64 MiB on the next successful
// checkpoint. This bounds disk usage during migration sweeps that touch many
// rows in one run.

/// Build the `PRAGMA cache_size` statement for `cache_mb` MiB of page cache.
///
/// SQLite treats a NEGATIVE `cache_size` as a memory budget in KiB, so
/// `cache_mb` MiB maps to `-(cache_mb * 1024)`. `cache_mb` is clamped to
/// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` here too (defence in depth: the
/// value normally arrives already clamped from `AppConfig::clamp_values`, but
/// callers may pass a raw value). The default (8 MiB) yields the historical
/// `PRAGMA cache_size = -8192;`.
pub(crate) fn cache_size_pragma(cache_mb: u32) -> String {
    let mb = cache_mb.clamp(
        crate::config::SQLITE_CACHE_MB_MIN,
        crate::config::SQLITE_CACHE_MB_MAX,
    );
    // mb <= 256, so mb * 1024 (<= 262_144) fits in i64 without overflow.
    let kib = i64::from(mb) * 1024;
    format!("PRAGMA cache_size = -{kib};\n")
}

/// The full per-connection pragma batch (static set + configurable cache_size).
pub(crate) fn connection_pragmas(cache_mb: u32) -> String {
    format!("{CONNECTION_PRAGMAS}{}", cache_size_pragma(cache_mb))
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
    /// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` (see [`cache_size_pragma`]).
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
        conn.execute_batch(&key_pragma(key))?;

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
                            Self::encrypt_existing(path, key)?;
                            // Re-open encrypted.
                            let enc = Connection::open_with_flags(
                                path,
                                OpenFlags::SQLITE_OPEN_READ_WRITE,
                            )?;
                            enc.execute_batch(&key_pragma(key))?;
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

    /// Like [`open`] but returns [`DbError::PlaintextMigrationBlocked`] instead
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
            conn.execute_batch(&key_pragma(key))?;
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
        // Wrapped in Zeroizing so the hex string is scrubbed from the heap
        // when `key_hex` goes out of scope (heap-dump leak fix).
        let mut raw_hex = zeroize::Zeroizing::new(String::with_capacity(64));
        for b in key {
            // Infallible: `fmt::Write for String` only grows a heap buffer and
            // never returns Err, so this formatted write cannot fail.
            write!(*raw_hex, "{:02x}", b).unwrap();
        }
        // The ATTACH SQL also contains the key hex; wrap it in Zeroizing too.
        let attach_sql = zeroize::Zeroizing::new(format!(
            "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\"",
            tmp_path.display(),
            *raw_hex
        ));

        // Open the plaintext source (no key pragma needed).
        let plaintext_conn = Connection::open(path)
            .map_err(|e| DbError::Migration(format!("open plaintext: {e}")))?;

        // ATTACH a new encrypted DB as 'encrypted'.

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

        // Crash-safety: fsync the tmp file's contents to disk BEFORE the
        // rename. Without this, a power-cut between DETACH and rename can
        // leave a zero-length destination after recovery (the rename
        // completes from the page cache, but the data pages were never
        // flushed).
        std::fs::File::open(&tmp_path)
            .and_then(|f| f.sync_all())
            .map_err(|e| DbError::Migration(format!("fsync tmp: {e}")))?;

        // Atomically replace the plaintext file with the encrypted copy.
        std::fs::rename(&tmp_path, path)
            .map_err(|e| DbError::Migration(format!("rename tmp->original: {e}")))?;

        // fsync the parent directory so the rename itself is durable. On
        // POSIX a rename is only crash-safe if the containing directory is
        // synced. Platforms that disallow fsync on a directory (Windows,
        // some FUSE setups) return EISDIR / EACCES / EINVAL — best-effort
        // only on those.
        if let Some(parent) = path.parent() {
            // An empty parent ("") means current dir — `File::open("")` errors
            // on most platforms, so guard against it.
            if !parent.as_os_str().is_empty() {
                if let Ok(dir) = std::fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }

        Ok(())
    }

    /// Open an in-memory (unencrypted) database.
    ///
    /// **Test-only (CopyPaste-9vcn).** This function creates a database WITHOUT
    /// SQLCipher encryption. The `#[cfg(any(test, feature = "test-helpers"))]`
    /// gate ensures no production build can call it accidentally and expose
    /// clipboard contents in plaintext. All production callers must use
    /// `Database::open` which requires a 32-byte encryption key.
    ///
    /// Within this crate the function is always available under `#[cfg(test)]`.
    /// External crates (e.g. copypaste-daemon's unit tests) must opt in via the
    /// `test-helpers` feature in their `[dev-dependencies]` entry for this crate.
    ///
    /// Uses the default 8 MiB cache; see
    /// [`Database::open_in_memory_with_cache_mb`] to tune it.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::open_in_memory_with_cache_mb(crate::config::SQLITE_CACHE_MB)
    }

    /// Like [`Database::open_in_memory`] but applies `cache_mb` MiB of page
    /// cache instead of the 8 MiB default.
    ///
    /// **Test-only** for the same reason as `open_in_memory` (CopyPaste-9vcn).
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
                checkpoint_with_retry(&self.conn)?;
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
        checkpoint_with_retry(&self.conn)?;

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
        enc.execute_batch(&key_pragma(new_key)).map_err(|e| {
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

impl Database {
    /// Read the current state of the v4 key-version sweep from `migration_state`.
    ///
    /// Returns `MigrationState::NotStarted` if the table row is absent (fresh
    /// install, schema just migrated), `MigrationState::Complete` if
    /// `completed_at IS NOT NULL`, or `MigrationState::InProgress { last_id }`
    /// otherwise.
    pub fn migration_state(&self) -> Result<MigrationState, DbError> {
        // Ensure the migration_state table exists (idempotent DDL).
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS migration_state (
                key                     TEXT PRIMARY KEY,
                key_version_in_progress INTEGER,
                last_processed_id       INTEGER NOT NULL DEFAULT 0,
                started_at              INTEGER,
                completed_at            INTEGER
            );",
        )?;

        let result = self.conn.query_row(
            "SELECT last_processed_id, completed_at \
             FROM migration_state WHERE key = 'v4-key-version-sweep'",
            [],
            |row| {
                let last_id: i64 = row.get(0)?;
                let completed_at: Option<i64> = row.get(1)?;
                Ok((last_id, completed_at))
            },
        );

        match result {
            Ok((_, Some(_))) => Ok(MigrationState::Complete),
            Ok((last_id, None)) => Ok(MigrationState::InProgress { last_id }),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(MigrationState::NotStarted),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// Run (or resume) the resumable v4 key-rotation sweep.
    ///
    /// Processes at most `BATCH_SIZE` rows per transaction, updates
    /// `last_processed_id` in the same transaction as the row rewrites, and
    /// sets `completed_at` on the final pass. Returns the total number of rows
    /// successfully rotated in this invocation.
    ///
    /// The sweep is idempotent: rows already at `key_version = 2` are ignored
    /// by the `WHERE key_version = 1` predicate. Calling this after
    /// `migration_state()` returns `Complete` is a no-op (returns 0).
    pub fn migration_v4_sweep_resumable(
        &self,
        v1_key: &[u8; 32],
        v2_key: &[u8; 32],
    ) -> Result<usize, DbError> {
        use super::migration_v4::{migrate_v1_to_v2_keys, BATCH_SIZE, INTER_BATCH_SLEEP};
        use rusqlite::params;

        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Ensure the table exists and the row is seeded.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS migration_state (
                key                     TEXT PRIMARY KEY,
                key_version_in_progress INTEGER,
                last_processed_id       INTEGER NOT NULL DEFAULT 0,
                started_at              INTEGER,
                completed_at            INTEGER
            );",
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'))",
            [],
        )?;

        // Short-circuit if already complete AND no key_version=1 rows remain.
        // We also check the actual row count because fresh installs are seeded
        // as Complete (no rows at schema migration time), but a test or a
        // direct SQL insert could add v1 rows afterward — we must still sweep.
        let state = self.migration_state()?;
        if state == MigrationState::Complete {
            let remaining_v1: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )?;
            if remaining_v1 == 0 {
                return Ok(0);
            }
            // State was Complete but v1 rows exist (e.g. added after a fresh
            // install). Reset to InProgress so the sweep runs.
            self.conn.execute(
                "UPDATE migration_state SET completed_at = NULL WHERE key = ?1",
                params![SWEEP_KEY],
            )?;
        }

        // Re-use the existing sweep, which processes all remaining v1 rows
        // in BATCH_SIZE batches with INTER_BATCH_SLEEP yields. We track
        // total rotated rows here and update migration_state on completion.
        let total_rotated = migrate_v1_to_v2_keys(self, v1_key, v2_key)
            .map_err(|e| DbError::Migration(e.to_string()))?;

        // Count remaining v1 rows to decide whether we're complete.
        let remaining: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;

        // `migrate_v1_to_v2_keys` is a self-bounded full pass: it loops
        // fetching `key_version = 1` batches until either none remain, a short
        // (< BATCH_SIZE) batch is processed, or a full batch rotates zero rows
        // (the termination guard). In every termination case it has ATTEMPTED
        // to rotate every row that is still at `key_version = 1` when it
        // returns. Therefore any rows remaining now are permanently
        // unrotatable (their auth tag does not verify under the current v1
        // key) — they were just tried and failed this pass.
        //
        // We mark the sweep Complete regardless of `remaining`:
        //   * remaining == 0 → every v1 row rotated cleanly (happy path).
        //   * remaining  > 0 → the leftover v1 rows are corrupt/legacy and can
        //     never be rotated. Leaving `completed_at = NULL` here would keep
        //     the write-gate armed FOREVER (the live-install bug), rejecting
        //     every new capture. The unreadable rows stay at `key_version = 1`
        //     (they were already unreadable); the gate releases so ingest
        //     resumes.
        //
        // Crash-safety / cursor-resume is preserved: we only reach this point
        // AFTER the full pass returned, so we never mark Complete before the
        // rows were attempted. A mid-pass crash leaves `completed_at = NULL`
        // and the next startup re-runs the pass from scratch (the
        // `WHERE key_version = 1` predicate is the cursor).
        let max_id: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM clipboard_items",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "UPDATE migration_state \
             SET last_processed_id = ?1, completed_at = strftime('%s','now') \
             WHERE key = ?2",
            params![max_id, SWEEP_KEY],
        )?;

        if remaining > 0 {
            tracing::warn!(
                remaining,
                "v4 migration: {remaining} key_version=1 row(s) could not be rotated \
                 (undecryptable under the current key); leaving them at key_version=1 \
                 and marking the sweep Complete so new captures are no longer gated"
            );
        }

        let _ = BATCH_SIZE; // ensure constant is referenced
        let _ = INTER_BATCH_SLEEP; // referenced by the batched inner sweep

        Ok(total_rotated)
    }

    /// Recovery helper: if the migration state is `InProgress` but there are
    /// no `key_version = 1` rows remaining, mark the sweep complete.
    ///
    /// This covers users who were seeded with an `InProgress` row (via the
    /// v6 schema migration `INSERT OR IGNORE`) on a fresh install that had
    /// zero clipboard rows — the gate was armed but could never clear itself
    /// because the sweep was never invoked. Call this after
    /// `migration_v4_sweep_resumable` returns.
    pub fn force_complete_if_no_v1_rows(&self) -> Result<(), DbError> {
        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Only act if the state is genuinely InProgress (completed_at IS NULL).
        let state = self.migration_state()?;
        if !matches!(state, MigrationState::InProgress { .. }) {
            return Ok(());
        }

        let v1_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;

        if v1_count == 0 {
            self.conn.execute(
                "UPDATE migration_state \
                 SET completed_at = strftime('%s','now') \
                 WHERE key = ?1",
                rusqlite::params![SWEEP_KEY],
            )?;
            tracing::info!(
                "force_complete_if_no_v1_rows: no v1 rows found, migration marked Complete"
            );
        }

        Ok(())
    }

    /// Escape hatch: unconditionally mark the v4 sweep Complete, clearing the
    /// write-gate even if `key_version = 1` rows remain.
    ///
    /// This is the backing primitive for the `COPYPASTE_FORCE_MIGRATION_COMPLETE`
    /// environment variable (mirrors `COPYPASTE_NO_AUTO_MIGRATE`). It exists for
    /// installs that were *already* stuck on a prior build — where the sweep
    /// logged `rotated=0 failed=N` and left `completed_at` NULL forever, so
    /// every clipboard capture was rejected with `MigrationInProgress`.
    ///
    /// Unlike [`force_complete_if_no_v1_rows`], this does NOT require zero v1
    /// rows: it seeds the sweep row if absent and sets `completed_at` no matter
    /// what. The remaining `key_version = 1` rows are left untouched (they were
    /// already unreadable under the current key); only the gate is released.
    pub fn force_migration_complete(&self) -> Result<(), DbError> {
        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Ensure the table + row exist so the UPDATE has something to hit.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS migration_state (
                key                     TEXT PRIMARY KEY,
                key_version_in_progress INTEGER,
                last_processed_id       INTEGER NOT NULL DEFAULT 0,
                started_at              INTEGER,
                completed_at            INTEGER
            );",
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'))",
            [],
        )?;
        self.conn.execute(
            "UPDATE migration_state \
             SET completed_at = strftime('%s','now') \
             WHERE key = ?1 AND completed_at IS NULL",
            rusqlite::params![SWEEP_KEY],
        )?;
        tracing::warn!(
            "force_migration_complete: write-gate force-cleared via \
             COPYPASTE_FORCE_MIGRATION_COMPLETE — any remaining key_version=1 \
             rows are left as-is (they were already unreadable)"
        );
        Ok(())
    }

    /// Count the rows still stranded at `key_version = 1` after a completed
    /// v4 sweep. These are legacy ciphertexts whose AEAD auth tag does not
    /// verify under the current v1 key (re-keyed device, lost key generation,
    /// or a pre-fix double-derivation bug). They can never be decrypted or
    /// rotated and are permanent dead weight in the database.
    ///
    /// Surfaced (not silently ignored) so the daemon can WARN with a count and
    /// point the user at the purge affordance.
    pub fn count_dead_v1_rows(&self) -> Result<usize, DbError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Permanently delete every row still stranded at `key_version = 1` — the
    /// undecryptable legacy ciphertexts that the v4 sweep could not rotate.
    ///
    /// This DESTROYS user data and is therefore opt-in only: it is the backing
    /// primitive for the `COPYPASTE_PURGE_DEAD_V1_ROWS=1` environment variable
    /// (mirrors `COPYPASTE_FORCE_MIGRATION_COMPLETE` / `COPYPASTE_NO_AUTO_MIGRATE`).
    /// The rows it removes are already permanently unreadable — there is no
    /// recoverable content — but we still gate the deletion behind an explicit
    /// flag rather than auto-deleting, per the "never delete user data without
    /// a flag" rule.
    ///
    /// Associated FTS rows are removed too so the search index stays consistent
    /// (the FTS `id` mirrors `clipboard_items.id`). Returns the number of rows
    /// deleted from `clipboard_items`.
    pub fn purge_dead_v1_rows(&self) -> Result<usize, DbError> {
        // Wrap both DELETEs in a single transaction so a crash between the two
        // cannot leave clipboard_items rows without their FTS counterparts
        // (mirrors the atomic FTS+row writes in items::insert_item_and_fts).
        // The external-content FTS5 table has no ON DELETE CASCADE, so we must
        // delete the FTS entries explicitly before removing the source rows.
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM clipboard_fts \
             WHERE id IN (SELECT id FROM clipboard_items WHERE key_version = 1)",
            [],
        )?;
        let deleted = tx.execute("DELETE FROM clipboard_items WHERE key_version = 1", [])?;
        tx.commit()?;
        if deleted > 0 {
            tracing::warn!(
                deleted,
                "purge_dead_v1_rows: permanently removed {deleted} undecryptable \
                 key_version=1 row(s) (COPYPASTE_PURGE_DEAD_V1_ROWS=1)"
            );
        }
        Ok(deleted)
    }

    /// One-time startup repair: find image/file rows that were encrypted with
    /// the v1 key but mistakenly stamped `key_version = 2` by the pre-fix
    /// writer in `daemon::handle_image` and `handle_file`.
    ///
    /// For each candidate row the function probes v1-decrypt: success means
    /// the row is mislabeled and is re-encrypted in-place with the v2 key;
    /// failure means the row is correctly v2-encrypted and is left alone.
    ///
    /// Returns the count of rows actually repaired (re-encrypted). Idempotent.
    pub fn repair_mislabeled_kv2_blob_rows(
        &self,
        v1_key: &[u8; 32],
        v2_key: &[u8; 32],
    ) -> Result<usize, DbError> {
        super::migration_v4::repair_mislabeled_kv2_blob_rows(self, v1_key, v2_key)
            .map_err(|e| DbError::Migration(e.to_string()))
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
    fn cache_size_pragma_maps_mb_to_negative_kib() {
        // Negative cache_size means "KiB of memory"; N MiB → -(N * 1024).
        assert_eq!(cache_size_pragma(8), "PRAGMA cache_size = -8192;\n");
        assert_eq!(cache_size_pragma(16), "PRAGMA cache_size = -16384;\n");
        assert_eq!(cache_size_pragma(1), "PRAGMA cache_size = -1024;\n");
    }

    #[test]
    fn cache_size_pragma_clamps_out_of_range() {
        use crate::config::{SQLITE_CACHE_MB_MAX, SQLITE_CACHE_MB_MIN};
        // 0 is below the floor → clamped up to the minimum.
        assert_eq!(
            cache_size_pragma(0),
            format!(
                "PRAGMA cache_size = -{};\n",
                i64::from(SQLITE_CACHE_MB_MIN) * 1024
            )
        );
        // A pathological value is clamped down to the ceiling.
        assert_eq!(
            cache_size_pragma(u32::MAX),
            format!(
                "PRAGMA cache_size = -{};\n",
                i64::from(SQLITE_CACHE_MB_MAX) * 1024
            )
        );
    }

    #[test]
    fn open_with_cache_mb_applies_configured_cache_size() {
        // A configured cache size is reflected in the live connection's
        // PRAGMA cache_size (negative ⇒ KiB), surviving apply_migrations which
        // would otherwise reset it to the default.
        let db = Database::open_in_memory_with_cache_mb(32).unwrap();
        let cache: i64 = db
            .conn()
            .query_row("PRAGMA cache_size", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cache, -(32 * 1024));
    }

    #[test]
    fn open_uses_default_cache_size() {
        // The plain open path keeps the historical 8 MiB (-8192 KiB) cache.
        let db = Database::open_in_memory().unwrap();
        let cache: i64 = db
            .conn()
            .query_row("PRAGMA cache_size", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cache, -(i64::from(crate::config::SQLITE_CACHE_MB) * 1024));
    }

    #[test]
    fn open_with_cache_mb_clamps_out_of_range_on_connection() {
        // An out-of-range request is clamped to the ceiling on the real conn.
        let db = Database::open_in_memory_with_cache_mb(u32::MAX).unwrap();
        let cache: i64 = db
            .conn()
            .query_row("PRAGMA cache_size", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            cache,
            -(i64::from(crate::config::SQLITE_CACHE_MB_MAX) * 1024)
        );
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
            let db = Database::open(&path, &old_key).unwrap();
            let _db = db.rekey(&new_key).unwrap();
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

    // ── Write-gate release after a stuck migration sweep (HIGH / v0.4) ─────
    //
    // Regression for the live-install bug: an install with legacy
    // `key_version = 1` rows that can NEVER be rotated (their auth tag does
    // not verify under the current v1 key) left the migration sweep logging
    // `rotated=0 failed=N` forever, `completed_at` stuck NULL, and EVERY new
    // capture rejected with `MigrationInProgress`. After a full sweep pass
    // attempts those rows and fails, the gate must release.

    /// Seed a `key_version = 1` text row whose ciphertext was produced under a
    /// DIFFERENT v1 key, so the real sweep key can never decrypt it (auth tag
    /// mismatch). These rows are the permanently-unrotatable legacy rows from
    /// the live install.
    fn seed_unrotatable_v1_text_row(db: &Database, foreign_v1_key: &[u8; 32]) {
        use crate::crypto::encrypt::{build_item_aad, encrypt_item_with_aad, AAD_SCHEMA_VERSION};
        let row_id = uuid::Uuid::new_v4().to_string();
        let item_id = uuid::Uuid::new_v4().to_string();
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) =
            encrypt_item_with_aad(b"legacy payload", foreign_v1_key, &aad).unwrap();
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
                rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
            )
            .unwrap();
    }

    /// A freshly-inserted `ClipboardItem` for the gate test. Content shape is
    /// irrelevant here — we only care that `insert_item` is no longer rejected.
    fn make_text_item() -> crate::storage::items::ClipboardItem {
        crate::storage::items::ClipboardItem::new_text(b"new capture".to_vec(), vec![0u8; 24], 1)
    }

    #[test]
    fn stuck_sweep_releases_write_gate_and_insert_succeeds() {
        let db = Database::open_in_memory().unwrap();
        // The sweep's real key.
        let v1_key = [0x10u8; 32];
        let v2_key = [0x20u8; 32];
        // Rows encrypted under a key the sweep will never have.
        let foreign = [0xFEu8; 32];

        for _ in 0..37 {
            seed_unrotatable_v1_text_row(&db, &foreign);
        }
        // Arm the gate as InProgress to model the live install precisely (the
        // v6 schema migration leaves `completed_at = NULL` for an upgrade that
        // still has key_version=1 rows). `open_in_memory` seeds it Complete
        // because the DB was empty when migrations ran; override that here.
        db.conn()
            .execute(
                "UPDATE migration_state SET completed_at = NULL \
                 WHERE key = 'v4-key-version-sweep'",
                [],
            )
            .unwrap();
        assert!(
            matches!(
                db.migration_state().unwrap(),
                MigrationState::InProgress { .. }
            ),
            "precondition: gate armed before the sweep"
        );

        // Run the sweep + the new force-complete pass.
        let rotated = db.migration_v4_sweep_resumable(&v1_key, &v2_key).unwrap();
        db.force_complete_if_no_v1_rows().unwrap();

        assert_eq!(rotated, 0, "no row was decryptable, so none may rotate");

        // (b) the gate must now read Complete even though 37 v1 rows remain.
        assert_eq!(
            db.migration_state().unwrap(),
            MigrationState::Complete,
            "gate must release after a full sweep pass attempts the unrotatable rows"
        );

        // The unrotatable rows are left at key_version=1 (still unreadable).
        let remaining_v1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining_v1, 37, "corrupt rows stay at key_version=1");

        // (c) a subsequent insert must SUCCEED (no MigrationInProgress).
        let item = make_text_item();
        crate::storage::items::insert_item(&db, &item)
            .expect("insert must succeed after the gate releases");
    }

    #[test]
    fn force_migration_complete_env_clears_a_stuck_gate() {
        // Escape hatch for already-stuck installs: even before any sweep runs,
        // COPYPASTE_FORCE_MIGRATION_COMPLETE=1 force-clears the gate.
        let db = Database::open_in_memory().unwrap();
        let foreign = [0xABu8; 32];
        for _ in 0..5 {
            seed_unrotatable_v1_text_row(&db, &foreign);
        }
        // Manually arm the gate as InProgress (the live install's state).
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO migration_state \
                 (key, key_version_in_progress, last_processed_id, started_at, completed_at) \
                 VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'), NULL)",
                [],
            )
            .unwrap();
        assert!(matches!(
            db.migration_state().unwrap(),
            MigrationState::InProgress { .. }
        ));

        db.force_migration_complete().unwrap();

        assert_eq!(
            db.migration_state().unwrap(),
            MigrationState::Complete,
            "force_migration_complete must clear the gate unconditionally"
        );
        let item = make_text_item();
        crate::storage::items::insert_item(&db, &item)
            .expect("insert must succeed after force_migration_complete");
    }

    // ── Fix A: surfacing + purging permanently-dead key_version=1 rows ─────

    #[test]
    fn count_and_purge_dead_v1_rows() {
        let db = Database::open_in_memory().unwrap();
        let foreign = [0xCDu8; 32];

        // Seed 7 undecryptable legacy rows + 1 readable v2 row (must survive).
        for _ in 0..7 {
            seed_unrotatable_v1_text_row(&db, &foreign);
        }
        let live = make_text_item();
        crate::storage::items::insert_item(&db, &live).expect("insert live v2 row");

        // count_dead_v1_rows surfaces exactly the stranded rows.
        assert_eq!(db.count_dead_v1_rows().unwrap(), 7);

        // purge removes only the v1 rows and reports the deleted count.
        let deleted = db.purge_dead_v1_rows().unwrap();
        assert_eq!(deleted, 7, "purge must delete all undecryptable v1 rows");
        assert_eq!(db.count_dead_v1_rows().unwrap(), 0, "no dead rows remain");

        // The live v2 row is untouched.
        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1, "the readable v2 row must survive the purge");

        // Purge is idempotent — a second run deletes nothing.
        assert_eq!(db.purge_dead_v1_rows().unwrap(), 0);
    }

    /// Fix 1: `purge_dead_v1_rows` must wrap both DELETEs in a single
    /// transaction so a crash between the two cannot leave items rows without
    /// their FTS counterparts. This test verifies that after a successful call
    /// the database is consistent: clipboard_items and clipboard_fts agree.
    #[test]
    fn purge_dead_v1_rows_is_atomic_fts_and_items_consistent() {
        let db = Database::open_in_memory().unwrap();
        let foreign = [0xABu8; 32];

        // Seed 3 dead v1 rows, each with a matching FTS entry.
        for _ in 0..3 {
            seed_unrotatable_v1_text_row(&db, &foreign);
        }
        let dead_ids: Vec<String> = db
            .conn()
            .prepare("SELECT id FROM clipboard_items WHERE key_version = 1")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for id in &dead_ids {
            db.conn()
                .execute(
                    "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, 'dead text')",
                    rusqlite::params![id],
                )
                .unwrap();
        }

        // After purge: clipboard_items and clipboard_fts must both be empty
        // for the purged ids (no orphan rows in either direction).
        let deleted = db.purge_dead_v1_rows().unwrap();
        assert_eq!(deleted, 3, "all 3 v1 rows must be deleted");

        let fts_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            fts_count, 0,
            "FTS must be empty after purge — no orphan rows"
        );
        let items_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(items_count, 0, "clipboard_items must be empty after purge");
    }

    #[test]
    fn purge_dead_v1_rows_removes_orphaned_fts_entries() {
        let db = Database::open_in_memory().unwrap();
        let foreign = [0xEFu8; 32];

        // Seed a dead v1 row and give it a matching FTS entry, mirroring the
        // (id, content_text) shape that insert_item writes.
        seed_unrotatable_v1_text_row(&db, &foreign);
        let dead_id: String = db
            .conn()
            .query_row(
                "SELECT id FROM clipboard_items WHERE key_version = 1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, 'stale text')",
                rusqlite::params![dead_id],
            )
            .unwrap();

        db.purge_dead_v1_rows().unwrap();

        let fts_remaining: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                rusqlite::params![dead_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_remaining, 0, "orphaned FTS entry must be purged too");
    }
}
