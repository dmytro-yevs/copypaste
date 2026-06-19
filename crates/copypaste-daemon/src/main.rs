// reason: the binary re-uses all modules from the lib crate (copypaste_daemon::*);
// items not called from main() are exercised by integration tests or the lib's
// own public surface — dead_code here is a false positive from the binary's view.
#![allow(dead_code)]

// Beta-bonus: modules now live in the library half of this crate
// (`src/lib.rs`) so that integration tests under `tests/*.rs` can reach
// them.  The binary re-uses them via the crate's own library name.
use copypaste_daemon::{daemon, paths};

fn main() -> anyhow::Result<()> {
    // Initialise structured logging — daily-rotating file (7-file retention)
    // plus a compact stderr sink for foreground / launchd runs. The guard MUST
    // be kept alive until the process exits so buffered log lines are flushed
    // before the non-blocking writer shuts down.
    use copypaste_daemon::logging;
    let _log_guard = logging::init();

    let support_dir = paths::app_support_dir();
    std::fs::create_dir_all(&support_dir)?;

    // v0.3: the menu-bar tray host moved to `copypaste-ui` (see
    // `crates/copypaste-ui/src/tray_host.rs`). The daemon is launched by
    // launchd and cannot reliably bring up an NSApplication main run loop,
    // which both `tray-icon` and `muda::Menu` require on macOS. The async
    // daemon now runs identically on every platform.
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(daemon::run())
}
