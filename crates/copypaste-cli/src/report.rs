//! What each command prints once its reply has arrived.
//!
//! Split from the request half in `main` because the two are different jobs
//! that happen to be indexed by the same enum: `run` decides which
//! [`copypaste_ipc::Method`] to send, this decides what a human is shown. The
//! `match` is exhaustive over [`Command`] in both places, so a new verb is a
//! compile error twice — once for the request it does not make and once for the
//! answer it does not print.

use std::path::Path;

use copypaste_ipc::ResponseData;

use crate::cli::{CloudAction, Command, PairAction};
use crate::error::CliError;
use crate::{client, now_ms, out, render};

/// Print the reply to `command`.
pub fn report(command: &Command, data: Option<ResponseData>) -> Result<(), CliError> {
    match command {
        Command::List { .. } => {
            let page = client::expect_page(data)?;
            out(&(render::items_table(&page.items, now_ms(), "no items yet")));
            warn_unreadable(page.skipped_undecryptable);
            // On stderr, so `copypaste list | …` is unchanged. Without it the
            // marker is only reachable through `--json`, and a `--limit` that
            // fills the page would look like the whole history.
            if let Some(cursor) = &page.next_cursor {
                eprintln!("there is more: copypaste list --cursor {cursor}");
            }
        }
        Command::Search { .. } => {
            let page = client::expect_page(data)?;
            out(&(render::items_table(&page.items, now_ms(), "no matches")));
            warn_unreadable(page.skipped_undecryptable);
        }
        Command::Status => {
            let status = client::expect_status(data)?;
            out(&(render::status_text(&status)));
        }
        Command::Add { .. } => match client::optional_item(&data) {
            Some(item) => out(&format!("added {}", item.id)),
            None => out("added"),
        },
        Command::Copy { id } => out(&format!("copied {id} to the clipboard")),
        Command::Delete { id } => out(&format!("deleted {id}")),
        Command::Pin { id } => out(&format!("pinned {id}")),
        Command::Unpin { id } => out(&format!("unpinned {id}")),
        // The count is rows *renumbered*, which is not the same as ids given:
        // pinned items the caller did not name are pushed behind the ones it
        // did, and they are renumbered too. Saying "reordered N pinned items"
        // is therefore the honest sentence.
        Command::Reorder { .. } => match client::optional_count(&data) {
            Some(count) => out(&format!(
                "reordered {count} pinned {}",
                plural(count, "item", "items")
            )),
            None => out("reordered the pinned items"),
        },
        Command::Clear { .. } => match client::optional_count(&data) {
            Some(count) => out(&format!(
                "deleted {count} {}",
                plural(count, "item", "items")
            )),
            None => out("cleared"),
        },
        Command::Shutdown => out("the daemon is stopping"),
        Command::Pair { action } => match action {
            PairAction::Create { .. } => {
                let pairing = client::expect_pairing(data)?;
                // The only place a code is ever rendered. Straight to stdout,
                // never through the logger, and the daemon does not keep a copy
                // that could be asked for again.
                out(&render::pairing_text(&pairing));
            }
            PairAction::Accept { .. } => {
                let peers = client::expect_peers(data)?;
                match peers.first() {
                    Some(peer) => out(&format!("paired with {} ({})", peer.name, peer.pairing_id)),
                    None => out("paired"),
                }
            }
        },
        Command::Peers => {
            let peers = client::expect_peers(data)?;
            out(&render::peers_table(&peers, now_ms(), "no paired devices"));
        }
        Command::Unpair { pairing_id } => out(&format!("unpaired {pairing_id}")),
        Command::Revoke { pairing_id, .. } => out(&format!(
            "revoked {pairing_id}; that code can never be used again"
        )),
        Command::Cloud { action } => match action {
            CloudAction::Sync => {
                let stats = client::expect_cloud_sync(data)?;
                out(&render::cloud_sync_text(&stats));
            }
            // Sign-in, sign-out and status all answer with the new status, so
            // the user sees the state they just moved the daemon into.
            _ => {
                let status = client::expect_cloud_status(data)?;
                out(&(render::cloud_status_text(&status, now_ms())));
            }
        },
        Command::Discover { .. } => {
            let found = client::expect_discovered(data)?;
            out(&render::discovered_table(
                &found.devices,
                now_ms(),
                "no devices are visible on this network",
            ));
        }
        Command::Export { output, .. } => {
            let export = client::expect_export(data)?;
            let encoded = serde_json::to_string_pretty(&export)
                .map_err(|e| CliError::local(format!("could not render the export: {e}")))?;
            match output {
                Some(path) => {
                    write_export(path, &encoded)?;
                    out(&format!("exported {} items", export.items.len()));
                }
                None => out(&encoded),
            }
            // On stderr, so `copypaste export > file` is still exactly the
            // export — and so the warning survives being redirected away.
            eprint!("{}", render::export_summary(&export));
        }
        Command::Import { .. } => {
            let result = client::expect_import(data)?;
            out(&format!(
                "imported {} {}, skipped {} already present",
                result.inserted,
                plural(u64::from(result.inserted), "item", "items"),
                result.skipped
            ));
        }
        Command::Backup { .. } => {
            let backup = client::expect_backup(data)?;
            out(&format!("wrote a backup of {} bytes", backup.size_bytes));
        }
        Command::Restore { .. } => out("restored this device's history from the backup"),
        Command::Config { .. } => {
            let applied = client::expect_config(data)?;
            out(&render::config_text(&applied));
        }
        Command::Watch => unreachable!("handled before the request is sent"),
        Command::Sync { .. } => {
            let results = client::expect_sync(data)?;
            out(&(render::sync_table(&results, "no paired devices")));
            // A run where every peer failed is not a success, even though the
            // request itself was answered: a script must be able to tell.
            if !results.is_empty() && results.iter().all(|r| r.error.is_some()) {
                return Err(CliError::local("no peer could be synced"));
            }
        }
    }
    Ok(())
}

/// Say when a page was shortened, and by how much.
///
/// v1 shortened the page and said nothing; the user saw fewer items with no
/// explanation (`CopyPaste-00zz`). On stderr so a `--json` consumer and a pipe
/// are unaffected.
fn warn_unreadable(skipped: u32) {
    if skipped > 0 {
        eprintln!(
            "warning: {skipped} {} could not be read and {} left out",
            plural(u64::from(skipped), "item", "items"),
            plural(u64::from(skipped), "was", "were")
        );
    }
}

fn write_export(path: &Path, encoded: &str) -> Result<(), CliError> {
    std::fs::write(path, format!("{encoded}\n"))
        .map_err(|e| CliError::local(format!("could not write the export: {e}")))
}

fn plural(count: u64, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 {
        one
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals_agree_with_their_count() {
        assert_eq!(plural(1, "item", "items"), "item");
        assert_eq!(plural(0, "item", "items"), "items");
        assert_eq!(plural(2, "item", "items"), "items");
    }
}
