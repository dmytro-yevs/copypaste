//! Starting up and shutting down: what `main` does around the work itself.

use std::path::PathBuf;

use anyhow::Context;
use tokio::sync::watch;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::server;

/// Whether a CopyPaste 0.4 history is on this device.
///
/// **Read-only, and never fatal.** `v1_database_in` opens nothing with a key
/// and writes nothing, and a user who has one still wants v2 to run — they want
/// to be told, not blocked (CLAUDE.md rule 3: the old file stays exactly as it
/// was, so a downgrade finds it intact).
///
/// Two directories, because there are two ways to meet one. Where v0.4.x put it
/// is the upgrade case and the only one that matters on macOS, where the two
/// resolvers disagree. `data_dir` is the `--data-dir` case, and on Linux it is
/// the same directory — which is exactly why v2's *filename* is different.
pub fn legacy_history_present(data_dir: &std::path::Path) -> bool {
    legacy_history_present_in(data_dir, copypaste_ipc::v1_data_dir())
}

fn legacy_history_present_in(data_dir: &std::path::Path, v1_data_dir: Option<PathBuf>) -> bool {
    std::iter::once(data_dir.to_path_buf())
        .chain(v1_data_dir)
        .any(|dir| copypaste_core::v1_database_in(&dir))
}

/// `canonical`'s filename, under `dir`. What `--data-dir` relocates.
///
/// The names come from `copypaste_ipc` rather than literals here: a second
/// definition is a second thing to keep in step, and the v2 database name is
/// what stops a v0.4 file being taken for ours (CLAUDE.md rule 3). The fallback
/// is unreachable — both resolvers end in a filename — and keeps the real path
/// rather than opening a directory.
pub fn relocate(canonical: &std::path::Path, dir: &std::path::Path) -> PathBuf {
    canonical
        .file_name()
        .map_or_else(|| canonical.to_path_buf(), |name| dir.join(name))
}
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
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
#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Store;
    use std::path::Path;

    /// A plaintext v0.4.x history, which is what a user who never upgraded past
    /// the pre-encryption builds still has. An *encrypted* one cannot be staged
    /// — v2 cannot derive v1's key — and `copypaste_core::storage::legacy`
    /// already covers that half.
    fn stage_v1(dir: &Path) {
        let conn = rusqlite::Connection::open(dir.join("clipboard.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_items (
                 id          TEXT PRIMARY KEY NOT NULL,
                 item_id     TEXT,
                 lamport_ts  INTEGER NOT NULL DEFAULT 0,
                 wall_time   INTEGER NOT NULL DEFAULT 0
             );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 12).unwrap();
    }

    /// CLAUDE.md rule 3's second obligation. The identification in
    /// `copypaste-core` was correct and never *asked*: `Store::open` only ever
    /// sees `copypaste-v2.db`, so the question that matters — is an old history
    /// sitting here — had no caller at all (post-merge review, finding 2).
    #[test]
    fn a_v0_4_history_beside_the_new_one_is_found() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!legacy_history_present_in(dir.path(), None));
        stage_v1(dir.path());
        assert!(legacy_history_present_in(dir.path(), None));
    }

    /// The probe must leave the disk as it found it: a user who downgrades has
    /// to find their history intact, and a created journal or a replayed WAL is
    /// a change to a file this build promised not to touch.
    #[test]
    fn finding_one_changes_nothing_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        stage_v1(dir.path());

        let before = snapshot(dir.path());
        assert!(legacy_history_present(dir.path()));
        assert_eq!(snapshot(dir.path()), before);
    }

    /// A v2 database is not an old one, whatever else is in the directory — the
    /// distinction the whole probe exists to keep.
    #[test]
    fn a_v2_database_alone_is_not_a_v0_4_history() {
        let dir = tempfile::tempdir().unwrap();
        let key = copypaste_core::Keyring::from_secret(&[3u8; 32]).db_key();
        let _store = Store::open(&dir.path().join("copypaste-v2.db"), &key).unwrap();
        assert!(!legacy_history_present_in(dir.path(), None));
    }

    /// Name plus length for everything in the directory, sorted.
    fn snapshot(dir: &Path) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    entry.metadata().unwrap().len(),
                )
            })
            .collect();
        entries.sort();
        entries
    }
}
