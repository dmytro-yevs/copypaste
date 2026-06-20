use clap::{Parser, Subcommand};

mod commands;
mod ipc;
mod paths;

#[derive(Parser)]
#[command(name = "copypaste", version, about = "CopyPaste clipboard history CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List clipboard history
    List {
        /// Maximum number of items to show (default: 50)
        #[arg(long, default_value_t = 50)]
        limit: u64,
        /// Number of items to skip (default: 0)
        #[arg(long, default_value_t = 0)]
        offset: u64,
    },
    /// Check if the daemon is running (prints version, uptime, history count)
    Status {
        /// Output machine-readable JSON instead of a human table
        #[arg(long)]
        json: bool,
    },
    /// Show total number of stored items
    Count,
    /// Delete a clipboard item by ID
    Delete {
        /// Item ID (UUID)
        id: String,
        /// Skip the interactive confirmation prompt
        #[arg(long, short)]
        force: bool,
        /// Print what would be deleted without actually deleting it
        #[arg(long)]
        dry_run: bool,
    },
    /// Search clipboard history
    Search {
        /// Search query
        query: String,
        /// Maximum results (default: 20)
        #[arg(long, default_value_t = 20)]
        limit: u64,
    },
    /// Copy a clipboard item back to the system clipboard
    Copy {
        /// Position in history (1 = most recent). Mutually exclusive with --id and --search.
        #[arg(value_name = "INDEX", conflicts_with_all = ["id", "search", "list"])]
        index: Option<u64>,

        /// Copy item by UUID.
        #[arg(long, conflicts_with_all = ["index", "search", "list"])]
        id: Option<String>,

        /// Fuzzy-search history and copy the first match.
        #[arg(long, value_name = "QUERY", conflicts_with_all = ["index", "id", "list"])]
        search: Option<String>,

        /// List recent history items (numbered) without copying.
        #[arg(long, conflicts_with_all = ["index", "id", "search"])]
        list: bool,

        /// Number of items to consider for INDEX and --list (default: 50).
        #[arg(long, default_value_t = 50)]
        limit: u64,
    },
    /// Export clipboard history as JSON
    Export {
        #[arg(long, default_value_t = 1000)]
        limit: u64,
        #[arg(long, short)]
        output: Option<String>,
        /// Overwrite the output file if it already exists
        #[arg(long, short)]
        force: bool,
        /// Include sensitive (flagged) items in the export.
        /// By default sensitive items are omitted. Use with care — the export
        /// file contains plaintext content; handle and store it securely.
        /// A confirmation prompt will be shown unless --yes is also passed.
        #[arg(long)]
        include_sensitive: bool,
        /// Skip the interactive confirmation prompt when --include-sensitive is
        /// used. Required for non-interactive / scripted invocations.
        /// (CopyPaste-phit: explicit double opt-in for bulk plaintext export.)
        #[arg(long)]
        yes: bool,
    },
    /// Watch clipboard in real-time (prints new items as they arrive)
    Watch {
        /// Poll interval in milliseconds (default: 2000)
        #[arg(long, default_value_t = 2000)]
        interval: u64,
    },
    /// Clear all clipboard history (irreversible)
    Clear {
        /// Skip confirmation prompt
        #[arg(long, short)]
        force: bool,
    },
    /// Show clipboard statistics
    Stats,
    /// Import clipboard items from a JSON file (exported by 'export')
    Import {
        /// Path to JSON file
        file: String,
    },
    /// Enable or disable private/pause mode (daemon stops recording new clipboard changes)
    Private {
        #[command(subcommand)]
        action: PrivateAction,
    },
    /// Pin a clipboard item by ID (removes its TTL so it is never auto-deleted)
    Pin {
        /// Item ID (UUID)
        id: String,
    },
    /// Unpin a clipboard item by ID (restores normal retention)
    Unpin {
        /// Item ID (UUID)
        id: String,
    },
    /// Manage the background daemon (start/stop/restart/install/uninstall)
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Create an encrypted SQLCipher backup of the local database
    Backup {
        /// Directory to write the backup file into (default: `<repo>/backups`)
        #[arg(long, short)]
        output: Option<String>,
        /// Show what would happen without touching disk
        #[arg(long)]
        dry_run: bool,
    },
    /// Restore the local database from a SQLCipher backup file
    Restore {
        /// Path to the backup file (.db.enc)
        path: String,
        /// Delete the existing live DB instead of renaming it aside
        #[arg(long)]
        force: bool,
        /// Show what would happen without touching disk
        #[arg(long)]
        dry_run: bool,
    },
    /// Display a QR code other devices can scan to pair with this one.
    ///
    /// Asks the daemon for a fresh, short-lived pairing token and renders it as
    /// a QR code in the terminal. Scan it from the CopyPaste Android app (or
    /// another desktop) to complete pairing automatically — no typing a code.
    PairQr {
        /// Print the raw payload string instead of rendering the QR code.
        #[arg(long)]
        raw: bool,
    },
    /// Reclaim free pages (VACUUM) and rebuild indexes (REINDEX) in the local DB
    ///
    /// Daemon MUST be running — the operation runs inside the daemon over IPC.
    /// Use `copypaste daemon start` before running this if needed.
    /// Typical reclaim: 10-40% after heavy churn.
    Vacuum {
        /// Print what would happen without modifying the database
        #[arg(long)]
        dry_run: bool,
        /// Skip VACUUM and only run REINDEX (faster, no disk-space requirement)
        #[arg(long)]
        reindex_only: bool,
    },
    /// Configure and diagnose Supabase cloud sync (setup/status/test/setup-sql)
    Cloud {
        #[command(subcommand)]
        action: CloudAction,
    },
}

#[derive(Subcommand)]
enum CloudAction {
    /// Store the Supabase project URL + anon key + account credentials in the daemon config
    Setup {
        /// Supabase project URL (must start with https://)
        #[arg(long)]
        url: String,
        /// Supabase anon/public API key (starts with eyJ…). If omitted, read
        /// from the SUPABASE_ANON_KEY env var or prompted on stdin — passing it
        /// as a flag leaks it into shell history and the process list.
        #[arg(long)]
        anon_key: Option<String>,
        /// Account email for the authenticated GoTrue sign-in (required by RLS)
        #[arg(long)]
        email: String,
        /// Account password. If omitted, read from the SUPABASE_PASSWORD env var
        /// or prompted on stdin — never pass it as a flag (shell-history leak).
        #[arg(long)]
        password: Option<String>,
    },
    /// Show current cloud-sync status (configured / signed in / last sync)
    Status,
    /// Validate the configured Supabase connection end-to-end
    Test,
    /// Print the idempotent provisioning SQL (schema + RLS) for the Supabase SQL Editor
    SetupSql,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon via the installed launchd plist
    Start,
    /// Stop the daemon (launchctl bootout)
    Stop,
    /// Restart the daemon (bootout + bootstrap)
    Restart,
    /// Copy the packaged plist into ~/Library/LaunchAgents/ and start the daemon
    Install,
    /// Stop the daemon and remove the plist from ~/Library/LaunchAgents/
    Uninstall,
}

#[derive(Subcommand)]
enum PrivateAction {
    /// Enable private mode — stop recording new clipboard changes
    On,
    /// Disable private mode — resume recording clipboard changes
    Off,
    /// Show current private mode state
    Status,
}

fn main() {
    let cli = Cli::parse();
    let socket = paths::socket_path();

    let result = match cli.command {
        Commands::List { limit, offset } => commands::list::run(&socket, limit, offset),
        Commands::Status { json } => commands::status::run(&socket, json),
        Commands::Count => commands::count::run(&socket),
        Commands::Delete { id, force, dry_run } => {
            commands::delete::run(&socket, &id, force, dry_run)
        }
        Commands::Search { query, limit } => commands::search::run(&socket, &query, limit),
        Commands::Copy {
            index,
            id,
            search,
            list,
            limit,
        } => commands::copy::run(
            &socket,
            index,
            id.as_deref(),
            search.as_deref(),
            list,
            limit,
        ),
        Commands::Export {
            limit,
            output,
            force,
            include_sensitive,
            yes,
        } => commands::export::run(
            &socket,
            limit,
            output.as_deref(),
            force,
            include_sensitive,
            yes,
        ),
        Commands::Watch { interval } => commands::watch::run(&socket, interval),
        Commands::Clear { force } => commands::clear::run(&socket, force),
        Commands::Stats => commands::stats::run(&socket),
        Commands::Import { file } => commands::import::run(&socket, &file),
        Commands::Private { action } => match action {
            PrivateAction::On => commands::private::run(&socket, true),
            PrivateAction::Off => commands::private::run(&socket, false),
            PrivateAction::Status => commands::private::run_get(&socket),
        },
        Commands::Pin { id } => commands::pin::run_pin(&socket, &id),
        Commands::Unpin { id } => commands::pin::run_unpin(&socket, &id),
        Commands::Daemon { action } => {
            let act = match action {
                DaemonAction::Start => commands::daemon::DaemonAction::Start,
                DaemonAction::Stop => commands::daemon::DaemonAction::Stop,
                DaemonAction::Restart => commands::daemon::DaemonAction::Restart,
                DaemonAction::Install => commands::daemon::DaemonAction::Install,
                DaemonAction::Uninstall => commands::daemon::DaemonAction::Uninstall,
            };
            commands::daemon::run(act)
        }
        Commands::Backup { output, dry_run } => {
            commands::backup::run_backup(&socket, output.as_deref(), dry_run)
        }
        Commands::Restore {
            path,
            force,
            dry_run,
        } => commands::backup::run_restore(&socket, &path, force, dry_run),
        Commands::PairQr { raw } => commands::pair_qr::run(&socket, raw),
        Commands::Vacuum {
            dry_run,
            reindex_only,
        } => commands::vacuum::run(
            &socket,
            commands::vacuum::Plan {
                dry_run,
                reindex_only,
            },
        ),
        Commands::Cloud { action } => match action {
            CloudAction::Setup {
                url,
                anon_key,
                email,
                password,
            } => commands::cloud::setup(&socket, &url, anon_key, &email, password),
            CloudAction::Status => commands::cloud::status(&socket),
            CloudAction::Test => commands::cloud::test(&socket),
            CloudAction::SetupSql => commands::cloud::setup_sql(),
        },
    };

    if let Err(e) = result {
        eprintln!("copypaste: {e}");
        // CopyPaste-liaz: process::exit(1) is safe here — `result` is already
        // consumed (moved into the Err variant pattern) and all local variables
        // in the match arms have been dropped before reaching this point.
        // Callers that hold Zeroizing<…> secrets (e.g. cloud::setup) must NOT
        // call process::exit while secrets are in scope; they return Err instead,
        // which unwinds the call stack and runs Drop before arriving here.
        std::process::exit(1);
    }
}
