//! `copypaste vacuum` — reclaim free pages and rebuild indexes in the
//! encrypted clipboard database.
//!
//! Why a CLI command? Over a long-running daemon, SQLite accumulates free
//! pages from DELETE/UPDATE traffic. `VACUUM` rewrites the file without those
//! pages, shrinking it on disk and defragmenting B-trees. `REINDEX` rebuilds
//! the FTS5 / B-tree indexes (useful after schema migrations or upgrades).
//!
//! ## Why we refuse to run while the daemon is up
//! `VACUUM` takes an exclusive lock on the database and rewrites every page.
//! Running it concurrently with the daemon's writers would either fail with
//! `SQLITE_BUSY` or starve the daemon. We chose to *fail fast* with a clear
//! message telling the user to stop the daemon first, rather than silently
//! racing or auto-stopping (auto-stopping is a footgun if the user has
//! pending sync work).
//!
//! ## Why we open the DB directly (instead of asking the daemon over IPC)
//! Adding a `vacuum` IPC method would put a long-blocking, exclusive-lock
//! operation on the daemon's request loop — at best it blocks every other
//! request for ~seconds, at worst it deadlocks against an active writer.
//! Directly opening the file (after confirming the daemon is stopped) keeps
//! the CLI's failure mode isolated to the CLI process.
//!
//! ## Exit codes
//! - 0 — operation succeeded (or `--dry-run` finished printing)
//! - 1 — daemon still running, keychain unavailable, or SQLite error

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::ipc::IpcClient;

/// Options assembled from clap and passed to [`run`]. Kept as a plain struct
/// so the inner [`vacuum_with_key`] helper can be tested without a real
/// keychain (tests construct a `Plan` + key by hand).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// When true, report what would happen but do NOT mutate the database.
    pub dry_run: bool,
    /// When true, skip `VACUUM` and only run `REINDEX`. Faster, doesn't
    /// require free space equal to current DB size.
    pub reindex_only: bool,
}

/// Public entry point invoked from `main.rs`.
///
/// Refuses to run while the daemon is alive (see module docs for the
/// rationale). On macOS, fetches the local-storage key from the Keychain.
/// On any other platform, returns an error explaining that vacuum currently
/// requires the macOS keychain — this matches today's daemon, which is the
/// only producer of the keychain entry.
pub fn run(socket_path: &Path, db_path: PathBuf, plan: Plan) -> Result<()> {
    // ── Step 1: refuse if daemon is up ────────────────────────────────────
    if daemon_is_running(socket_path) {
        return Err(anyhow!(
            "daemon is running — VACUUM/REINDEX require exclusive DB access.\n\
             Stop the daemon first:  copypaste daemon stop\n\
             Then retry:             copypaste vacuum"
        ));
    }

    // ── Step 2: confirm DB exists ────────────────────────────────────────
    if !db_path.exists() {
        return Err(anyhow!(
            "database file not found at: {}\n\
             (set COPYPASTE_DB to override the default path)",
            db_path.display()
        ));
    }

    // ── Step 3: fetch SQLCipher key from Keychain ────────────────────────
    // We resolve the key here (not inside vacuum_with_key) so unit tests
    // can bypass the keychain by calling vacuum_with_key directly with a
    // known test key. The keychain call is a side effect that does not
    // belong in pure logic.
    let key = load_db_key().context("failed to load database key from keychain")?;

    vacuum_with_key(&db_path, &key, plan)
}

/// Inner worker — no I/O outside the database file and stdout.
///
/// Split from [`run`] so unit tests can drive it with a temp DB and a
/// synthetic key, without touching the real keychain or the user's home
/// directory.
pub fn vacuum_with_key(db_path: &Path, key: &[u8; 32], plan: Plan) -> Result<()> {
    let size_before = file_size(db_path)?;

    println!("Database: {}", db_path.display());
    println!("Before:   {}", format_size(size_before));

    if plan.dry_run {
        // For dry-run we still want to *open* the DB to confirm the key
        // works (catches "you stopped the daemon but the keychain entry
        // is gone" before the user wastes time on `daemon stop` again).
        // We open in a separate scope so the connection is dropped before
        // we report — no chance of accidental mutation.
        verify_key_opens_db(db_path, key)
            .context("dry-run: failed to open database with stored key")?;

        if plan.reindex_only {
            println!("Plan:     REINDEX (skipped — dry-run)");
        } else {
            println!("Plan:     VACUUM + REINDEX (skipped — dry-run)");
        }
        println!(
            "After:    {} (unchanged — dry-run)",
            format_size(size_before)
        );
        return Ok(());
    }

    // ── Real run ─────────────────────────────────────────────────────────
    let db = copypaste_core::Database::open(db_path, key)
        .context("failed to open encrypted database (wrong key? corrupted file?)")?;

    if !plan.reindex_only {
        // Checkpoint WAL into the main file first. Otherwise VACUUM only
        // shrinks the main file while -wal/-shm still hold recent pages,
        // and the reported "after" size is misleading.
        let _ = db.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        db.conn()
            .execute_batch("VACUUM")
            .context("VACUUM failed (out of disk space? db locked?)")?;
        println!("Plan:     VACUUM + REINDEX");
    } else {
        println!("Plan:     REINDEX only");
    }

    db.conn()
        .execute_batch("REINDEX")
        .context("REINDEX failed")?;

    // Drop the connection so OS file metadata is up-to-date before we stat.
    drop(db);

    let size_after = file_size(db_path)?;
    println!("After:    {}", format_size(size_after));

    // Reclaimed bytes. Use signed math so growth (rare — REINDEX can grow
    // slightly) doesn't underflow.
    let delta = size_before as i64 - size_after as i64;
    if delta > 0 {
        let pct = (delta as f64 / size_before.max(1) as f64) * 100.0;
        println!("Reclaimed: {} ({:.1}%)", format_size(delta as u64), pct);
    } else if delta < 0 {
        println!("Grew by:  {}", format_size((-delta) as u64));
    } else {
        println!("Reclaimed: 0 bytes (already compact)");
    }

    Ok(())
}

/// Probe the daemon socket. Returns true if we could establish a connection
/// — that's enough proof the daemon is alive. We deliberately do NOT try a
/// `status` round-trip because connect-only is faster and the only thing we
/// need to know is "will this process race us for the DB lock?".
fn daemon_is_running(socket_path: &Path) -> bool {
    IpcClient::connect(socket_path).is_ok()
}

/// Open the DB just long enough to confirm the key decrypts it. The
/// connection is dropped on return — no statements are executed.
fn verify_key_opens_db(db_path: &Path, key: &[u8; 32]) -> Result<()> {
    let _db = copypaste_core::Database::open(db_path, key)?;
    Ok(())
}

/// File size in bytes. Returns an error if the file is missing — we already
/// checked existence in `run`, but `vacuum_with_key` is also called directly
/// in tests so this guard belongs here too.
fn file_size(p: &Path) -> Result<u64> {
    Ok(std::fs::metadata(p)
        .with_context(|| format!("stat {}", p.display()))?
        .len())
}

/// Pretty-print bytes with one decimal place at the largest fitting unit.
/// Keeps output stable for shell scripts (no surprise unit jumps mid-line).
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ── Keychain access ──────────────────────────────────────────────────────
//
// Mirrors `copypaste-daemon/src/keychain.rs` (same SERVICE/ACCOUNT). We
// duplicate the *fetch* (not the full keypair-management module) because:
//   1. `copypaste-daemon` is a binary crate, not a library — its
//      `keychain` module isn't importable from the CLI.
//   2. We only need the raw 32-byte secret to derive the local enc key, not
//      the full DeviceKeypair API surface.
// If a third caller appears we'll promote this into a shared crate.

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.copypaste.daemon";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "device-secret-key";

#[cfg(target_os = "macos")]
fn load_db_key() -> Result<zeroize::Zeroizing<[u8; 32]>> {
    use copypaste_core::DeviceKeypair;
    use security_framework::passwords::get_generic_password;

    let bytes = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| anyhow!("keychain lookup failed: {e} (was the daemon ever started?)"))?;
    if bytes.len() != 32 {
        return Err(anyhow!(
            "keychain entry has wrong length: expected 32 bytes, got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let kp = DeviceKeypair::from_secret_bytes(&arr)
        .map_err(|e| anyhow!("invalid keypair bytes in keychain: {e}"))?;
    Ok(kp.local_enc_key())
}

#[cfg(not(target_os = "macos"))]
fn load_db_key() -> Result<zeroize::Zeroizing<[u8; 32]>> {
    Err(anyhow!(
        "vacuum currently requires macOS keychain access; \
         this platform is not yet supported"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Database;
    use tempfile::tempdir;

    /// Helper: create an encrypted DB with `n` rows so VACUUM has something
    /// meaningful to reclaim after we delete them. Returns the file path so
    /// the test can stat it before/after.
    fn make_db_with_rows(dir: &Path, n: usize, key: &[u8; 32]) -> PathBuf {
        let path = dir.join("vac.db");
        let db = Database::open(&path, key).expect("open");
        for i in 0..n {
            db.conn()
                .execute(
                    "INSERT INTO clipboard_items \
                     (id, item_id, content_type, content, content_nonce, \
                      is_sensitive, is_synced, lamport_ts, wall_time) \
                     VALUES (?1,?2,?3,?4,?5,0,0,?6,?6)",
                    rusqlite::params![
                        format!("id-{i}"),
                        format!("item-{i}"),
                        "text/plain",
                        // 4 KiB payload per row — enough that deletes free
                        // pages VACUUM can visibly reclaim.
                        vec![b'x'; 4096] as Vec<u8>,
                        vec![0u8; 24] as Vec<u8>,
                        i as i64,
                    ],
                )
                .expect("insert");
        }
        drop(db);
        path
    }

    /// VACUUM must invoke the SQLCipher-aware code path. We assert this
    /// indirectly: after VACUUM the file is still openable with the SAME
    /// key (proving SQLCipher pragma was applied — a plaintext VACUUM
    /// would have nuked encryption) and rejected with a WRONG key.
    /// This is the strongest behavioural check available without
    /// pattern-matching SQLite internals.
    #[test]
    fn vacuum_invokes_correct_sqlcipher_pragma() {
        let dir = tempdir().unwrap();
        let key = [0x42u8; 32];
        let db_path = make_db_with_rows(dir.path(), 10, &key);

        vacuum_with_key(
            &db_path,
            &key,
            Plan {
                dry_run: false,
                reindex_only: false,
            },
        )
        .expect("vacuum should succeed");

        // Still encrypted with the original key.
        Database::open(&db_path, &key).expect("must open with original key after VACUUM");

        // Wrong key still rejected — proves SQLCipher header survived.
        let wrong = [0x99u8; 32];
        assert!(
            Database::open(&db_path, &wrong).is_err(),
            "VACUUM must NOT decrypt the database"
        );
    }

    /// `--dry-run` must NOT mutate the file. We check file size before
    /// and after (mtime would also work but is more brittle on some FS).
    #[test]
    fn dry_run_does_not_modify_db() {
        let dir = tempdir().unwrap();
        let key = [0x10u8; 32];
        let db_path = make_db_with_rows(dir.path(), 5, &key);

        // Capture size + content hash before.
        let before_size = std::fs::metadata(&db_path).unwrap().len();
        let before_bytes = std::fs::read(&db_path).unwrap();

        vacuum_with_key(
            &db_path,
            &key,
            Plan {
                dry_run: true,
                reindex_only: false,
            },
        )
        .expect("dry-run should succeed");

        // File size must be identical.
        let after_size = std::fs::metadata(&db_path).unwrap().len();
        assert_eq!(before_size, after_size, "dry-run must not change file size");

        // Byte-for-byte identical — even a no-op open in WAL mode could
        // touch -wal/-shm, but the main file must be untouched in dry-run.
        let after_bytes = std::fs::read(&db_path).unwrap();
        assert_eq!(
            before_bytes, after_bytes,
            "dry-run must not modify main DB file bytes"
        );
    }

    /// When a daemon is reachable on the socket, `run` must refuse and
    /// the error message must mention `daemon stop` so the user knows
    /// the fix. We simulate a running daemon by binding a UnixListener
    /// (any process accepting on the socket counts).
    #[test]
    fn vacuum_with_daemon_running_returns_clear_error() {
        use std::os::unix::net::UnixListener;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("daemon.sock");
        let _listener = UnixListener::bind(&sock).expect("bind mock daemon socket");

        // DB path doesn't need to exist — we expect to fail at the
        // daemon-detect step, before any file I/O.
        let fake_db = dir.path().join("nonexistent.db");

        let err = run(
            &sock,
            fake_db,
            Plan {
                dry_run: false,
                reindex_only: false,
            },
        )
        .expect_err("must refuse to run while daemon is alive");

        let msg = format!("{err:#}");
        assert!(
            msg.contains("daemon is running"),
            "error must explain why: {msg}"
        );
        assert!(
            msg.contains("daemon stop"),
            "error must recommend the fix: {msg}"
        );
    }

    /// `--reindex-only` must skip VACUUM. We can't directly observe
    /// "VACUUM was skipped", but we can verify the operation succeeds on
    /// a DB and leaves it readable with the same key.
    #[test]
    fn reindex_only_succeeds_and_preserves_data() {
        let dir = tempdir().unwrap();
        let key = [0x33u8; 32];
        let db_path = make_db_with_rows(dir.path(), 3, &key);

        vacuum_with_key(
            &db_path,
            &key,
            Plan {
                dry_run: false,
                reindex_only: true,
            },
        )
        .expect("reindex-only should succeed");

        let db = Database::open(&db_path, &key).expect("open after reindex");
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3, "REINDEX must preserve all rows");
    }

    /// Bytes-to-string formatter sanity check — locks the user-visible
    /// output format so shell scripts that parse `Reclaimed:` stay stable.
    #[test]
    fn format_size_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2 * 1024), "2.0 KiB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_size(3u64 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    /// Missing DB file path must produce a clear error (not a panic) when
    /// reached from `vacuum_with_key`. The `run` path catches this earlier,
    /// but the helper is also reachable from tests / future callers.
    #[test]
    fn missing_db_path_errors_cleanly() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist.db");
        let err = vacuum_with_key(
            &missing,
            &[0u8; 32],
            Plan {
                dry_run: true,
                reindex_only: false,
            },
        )
        .expect_err("missing db must error");
        assert!(format!("{err:#}").contains("does-not-exist.db"));
    }
}
