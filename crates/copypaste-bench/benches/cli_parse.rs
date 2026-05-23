//! Beta-bonus benchmark: `clap` argv parsing for the CopyPaste CLI.
//!
//! Measures the cost of `Parser::parse_from(&[..])` for the hot commands
//! invoked on every shell prompt completion / shell wrapper invocation:
//!
//!   * `copypaste pin <id>`
//!   * `copypaste history` (alias of `list`)
//!   * `copypaste export --limit 1000 --output dump.json`
//!   * `copypaste import --dedup dump.json`
//!   * `copypaste daemon start`
//!
//! ## Why a local Parser mirror?
//!
//! `copypaste-cli` is a binary-only crate (no `[lib]` target), so the real
//! `Cli` struct in `crates/copypaste-cli/src/main.rs` is not importable from
//! another crate. Per the bench task constraints we must NOT touch other
//! crates, so we mirror the grammar of the hot commands locally here.
//!
//! The mirror is intentionally **shape-faithful** (same subcommand names,
//! same flag types: `String`, `u64`, `bool`) so the clap-derive codegen we
//! exercise is representative of what the production CLI runs. If the real
//! CLI grammar drifts, this bench will under-/over-estimate but will keep
//! compiling — that is the correct tradeoff for a perf harness.

use clap::{Parser, Subcommand};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

// ---------------------------------------------------------------------------
// Local CLI mirror — shape-faithful to crates/copypaste-cli/src/main.rs
// for the five commands the bench covers. Other commands omitted on purpose
// to keep the parser graph small.
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "copypaste", version, about = "CopyPaste CLI (bench mirror)")]
struct BenchCli {
    #[command(subcommand)]
    command: BenchCommands,
}

#[derive(Subcommand)]
enum BenchCommands {
    /// Pin a clipboard item by ID.
    Pin {
        /// Item ID (UUID).
        id: String,
    },
    /// List clipboard history (alias: `history`).
    #[command(alias = "history")]
    List {
        #[arg(long, default_value_t = 50)]
        limit: u64,
        #[arg(long, default_value_t = 0)]
        offset: u64,
    },
    /// Export clipboard history as JSON.
    Export {
        #[arg(long, default_value_t = 1000)]
        limit: u64,
        #[arg(long, short)]
        output: Option<String>,
        #[arg(long, short)]
        force: bool,
    },
    /// Import clipboard items from a JSON file.
    Import {
        /// Path to JSON file.
        file: String,
        /// Skip rows whose content hash already exists.
        #[arg(long)]
        dedup: bool,
    },
    /// Manage the background daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    Start,
    Stop,
    Restart,
    Install,
    Uninstall,
}

// ---------------------------------------------------------------------------
// Argv fixtures — owned `String` vectors so the bench loop only measures the
// `parse_from` call, not allocation of the input.
// ---------------------------------------------------------------------------

fn argv_pin() -> Vec<String> {
    vec![
        "copypaste".into(),
        "pin".into(),
        "11111111-2222-3333-4444-555555555555".into(),
    ]
}

fn argv_history() -> Vec<String> {
    vec!["copypaste".into(), "history".into()]
}

fn argv_export() -> Vec<String> {
    vec![
        "copypaste".into(),
        "export".into(),
        "--limit".into(),
        "1000".into(),
        "--output".into(),
        "dump.json".into(),
    ]
}

fn argv_import_dedup() -> Vec<String> {
    vec![
        "copypaste".into(),
        "import".into(),
        "--dedup".into(),
        "dump.json".into(),
    ]
}

fn argv_daemon_start() -> Vec<String> {
    vec!["copypaste".into(), "daemon".into(), "start".into()]
}

// ---------------------------------------------------------------------------
// Benches
// ---------------------------------------------------------------------------

fn bench_parse_each(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_parse_each");
    let fixtures: &[(&str, fn() -> Vec<String>)] = &[
        ("pin", argv_pin),
        ("history", argv_history),
        ("export", argv_export),
        ("import_dedup", argv_import_dedup),
        ("daemon_start", argv_daemon_start),
    ];
    for &(label, builder) in fixtures {
        let argv = builder();
        // One "element" = one full argv parse. Useful for "how many CLI
        // invocations per second can a single core handle?" questions.
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &argv, |b, argv| {
            b.iter(|| {
                let cli = BenchCli::parse_from(black_box(argv));
                black_box(cli);
            });
        });
    }
    group.finish();
}

fn bench_parse_all_sequence(c: &mut Criterion) {
    // Sanity bench: parse every fixture once per iteration. Mirrors the cost
    // a shell completion script would pay if it had to validate several
    // subcommands in one shot (e.g. building a wrapper around the binary).
    let argvs: Vec<Vec<String>> = vec![
        argv_pin(),
        argv_history(),
        argv_export(),
        argv_import_dedup(),
        argv_daemon_start(),
    ];
    let mut group = c.benchmark_group("cli_parse_sequence");
    group.throughput(Throughput::Elements(argvs.len() as u64));
    group.bench_function("all_hot_commands", |b| {
        b.iter(|| {
            for argv in &argvs {
                let cli = BenchCli::parse_from(black_box(argv));
                black_box(cli);
            }
        });
    });
    group.finish();
}

criterion_group!(benches, bench_parse_each, bench_parse_all_sequence);
criterion_main!(benches);
