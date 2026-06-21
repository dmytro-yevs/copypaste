use rusqlite::Connection;
use std::path::Path;

use super::error::DbError;

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
pub(super) fn checkpoint_with_retry(conn: &Connection) -> Result<(), DbError> {
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

/// Migrate an unencrypted file to SQLCipher in-place.
///
/// Strategy: open the plaintext source, ATTACH a new encrypted destination,
/// use `sqlcipher_export()` to copy all content, DETACH, then atomically
/// replace the original file. This is the SQLCipher-recommended migration path.
///
/// `sqlcipher_export()` is available on any connection compiled with the
/// `bundled-sqlcipher` feature.
pub(super) fn encrypt_existing(path: &Path, key: &[u8; 32]) -> Result<(), DbError> {
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
    let plaintext_conn =
        Connection::open(path).map_err(|e| DbError::Migration(format!("open plaintext: {e}")))?;

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
