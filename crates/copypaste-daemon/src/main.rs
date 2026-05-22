#![allow(dead_code)]

mod clipboard;
mod daemon;
#[cfg(unix)]
mod ipc;
mod keychain;
mod logging;
mod paths;
mod platform;
mod protocol;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured logging (rotating file + stderr).
    // The guard MUST be kept alive until the process exits so that buffered
    // log lines are flushed before the non-blocking writer shuts down.
    let _log_guard = logging::init();

    let support_dir = paths::app_support_dir();
    std::fs::create_dir_all(&support_dir)?;

    daemon::run().await
}
