#![allow(dead_code)]

// Beta-bonus: modules now live in the library half of this crate
// (`src/lib.rs`) so that integration tests under `tests/*.rs` can reach
// them.  The binary re-uses them via the crate's own library name.
use copypaste_daemon::{daemon, paths};

#[cfg(target_os = "macos")]
use copypaste_daemon::tray;

fn main() -> anyhow::Result<()> {
    // Initialise structured logging — daily-rotating file in `paths::log_dir()`
    // (7-file retention) plus a compact stdout sink for foreground runs. The
    // guard MUST be kept alive until the process exits so that buffered log
    // lines are flushed before the non-blocking writer shuts down.
    //
    // The shared helper lives in `copypaste-core::logging` so the same rotation
    // policy applies to any future binary (CLI long-running modes, agents…).
    let log_dir = paths::log_dir();
    let _log_guard =
        copypaste_core::logging::init_with_file_rotation(&log_dir, "copypaste-daemon");

    let support_dir = paths::app_support_dir();
    std::fs::create_dir_all(&support_dir)?;

    #[cfg(target_os = "macos")]
    {
        run_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On non-macOS platforms run the async daemon directly.
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(daemon::run())
    }
}

/// macOS entry point.
///
/// On macOS, AppKit / Cocoa requires that the tray icon event loop runs on the
/// **main thread**. We therefore:
///
/// 1. Spin up a `tokio` multi-thread runtime on a background OS thread.
/// 2. Submit the async daemon task to it.
/// 3. Run the tray event loop on the main thread.
/// 4. When the tray quits, signal the daemon to shut down.
#[cfg(target_os = "macos")]
fn run_macos() -> anyhow::Result<()> {
    use std::sync::Arc;

    // Shared state between tray (main thread) and daemon (background thread).
    let state = Arc::new(tray::TrayState::new());
    let daemon_state = state.clone();

    // Build the tokio runtime that will host the async daemon.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Spawn the async daemon on the background runtime.
    // We hold a JoinHandle so we can wait for clean shutdown.
    let daemon_handle = rt.spawn(async move {
        if let Err(e) = daemon::run_with_quit_flag(daemon_state.quit_requested.clone()).await {
            tracing::error!("daemon error: {e}");
        }
    });

    // Run the tray icon on the main thread (blocks until Quit).
    tray::run_tray(state.clone());

    // Tray quit — signal daemon and wait for it to stop.
    state
        .quit_requested
        .store(true, std::sync::atomic::Ordering::Relaxed);

    // Give the daemon a moment to drain in-flight work then shut down the runtime.
    rt.block_on(async {
        match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            daemon_handle,
        )
        .await
        {
            Ok(Ok(())) => tracing::info!("daemon stopped cleanly"),
            Ok(Err(e)) => tracing::warn!("daemon join error: {e}"),
            Err(_) => tracing::warn!("daemon did not stop within 3s; forcing runtime shutdown"),
        }
    });

    Ok(())
}
