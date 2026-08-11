//! The command surface: every verb, flag and default, in one place.
//!
//! Separated from `main` so that the mapping onto [`copypaste_ipc::Method`] and
//! the rendering of a reply are readable without scrolling past three hundred
//! lines of `#[arg]`. The tests that pin the defaults and the required
//! arguments move with the definitions they are about.

use clap::{Parser, Subcommand};
use copypaste_ipc::{ConfigPatch, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "copypaste",
    version,
    about = "Clipboard history, from the terminal.",
    long_about = "Clipboard history, from the terminal.\n\n\
                  Talks to the CopyPaste daemon over a local socket; start it with \
                  `copypaste-daemon` if commands report that it is unreachable."
)]
pub(crate) struct Cli {
    /// Print the daemon's raw response as JSON, for scripting.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Show recent clipboard items, newest first. Pinned items sort ahead.
    List {
        /// How many items to show.
        #[arg(long, short = 'n', default_value_t = DEFAULT_LIST_PAGE, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
        /// Continue from where a previous `list` stopped.
        ///
        /// A marker, not a row number: the history grows at the top while it is
        /// being read, so skipping N rows would show one twice and another not
        /// at all. `list` prints the marker for the next page when there is one.
        #[arg(long)]
        cursor: Option<String>,
    },

    /// Full-text search over clipboard history.
    ///
    /// Sensitive items are never indexed, so they never appear in results.
    Search {
        /// What to search for.
        query: String,
        /// How many matches to show.
        #[arg(long, short = 'n', default_value_t = DEFAULT_SEARCH_PAGE, value_parser = clap::value_parser!(u32).range(1..))]
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

    /// Set the order pinned items are shown in.
    ///
    /// Takes the whole pinned list, first to last. Ids that are not pinned, or
    /// no longer exist, are ignored; pinned items not named keep their order
    /// and move behind the ones that are.
    ///
    /// The order is local to this device — a pin never travels to a peer.
    Reorder {
        /// The pinned item ids, in the order wanted.
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },

    /// Report whether the daemon is running and what it is doing.
    Status,

    /// Stop the daemon.
    ///
    /// It finishes what it is doing, removes its socket, and exits — the same
    /// unwind a `SIGTERM` produces. History is untouched.
    Shutdown,

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
    /// unpairs. The code that made the pairing still works, so re-entering it
    /// pairs the two devices again — use `revoke` for a device you no longer
    /// have.
    Unpair {
        /// The pairing id, as shown by `copypaste peers`.
        pairing_id: String,
    },

    /// Cut a device off for good, so its code can never be used again.
    ///
    /// What a lost or stolen device needs, and what `unpair` deliberately is
    /// not: the pairing id is refused from here on, so a code that was written
    /// down or a stale copy of the device list cannot bring the pairing back.
    /// It cannot be undone, and the other device is not told — pairing the two
    /// again means a new code on both.
    Revoke {
        /// The pairing id, as shown by `copypaste peers`.
        pairing_id: String,

        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
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

    /// Show devices visible on the local network.
    Discover {
        /// Re-advertise and browse again before answering.
        #[arg(long)]
        rescan: bool,
    },

    /// Write clipboard history to a file, as JSON.
    ///
    /// Sensitive items are withheld unless `--include-sensitive` is passed, and
    /// everything left out is reported on stderr rather than hidden.
    Export {
        /// Where to write. Written to stdout when omitted.
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<PathBuf>,
        /// Export at most this many items, newest first. All of them by default.
        #[arg(long, short = 'n', default_value_t = 0)]
        limit: u32,
        /// Include items the detector flagged.
        ///
        /// An export is a plaintext file. Passing this puts credentials in it.
        #[arg(long)]
        include_sensitive: bool,
    },

    /// Read a file written by `copypaste export` back into history.
    ///
    /// Every item goes through the same checks a copy does, so the detector
    /// runs again and duplicates collapse.
    Import {
        /// The file to read. Read from stdin when omitted.
        file: Option<PathBuf>,
    },

    /// Copy the encrypted database to a file.
    ///
    /// The backup is readable only with this device's key, and only by this
    /// user. It will not overwrite an existing file.
    Backup {
        /// Where to write the backup.
        dest: PathBuf,
    },

    /// Replace this device's history with a backup.
    ///
    /// The backup is checked against this device's key before anything is
    /// replaced, so a damaged or foreign file leaves history untouched.
    Restore {
        /// The backup to read.
        src: PathBuf,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Read or change the daemon's settings.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Print a line every time history changes, until interrupted.
    ///
    /// A push subscription, not a poll: the daemon writes when something
    /// happens and this process is idle in between.
    Watch,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigAction {
    /// Print every setting and its value.
    Show,
    /// Change one or more settings. A rejected value changes nothing.
    Set {
        /// How often the clipboard is polled, in milliseconds.
        #[arg(long)]
        poll_interval_ms: Option<u64>,
        /// How many items to keep before evicting the oldest unpinned ones.
        #[arg(long)]
        history_limit: Option<u32>,
        /// Maximum bytes held by unpinned items. Minimum: 50 MiB.
        #[arg(long)]
        storage_quota_bytes: Option<u64>,
        /// Delete items older than this many days. 0 disables it.
        #[arg(long)]
        retention_days: Option<u32>,
        /// Treat two identical copies within this many seconds as one item.
        #[arg(long)]
        dedup_window_secs: Option<u32>,
        /// Ignore text copies larger than this many bytes. Minimum: 64 KiB.
        #[arg(long)]
        max_text_size_bytes: Option<u64>,
        /// Ignore encoded images larger than this many bytes. Minimum: 1 MiB.
        #[arg(long)]
        max_image_size_bytes: Option<u64>,
        /// Ignore files larger than this many bytes. Range: 1-100 MiB.
        #[arg(long)]
        max_file_size_bytes: Option<u64>,
        /// Maximum decoded image memory in MiB. Minimum: 1 MiB.
        #[arg(long)]
        max_decoded_image_mb: Option<u32>,
        /// Delete a flagged item after this many seconds. 0 disables it.
        #[arg(long)]
        sensitive_ttl_secs: Option<u64>,
        /// Bundle ids never to capture from, comma-separated. Empty clears it.
        #[arg(long, value_name = "IDS")]
        excluded_apps: Option<String>,
        /// Whether to advertise this device on the LAN.
        #[arg(long)]
        lan_visibility: Option<bool>,
        /// Master switch for every sync transport.
        #[arg(long)]
        sync_enabled: Option<bool>,
        /// Post a notification when something is captured in the background.
        ///
        /// The daemon records the preference; the app is what posts it, because
        /// an unbundled background process cannot.
        #[arg(long)]
        notify_on_copy: Option<bool>,
        /// Play a sound when something is captured. macOS only.
        #[arg(long)]
        sound_on_copy: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum CloudAction {
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
pub(crate) enum PairAction {
    /// Mint a pairing code on this device, to be entered on the other one.
    ///
    /// The code is a secret — anyone who has it can pair with this device — and
    /// it is shown exactly once. It is never written to a log or stored in
    /// readable form.
    Create,

    /// Join the ceremony started on another device.
    Join {
        /// The code shown by `copypaste pair create` on the other device.
        code: String,
        /// Where that device is listening, as `host:port`.
        #[arg(long, value_name = "HOST:PORT")]
        addr: String,
    },

    /// Show the current ceremony state and handshake-bound security code.
    Progress,

    /// Confirm that the security code matches the other device.
    Confirm,

    /// Reject the current ceremony because the security code does not match.
    Reject,

    /// Cancel the current ceremony or unused invitation.
    Cancel,
}

/// Turn the `config set` flags into a patch. Absent flags are absent fields, so
/// setting one setting leaves the rest alone.
pub(crate) fn config_patch(action: &ConfigAction) -> ConfigPatch {
    let ConfigAction::Set {
        poll_interval_ms,
        history_limit,
        storage_quota_bytes,
        retention_days,
        dedup_window_secs,
        max_text_size_bytes,
        max_image_size_bytes,
        max_file_size_bytes,
        max_decoded_image_mb,
        sensitive_ttl_secs,
        excluded_apps,
        lan_visibility,
        sync_enabled,
        notify_on_copy,
        sound_on_copy,
    } = action
    else {
        return ConfigPatch::default();
    };
    ConfigPatch {
        private_mode: None,
        poll_interval_ms: *poll_interval_ms,
        history_limit: *history_limit,
        storage_quota_bytes: *storage_quota_bytes,
        retention_days: *retention_days,
        dedup_window_secs: *dedup_window_secs,
        max_text_size_bytes: *max_text_size_bytes,
        max_image_size_bytes: *max_image_size_bytes,
        max_file_size_bytes: *max_file_size_bytes,
        max_decoded_image_mb: *max_decoded_image_mb,
        sensitive_ttl_secs: *sensitive_ttl_secs,
        excluded_app_bundle_ids: excluded_apps.as_ref().map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect()
        }),
        lan_visibility: *lan_visibility,
        sync_enabled: *sync_enabled,
        notify_on_copy: *notify_on_copy,
        sound_on_copy: *sound_on_copy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_defaults_come_from_the_ipc_contract() {
        let Command::List { limit, .. } = Cli::try_parse_from(["copypaste", "list"])
            .expect("list parses with no flags")
            .command
        else {
            panic!("expected list");
        };
        assert_eq!(limit, DEFAULT_LIST_PAGE);

        let Command::Search { limit, .. } = Cli::try_parse_from(["copypaste", "search", "query"])
            .expect("search parses with no flags")
            .command
        else {
            panic!("expected search");
        };
        assert_eq!(limit, DEFAULT_SEARCH_PAGE);
    }

    #[test]
    fn config_set_maps_every_flag_to_the_patch() {
        let cli = Cli::try_parse_from([
            "copypaste",
            "config",
            "set",
            "--poll-interval-ms",
            "250",
            "--history-limit",
            "400",
            "--storage-quota-bytes",
            "52428800",
            "--retention-days",
            "30",
            "--dedup-window-secs",
            "15",
            "--max-text-size-bytes",
            "10485760",
            "--max-image-size-bytes",
            "67108864",
            "--max-file-size-bytes",
            "104857600",
            "--max-decoded-image-mb",
            "50",
            "--sensitive-ttl-secs",
            "60",
            "--excluded-apps",
            "com.example.One, , org.example.Two",
            "--lan-visibility",
            "true",
            "--sync-enabled",
            "false",
            "--notify-on-copy",
            "true",
            "--sound-on-copy",
            "false",
        ])
        .expect("config flags should parse");
        let Command::Config { action } = cli.command else {
            panic!("expected config command");
        };

        assert_eq!(
            config_patch(&action),
            ConfigPatch {
                private_mode: None,
                poll_interval_ms: Some(250),
                history_limit: Some(400),
                storage_quota_bytes: Some(52_428_800),
                retention_days: Some(30),
                dedup_window_secs: Some(15),
                max_text_size_bytes: Some(10_485_760),
                max_image_size_bytes: Some(67_108_864),
                max_file_size_bytes: Some(104_857_600),
                max_decoded_image_mb: Some(50),
                sensitive_ttl_secs: Some(60),
                excluded_app_bundle_ids: Some(vec![
                    "com.example.One".to_string(),
                    "org.example.Two".to_string(),
                ]),
                lan_visibility: Some(true),
                sync_enabled: Some(false),
                notify_on_copy: Some(true),
                sound_on_copy: Some(false),
            }
        );
    }
}
