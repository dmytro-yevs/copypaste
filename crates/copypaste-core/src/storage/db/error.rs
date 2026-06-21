use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("Schema migration error: {0}")]
    Schema(#[from] super::super::schema::SchemaError),
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
