//! The CopyPaste daemon.
//!
//! Three moving parts, wired together here and nowhere else:
//!
//! * [`capture`] — polls the clipboard and ingests what it finds,
//! * [`server`] — answers `copypaste_ipc::Request`s on a `0600` Unix socket,
//! * [`clipboard`] — the platform pasteboard behind a trait,
//! * [`p2p`] — peer sync: an inbound listener on its own TCP port, mDNS
//!   discovery, and the five pairing/sync IPC operations,
//! * [`cloud`] — cloud sync: the account, the adaptive poll loop, and the four
//!   cloud IPC operations,
//! * [`meta`] — this device's identity and item attribution; the sync view both
//!   transports read and write through is [`copypaste_core::StoreSource`],
//!   built here by [`sync`].
//!
//! Everything they share lives in one [`AppState`], built once here — see
//! [`state`]. `AppState` is re-exported so its 17 consumers keep naming it
//! `crate::AppState`; `docs/rewrite/module-audit.md` records the surface split
//! that re-export defers.
//!
//! Note the deliberate absence of `#![forbid(unsafe_code)]` at the crate root:
//! `clipboard` talks to NSPasteboard through `objc2` and needs `unsafe`. The
//! other two modules do not use it.

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
use crate::startup::{
    halt_or_fail, init_tracing, legacy_history_present, relocate, remove_socket, wait_for_shutdown,
};

pub use crate::state::AppState;

/// Reported by `status`. Single source: the crate version.
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing();

    // `--data-dir` moves both the database and the socket, so an isolated
    // instance cannot answer for — or be answered by — the real one.
    let (db_path, socket_path) = match &args.data_dir {
        Some(dir) => (
            relocate(&copypaste_ipc::database_path(), dir),
            relocate(&copypaste_ipc::socket_path(), dir),
        ),
        None => (copypaste_ipc::database_path(), copypaste_ipc::socket_path()),
    };
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

    if !args.foreground {
        warn!(
            "running in the foreground; the daemon does not fork — leave \
             backgrounding to the service manager (launchd) and pass \
             --foreground to silence this notice"
        );
    }

    // Before the store is opened, so the answer describes the disk as the user
    // left it rather than as this run has already changed it: `Store::open`
    // creates `copypaste-v2.db` on a first run, and on Linux that is the same
    // directory the probe reads.
    let legacy_history = legacy_history_present(&data_dir);
    if legacy_history {
        info!("a CopyPaste 0.4 history is present; it has not been opened or changed");
    }

    // Order matters: the keyring unlocks the database key, the database is
    // opened with it, and neither the socket nor the capture loop exists until
    // both succeeded. A daemon that cannot store what it captures should not
    // start capturing.
    //
    // The two failures with a fixed sentence — a v0.4 history, a device key
    // that cannot be used — do not exit here. Exiting leaves the app with no
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

    let source = clipboard::new_source();

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
    // `lan_visibility` is read once, here: the mDNS registration is made at
    // start, so this is the moment it either happens or does not. It is the one
    // setting `ConfigData::field_liveness` marks `NeedsRestart`, and this line
    // is why.
    let discovery = if settings.get().lan_visibility {
        // Never fatal: a host without multicast still pairs and still syncs to
        // an explicit address, and that is the common case on a locked-down
        // network.
        let pairing_ids: Vec<String> = peers
            .list()
            .iter()
            .map(|peer| peer.pairing_id.clone())
            .collect();
        match Discovery::start(meta.device_name(), &pairing_ids, args.port) {
            Ok(discovery) => Some(discovery),
            Err(e) => {
                warn!(error = %e, "could not start discovery; peers must be given an address");
                Some(
                    Discovery::start("CopyPaste device", &[], args.port)
                        .context("start discovery with a fallback name")?,
                )
            }
        }
    } else {
        info!("LAN visibility is off; not advertising and not browsing");
        None
    };
    let device_id = meta.device_id().to_string();
    let device_name = meta.device_name().to_string();
    let p2p = P2p::new(peers, discovery, args.port);

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
    state.set_legacy_history_present(legacy_history);
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
    if let Err(e) = realtime_task.await {
        warn!(error = ?e, "cloud realtime loop did not shut down cleanly");
    }
    if let Err(e) = peer_sync.await {
        warn!(error = ?e, "peer sync loop did not shut down cleanly");
    }
    if let Err(e) = server.await {
        warn!(error = ?e, "ipc server did not shut down cleanly");
    }

    remove_socket(&socket_path);
    Ok(())
}
