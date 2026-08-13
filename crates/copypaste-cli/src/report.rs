//! What each command prints once its reply has arrived.
//!
//! Split from the request half in `main` because the two are different jobs
//! that happen to be indexed by the same enum: `run` decides which
//! [`copypaste_ipc::Method`] to send, this decides what a human is shown. The
//! `match` is exhaustive over [`Command`] in both places, so a new verb is a
//! compile error twice — once for the request it does not make and once for the
//! answer it does not print.

use std::path::Path;

use copypaste_ipc::{ExportData, ImportData, ResponseData};

use crate::cli::{CloudAction, Command, PairAction};
use crate::error::CliError;
use crate::{client, now_ms, out, render};

/// Apply command work that remains necessary when human-readable output is off.
pub fn apply_side_effects(command: &Command, data: Option<ResponseData>) -> Result<(), CliError> {
    match command {
        Command::Export {
            output: Some(path), ..
        } => write_export(path, &client::expect_export(data)?),
        _ => Ok(()),
    }
}

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
            PairAction::Create => {
                let pairing = client::expect_pairing(data)?;
                // The only place a code is ever rendered. Straight to stdout,
                // never through the logger, and the daemon does not keep a copy
                // that could be asked for again.
                out(&render::pairing_text(&pairing));
            }
            PairAction::Join { .. }
            | PairAction::Progress
            | PairAction::Confirm
            | PairAction::Reject
            | PairAction::Cancel => {
                let progress = client::expect_pairing_progress(data)?;
                out(&render::pairing_progress_text(&progress));
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
            match output {
                Some(path) => {
                    write_export(path, &export)?;
                    out(&format!("exported {} items", export.items.len()));
                }
                None => out(&encode_export(&export)?),
            }
            // On stderr, so `copypaste export > file` is still exactly the
            // export — and so the warning survives being redirected away.
            eprint!("{}", render::export_summary(&export));
        }
        Command::Import { .. } => {
            let result = client::expect_import(data)?;
            out(&import_summary(&result));
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

fn import_summary(result: &ImportData) -> String {
    let mut summary = format!(
        "imported {} {}; skipped {} {}",
        result.inserted,
        plural(u64::from(result.inserted), "item", "items"),
        result.skipped,
        plural(u64::from(result.skipped), "item", "items")
    );
    if result.skipped > 0 {
        summary.push_str(&skip_reasons(result));
    }
    if result.pins_failed > 0 {
        summary.push_str(&format!(
            "; {} {} could not be pinned",
            result.pins_failed,
            plural(u64::from(result.pins_failed), "item", "items")
        ));
    }
    summary
}

fn skip_reasons(result: &ImportData) -> String {
    let mut reasons = Vec::new();
    if result.skipped_duplicate > 0 {
        reasons.push(format!(
            "{} {} already present",
            result.skipped_duplicate,
            plural(
                u64::from(result.skipped_duplicate),
                "duplicate",
                "duplicates"
            )
        ));
    }
    if result.skipped_empty > 0 {
        reasons.push(format!(
            "{} empty or invalid {}",
            result.skipped_empty,
            plural(u64::from(result.skipped_empty), "item", "items")
        ));
    }
    if result.skipped_too_large > 0 {
        reasons.push(format!(
            "{} {} over the size limit",
            result.skipped_too_large,
            plural(u64::from(result.skipped_too_large), "item", "items")
        ));
    }

    let classified = result
        .skipped_duplicate
        .saturating_add(result.skipped_empty)
        .saturating_add(result.skipped_too_large);
    let unclassified = result.skipped.saturating_sub(classified);
    if unclassified > 0 {
        reasons.push(format!(
            "{unclassified} {} without a reported reason",
            plural(u64::from(unclassified), "item", "items")
        ));
    }

    if reasons.is_empty() {
        return String::new();
    }
    format!(": {}", reasons.join(", "))
}

fn encode_export(export: &ExportData) -> Result<String, CliError> {
    serde_json::to_string_pretty(export)
        .map_err(|e| CliError::local(format!("could not render the export: {e}")))
}

fn write_export(path: &Path, export: &ExportData) -> Result<(), CliError> {
    use std::io::Write;

    let encoded = encode_export(export)?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    // An export is a whole clipboard history, so it is created owner-only
    // rather than tightened afterwards. **Unverified on Windows**: the file
    // inherits the destination directory's ACL, which is owner-only under the
    // user profile and is not wherever else the user points `--out`. Setting a
    // DACL needs a design decision this cfg deliberately does not make.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(path)
        .map_err(|e| CliError::local(format!("could not create the export: {e}")))?;
    file.write_all(format!("{encoded}\n").as_bytes())
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

    fn import_data(
        inserted: u32,
        skipped_duplicate: u32,
        skipped_empty: u32,
        skipped_too_large: u32,
    ) -> ImportData {
        ImportData {
            inserted,
            skipped: skipped_duplicate + skipped_empty + skipped_too_large,
            skipped_duplicate,
            skipped_empty,
            skipped_too_large,
            pins_failed: 0,
        }
    }

    #[test]
    fn plurals_agree_with_their_count() {
        assert_eq!(plural(1, "item", "items"), "item");
        assert_eq!(plural(0, "item", "items"), "items");
        assert_eq!(plural(2, "item", "items"), "items");
    }

    #[test]
    fn a_mixed_import_names_every_skip_reason_exactly() {
        assert_eq!(
            import_summary(&import_data(1, 2, 1, 3)),
            "imported 1 item; skipped 6 items: 2 duplicates already present, \
             1 empty or invalid item, 3 items over the size limit"
        );
    }

    #[test]
    fn all_duplicate_over_limit_and_empty_reports_are_exact() {
        assert_eq!(
            import_summary(&import_data(0, 2, 0, 0)),
            "imported 0 items; skipped 2 items: 2 duplicates already present"
        );
        assert_eq!(
            import_summary(&import_data(0, 0, 0, 2)),
            "imported 0 items; skipped 2 items: 2 items over the size limit"
        );
        assert_eq!(
            import_summary(&import_data(0, 0, 2, 0)),
            "imported 0 items; skipped 2 items: 2 empty or invalid items"
        );
        assert_eq!(
            import_summary(&import_data(2, 0, 0, 0)),
            "imported 2 items; skipped 0 items"
        );
    }

    #[test]
    fn an_older_reply_is_not_mislabeled_as_all_duplicates() {
        let older = ImportData {
            inserted: 7,
            skipped: 2,
            skipped_duplicate: 0,
            skipped_empty: 0,
            skipped_too_large: 0,
            pins_failed: 0,
        };
        assert_eq!(
            import_summary(&older),
            "imported 7 items; skipped 2 items: 2 items without a reported reason"
        );
    }

    /// DMY-156: a pin that could not be written is a fact about the import, not
    /// a skip reason — the item is there and unpinned, so it must be named even
    /// when nothing was skipped.
    #[test]
    fn a_pin_that_could_not_be_written_is_named_alongside_the_skips() {
        assert_eq!(
            import_summary(&ImportData {
                pins_failed: 1,
                ..import_data(2, 0, 0, 0)
            }),
            "imported 2 items; skipped 0 items; 1 item could not be pinned"
        );
        assert_eq!(
            import_summary(&ImportData {
                pins_failed: 2,
                ..import_data(3, 1, 0, 0)
            }),
            "imported 3 items; skipped 1 item: 1 duplicate already present; \
             2 items could not be pinned"
        );
    }
}
