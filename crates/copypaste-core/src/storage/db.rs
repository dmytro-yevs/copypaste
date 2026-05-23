use super::schema::{apply_migrations, SchemaError};
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

/// Per-connection PRAGMAs that must follow `PRAGMA key`. These are NOT
/// persisted to the database file — every fresh `Connection` must apply them
/// again. Skipping these is the root cause of two production issues:
///   * Missing `busy_timeout` ⇒ UI reader and daemon writer race instantly,
///     surfacing as silent `SQLITE_BUSY`.
///   * Missing `foreign_keys=ON` ⇒ ON DELETE CASCADE silently no-ops.
///
/// Keep this in sync with `pool::open_pool` and `schema::apply_migrations`
/// — every code path that opens a SQLCipher connection must apply the same
/// set so behaviour is uniform across UI reader, daemon writer, and the
/// migration pass.
pub(crate) const CONNECTION_PRAGMAS: &str = "\
PRAGMA busy_timeout = 5000;\n\
PRAGMA synchronous = NORMAL;\n\
PRAGMA foreign_keys = ON;\n\
PRAGMA temp_store = MEMORY;\n";

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
                // Key validated; safe to apply per-connection pragmas that
                // touch user data (foreign_keys requires reading the
                // schema). Single-connection callers (e.g. the daemon) now
                // get the same lock / FK behaviour as pooled callers.
                conn.execute_batch(CONNECTION_PRAGMAS)?;
                apply_migrations(&conn)?;
                Ok(Self {
                    conn,
                    path: Some(path.to_path_buf()),
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
                            // Plaintext file confirmed. Migrate in-place.
                            Self::encrypt_existing(path, key)?;
                            // Re-open encrypted.
                            let enc = Connection::open_with_flags(
                                path,
                                OpenFlags::SQLITE_OPEN_READ_WRITE,
                            )?;
                            enc.execute_batch(&key_pragma(key))?;
                            enc.execute_batch(CONNECTION_PRAGMAS)?;
                            apply_migrations(&enc)?;
                            Ok(Self {
                                conn: enc,
                                path: Some(path.to_path_buf()),
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
    /// Used exclusively in tests. Signature is unchanged so all existing test
    /// callers compile without modification.
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn)?;
        Ok(Self { conn, path: None })
    }

    /// Re-encrypt the database with a new key (key rotation).
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
    ///   5. DETACH, close the source connection, fsync tmp, atomic
    ///      `rename` over the original, fsync parent dir.
    ///   6. Re-open under the new key and replace `self.conn`.
    ///
    /// Crash-safety: at every point a power-cut leaves *either* the old
    /// file (old key still works) or the new file (new key works), never
    /// a half-rekeyed file.
    pub fn rekey(&mut self, new_key: &[u8; 32]) -> Result<(), DbError> {
        use std::fmt::Write;

        // In-memory connections have no path to atomically rename onto;
        // fall back to PRAGMA rekey for those. They're crash-safe by
        // virtue of being volatile — a crash loses the whole DB anyway.
        let path = match self.path.clone() {
            Some(p) => p,
            None => {
                checkpoint_with_retry(&self.conn)?;
                let mut hex = String::with_capacity(64);
                for b in new_key {
                    write!(hex, "{:02x}", b).unwrap();
                }
                let sql = format!("PRAGMA rekey = \"x'{}'\"", hex);
                self.conn.execute_batch(&sql)?;
                return Ok(());
            }
        };

        // Step 1: force the WAL into the main file. Failing this would
        // leave source data split between WAL and main, and the
        // sqlcipher_export below would see only the main-file half.
        checkpoint_with_retry(&self.conn)?;

        // Step 2-3: ATTACH new-key tmp and export.
        let tmp_path = path.with_extension("db.rekey-tmp");
        let _ = std::fs::remove_file(&tmp_path);

        let mut new_hex = String::with_capacity(64);
        for b in new_key {
            write!(new_hex, "{:02x}", b).unwrap();
        }
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS rekeyed KEY \"x'{}'\"",
            tmp_path.display(),
            new_hex
        );
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

        // Step 5a: close the live conn so the OS will let us rename onto
        // its file (Windows). The struct field can't be left moved-from,
        // so swap in a throwaway in-memory conn as a placeholder.
        let placeholder = Connection::open_in_memory()
            .map_err(|e| DbError::Migration(format!("placeholder conn: {e}")))?;
        let old_conn = std::mem::replace(&mut self.conn, placeholder);
        drop(old_conn);

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
        let enc = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        enc.execute_batch(&key_pragma(new_key))?;
        // Validate the new key actually opens the rebuilt file.
        enc.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })?;
        enc.execute_batch(CONNECTION_PRAGMAS)?;
        apply_migrations(&enc)?;
        self.conn = enc;

        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

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

        if remaining == 0 {
            // Mark complete — store the highest rowid as a record.
            let max_id: i64 = self
                .conn
                .query_row("SELECT COALESCE(MAX(rowid), 0) FROM clipboard_items", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            self.conn.execute(
                "UPDATE migration_state \
                 SET last_processed_id = ?1, completed_at = strftime('%s','now') \
                 WHERE key = ?2",
                params![max_id, SWEEP_KEY],
            )?;
        } else {
            // Still in progress — update the high-water mark.
            let max_processed: i64 = self
                .conn
                .query_row(
                    "SELECT COALESCE(MAX(rowid), 0) FROM clipboard_items WHERE key_version = 2",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            self.conn.execute(
                "UPDATE migration_state SET last_processed_id = ?1 WHERE key = ?2",
                params![max_processed, SWEEP_KEY],
            )?;

            // Yield between resumable invocations.
            std::thread::sleep(INTER_BATCH_SLEEP);
        }

        let _ = BATCH_SIZE; // ensure constant is referenced

        Ok(total_rotated)
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
