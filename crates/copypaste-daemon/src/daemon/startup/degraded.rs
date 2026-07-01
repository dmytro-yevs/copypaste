//! DEGRADED-mode daemon loop: in-memory placeholder DB, `ready=false` IPC
//! server, signal/quit wait — never touches the real encrypted DB.

use copypaste_core::{Database, DeviceKeypair};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::paths;

use super::state_files::load_private_mode;

/// Run the daemon in DEGRADED mode (acceptance criteria #1–#3).
///
/// Entered when the SQLCipher key is unavailable (Keychain ACL no longer trusts
/// this binary after a reinstall, prompt unanswered, access denied) AND an
/// encrypted DB already exists, OR when an opened key turns out to be the wrong
/// one (SQLITE_NOTADB). We:
///
/// * NEVER `Error:`/exit — the process stays alive so the UI keeps a live
///   socket and can show a recovery banner instead of a dead daemon.
/// * Bind the IPC socket with `ready = false` and a `degraded_reason`, so every
///   DB-touching method returns `IPC_NOT_READY` and `status` reports
///   `status="degraded"` + `degraded_reason`.
/// * NEVER open / write / recreate the real encrypted DB. The IpcServer needs
///   *a* `Database` handle, so we hand it a throwaway in-memory one — the real
///   `~/.../clipboard.db` on disk is left byte-for-byte untouched and remains
///   recoverable on a later correct-key launch.
/// * Do NOT start the clipboard monitor, P2P, sync, or cloud subsystems — there
///   is no usable key, and writing captures with an ephemeral key would corrupt
///   nothing on disk but would also be pointless and confusing.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(crate) async fn run_degraded(
    reason: &'static str,
    quit_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let shutdown_token = CancellationToken::new();

    #[cfg(unix)]
    {
        // Throwaway in-memory DB: satisfies IpcServer's type contract WITHOUT
        // touching the encrypted file on disk. `ready = false` gates every
        // DB-touching method, so this in-memory DB is never actually queried
        // for user data — it only backs the `ensure_revoked_devices_table` DDL
        // in `serve()` and the readiness gate.
        let placeholder_db =
            Arc::new(Mutex::new(Database::open_in_memory().map_err(|e| {
                anyhow::anyhow!("degraded: in-memory placeholder DB: {e}")
            })?));
        // PG-14 (CopyPaste-tpvi): Load private-mode from the filesystem flag
        // file — NOT the DB, which is unavailable in degraded mode.  The flag
        // file is completely independent of the encrypted database, so
        // load_private_mode() is safe to call here.  Defaulting to `false`
        // (capture ON) when the prior state was private would be a silent
        // privacy regression; we mirror the normal-startup path instead.
        let prior_private = load_private_mode();
        if prior_private {
            tracing::warn!(
                "degraded boot: private mode was ON before this degraded boot; \
                 preserving capture-OFF state from persisted flag"
            );
        }
        let private_mode = Arc::new(AtomicBool::new(prior_private));
        // An ephemeral key for the placeholder server — never used against real
        // data (DB methods are gated off by `ready = false`).
        let dummy_key: Arc<zeroize::Zeroizing<[u8; 32]>> =
            Arc::new(DeviceKeypair::generate().local_enc_key());
        let dummy_pub: Arc<[u8; 32]> = Arc::new([0u8; 32]);

        let ready = Arc::new(AtomicBool::new(false));
        let server = crate::ipc::IpcServer::new_with_ready(
            placeholder_db,
            private_mode,
            dummy_key,
            dummy_pub,
            ready,
        )
        .with_degraded_reason(reason);

        let socket_path = paths::socket_path();
        let socket_clone = socket_path.clone();
        let ipc_shutdown = shutdown_token.clone();
        let ipc_handle = tokio::spawn(async move {
            if let Err(e) = server.serve(&socket_clone, ipc_shutdown).await {
                tracing::error!("degraded IPC server error: {e}");
            }
        });

        tracing::warn!(
            reason,
            "DEGRADED daemon running: IPC socket bound, DB-touching requests \
             return IPC_NOT_READY, `status` reports degraded_reason. Re-grant \
             the Keychain prompt and relaunch to recover."
        );

        // Wait for shutdown (tray quit flag, SIGINT, or SIGTERM), mirroring the
        // healthy loop's shutdown wiring but with no clipboard polling.
        #[cfg(unix)]
        let mut sigterm = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate())?
        };
        let mut quit_ticker = tokio::time::interval(Duration::from_millis(
            crate::daemon::DEGRADED_QUIT_POLL_INTERVAL_MS,
        ));
        loop {
            if quit_flag.load(Ordering::Relaxed) {
                tracing::info!("quit flag set, shutting down degraded daemon");
                break;
            }
            tokio::select! {
                _ = quit_ticker.tick() => { /* re-check quit_flag at the top */ }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down degraded daemon");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down degraded daemon");
                    break;
                }
            }
        }

        shutdown_token.cancel();
        let _ = ipc_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(not(unix))]
    {
        // No Unix socket transport on non-unix; just wait for Ctrl+C so the
        // process does not busy-exit. Degraded mode is a macOS/unix concern.
        let _ = tokio::signal::ctrl_c().await;
    }

    tracing::info!("degraded daemon stopped");
    Ok(())
}
