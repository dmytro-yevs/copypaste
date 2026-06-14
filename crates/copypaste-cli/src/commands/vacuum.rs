//! `copypaste vacuum` — reclaim free pages and rebuild indexes in the
//! encrypted clipboard database via the running daemon.
//!
//! ## Architecture change (CopyPaste-wmv / CopyPaste-29x)
//!
//! The original implementation opened the SQLCipher file *directly* from the
//! CLI process, which required copypaste-core (Database), the macOS Keychain
//! (DeviceKeypair), rusqlite, and zeroize — violating the architectural rule
//! that `copypaste-cli` must speak IPC only and never link `copypaste-core`.
//!
//! The daemon now exposes a `vacuum` IPC verb (METHOD_VACUUM) that:
//!   * Holds the write-lock for the duration of the operation (exclusive
//!     access is already serialised through the daemon's Mutex<Database>).
//!   * Runs `PRAGMA wal_checkpoint(TRUNCATE)` + `VACUUM` + `REINDEX` on a
//!     blocking thread so the async executor is not starved.
//!   * Returns `{ ok, size_before, size_after, reclaimed }` which the CLI
//!     prints for the user.
//!
//! ## Required daemon state
//!
//! The daemon MUST be running. Unlike the old CLI path (which required the
//! daemon to be *stopped* so the CLI could grab the exclusive lock), the IPC
//! path acquires the lock inside the daemon, which is always safe because the
//! daemon serialises all DB access through its own Mutex.
//!
//! ## Exit codes
//! - 0 — operation succeeded (or `--dry-run` finished printing)
//! - 1 — daemon not running, IPC call failed, or daemon returned an error

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::METHOD_VACUUM;
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Options assembled from clap and passed to [`run`].
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
/// Sends a `vacuum` IPC request to the running daemon and prints the result.
/// The daemon must be running — unlike the old direct-DB path, this approach
/// does not require the daemon to be stopped first.
pub fn run(socket_path: &Path, plan: Plan) -> Result<()> {
    let mut client = IpcClient::connect(socket_path).with_context(|| {
        format!(
            "daemon is not running (could not connect to socket: {})\n\
             Start the daemon first:  copypaste daemon start\n\
             Then retry:             copypaste vacuum",
            socket_path.display()
        )
    })?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_VACUUM,
        serde_json::json!({
            "reindex_only": plan.reindex_only,
            "dry_run": plan.dry_run,
        }),
    );

    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for vacuum"))?;

    // Parse the structured response fields.
    let size_before = data["size_before"]
        .as_u64()
        .ok_or_else(|| anyhow!("daemon response missing 'size_before' field"))?;
    let size_after = data["size_after"]
        .as_u64()
        .ok_or_else(|| anyhow!("daemon response missing 'size_after' field"))?;
    let reclaimed = data["reclaimed"]
        .as_i64()
        .ok_or_else(|| anyhow!("daemon response missing 'reclaimed' field"))?;

    // Print results in the same style as the old direct path so shell scripts
    // that parse this output continue to work unchanged.
    println!("Before:   {}", format_size(size_before));

    if plan.dry_run {
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

    if plan.reindex_only {
        println!("Plan:     REINDEX only");
    } else {
        println!("Plan:     VACUUM + REINDEX");
    }
    println!("After:    {}", format_size(size_after));

    if reclaimed > 0 {
        let pct = (reclaimed as f64 / size_before.max(1) as f64) * 100.0;
        println!("Reclaimed: {} ({:.1}%)", format_size(reclaimed as u64), pct);
    } else if reclaimed < 0 {
        println!("Grew by:  {}", format_size((-reclaimed) as u64));
    } else {
        println!("Reclaimed: 0 bytes (already compact)");
    }

    Ok(())
}

/// Pretty-print bytes with one decimal place at the largest fitting unit.
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, Plan) -> Result<()> = run;
    }

    #[test]
    fn format_size_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2 * 1024), "2.0 KiB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_size(3u64 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    /// The Plan struct must be constructible from code outside this module
    /// (main.rs builds it from clap output).
    #[test]
    fn plan_is_constructible() {
        let p = Plan {
            dry_run: true,
            reindex_only: false,
        };
        assert!(p.dry_run);
        assert!(!p.reindex_only);
    }
}
