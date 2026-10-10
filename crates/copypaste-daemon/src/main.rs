//! The CopyPaste daemon.
//!
//! `clipboard` uses `unsafe` for NSPasteboard through `objc2`, so this crate
//! cannot forbid it globally.

mod cadence;
mod capture;
mod cli;
mod clipboard;
#[cfg(target_os = "macos")]
mod macos_workspace;
mod meta;
mod notify;
mod p2p;
mod runtime;
#[cfg(test)]
mod runtime_tests;
mod server;
mod settings;
mod shutdown;
mod startup;
mod state;
mod sync;

#[cfg(test)]
mod testutil;

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use copypaste_core::{Keyring, Store};
use copypaste_p2p::discovery::Discovery;
use tracing::{info, warn};

use crate::cli::Args;
use crate::meta::Meta;
use crate::p2p::P2p;
#[cfg(not(target_os = "macos"))]
use crate::runtime::run_with_bounded_shutdown;
use crate::settings::Settings;
use crate::startup::{halt_or_fail, relocate, wait_for_shutdown, watch_app_parent};

pub use crate::state::AppState;

/// Reported by `status`. Single source: the crate version.
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--inference-worker")
    {
        return copypaste_modules::run_inference_worker(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
        )
        .map_err(Into::into);
    }
    #[cfg(target_os = "macos")]
    {
        macos_workspace::run(run())
    }
    #[cfg(not(target_os = "macos"))]
    {
        run_with_bounded_shutdown(run())
    }
}

async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    // Start this before opening durable state. If the app died during its own
    // startup, EOF is noticed while the daemon is still coming up rather than
    // only after it has bound the socket and started capture.
    watch_app_parent(args.app_parent)?;

    // `--data-dir` moves both defaults; an explicit `COPYPASTE_SOCKET` still
    // wins so every process can name the same isolated instance.
    let db_path = args
        .data_dir
        .as_ref()
        .map_or_else(copypaste_ipc::database_path, |dir| {
            relocate(&copypaste_ipc::database_path(), dir)
        });
    let socket_path = copypaste_ipc::paths::socket_path_for_data_dir(args.data_dir.as_deref());
    // One derivation of "the data directory", used by everything that lives
    // beside the database: the paired-device file, so an isolated instance has
    // isolated pairings too, and the device secret, so it cannot be left behind
    // when `--data-dir` moves the history (security review F-11). A second
    // derivation is how the secret ends up somewhere the next start does not
    // look.
    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let peers_path = data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME);
    std::fs::create_dir_all(&data_dir).context("create the data directory")?;
    let _runtime_log = copypaste_runtime_log::init(
        &data_dir.join("logs"),
        copypaste_runtime_log::Process::Daemon,
    )
    .context("initialize runtime logging")?;
    info!("daemon process started pid={}", std::process::id());

    if !args.foreground {
        warn!(
            "running in the foreground; the daemon does not fork — leave \
             backgrounding to the service manager (launchd) and pass \
             --foreground to silence this notice"
        );
    }

    // Order matters: the keyring unlocks the database key, the database is
    // opened with it, and neither the socket nor the capture loop exists until
    // both succeeded. A daemon that cannot store what it captures should not
    // start capturing.
    //
    // A device key failure with a fixed sentence does not exit here. Exiting leaves the app with no
    // socket to ask, so it reports the service as merely down and offers to
    // start it again; see `server::halted`.
    let keyring = match Keyring::load_or_create(&data_dir) {
        Ok(keyring) => Arc::new(keyring),
        Err(e) => return halt_or_fail(&socket_path, e, "unlock the keyring").await,
    };
    let store = match Store::open(&db_path, &keyring.db_key()) {
        Ok(store) => store,
        Err(e) => return halt_or_fail(&socket_path, e, "open the history database").await,
    };
    let source = clipboard::new_source(&data_dir).context("initialize the clipboard backend")?;

    // Peer sync. The identity is minted in the database the store just opened,
    // so it must come second; the peer file and discovery do not
    // depend on either.
    let meta = Meta::open_system(&store).context("resolve this device's identity")?;
    if let Some(name) = args.device_name.as_deref() {
        meta.set_device_name(name).context("set the device name")?;
    }
    let settings = Settings::load(&meta);
    let peers = copypaste_core::peer_store::open(&store, &keyring, &peers_path)
        .context("open the paired-device list")?;
    let device_name = meta.device_name();
    let discovery = match Discovery::dormant(&device_name, args.port) {
        Ok(discovery) => Some(discovery),
        Err(e) => {
            warn!(error = %e, "could not start discovery; peers must be given an address");
            Some(
                Discovery::dormant("CopyPaste device", args.port)
                    .context("start discovery with a fallback name")?,
            )
        }
    };
    let lan_visibility = settings.get().lan_visibility;
    if !lan_visibility {
        info!("LAN visibility is off; not advertising and not browsing");
    }
    let device_id = meta.device_id().to_string();
    let p2p = P2p::new(peers, discovery, args.port, lan_visibility);

    let state = Arc::new(AppState::new(
        store,
        keyring,
        source,
        meta,
        p2p,
        settings,
        db_path.clone(),
    ));
    state.set_ready(true);
    sync::install_module_services(&state)?;
    info!(
        version = DAEMON_VERSION,
        backend = state.backend_name(),
        %device_id,
        %device_name,
        peer_port = args.port,
        "daemon starting"
    );

    let listener = server::bind(&socket_path)?;
    // A peer port already in use is not fatal: the rest of the daemon is still
    // worth running, and this device can still sync by dialling out.
    let peer_listener = match state.p2p.node().bind_listener() {
        Ok(listener) => tokio::net::TcpListener::from_std(listener).ok(),
        Err(e) => {
            warn!(error = %e, port = args.port, "could not bind the peer port; not accepting peers");
            None
        }
    };
    let shutdown_rx = state.shutdown_rx();

    let device_names = tokio::spawn(meta::run_name_refresh(
        Arc::clone(&state),
        shutdown_rx.clone(),
    ));
    let capture = tokio::spawn(capture::run(Arc::clone(&state), shutdown_rx.clone()));
    let pairing_changes = state.p2p.node().subscribe_pairing_changes();
    let pairing_events = tokio::spawn(p2p::forward_pairing_changes(
        Arc::clone(&state),
        pairing_changes,
        shutdown_rx.clone(),
    ));
    let peers_task = peer_listener.map(|listener| {
        tokio::spawn(p2p::listen(
            listener,
            Arc::clone(&state),
            shutdown_rx.clone(),
        ))
    });
    let modules = Arc::clone(&state.modules);
    let module_shutdown = shutdown_rx.clone();
    let module_sync = tokio::spawn(async move {
        modules.run_sync(module_shutdown).await;
    });
    // Peer sync on a cadence. Without it a paired device only ever syncs when
    // the *other* side dials in or a human runs `copypaste sync`.
    let peer_sync = tokio::spawn(p2p::poll::run(Arc::clone(&state), shutdown_rx.clone()));
    let server = tokio::spawn(server::run(listener, Arc::clone(&state), shutdown_rx));

    // Either a signal or a client asking. One path out, so the IPC verb
    // unwinds exactly as SIGTERM does rather than through a second teardown
    // nobody exercises.
    wait_for_shutdown(state.shutdown_rx()).await?;
    info!("shutting down");
    state.set_ready(false);
    state.request_shutdown();

    // Each loop finishes the unit of work it is in before observing the signal,
    // so a capture already past the clipboard read still reaches the database —
    // but the wait for them is bounded, because the peer flush and the socket
    // removal below are what a killed daemon never reaches.
    let loops = vec![
        ("device names", device_names),
        ("module sync", module_sync),
        ("peer sync", peer_sync),
        ("pairing events", pairing_events),
    ];
    let mut loops = loops;
    loops.extend(peers_task.map(|task| ("peer listener", task)));
    shutdown::stop_loops(loops).await;
    let capture_result = capture
        .await
        .context("join the critical clipboard capture task")
        .and_then(|result| result.context("finish the critical clipboard capture task"));
    state.wait_for_admitted_requests().await;
    let flush_result =
        shutdown::flush_peers_before_listener_release(&state, || state.release_drain_listener())
            .await;
    let server_result = server.await.context("join the IPC listener task");
    shutdown::release_endpoint(&socket_path);
    capture_result?;
    flush_result?;
    server_result?;
    info!("daemon process stopped pid={}", std::process::id());
    Ok(())
}
