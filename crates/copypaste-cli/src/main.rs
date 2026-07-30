//! `copypaste` — the command-line client.
//!
//! The CLI speaks IPC and nothing else: it does not depend on
//! `copypaste-core`, so it cannot open the database, hold a key, or decide what
//! is sensitive. Everything it knows comes back over the socket as typed
//! [`copypaste_ipc`] values.
//!
//! Layout:
//! * [`client`] — socket, framing, typed request/response
//! * [`render`] — human-readable output (pure functions)
//! * [`cloud`] — where `cloud sign-in` gets its two secrets, and why not argv
//! * [`error`] — exit codes and the "no paths in errors" rule

#![forbid(unsafe_code)]

mod cli;
mod client;
mod cloud;
mod error;
mod render;

use clap::Parser;
use cli::{config_patch, Cli, CloudAction, Command, ConfigAction, PairAction};
use copypaste_ipc::{ExportData, Method};
use error::CliError;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let code = match run(cli).await {
        Ok(()) => error::EXIT_OK,
        Err(err) => {
            eprintln!("error: {}", err.user_message());
            err.exit_code()
        }
    };
    // `process::exit` skips destructors, and stdout is buffered.
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

async fn run(cli: Cli) -> Result<(), CliError> {
    // Two commands are not one request and one reply, so they are handled
    // before the mapping below rather than bent into it.
    if let Command::Watch = &cli.command {
        return watch(cli.json).await;
    }

    let method = match &cli.command {
        Command::List { limit, offset } => Method::List {
            limit: *limit,
            offset: *offset,
        },
        Command::Search { query, limit } => Method::Search {
            query: query.clone(),
            limit: *limit,
        },
        Command::Add { text } => Method::Add {
            content: read_content(text.as_deref())?,
        },
        Command::Copy { id } => Method::Copy { id: id.clone() },
        Command::Delete { id } => Method::Delete { id: id.clone() },
        Command::Clear { yes } => {
            // Destroying history needs an explicit decision, and a piped stdin
            // is not one (CLAUDE.md rule 4: data loss is the worst outcome).
            if !yes && !confirm_clear()? {
                out("cancelled; nothing was deleted");
                return Ok(());
            }
            Method::DeleteAll
        }
        Command::Pin { id } => Method::Pin {
            id: id.clone(),
            pinned: true,
        },
        Command::Unpin { id } => Method::Pin {
            id: id.clone(),
            pinned: false,
        },
        Command::Status => Method::Status,
        Command::Pair { action } => match action {
            PairAction::Create { name } => Method::PairCreate { name: name.clone() },
            PairAction::Accept { code, addr } => Method::PairAccept {
                code: code.clone(),
                addr: addr.clone(),
            },
        },
        Command::Peers => Method::Peers,
        Command::Unpair { pairing_id } => Method::Unpair {
            pairing_id: pairing_id.clone(),
        },
        Command::Sync { peer } => Method::SyncNow {
            pairing_id: peer.clone(),
        },
        Command::Discover { rescan } => {
            if *rescan {
                Method::Rescan
            } else {
                Method::Discovered
            }
        }
        Command::Export {
            limit,
            include_sensitive,
            ..
        } => Method::Export {
            limit: *limit,
            include_sensitive: *include_sensitive,
        },
        Command::Import { file } => Method::Import {
            items: read_export(file.as_deref())?.items,
        },
        Command::Backup { dest } => Method::Backup {
            dest_path: dest.to_string_lossy().into_owned(),
        },
        Command::Restore { src, yes } => {
            // Restoring replaces every item on this device. A piped stdin is
            // not a decision (CLAUDE.md rule 4: data loss is the worst
            // outcome).
            if !yes && !confirm_restore()? {
                out("cancelled; nothing was changed");
                return Ok(());
            }
            Method::Restore {
                src_path: src.to_string_lossy().into_owned(),
                confirm: true,
            }
        }
        Command::Config { action } => match action {
            ConfigAction::Show => Method::GetConfig,
            ConfigAction::Set { .. } => Method::SetConfig {
                patch: config_patch(action),
            },
        },
        Command::Watch => unreachable!("handled above"),
        Command::Cloud { action } => match action {
            CloudAction::SignIn { email } => {
                let (password, passphrase) = cloud::read_credentials()?;
                Method::CloudSignIn {
                    email: email.clone(),
                    password,
                    passphrase,
                }
            }
            CloudAction::SignOut => Method::CloudSignOut,
            CloudAction::Status => Method::CloudStatus,
            CloudAction::Sync => Method::CloudSyncNow,
        },
    };

    let response = client::request(method).await?;

    if cli.json {
        // Printed before the ok/error split so a failing command still yields a
        // machine-readable answer; the exit code below still reflects it.
        let text = serde_json::to_string_pretty(&response)
            .map_err(|e| CliError::local(format!("could not render the reply as JSON: {e}")))?;
        out(&text);
    }

    let data = client::into_data(response)?;
    if cli.json {
        return Ok(());
    }

    match &cli.command {
        Command::List { .. } => {
            let page = client::expect_page(data)?;
            out(&(render::items_table(&page.items, now_ms(), "no items yet")));
            warn_unreadable(page.skipped_undecryptable);
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
        Command::Clear { .. } => match client::optional_count(&data) {
            Some(count) => out(&format!(
                "deleted {count} {}",
                plural(count, "item", "items")
            )),
            None => out("cleared"),
        },
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
        Command::Watch => unreachable!("handled above"),
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

/// Subscribe and print a line per change until interrupted.
async fn watch(json: bool) -> Result<(), CliError> {
    client::watch(|event| {
        let line = if json {
            serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string())
        } else {
            render::event_text(&event)
        };
        out(&line);
        true
    })
    .await
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

/// Read an export file, or stdin.
fn read_export(path: Option<&Path>) -> Result<ExportData, CliError> {
    let raw = match path {
        Some(path) => std::fs::read_to_string(path).map_err(|e| {
            // The user supplied the path, so naming *what went wrong* is
            // useful; the path itself is scrubbed on the way out by
            // `CliError::user_message` (CLAUDE.md rule 4).
            CliError::local(format!("could not read the import file: {e}"))
        })?,
        None => {
            let stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Err(CliError::local(
                    "nothing to import: pass a file, or pipe one on stdin",
                ));
            }
            let mut buffer = String::new();
            stdin
                .lock()
                .read_to_string(&mut buffer)
                .map_err(|e| CliError::local(format!("could not read stdin: {e}")))?;
            buffer
        }
    };

    serde_json::from_str(&raw).map_err(|e| {
        CliError::local(format!(
            "that file is not a CopyPaste export: {e}. Expected the JSON written by `copypaste export`."
        ))
    })
}

fn write_export(path: &Path, encoded: &str) -> Result<(), CliError> {
    std::fs::write(path, format!("{encoded}\n"))
        .map_err(|e| CliError::local(format!("could not write the export: {e}")))
}

fn confirm_restore() -> Result<bool, CliError> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(CliError::local(
            "refusing to replace this device's history without confirmation: pass --yes",
        ));
    }
    eprint!("Replace this device's clipboard history with the backup? [y/N] ");
    let _ = std::io::stderr().flush();

    let mut answer = String::new();
    stdin
        .read_line(&mut answer)
        .map_err(|e| CliError::local(format!("could not read the answer: {e}")))?;
    Ok(is_affirmative(&answer))
}

/// Write one block to stdout, treating a closed pipe as a normal end.
///
/// `println!` panics when its write fails, so `copypaste list | head -1` used to
/// end in a Rust backtrace instead of behaving like every other Unix tool. A
/// downstream reader that stopped reading is not this program's failure: exit
/// quietly, the way SIGPIPE would have.
fn out(text: &str) {
    use std::io::ErrorKind;
    let mut stdout = std::io::stdout().lock();
    let wrote = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.write_all(b"\n"));
    if let Err(e) = wrote {
        if e.kind() == ErrorKind::BrokenPipe {
            std::process::exit(error::EXIT_OK);
        }
        // Anything else — a full disk, a closed descriptor — is worth saying,
        // once, on the stream that is still open.
        eprintln!("error: could not write output: {e}");
        std::process::exit(error::EXIT_OTHER);
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn plural(count: u64, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 {
        one
    } else {
        many
    }
}

/// The content for `add`: the argument if given, otherwise stdin.
fn read_content(text: Option<&str>) -> Result<String, CliError> {
    if let Some(text) = text {
        if text.is_empty() {
            return Err(CliError::local("nothing to add: TEXT is empty"));
        }
        return Ok(text.to_string());
    }

    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        // Blocking on an interactive terminal looks like a hang.
        return Err(CliError::local(
            "nothing to add: pass the text as an argument, or pipe it on stdin",
        ));
    }

    let mut buffer = String::new();
    stdin
        .lock()
        .read_to_string(&mut buffer)
        .map_err(|e| CliError::local(format!("could not read stdin: {e}")))?;

    let content = strip_one_trailing_newline(&buffer);
    if content.is_empty() {
        return Err(CliError::local("nothing to add: stdin was empty"));
    }
    Ok(content.to_string())
}

/// Drop the single trailing newline a shell adds, and nothing else.
///
/// `echo hi | copypaste add` should store `hi`, but content that genuinely ends
/// in blank lines must keep them.
fn strip_one_trailing_newline(input: &str) -> &str {
    input
        .strip_suffix("\r\n")
        .or_else(|| input.strip_suffix('\n'))
        .unwrap_or(input)
}

fn confirm_clear() -> Result<bool, CliError> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(CliError::local(
            "refusing to delete every item without confirmation: pass --yes",
        ));
    }
    // Prompt on stderr so `copypaste clear > file` still captures only output.
    eprint!("Delete all clipboard items? This cannot be undone. [y/N] ");
    let _ = std::io::stderr().flush();

    let mut answer = String::new();
    stdin
        .read_line(&mut answer)
        .map_err(|e| CliError::local(format!("could not read the answer: {e}")))?;
    Ok(is_affirmative(&answer))
}

/// Only an explicit yes counts. Anything else, including empty, is no.
fn is_affirmative(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("should parse")
    }

    #[test]
    fn clap_definition_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn list_has_sensible_defaults() {
        match parse(&["copypaste", "list"]).command {
            Command::List { limit, offset } => {
                assert_eq!(limit, 50);
                assert_eq!(offset, 0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn list_accepts_limit_and_offset() {
        match parse(&["copypaste", "list", "--limit", "5", "--offset", "10"]).command {
            Command::List { limit, offset } => {
                assert_eq!(limit, 5);
                assert_eq!(offset, 10);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_zero_limit_is_rejected_rather_than_silently_corrected() {
        assert!(Cli::try_parse_from(["copypaste", "list", "--limit", "0"]).is_err());
    }

    #[test]
    fn search_requires_a_query_and_defaults_its_limit() {
        assert!(Cli::try_parse_from(["copypaste", "search"]).is_err());
        match parse(&["copypaste", "search", "token"]).command {
            Command::Search { query, limit } => {
                assert_eq!(query, "token");
                assert_eq!(limit, 20);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn json_is_accepted_before_and_after_the_subcommand() {
        assert!(parse(&["copypaste", "list", "--json"]).json);
        assert!(parse(&["copypaste", "--json", "list"]).json);
        assert!(!parse(&["copypaste", "list"]).json);
        assert!(parse(&["copypaste", "status", "--json"]).json);
    }

    #[test]
    fn add_text_is_optional_so_stdin_can_supply_it() {
        match parse(&["copypaste", "add"]).command {
            Command::Add { text } => assert!(text.is_none()),
            other => panic!("{other:?}"),
        }
        match parse(&["copypaste", "add", "hello"]).command {
            Command::Add { text } => assert_eq!(text.as_deref(), Some("hello")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn id_taking_commands_require_an_id() {
        for verb in ["copy", "delete", "pin", "unpin"] {
            assert!(
                Cli::try_parse_from(["copypaste", verb]).is_err(),
                "{verb} should require an id"
            );
        }
    }

    #[test]
    fn pin_and_unpin_are_the_same_method_with_opposite_state() {
        let pin = parse(&["copypaste", "pin", "abc"]).command;
        let unpin = parse(&["copypaste", "unpin", "abc"]).command;
        assert!(matches!(pin, Command::Pin { .. }));
        assert!(matches!(unpin, Command::Unpin { .. }));
    }

    #[test]
    fn clear_takes_yes() {
        match parse(&["copypaste", "clear"]).command {
            Command::Clear { yes } => assert!(!yes),
            other => panic!("{other:?}"),
        }
        match parse(&["copypaste", "clear", "--yes"]).command {
            Command::Clear { yes } => assert!(yes),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_subcommand_is_required() {
        assert!(Cli::try_parse_from(["copypaste"]).is_err());
    }

    #[test]
    fn unknown_subcommands_are_rejected() {
        assert!(Cli::try_parse_from(["copypaste", "nuke"]).is_err());
    }

    #[test]
    fn stdin_loses_exactly_one_trailing_newline() {
        assert_eq!(strip_one_trailing_newline("hi\n"), "hi");
        assert_eq!(strip_one_trailing_newline("hi\r\n"), "hi");
        assert_eq!(strip_one_trailing_newline("hi\n\n"), "hi\n");
        assert_eq!(strip_one_trailing_newline("hi"), "hi");
    }

    #[test]
    fn explicit_empty_text_is_refused() {
        let err = read_content(Some("")).unwrap_err();
        assert_eq!(err.exit_code(), error::EXIT_OTHER);
    }

    #[test]
    fn whitespace_only_text_is_still_content() {
        // A tab or a run of spaces is a legitimate thing to keep on a clipboard.
        assert_eq!(read_content(Some("\t")).unwrap(), "\t");
    }

    #[test]
    fn only_an_explicit_yes_confirms_a_clear() {
        for yes in ["y", "Y", "yes", "YES", " yes \n"] {
            assert!(is_affirmative(yes), "{yes:?}");
        }
        for no in ["", "\n", "n", "no", "sure", "yep", "yeah"] {
            assert!(!is_affirmative(no), "{no:?}");
        }
    }

    #[test]
    fn pair_create_defaults_its_name_and_accepts_one() {
        match parse(&["copypaste", "pair", "create"]).command {
            Command::Pair {
                action: PairAction::Create { name },
            } => assert_eq!(name, "unnamed device"),
            other => panic!("{other:?}"),
        }
        match parse(&["copypaste", "pair", "create", "--name", "phone"]).command {
            Command::Pair {
                action: PairAction::Create { name },
            } => assert_eq!(name, "phone"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn pair_accept_requires_both_a_code_and_an_address() {
        assert!(Cli::try_parse_from(["copypaste", "pair", "accept"]).is_err());
        assert!(Cli::try_parse_from(["copypaste", "pair", "accept", "CODE"]).is_err());
        match parse(&[
            "copypaste",
            "pair",
            "accept",
            "ABCD-EFGH",
            "--addr",
            "127.0.0.1:47654",
        ])
        .command
        {
            Command::Pair {
                action: PairAction::Accept { code, addr },
            } => {
                assert_eq!(code, "ABCD-EFGH");
                assert_eq!(addr, "127.0.0.1:47654");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sync_targets_everything_unless_a_peer_is_named() {
        match parse(&["copypaste", "sync"]).command {
            Command::Sync { peer } => assert!(peer.is_none()),
            other => panic!("{other:?}"),
        }
        match parse(&["copypaste", "sync", "--peer", "abc123"]).command {
            Command::Sync { peer } => assert_eq!(peer.as_deref(), Some("abc123")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unpair_requires_a_pairing_id() {
        assert!(Cli::try_parse_from(["copypaste", "unpair"]).is_err());
        match parse(&["copypaste", "unpair", "abc123"]).command {
            Command::Unpair { pairing_id } => assert_eq!(pairing_id, "abc123"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn peer_commands_map_onto_the_wire_methods() {
        // The mapping is the whole of `run`'s first half; a typo here is a
        // command that silently does something else.
        for (args, expected) in [
            (vec!["copypaste", "peers"], r#""method":"peers""#),
            (vec!["copypaste", "sync"], r#""method":"sync_now""#),
            (vec!["copypaste", "unpair", "x"], r#""method":"unpair""#),
            (
                vec!["copypaste", "pair", "create"],
                r#""method":"pair_create""#,
            ),
        ] {
            let method = method_for(&parse(&args).command).expect("a method");
            let json = serde_json::to_string(&method).unwrap();
            assert!(json.contains(expected), "{json}");
        }
    }

    /// The method a command sends, for the cases that need no stdin or prompt.
    fn method_for(command: &Command) -> Option<Method> {
        Some(match command {
            Command::Peers => Method::Peers,
            Command::Sync { peer } => Method::SyncNow {
                pairing_id: peer.clone(),
            },
            Command::Unpair { pairing_id } => Method::Unpair {
                pairing_id: pairing_id.clone(),
            },
            Command::Pair {
                action: PairAction::Create { name },
            } => Method::PairCreate { name: name.clone() },
            _ => return None,
        })
    }

    #[test]
    fn plurals_read_correctly() {
        assert_eq!(plural(0, "item", "items"), "items");
        assert_eq!(plural(1, "item", "items"), "item");
        assert_eq!(plural(2, "item", "items"), "items");
    }
}
