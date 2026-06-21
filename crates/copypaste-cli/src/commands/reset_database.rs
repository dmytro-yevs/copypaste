//! `copypaste reset-database` — wipe and recreate the local clipboard DB.
//!
//! This is the explicit escape hatch for a daemon stuck in degraded mode because
//! the existing database cannot be decrypted (key mismatch / "file is not a
//! database"). It sends the [`METHOD_RESET_DATABASE`] IPC verb to the running
//! daemon, which holds the DB write-lock, deletes the existing database files,
//! and recreates a fresh empty encrypted database in-place.
//!
//! ## Safety
//!
//! The command requires EITHER `--confirm` (for scripted use) OR an explicit
//! interactive confirmation by typing "reset" at the prompt. This prevents
//! accidental erasure of clipboard history.
//!
//! The daemon enforces its own `confirm = true` guard — this CLI-level check is
//! defence-in-depth: a correct daemon will refuse the call regardless.
//!
//! ## When to use
//!
//! - `copypaste status` reports `daemon: degraded`.
//! - The daemon log shows "file is not a database" or "key mismatch".
//! - Other commands fail with `error [ipc_not_ready]`.
//!
//! After a successful reset the daemon returns to `ready` in-place (no restart
//! required). Subsequent `history_page` / `status` calls succeed against the
//! new empty database.
//!
//! ## Exit codes
//! - 0 — database reset succeeded; daemon is now ready.
//! - 1 — aborted by user, daemon not running, or daemon returned an error.

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::METHOD_RESET_DATABASE;
use std::path::Path;

use crate::ipc::IpcClient;

/// Run `copypaste reset-database`.
///
/// * `socket_path` — path to the daemon's UNIX socket.
/// * `confirm` — skip the interactive confirmation prompt (for scripts /
///   `--yes` / `--confirm` flag).
pub fn run(socket_path: &Path, confirm: bool) -> Result<()> {
    // Require explicit confirmation — either the flag or interactive prompt.
    if !confirm {
        eprintln!("WARNING: This will PERMANENTLY DELETE all local clipboard history.");
        eprintln!("         The daemon will recreate an empty database in its place.");
        eprintln!("         Use this command to recover a daemon stuck in degraded mode.");
        eprintln!();
        eprint!("Type 'reset' to confirm permanent erasure of clipboard history: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation from stdin")?;
        if input.trim() != "reset" {
            return Err(anyhow!(
                "aborted: type 'reset' to confirm, or pass --confirm to skip this prompt"
            ));
        }
    }

    let mut client = IpcClient::connect(socket_path).with_context(|| {
        format!(
            "daemon is not running (could not connect to socket: {})\n\
             The daemon must be running for reset-database to work.\n\
             Start the daemon first: copypaste daemon start",
            socket_path.display()
        )
    })?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_RESET_DATABASE,
        serde_json::json!({ "confirm": true }),
    );

    let resp = client.call(&req)?;

    if !resp.ok {
        let msg = resp.error.as_deref().unwrap_or("unknown error");
        return Err(anyhow!("reset-database failed: {msg}"));
    }

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for reset-database"))?;

    let reset = data["reset"].as_bool().unwrap_or(false);
    let ready = data["ready"].as_bool().unwrap_or(false);

    if reset && ready {
        eprintln!("Database reset complete. The daemon is now ready with an empty history.");
    } else {
        return Err(anyhow!(
            "reset-database returned unexpected response: reset={reset}, ready={ready}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run;
    }

    /// Verify the method constant has the correct wire name so the daemon
    /// can dispatch it correctly.
    #[test]
    fn method_constant_has_correct_wire_name() {
        assert_eq!(METHOD_RESET_DATABASE, "reset_database");
    }
}
