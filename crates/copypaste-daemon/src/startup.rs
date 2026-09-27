//! Starting up and shutting down: what `main` does around the work itself.

use std::path::PathBuf;

use crate::server;
use anyhow::Context;
use tokio::sync::watch;
use tracing::{info, warn};

/// `canonical`'s filename, under `dir`. What `--data-dir` relocates.
///
/// The names come from `copypaste_ipc` rather than literals here: a second
/// definition is a second thing to keep in step. The fallback is unreachable —
/// both resolvers end in a filename — and keeps the real path rather than
/// opening a directory.
pub fn relocate(canonical: &std::path::Path, dir: &std::path::Path) -> PathBuf {
    canonical
        .file_name()
        .map_or_else(|| canonical.to_path_buf(), |name| dir.join(name))
}
/// A startup failure that has a fixed sentence holds the socket instead of
/// exiting, so the app hears the condition rather than inferring a crash.
///
/// Every other failure exits exactly as before: there is nothing a client could
/// be told that is more use than the exit status and the log line.
pub async fn halt_or_fail<E>(
    socket_path: &std::path::Path,
    error: E,
    doing: &str,
) -> anyhow::Result<()>
where
    E: crate::server::messages::Refusal + std::error::Error + Send + Sync + 'static,
{
    let Some(refusal) = error.refusal() else {
        return Err(anyhow::Error::new(error).context(doing.to_string()));
    };
    warn!(error = %error, doing, "cannot serve a history; holding the socket to say why");

    let stop = watch::channel(false).0;
    let served = tokio::spawn(server::serve_halted(
        server::bind(socket_path)?,
        refusal,
        stop.clone(),
    ));
    wait_for_shutdown(stop.subscribe()).await?;
    let _ = stop.send(true);
    if let Err(e) = served.await {
        warn!(error = ?e, "the halted ipc server did not shut down cleanly");
    }
    remove_socket(socket_path);
    Ok(())
}

pub fn remove_socket(socket_path: &std::path::Path) {
    match std::fs::remove_file(socket_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => warn!(error = %e, "could not remove the socket on shutdown"),
    }
}

/// Resolves on SIGINT, SIGTERM, or a client's `shutdown` request.
///
/// launchd sends SIGTERM; a terminal sends SIGINT; the app sends the IPC verb,
/// because it cannot signal a process it did not start. All three must unwind
/// the same way — an aborted process leaves the socket file behind and the next
/// start has to treat it as stale.
#[cfg(unix)]
pub async fn wait_for_shutdown(mut requested: watch::Receiver<bool>) -> anyhow::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).context("install the SIGTERM handler")?;
    let mut sigint = signal(SignalKind::interrupt()).context("install the SIGINT handler")?;
    tokio::select! {
        _ = sigterm.recv() => info!("received SIGTERM"),
        _ = sigint.recv() => info!("received SIGINT"),
        _ = requested.wait_for(|stopping| *stopping) => info!("shutdown was requested over ipc"),
    }
    Ok(())
}

/// The same three ways in, spelled in console control events.
///
/// Windows has no SIGTERM: the Service Control Manager and a closing console
/// both arrive as `CTRL_CLOSE`/`CTRL_SHUTDOWN`, and the process is killed a few
/// seconds later whatever the handler does — so the teardown this unblocks has
/// to stay short.
#[cfg(windows)]
pub async fn wait_for_shutdown(mut requested: watch::Receiver<bool>) -> anyhow::Result<()> {
    use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_shutdown};

    let mut closing = ctrl_close().context("install the console-close handler")?;
    let mut stopping = ctrl_shutdown().context("install the system-shutdown handler")?;
    let mut interrupted = ctrl_break().context("install the Ctrl-Break handler")?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => info!("received Ctrl-C"),
        _ = interrupted.recv() => info!("received Ctrl-Break"),
        _ = closing.recv() => info!("the console is closing"),
        _ = stopping.recv() => info!("the system is shutting down"),
        _ = requested.wait_for(|stopping| *stopping) => info!("shutdown was requested over ipc"),
    }
    Ok(())
}
