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
    /// Check if the daemon is running
    Status,
    /// Show total number of stored items
    Count,
    /// Delete a clipboard item by ID
    Delete {
        /// Item ID (UUID)
        id: String,
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
        /// Item ID (UUID)
        id: String,
    },
    /// Export clipboard history as JSON
    Export {
        #[arg(long, default_value_t = 1000)]
        limit: u64,
        #[arg(long, short)]
        output: Option<String>,
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
        Commands::Status => commands::status::run(&socket),
        Commands::Count => commands::count::run(&socket),
        Commands::Delete { id } => commands::delete::run(&socket, &id),
        Commands::Search { query, limit } => commands::search::run(&socket, &query, limit),
        Commands::Copy { id } => commands::copy::run(&socket, &id),
        Commands::Export { limit, output } => commands::export::run(&socket, limit, output.as_deref()),
        Commands::Watch { interval } => commands::watch::run(&socket, interval),
        Commands::Clear { force } => commands::clear::run(&socket, force),
        Commands::Stats => commands::stats::run(&socket),
        Commands::Import { file } => commands::import::run(&socket, &file),
        Commands::Private { action } => match action {
            PrivateAction::On => commands::private::run(&socket, true),
            PrivateAction::Off => commands::private::run(&socket, false),
            PrivateAction::Status => commands::private::run_get(&socket),
        },
    };

    if let Err(e) = result {
        eprintln!("copypaste: {e}");
        std::process::exit(1);
    }
}
