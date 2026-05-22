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
    };

    if let Err(e) = result {
        eprintln!("copypaste: {e}");
        std::process::exit(1);
    }
}
