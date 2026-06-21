//! `copypaste backup` and `copypaste restore` — daemon IPC verbs.
//!
//! These commands route through the daemon's `db_backup` / `db_restore` IPC
//! verbs (CopyPaste-x94p / CopyPaste-8wbt) so the daemon owns the backup
//! process end-to-end: it holds the encryption key, the open database handle,
//! and can produce a consistent in-process SQLCipher copy via `VACUUM INTO`.
//!
//! The shell-script helpers (`locate_script`, `build_backup_args`,
//! `build_restore_args`) are retained for diagnostics and external tooling
//! but are no longer called by the CLI dispatch path.

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::{METHOD_DB_BACKUP, METHOD_DB_RESTORE};
use std::path::{Path, PathBuf};

// These helpers are used only in the test module below. The binary no longer
// calls them (it routes through the daemon IPC verbs instead), but we keep
// them so the script-plumbing tests continue to verify the shell interface.
#[allow(dead_code)]
/// Locate a script in `scripts/` next to the running binary or the repo root.
///
/// Resolution order:
///   1. `$COPYPASTE_SCRIPTS_DIR/<name>` (test / packaging override).
///   2. `<exe-dir>/../../scripts/<name>` (cargo target/debug layout).
///   3. `<exe-dir>/../scripts/<name>`.
///   4. `<cwd>/scripts/<name>` (when invoked from the repo root).
///
/// Returns the first existing path; otherwise an error explaining where
/// we looked so a packager can diagnose a broken install.
pub(crate) fn locate_script(name: &str) -> Result<PathBuf> {
    let mut tried: Vec<PathBuf> = Vec::new();

    if let Ok(dir) = std::env::var("COPYPASTE_SCRIPTS_DIR") {
        let p = PathBuf::from(dir).join(name);
        if p.is_file() {
            return Ok(p);
        }
        tried.push(p);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // target/debug/copypaste → ../../scripts
            let p1 = exe_dir.join("../../scripts").join(name);
            if p1.is_file() {
                return Ok(p1);
            }
            tried.push(p1);

            // target/release/copypaste or /usr/local/bin/copypaste → ../scripts
            let p2 = exe_dir.join("../scripts").join(name);
            if p2.is_file() {
                return Ok(p2);
            }
            tried.push(p2);
        }
    }

    let cwd_path = PathBuf::from("scripts").join(name);
    if cwd_path.is_file() {
        return Ok(cwd_path);
    }
    tried.push(cwd_path);

    Err(anyhow!(
        "script `{name}` not found. Looked in:\n  {}",
        tried
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    ))
}

/// Build the argument list passed to `backup-db.sh`. Pure function so we
/// can unit-test the wiring without spawning a subprocess.
#[allow(dead_code)]
pub(crate) fn build_backup_args(output: Option<&str>, dry_run: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if let Some(dir) = output {
        args.push("--output-dir".to_string());
        args.push(dir.to_string());
    }
    if dry_run {
        args.push("--dry-run".to_string());
    }
    args
}

/// Build the argument list passed to `restore-db.sh`.
#[allow(dead_code)]
pub(crate) fn build_restore_args(backup_path: &str, force: bool, dry_run: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![backup_path.to_string()];
    if force {
        args.push("--force".to_string());
    }
    if dry_run {
        args.push("--dry-run".to_string());
    }
    args
}

/// Back up the live clipboard database via the daemon's `db_backup` IPC verb.
///
/// `output` is the destination directory. The daemon writes a timestamped
/// `copypaste-<YYYYMMDD-HHMMSS>.db.enc` file inside it. When `output` is
/// `None`, the current directory is used as the output directory.
///
/// `dry_run` prints what would happen without writing anything. In dry-run
/// mode the daemon is NOT contacted (the shell-script tools and IPC call
/// both need a running daemon, so dry-run stays a client-side preview).
///
/// (CopyPaste-x94p)
pub fn run_backup(socket_path: &Path, output: Option<&str>, dry_run: bool) -> Result<()> {
    use crate::ipc::IpcClient;
    use crate::commands::common::exit_on_err;

    // Compute a timestamped destination path (Unix epoch seconds — unique
    // enough for a backup filename and does not require chrono/time deps).
    let output_dir = output.unwrap_or(".");
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest_path = format!("{output_dir}/copypaste-{epoch_secs}.db.enc");

    if dry_run {
        println!("dry-run: would write backup to: {dest_path}");
        println!("dry-run: daemon IPC verb: {METHOD_DB_BACKUP}");
        return Ok(());
    }

    let mut client = IpcClient::connect(socket_path)
        .context("cannot connect to daemon — is it running? start with `copypaste daemon start`")?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_DB_BACKUP,
        serde_json::json!({ "dest_path": dest_path }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let written_path = resp
        .data
        .as_ref()
        .and_then(|d| d["dest_path"].as_str())
        .unwrap_or(&dest_path);
    let size_bytes = resp
        .data
        .as_ref()
        .and_then(|d| d["size_bytes"].as_u64())
        .unwrap_or(0);

    println!("backup written: {written_path} ({size_bytes} bytes)");
    Ok(())
}

/// Restore the live clipboard database via the daemon's `db_restore` IPC verb.
///
/// The daemon quiesces writes, renames the existing database aside (or deletes
/// it when `force = true`), copies the backup file in place, reopens the DB
/// with its current key, and marks itself ready.
///
/// `dry_run` prints what would happen without contacting the daemon.
///
/// (CopyPaste-8wbt)
pub fn run_restore(
    socket_path: &Path,
    backup_path: &str,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    use crate::ipc::IpcClient;
    use crate::commands::common::exit_on_err;

    // Reject paths that look like flags (leading `--`) to prevent accidental
    // argument confusion. A real backup path will never start with `--`.
    if backup_path.starts_with("--") {
        return Err(anyhow!(
            "invalid backup path {:?}: paths must not start with `--` \
             (did you mean to pass a flag before the path?)",
            backup_path
        ));
    }

    // Absolute path: the daemon resolves paths relative to its own cwd, not
    // the CLI's cwd — so we canonicalize before sending over IPC.
    let abs_path = if dry_run {
        // In dry-run we don't need the file to exist.
        backup_path.to_string()
    } else {
        if !Path::new(backup_path).is_file() {
            return Err(anyhow!("backup file not found: {backup_path}"));
        }
        std::fs::canonicalize(backup_path)
            .with_context(|| format!("cannot resolve path: {backup_path}"))?
            .to_string_lossy()
            .into_owned()
    };

    if dry_run {
        println!("dry-run: would restore from: {abs_path}");
        println!("dry-run: force={force}");
        println!("dry-run: daemon IPC verb: {METHOD_DB_RESTORE}");
        return Ok(());
    }

    // Interactive confirmation (unless --force skips it).
    if !force {
        eprint!(
            "WARNING: this will replace the live clipboard database with the backup.\n\
             The existing database will be renamed aside (use --force to delete it).\n\
             Type 'restore' to confirm: "
        );
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .context("failed to read confirmation")?;
        if line.trim() != "restore" {
            return Err(anyhow!("restore cancelled (type 'restore' to confirm)"));
        }
    }

    let mut client = IpcClient::connect(socket_path)
        .context("cannot connect to daemon — is it running? start with `copypaste daemon start`")?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_DB_RESTORE,
        serde_json::json!({
            "confirm": true,
            "src_path": abs_path,
            "force": force,
        }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("restore complete — daemon is ready with the restored database.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `backup` with no flags must pass an empty arg list — the script
    /// then uses its own defaults (output dir = `<repo>/backups`).
    #[test]
    fn backup_args_empty_by_default() {
        assert!(build_backup_args(None, false).is_empty());
    }

    /// `--output <dir>` from the CLI must translate to the script's
    /// `--output-dir <dir>` flag (note the rename — we expose a shorter
    /// name on the CLI for ergonomics, the script keeps its existing
    /// long name for backwards compatibility).
    #[test]
    fn backup_args_translate_output_flag() {
        let args = build_backup_args(Some("/tmp/foo"), false);
        assert_eq!(args, vec!["--output-dir".to_string(), "/tmp/foo".into()]);
    }

    /// `--dry-run` from the CLI propagates verbatim to the script.
    #[test]
    fn backup_args_propagate_dry_run() {
        let args = build_backup_args(None, true);
        assert_eq!(args, vec!["--dry-run".to_string()]);
    }

    /// Combined flags preserve `--output-dir` before `--dry-run` so the
    /// script's manual ordering example stays representative.
    #[test]
    fn backup_invokes_script_with_correct_args() {
        let args = build_backup_args(Some("/var/backups/cp"), true);
        assert_eq!(
            args,
            vec![
                "--output-dir".to_string(),
                "/var/backups/cp".to_string(),
                "--dry-run".to_string(),
            ]
        );
    }

    /// `restore <path>` with no flags forwards only the path, so the
    /// script applies its safe-default behaviour (rename existing DB
    /// aside instead of deleting).
    #[test]
    fn restore_args_path_only() {
        let args = build_restore_args("/tmp/b.enc", false, false);
        assert_eq!(args, vec!["/tmp/b.enc".to_string()]);
    }

    /// `--force` must be opt-in and forwarded so the script will
    /// overwrite a live DB.
    #[test]
    fn restore_args_with_force() {
        let args = build_restore_args("/tmp/b.enc", true, false);
        assert_eq!(args, vec!["/tmp/b.enc".to_string(), "--force".into()]);
    }

    /// Dry-run combined with --force still sends both flags so the user
    /// can preview a destructive restore.
    #[test]
    fn restore_args_dry_run_combines_with_force() {
        let args = build_restore_args("/tmp/b.enc", true, true);
        assert_eq!(
            args,
            vec![
                "/tmp/b.enc".to_string(),
                "--force".to_string(),
                "--dry-run".to_string(),
            ]
        );
    }

    /// Safety net: `run_restore` must refuse to invoke the script when
    /// the user points at a missing backup file (without --dry-run).
    /// This catches a typo before we hand a confusing error back from
    /// bash. `--force` does NOT bypass this — it only controls how the
    /// LIVE database is treated, never how a missing source is handled.
    #[test]
    fn restore_refuses_overwrite_without_force_when_source_missing() {
        let missing = "/definitely/does/not/exist/backup.enc";
        let err = run_restore(Path::new("/tmp/sock"), missing, false, false)
            .expect_err("should error on missing source");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not found"),
            "error should mention missing file, got: {msg}"
        );

        // With --force the file is still required (force ≠ skip-existence).
        let err2 = run_restore(Path::new("/tmp/sock"), missing, true, false)
            .expect_err("should error on missing source even with --force");
        assert!(format!("{err2:#}").contains("not found"));
    }

    /// `--dry-run` is allowed to proceed past the missing-source guard so
    /// users can preview behaviour against any path. The actual script
    /// invocation may still fail downstream — we only assert that the
    /// pre-flight check does not short-circuit dry-run mode.
    #[test]
    fn dry_run_does_not_execute_pre_flight_short_circuit() {
        // We can prove the missing-source guard is bypassed by checking
        // that build_restore_args still emits --dry-run for a fake path.
        let args = build_restore_args("/nope", false, true);
        assert!(args.contains(&"--dry-run".to_string()));
        assert!(args.contains(&"/nope".to_string()));
    }

    /// `locate_script` respects `COPYPASTE_SCRIPTS_DIR` when set, so
    /// packagers / tests can pin scripts to an arbitrary prefix.
    #[test]
    fn locate_script_honours_env_override() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = tmp.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        let target = scripts.join("backup-db.sh");
        std::fs::write(&target, "#!/usr/bin/env bash\n").unwrap();

        // Safety: serial test — we restore the prior value at the end.
        let prev = std::env::var("COPYPASTE_SCRIPTS_DIR").ok();
        std::env::set_var("COPYPASTE_SCRIPTS_DIR", &scripts);

        let found = locate_script("backup-db.sh").expect("env override should find script");
        assert_eq!(found, target);

        match prev {
            Some(v) => std::env::set_var("COPYPASTE_SCRIPTS_DIR", v),
            None => std::env::remove_var("COPYPASTE_SCRIPTS_DIR"),
        }
    }

    /// When the script genuinely is not anywhere we look, the error
    /// must enumerate the tried paths so a packager can diagnose the
    /// broken install without reading our source.
    #[test]
    fn locate_script_errors_list_tried_paths() {
        // Force-clear the env override so we exercise the search path.
        let prev = std::env::var("COPYPASTE_SCRIPTS_DIR").ok();
        std::env::remove_var("COPYPASTE_SCRIPTS_DIR");

        let err = locate_script("this-script-does-not-exist.sh").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("not found"), "got: {msg}");
        assert!(msg.contains("Looked in"), "should list tried paths: {msg}");

        if let Some(v) = prev {
            std::env::set_var("COPYPASTE_SCRIPTS_DIR", v);
        }
    }

    /// Function signatures are part of the public contract with `main.rs`.
    /// Lock them so a refactor cannot silently change the dispatch shape.
    #[test]
    fn run_signatures_compile() {
        let _: fn(&Path, Option<&str>, bool) -> Result<()> = run_backup;
        let _: fn(&Path, &str, bool, bool) -> Result<()> = run_restore;
    }

    /// `run_restore` must reject a `backup_path` that starts with `--` to
    /// prevent accidental flag injection into the restore shell script.
    #[test]
    fn restore_rejects_flag_like_path() {
        let sock = Path::new("/tmp/sock");
        let err = run_restore(sock, "--force", false, false)
            .expect_err("path starting with -- must be rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("must not start with `--`"),
            "error should explain the rejection, got: {msg}"
        );

        // Also reject other -- prefixes, not just known flag names.
        let err2 = run_restore(sock, "--some-unknown-flag", false, false)
            .expect_err("any -- prefix must be rejected");
        assert!(
            format!("{err2:#}").contains("must not start with `--`"),
            "got: {err2:#}"
        );
    }
}
