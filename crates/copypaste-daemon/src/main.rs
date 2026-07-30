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
//! * [`meta`] — the sync view of the history that both transports share, and
//!   [`merge`], the one comparator they both apply.
//!
//! Everything they share lives in one [`AppState`] behind one `Arc`. v1 grew a
//! 38-field context with 13 `Arc<Mutex<Option<T>>>` slots and 20 builder
//! methods, which meant no reader could tell which fields were populated at any
//! given moment; the `Option`s existed only because construction was spread
//! across those builders. Here construction happens once, in `main`, and every
//! field is always present.
//!
//! Note the deliberate absence of `#![forbid(unsafe_code)]` at the crate root:
//! `clipboard` talks to NSPasteboard through `objc2` and needs `unsafe`. The
//! other two modules do not use it.

mod cadence;
mod capture;
mod clipboard;
mod cloud;
mod dbfile;
mod merge;
mod meta;
mod p2p;
mod server;
mod settings;

#[cfg(test)]
mod testutil;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context;
use clap::Parser;
use copypaste_core::{Detector, Keyring, Store};
use copypaste_ipc::{EventData, EventKind};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;
use tokio::sync::{broadcast, watch};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::clipboard::ClipboardSource;
use crate::cloud::Cloud;
use crate::meta::Meta;
use crate::p2p::P2p;
use crate::settings::Settings;

/// Reported by `status`. Single source: the crate version.
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[command(
    name = "copypaste-daemon",
    version,
    about = "CopyPaste clipboard daemon"
)]
struct Args {
    /// Directory holding the database and the IPC socket.
    ///
    /// Defaults to the platform application-data directory resolved by
    /// `copypaste_ipc`. Overriding it runs an instance that is fully isolated
    /// from the user's real history — that is what the tests and `--data-dir`
    /// demos rely on.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Stay attached to the terminal.
    ///
    /// The daemon never forks: backgrounding is the service manager's job
    /// (launchd on macOS). The flag exists so a service definition can state
    /// its intent, and it suppresses the notice printed when it is absent.
    #[arg(long)]
    foreground: bool,

    /// TCP port the peer listener binds.
    ///
    /// Fixed by default so an explicit address is short to type. Overriding it
    /// is what lets two daemons run on one host, which is how the peer-sync
    /// demo works; the pairing this daemon mints reports whichever port is in
    /// use, so the other device does not have to be told separately.
    #[arg(long, default_value_t = copypaste_p2p::DEFAULT_PORT)]
    port: u16,

    /// What peers call this device.
    ///
    /// Cosmetic and peer-visible. Stored on first run and kept afterwards, so
    /// passing it once is enough and a hostname change does not rename the
    /// device on every peer.
    #[arg(long, value_name = "NAME")]
    device_name: Option<String>,

    /// Supabase project URL for cloud sync, e.g. `https://abc.supabase.co`.
    ///
    /// Falls back to `COPYPASTE_CLOUD_URL`. Without both this and the anon key
    /// the daemon runs with cloud sync unconfigured, which is a supported
    /// state: peer sync and local history do not depend on it.
    #[arg(long, value_name = "URL")]
    cloud_url: Option<String>,

    /// Supabase publishable anon key. Falls back to `COPYPASTE_CLOUD_ANON_KEY`.
    ///
    /// Not a secret in the usual sense — row-level security is what restricts
    /// access — so it is ordinary configuration rather than a credential.
    #[arg(long, value_name = "KEY")]
    cloud_anon_key: Option<String>,
}

/// Resolve the deployment from flags, then the environment.
///
/// Both halves are required: a URL with no key cannot authenticate and a key
/// with no URL has nothing to talk to, so a half-configuration is reported as
/// unconfigured rather than failing at the first request.
fn cloud_config(args: &Args) -> Option<copypaste_cloud::CloudConfig> {
    fn resolve(flag: Option<&String>, var: &str) -> Option<String> {
        flag.cloned()
            .or_else(|| std::env::var(var).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }
    Some(copypaste_cloud::CloudConfig {
        url: resolve(args.cloud_url.as_ref(), "COPYPASTE_CLOUD_URL")?,
        anon_key: resolve(args.cloud_anon_key.as_ref(), "COPYPASTE_CLOUD_ANON_KEY")?,
    })
}

/// Everything the daemon shares between the capture loop and the IPC server.
///
/// Held as `Arc<AppState>`; no field is optional and none is rebuilt after
/// construction. The only interior mutability is the clipboard handle (the
/// platform source needs `&mut` and is used from both halves) and two flags
/// that `status` reports.
pub struct AppState {
    pub store: Store,
    pub keyring: Keyring,
    /// Behind an `Arc` because the cloud upload gate holds one too: the
    /// `SensitiveGuard` the driver requires is a closure that outlives this
    /// call, and a second `Detector` would be a second ruleset (`CLAUDE.md`
    /// rule 1) that could disagree with the one capture uses.
    pub detector: Arc<Detector>,
    /// `std::sync::Mutex`, not `tokio`'s: the guard is never held across an
    /// `.await`. Every caller takes it, does one pasteboard call, drops it — so
    /// a `copy` request cannot be blocked behind a capture tick for longer than
    /// a single pasteboard access.
    clipboard: Mutex<Box<dyn ClipboardSource>>,
    /// The sync view of the history, and this device's identity. Shared by both
    /// transports — see [`meta`].
    pub meta: Meta,
    /// Peer sync: the paired devices and discovery. Always present — a daemon
    /// with no peers still has an identity and still listens.
    pub p2p: P2p,
    /// Cloud sync. Always present too, and unconfigured is an ordinary state:
    /// the deployment may not be set, or nobody may be signed in.
    pub cloud: Cloud,
    /// The live settings. Every consumer reads it at the moment it acts, which
    /// is what makes a change take effect without a restart.
    pub settings: Settings,
    /// Where the history database is, for `backup` and `restore`. Never put in
    /// a client-visible string: it discloses the local username.
    db_path: PathBuf,
    /// Push channel for [`copypaste_ipc::Method::Watch`] subscribers.
    ///
    /// `broadcast` rather than a list of senders: a subscriber that stops
    /// draining lags and is told so, instead of applying backpressure to the
    /// capture loop. A dropped event is safe — an event says only *that*
    /// something changed, and the client re-reads.
    events: broadcast::Sender<EventData>,
    backend_name: &'static str,
    ready: AtomicBool,
    capture_running: AtomicBool,
}

/// How many change events are buffered per subscriber before it is told it
/// lagged. Small on purpose: the recovery for a lag is one extra re-read.
const EVENT_BUFFER: usize = 32;

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Store,
        keyring: Keyring,
        detector: Arc<Detector>,
        clipboard: Box<dyn ClipboardSource>,
        meta: Meta,
        p2p: P2p,
        cloud: Cloud,
        settings: Settings,
        db_path: PathBuf,
    ) -> Self {
        let backend_name = clipboard.backend_name();
        Self {
            store,
            keyring,
            detector,
            clipboard: Mutex::new(clipboard),
            meta,
            p2p,
            cloud,
            settings,
            db_path,
            events: broadcast::channel(EVENT_BUFFER).0,
            backend_name,
            ready: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    /// A stream of change events, for a `watch` subscriber.
    pub fn subscribe(&self) -> broadcast::Receiver<EventData> {
        self.events.subscribe()
    }

    /// History changed *on this device* — a capture, an `add`, a delete, a pin,
    /// an import, a restore.
    ///
    /// Three consumers, and the split from [`AppState::note_remote_change`] is
    /// what keeps them from feeding each other: both wake the watchers, but only
    /// a local change pulls the two sync loops to their floor. A round that
    /// applied a peer's row would otherwise reset the cadence, provoke an
    /// immediate empty round on the other device, and ring back.
    pub fn note_local_change(&self) {
        self.publish(EventKind::Items);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// History changed because a peer or the cloud delivered something.
    pub fn note_remote_change(&self) {
        self.publish(EventKind::Items);
    }

    pub fn note_peers_changed(&self) {
        self.publish(EventKind::Peers);
    }

    fn publish(&self, event: EventKind) {
        // `send` fails only when nobody is listening, which is the ordinary
        // case: the CLI does not subscribe and the app may not be running.
        let _ = self.events.send(EventData {
            event,
            item_count: self.store.count().unwrap_or(0),
        });
    }

    /// Lock recovery rather than propagation: a poisoned clipboard mutex means
    /// a previous pasteboard call panicked, not that the handle is unusable.
    /// Refusing every later `copy` would be a worse outcome than retrying.
    pub fn clipboard(&self) -> MutexGuard<'_, Box<dyn ClipboardSource>> {
        self.clipboard
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    pub fn capture_running(&self) -> bool {
        self.capture_running.load(Ordering::Acquire)
    }

    pub fn set_capture_running(&self, running: bool) {
        self.capture_running.store(running, Ordering::Release);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing();

    // `--data-dir` moves both the database and the socket, so an isolated
    // instance cannot answer for — or be answered by — the real one.
    let (db_path, socket_path) = match &args.data_dir {
        Some(dir) => (dir.join("copypaste-v2.db"), dir.join("daemon.sock")),
        None => (copypaste_ipc::database_path(), copypaste_ipc::socket_path()),
    };
    // The paired-device file lives beside the database, under whichever data
    // directory is in force, so an isolated instance has isolated pairings too.
    let peers_path = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(copypaste_p2p::peers::DEFAULT_FILE_NAME);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("create the data directory")?;
    }

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
    let keyring = Keyring::load_or_create().context("unlock the keyring")?;
    let store = Store::open(&db_path, &keyring.db_key()).context("open the history database")?;
    let detector = Arc::new(Detector::new().context("build the sensitive-content detector")?);
    let source = clipboard::new_source();

    // Peer sync. The metadata connection opens the database the store just
    // migrated, so it must come second; the peer file and discovery do not
    // depend on either.
    let hostname = gethostname::gethostname().to_string_lossy().into_owned();
    let mut meta =
        Meta::open(&db_path, &keyring.db_key(), &hostname).context("open the sync metadata")?;
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
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

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

    wait_for_shutdown().await?;
    info!("shutting down");
    state.set_ready(false);
    let _ = shutdown_tx.send(true);

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

    match std::fs::remove_file(&socket_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => warn!(error = %e, "could not remove the socket on shutdown"),
    }
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Resolves on SIGINT or SIGTERM. launchd sends SIGTERM; a terminal sends
/// SIGINT. Both must unwind the same way — an aborted process leaves the socket
/// file behind and the next start has to treat it as stale.
async fn wait_for_shutdown() -> anyhow::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).context("install the SIGTERM handler")?;
    let mut sigint = signal(SignalKind::interrupt()).context("install the SIGINT handler")?;
    tokio::select! {
        _ = sigterm.recv() => info!("received SIGTERM"),
        _ = sigint.recv() => info!("received SIGINT"),
    }
    Ok(())
}
