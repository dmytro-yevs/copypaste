//! The CopyPaste daemon.
//!
//! `clipboard` uses `unsafe` for NSPasteboard through `objc2`, so this crate
//! cannot forbid it globally.

mod cadence;
mod capture;
mod cli;
mod clipboard;
mod cloud;
mod dbfile;
mod meta;
mod notify;
mod p2p;
mod server;
mod settings;
mod startup;
mod state;
mod sync;

#[cfg(test)]
mod testutil;

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use copypaste_core::{Detector, Keyring, Store};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;
use tracing::{info, warn};

use crate::cli::{cloud_config, Args};
use crate::cloud::Cloud;
use crate::meta::Meta;
use crate::p2p::P2p;
use crate::settings::Settings;
use crate::startup::{halt_or_fail, relocate, remove_socket, wait_for_shutdown};

pub use crate::state::AppState;

/// Reported by `status`. Single source: the crate version.
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    initialize_macos_workspace();
    let args = Args::parse();

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
    let detector = Arc::new(Detector::new().context("build the sensitive-content detector")?);

    // The third enforcement layer for "sensitive items must never reach the
    // search index" (CLAUDE.md rule 4). `is_sensitive` is decided once at
    // capture, so a row taken before a detector rule existed keeps its
    // plaintext searchable; this is the only thing that ever revisits it. It
    // touches the index and never the history. Not fatal: a history that
    // cannot be purged is still a history, and refusing to start would cost
    // the user access to it over a background sweep.
    let mut index_purged = 0u64;
    match copypaste_core::purge_indexed_secrets(&store, &detector) {
        Ok(report) if report.purged > 0 => {
            index_purged = report.purged;
            tracing::info!(
                purged = report.purged,
                scanned = report.scanned,
                "removed search-index rows the current ruleset calls sensitive"
            )
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "the search-index purge did not finish"),
    }

    let source = clipboard::new_source(&data_dir).context("initialize the clipboard backend")?;

    // Peer sync. The identity is minted in the database the store just
    // migrated, so it must come second; the peer file and discovery do not
    // depend on either.
    let hostname = gethostname::gethostname().to_string_lossy().into_owned();
    let mut meta = Meta::open(&store, &hostname).context("resolve this device's identity")?;
    if let Some(name) = args.device_name.as_deref() {
        meta.set_device_name(name).context("set the device name")?;
    }
    let settings = Settings::load(&meta);
    let peers = PeerStore::open(&peers_path).context("open the paired-device list")?;
    let discovery = match Discovery::dormant(meta.device_name(), args.port) {
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
    let device_name = meta.device_name().to_string();
    let p2p = P2p::new(peers, discovery, args.port, lan_visibility);

    // Cloud sync. Unconfigured is a supported state, and so is configured but
    // signed out: `Cloud::restore` reads back an account only if a previous run
    // signed in, and reports nothing when it did not.
    let config = cloud_config(&args);
    let cloud_configured = config.is_some();
    let cloud = Cloud::new(config);

    let state = Arc::new(AppState::new(
        store,
        keyring,
        detector,
        source,
        meta,
        p2p,
        cloud,
        settings,
        db_path.clone(),
    ));
    state.set_index_purged(index_purged);
    state.set_ready(true);
    let cloud_signed_in = state.cloud.restore(&state);
    info!(
        version = DAEMON_VERSION,
        backend = state.backend_name(),
        %device_id,
        %device_name,
        peer_port = args.port,
        cloud_configured,
        cloud_signed_in,
        "daemon starting"
    );

    let listener = server::bind(&socket_path)?;
    // A peer port already in use is not fatal: the rest of the daemon is still
    // worth running, and this device can still sync by dialling out.
    let peer_listener = match p2p::bind(args.port) {
        Ok(listener) => tokio::net::TcpListener::from_std(listener).ok(),
        Err(e) => {
            warn!(error = %e, port = args.port, "could not bind the peer port; not accepting peers");
            None
        }
    };
    let shutdown_rx = state.shutdown_rx();

    let capture = tokio::spawn(capture::run(Arc::clone(&state), shutdown_rx.clone()));
    let peers_task = peer_listener.map(|listener| {
        tokio::spawn(p2p::listen(
            listener,
            Arc::clone(&state),
            shutdown_rx.clone(),
        ))
    });
    let cloud_task = tokio::spawn(cloud::run(Arc::clone(&state), shutdown_rx.clone()));
    let refresh_task = tokio::spawn(cloud::refresh::run(Arc::clone(&state), shutdown_rx.clone()));
    // The push half of cloud sync. Without it the five-minute idle ceiling in
    // `copypaste_cloud::sync::cadence` has nothing behind it: that ceiling is
    // justified in its own doc comment by realtime existing, and the poll is
    // only allowed to be slow because something else is fast.
    let realtime_task = tokio::spawn(cloud::realtime::run(
        Arc::clone(&state),
        shutdown_rx.clone(),
    ));
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

    // Both tasks finish the unit of work they are in before observing the
    // signal, so a capture already past the clipboard read still reaches the
    // database.
    if let Err(e) = capture.await {
        warn!(error = ?e, "capture loop did not shut down cleanly");
    }
    if let Some(peers_task) = peers_task {
        if let Err(e) = peers_task.await {
            warn!(error = ?e, "peer listener did not shut down cleanly");
        }
    }
    if let Err(e) = cloud_task.await {
        warn!(error = ?e, "cloud sync loop did not shut down cleanly");
    }
    if let Err(e) = refresh_task.await {
        warn!(error = ?e, "cloud refresh loop did not shut down cleanly");
    }
    if let Err(e) = realtime_task.await {
        warn!(error = ?e, "cloud realtime loop did not shut down cleanly");
    }
    if let Err(e) = peer_sync.await {
        warn!(error = ?e, "peer sync loop did not shut down cleanly");
    }
    if let Err(e) = server.await {
        warn!(error = ?e, "ipc server did not shut down cleanly");
    }

    if let Err(e) = state.p2p.peers().flush() {
        warn!(error = ?e, "could not persist the paired-device list on shutdown");
    }

    remove_socket(&socket_path);
    Ok(())
}

/// A daemon is a plain process, unlike the Tauri app. AppKit must be loaded
/// before `NSWorkspace` can report the foreground process for capture source
/// attribution.
#[cfg(target_os = "macos")]
fn initialize_macos_workspace() {
    use objc2_app_kit::NSApplicationLoad;

    if !unsafe { NSApplicationLoad() }.as_bool() {
        warn!(
            "macOS workspace services could not initialize; source application attribution may be unavailable"
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn initialize_macos_workspace() {}
