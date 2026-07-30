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
mod meta;
mod notify;
mod p2p;
mod server;
mod settings;
mod sync;

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
    /// Behind an `Arc` because `copypaste_core::StoreSource` holds one for as
    /// long as the peer listener runs, and the device secret is not `Clone` on
    /// purpose.
    pub keyring: Arc<Keyring>,
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
    /// The shutdown signal every long-running task selects on.
    ///
    /// It lives here rather than in `main` because
    /// [`copypaste_ipc::Method::Shutdown`] has to reach it: an app that did not
    /// start this daemon cannot signal the process, and ADR-0004's
    /// protocol-mismatch state has nothing to offer without it. `main` holds no
    /// second sender — it calls [`AppState::request_shutdown`] on SIGTERM like
    /// everything else, so there is one teardown path rather than two.
    shutdown: watch::Sender<bool>,
    backend_name: &'static str,
    ready: AtomicBool,
    capture_running: AtomicBool,
    /// A CopyPaste 0.4 history was found on this device at startup.
    ///
    /// A flag rather than a constructor argument for the same reason the two
    /// above are: it is decided outside construction and read by `status`,
    /// and threading it through `AppState::new` would touch every caller for
    /// a value none of them has an opinion about.
    legacy_history: AtomicBool,
}

/// How many change events are buffered per subscriber before it is told it
/// lagged. Small on purpose: the recovery for a lag is one extra re-read.
const EVENT_BUFFER: usize = 32;

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Store,
        keyring: Arc<Keyring>,
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
            shutdown: watch::channel(false).0,
            backend_name,
            ready: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
            legacy_history: AtomicBool::new(false),
        }
    }

    /// A receiver every long-running task selects on.
    pub fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    /// Begin an orderly shutdown. Idempotent, and safe from any thread.
    ///
    /// Every task finishes the unit of work it is in before observing this, so
    /// a capture already past the clipboard read still reaches the database.
    pub fn request_shutdown(&self) {
        // Fails only if every receiver has been dropped, which means the tasks
        // this would have stopped are already gone.
        let _ = self.shutdown.send(true);
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
        self.publish(EventKind::Items, false, 0);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// The auto-wipe sweep deleted `count` detected secrets.
    ///
    /// A local change like any other to the sync loops, but the only one the
    /// user did not ask for, so the count travels with it — see
    /// [`copypaste_ipc::EventData::swept`].
    pub fn note_sensitive_swept(&self, count: u32) {
        self.publish(EventKind::Items, false, count);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// The user copied something and it was stored.
    ///
    /// [`AppState::note_local_change`] plus the one bit that distinguishes a
    /// capture from a delete, a pin or an import: a client cannot post the
    /// "copied" notification or play the sound on an event that only says
    /// "history changed", because it would fire on every one of those too.
    ///
    /// The daemon does not consult `notify_on_copy` here. The event states what
    /// happened; the setting says what to do about it, and the surface that
    /// owns the notification is the one that reads it — see
    /// [`copypaste_ipc::EventData::captured`].
    pub fn note_capture(&self) {
        self.publish(EventKind::Items, true, 0);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// History changed because a peer or the cloud delivered something.
    pub fn note_remote_change(&self) {
        self.publish(EventKind::Items, false, 0);
    }

    pub fn note_peers_changed(&self) {
        self.publish(EventKind::Peers, false, 0);
    }

    fn publish(&self, event: EventKind, captured: bool, swept: u32) {
        // `send` fails only when nobody is listening, which is the ordinary
        // case: the CLI does not subscribe and the app may not be running.
        let _ = self.events.send(EventData {
            event,
            item_count: self.store.count().unwrap_or(0),
            captured,
            swept,
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

    pub fn legacy_history_present(&self) -> bool {
        self.legacy_history.load(Ordering::Acquire)
    }

    pub fn set_legacy_history_present(&self, present: bool) {
        self.legacy_history.store(present, Ordering::Release);
    }
}

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
fn legacy_history_present(data_dir: &std::path::Path) -> bool {
    std::iter::once(data_dir.to_path_buf())
        .chain(copypaste_ipc::v1_data_dir())
        .any(|dir| copypaste_core::v1_database_in(&dir))
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
    match copypaste_core::purge_indexed_secrets(&store, &detector) {
        Ok(report) if report.purged > 0 => tracing::info!(
            purged = report.purged,
            scanned = report.scanned,
            "removed search-index rows the current ruleset calls sensitive"
        ),
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

fn init_tracing() {
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
async fn halt_or_fail<E>(socket_path: &std::path::Path, error: E, doing: &str) -> anyhow::Result<()>
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

fn remove_socket(socket_path: &std::path::Path) {
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
async fn wait_for_shutdown(mut requested: watch::Receiver<bool>) -> anyhow::Result<()> {
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
        assert!(!legacy_history_present(dir.path()));
        stage_v1(dir.path());
        assert!(legacy_history_present(dir.path()));
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
        assert!(!legacy_history_present(dir.path()));
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
