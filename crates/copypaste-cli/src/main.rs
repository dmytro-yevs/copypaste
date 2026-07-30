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

mod client;
mod cloud;
mod error;
mod render;

use clap::{Parser, Subcommand};
use copypaste_ipc::Method;
use error::CliError;
use std::io::{IsTerminal, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(
    name = "copypaste",
    version,
    about = "Clipboard history, from the terminal.",
    long_about = "Clipboard history, from the terminal.\n\n\
                  Talks to the CopyPaste daemon over a local socket; start it with \
                  `copypaste-daemon` if commands report that it is unreachable."
)]
struct Cli {
    /// Print the daemon's raw response as JSON, for scripting.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show recent clipboard items, newest first. Pinned items sort ahead.
    List {
        /// How many items to show.
        #[arg(long, short = 'n', default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
        /// How many items to skip.
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },

    /// Full-text search over clipboard history.
    ///
    /// Sensitive items are never indexed, so they never appear in results.
    Search {
        /// What to search for.
        query: String,
        /// How many matches to show.
        #[arg(long, short = 'n', default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
    },

    /// Add an item to history without going through the clipboard.
    Add {
        /// The text to add. Read from stdin when omitted.
        text: Option<String>,
    },

    /// Put an item back on the system clipboard.
    Copy {
        /// The item id, as shown by `copypaste list`.
        id: String,
    },

    /// Delete one item.
    Delete {
        /// The item id, as shown by `copypaste list`.
        id: String,
    },

    /// Delete every item. This cannot be undone.
    Clear {
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Pin an item so it stays at the top and survives `clear`.
    Pin {
        /// The item id, as shown by `copypaste list`.
        id: String,
    },

    /// Remove a pin.
    Unpin {
        /// The item id, as shown by `copypaste list`.
        id: String,
    },

    /// Report whether the daemon is running and what it is doing.
    Status,

    /// Pair this device with another one.
    Pair {
        #[command(subcommand)]
        action: PairAction,
    },

    /// List paired devices.
    Peers,

    /// Forget a paired device.
    ///
    /// Local and one-sided: the other device keeps its half until it also
    /// unpairs.
    Unpair {
        /// The pairing id, as shown by `copypaste peers`.
        pairing_id: String,
    },

    /// Sync clipboard history with paired devices.
    Sync {
        /// Sync with one device instead of all of them.
        #[arg(long, value_name = "PAIRING_ID")]
        peer: Option<String>,
    },

    /// Sync clipboard history through a cloud account.
    Cloud {
        #[command(subcommand)]
        action: CloudAction,
    },
}

#[derive(Subcommand, Debug)]
enum CloudAction {
    /// Sign in to the sync account and unlock the sync key.
    ///
    /// The password and the sync passphrase are read from
    /// `COPYPASTE_CLOUD_PASSWORD` and `COPYPASTE_SYNC_PASSPHRASE`, or from
    /// stdin as two lines. There is no flag for either: process arguments are
    /// readable by every process running as this user.
    ///
    /// The passphrase is what the rows are encrypted with, and the backend
    /// never sees it. Use the same one on every device, or each device will
    /// hold rows the others cannot read.
    SignIn {
        /// The account email address.
        #[arg(long, short = 'e', value_name = "EMAIL")]
        email: String,
    },

    /// Forget the account, its tokens and the sync key on this device.
    ///
    /// Local history is untouched, and the daemon keeps the deployment it is
    /// configured with.
    SignOut,

    /// Whether cloud sync is configured, signed in, and when it last ran.
    Status,

    /// Run one cloud sync round now instead of waiting for the poll.
    Sync,
}

#[derive(Subcommand, Debug)]
enum PairAction {
    /// Mint a pairing code on this device, to be entered on the other one.
    ///
    /// The code is a secret — anyone who has it can pair with this device — and
    /// it is shown exactly once. It is never written to a log or stored in
    /// readable form.
    Create {
        /// What to call the other device until it says its own name.
        #[arg(long, short = 'n', default_value = "unnamed device")]
        name: String,
    },

    /// Consume a code from another device and complete the pairing.
    ///
    /// The pairing is only kept if a sync session with that device succeeds, so
    /// a wrong code or an unreachable address leaves nothing behind.
    Accept {
        /// The code shown by `copypaste pair create` on the other device.
        code: String,
        /// Where that device is listening, as `host:port`.
        #[arg(long, value_name = "HOST:PORT")]
        addr: String,
    },
}

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
                println!("cancelled; nothing was deleted");
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
        println!("{text}");
    }

    let data = client::into_data(response)?;
    if cli.json {
        return Ok(());
    }

    match &cli.command {
        Command::List { .. } => {
            let items = client::expect_items(data)?;
            println!("{}", render::items_table(&items, now_ms(), "no items yet"));
        }
        Command::Search { .. } => {
            let items = client::expect_items(data)?;
            println!("{}", render::items_table(&items, now_ms(), "no matches"));
        }
        Command::Status => {
            let status = client::expect_status(data)?;
            println!("{}", render::status_text(&status));
        }
        Command::Add { .. } => match client::optional_item(&data) {
            Some(item) => println!("added {}", item.id),
            None => println!("added"),
        },
        Command::Copy { id } => println!("copied {id} to the clipboard"),
        Command::Delete { id } => println!("deleted {id}"),
        Command::Pin { id } => println!("pinned {id}"),
        Command::Unpin { id } => println!("unpinned {id}"),
        Command::Clear { .. } => match client::optional_count(&data) {
            Some(count) => println!("deleted {count} {}", plural(count, "item", "items")),
            None => println!("cleared"),
        },
        Command::Pair { action } => match action {
            PairAction::Create { .. } => {
                let pairing = client::expect_pairing(data)?;
                // The only place a code is ever rendered. Straight to stdout,
                // never through the logger, and the daemon does not keep a copy
                // that could be asked for again.
                println!("{}", render::pairing_text(&pairing));
            }
            PairAction::Accept { .. } => {
                let peers = client::expect_peers(data)?;
                match peers.first() {
                    Some(peer) => println!("paired with {} ({})", peer.name, peer.pairing_id),
                    None => println!("paired"),
                }
            }
        },
        Command::Peers => {
            let peers = client::expect_peers(data)?;
            println!(
                "{}",
                render::peers_table(&peers, now_ms(), "no paired devices")
            );
        }
        Command::Unpair { pairing_id } => println!("unpaired {pairing_id}"),
        Command::Cloud { action } => match action {
            CloudAction::Sync => {
                let stats = client::expect_cloud_sync(data)?;
                println!("{}", render::cloud_sync_text(&stats));
            }
            // Sign-in, sign-out and status all answer with the new status, so
            // the user sees the state they just moved the daemon into.
            _ => {
                let status = client::expect_cloud_status(data)?;
                println!("{}", render::cloud_status_text(&status, now_ms()));
            }
        },
        Command::Sync { .. } => {
            let results = client::expect_sync(data)?;
            println!("{}", render::sync_table(&results, "no paired devices"));
            // A run where every peer failed is not a success, even though the
            // request itself was answered: a script must be able to tell.
            if !results.is_empty() && results.iter().all(|r| r.error.is_some()) {
                return Err(CliError::local("no peer could be synced"));
            }
        }
    }

    Ok(())
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
