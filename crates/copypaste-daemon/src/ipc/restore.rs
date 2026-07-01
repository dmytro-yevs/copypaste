//! SQLCipher backup validate + atomic restore routine (split from
//! ipc/mod.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.19). Pure
//! filesystem + SQLCipher logic, no IPC state — kept unit-testable with
//! temp directories (CopyPaste-8wbt / crh3.6).
use super::*;

/// Validate `src_path` as a SQLCipher backup encrypted with `key`, then
/// atomically swap it into `db_path`, returning the freshly-opened restored
/// [`Database`]. This is the core of the `db_restore` IPC verb, extracted as a
/// pure filesystem + SQLCipher routine (no IPC state) so it is unit-testable
/// with temp directories (CopyPaste-8wbt / crh3.6).
///
/// Safety contract:
/// * **Validation runs on a throwaway staging copy** — a wrong-key, plaintext,
///   corrupt, or non-CopyPaste backup leaves the live files at `db_path`
///   completely untouched and returns `Err`.
/// * **The live DB is moved aside (never deleted) before the swap**, for BOTH
///   `force` values, so a failure during the swap rolls the originals back.
///   `force` only decides whether the aside safety copy
///   (`clipboard.db.before-restore-<ts>`) is removed on success.
/// * The caller must keep its existing `Database` handle until it installs the
///   returned one: on a rolled-back failure that handle stays valid (its inode
///   is renamed aside and back, never replaced).
pub(super) fn restore_database_file(
    src_path: &std::path::Path,
    db_path: &std::path::Path,
    key: &[u8; 32],
    force: bool,
) -> Result<Database, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Best-effort removal of a DB triple (main + WAL + SHM) sharing `base`.
    let remove_triple = |base: &std::path::Path| {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = base.as_os_str().to_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(p));
        }
    };

    // ── PHASE A — validate on a throwaway staging copy. `db_path` is NOT
    //    touched here, so any failure leaves the live DB fully intact.
    let staging = {
        let mut s = db_path.as_os_str().to_os_string();
        s.push(format!(".restore-staging-{ts}"));
        std::path::PathBuf::from(s)
    };
    remove_triple(&staging);
    std::fs::copy(src_path, &staging).map_err(|e| {
        format!(
            "db_restore: failed to stage backup copy at {}: {e}",
            staging.display()
        )
    })?;

    // `open_no_auto_migrate` REJECTS plaintext/garbage files (no silent
    // plaintext→SQLCipher migration), so only a genuine SQLCipher DB encrypted
    // with `key` validates.
    let validation = (|| -> Result<(), String> {
        let probe = Database::open_no_auto_migrate(&staging, key).map_err(|e| {
            format!(
                "db_restore: backup did not open with the current key (wrong key, \
                 corrupt, or not a CopyPaste database): {e}"
            )
        })?;
        // integrity_check catches a backup that decrypts but is structurally
        // corrupt / truncated.
        let integrity: String = probe
            .conn()
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| format!("db_restore: integrity_check failed: {e}"))?;
        if integrity != "ok" {
            return Err(format!(
                "db_restore: backup integrity_check returned '{integrity}' (corrupt backup)"
            ));
        }
        // Schema sanity: a legitimate backup carries the clipboard schema.
        probe
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(|e| {
                format!(
                    "db_restore: backup is missing the clipboard_items table — not a \
                     CopyPaste database: {e}"
                )
            })?;
        Ok(())
    })();
    remove_triple(&staging);
    validation?;

    // ── PHASE B — swap. Validation passed; every step is rollback-safe.
    //
    // Move the live DB aside (BOTH modes) so a late failure can roll back.
    let mut moved: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let mut orig = db_path.as_os_str().to_os_string();
        orig.push(suffix);
        let orig = std::path::PathBuf::from(orig);
        if orig.exists() {
            let mut aside = db_path.as_os_str().to_os_string();
            aside.push(format!("{suffix}.before-restore-{ts}"));
            let aside = std::path::PathBuf::from(aside);
            std::fs::rename(&orig, &aside)
                .map_err(|e| format!("db_restore: could not move {} aside: {e}", orig.display()))?;
            moved.push((orig, aside));
        }
    }

    // Roll back: drop any partially-written restored file, then move every
    // aside file back to its original path.
    let rollback = |moved: &[(std::path::PathBuf, std::path::PathBuf)]| {
        remove_triple(db_path);
        for (orig, aside) in moved {
            let _ = std::fs::rename(aside, orig);
        }
    };

    // Place the validated backup.
    if let Err(e) = std::fs::copy(src_path, db_path) {
        let msg = format!("db_restore: failed to copy backup into place: {e}");
        rollback(&moved);
        return Err(msg);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(db_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(db_path, perms);
        }
    }

    let restored = match Database::open_no_auto_migrate(db_path, key) {
        Ok(db) => db,
        Err(e) => {
            let msg = format!(
                "db_restore: failed to open restored DB (rolled back to prior database): {e}"
            );
            rollback(&moved);
            return Err(msg);
        }
    };

    // Ensure the additive audit table exists (matches the normal startup path).
    if let Err(e) = ensure_revoked_devices_table(restored.conn()) {
        tracing::warn!("db_restore: ensure_revoked_devices_table failed: {e}");
    }

    // Success. With `force`, drop the aside safety copy; otherwise keep it as
    // clipboard.db.before-restore-<ts>.
    if force {
        for (_orig, aside) in &moved {
            let _ = std::fs::remove_file(aside);
        }
    }

    Ok(restored)
}
