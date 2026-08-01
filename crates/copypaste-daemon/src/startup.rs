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
pub async fn wait_for_shutdown(mut requested: watch::Receiver<bool>) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).context("install the SIGTERM handler")?;
    let mut sigint = signal(SignalKind::interrupt()).context("install the SIGINT handler")?;
    tokio::select! {
        _ = sigterm.recv() => info!("received SIGTERM"),
        _ = sigint.recv() => info!("received SIGINT"),
        _ = requested.wait_for(|stopping| *stopping) => info!("shutdown was requested over ipc"),
    }
    Ok(())
}
