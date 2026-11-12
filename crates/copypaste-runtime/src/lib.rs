//! The CopyPaste runtime: storage, settings, peer sync and the request
//! dispatch shared by the desktop daemon and the Android in-process host.
//!
//! Platform hosts own native clipboard capture, lifecycle and the pieces in
//! [`RuntimeHost`]. This crate owns no cryptography, merge rules, or handshake
//! protocol; those remain in `copypaste-core` and `copypaste-p2p`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use copypaste_core::rules::compile::CompiledRuleSet;
use copypaste_core::storage::local_state::LibraryRevision;
use copypaste_core::{now_ms, p2p_contract, ClipboardPayload, Keyring, Store, StoreSource};
use copypaste_ipc::{
    CapabilityGuarantee, CapabilityState, CapabilityStatus, EventData, EventKind, LibraryKinds,
    OperationProgress, PlatformCapability,
};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;
use copypaste_p2p::Node;
use tokio::sync::{broadcast, watch};

pub mod capture_admission;
mod deadlines;
mod dispatch;
mod features;
pub mod host;
pub mod messages;
pub mod meta;
pub mod p2p;
mod peer_sync;
pub mod settings;

use capture_admission::{CaptureAdmission, CaptureKind, CaptureScope};
pub use deadlines::ResumeWaker;
pub use dispatch::{BlockingAdmission, Route, MAX_REORDER_IDS};
pub use host::{ClipboardWriter, PlatformStatus, RuntimeHost, WriteOptions};
pub use meta::{Meta, Origin};
pub use p2p::P2p;
pub use settings::{Settings, SettingsError, SettingsReadGuard, SettingsTransition};

/// Change events buffered per subscriber before it is told it lagged. Small on
/// purpose: the recovery for a lag is one extra re-read.
pub const EVENT_BUFFER: usize = 256;

/// `Library` events are coalesced to at most one per this window.
const LIBRARY_COALESCE_MS: i64 = 500;
/// `Progress` events are coalesced to at most one per this window.
const PROGRESS_COALESCE_MS: i64 = 250;
/// Concurrent operations whose progress is coalesced.
const MAX_PROGRESS_OPERATIONS: usize = 32;

/// What a host opens before the runtime is composed over it. The host owns
/// the keyring and the store so it can report their failures its own way.
pub struct RuntimeParts {
    pub data_dir: PathBuf,
    pub store: Store,
    pub keyring: Arc<Keyring>,
    pub meta: Meta,
    pub settings: Arc<Settings>,
    pub peers: PeerStore,
    pub port: u16,
    pub host: Arc<dyn RuntimeHost>,
}

/// Storage, settings and peer networking shared by every platform host.
pub struct Runtime {
    pub store: Store,
    pub keyring: Arc<Keyring>,
    pub meta: Meta,
    pub p2p: P2p,
    pub settings: Arc<Settings>,
    pub modules: Arc<copypaste_modules::ModuleHost>,
    pub spoilers: copypaste_core::spoiler::Spoilers,
    pub instant_clipboard: Arc<copypaste_core::sync::InstantClipboard>,
    pub file_downloads: Arc<copypaste_core::file_download::FileDownloads>,
    /// The sync view both transports read and write through. Its hooks hold
    /// no reference back to the runtime, so it can be held here.
    pub source: Arc<StoreSource>,
    host: Arc<dyn RuntimeHost>,
    changes: Arc<Changes>,
    deadlines: Arc<deadlines::Deadlines>,
    /// The content rules compiled for one rules revision.
    compiled_rules: Mutex<Option<(i64, Arc<CompiledRuleSet>)>>,
    paste_queue: features::PasteQueue,
    ai: features::AiState,
    runtime_events: broadcast::Sender<copypaste_ipc::RuntimeEventData>,
    shutdown: watch::Sender<bool>,
    listener_started: AtomicBool,
    module_sync_started: AtomicBool,
    capture_running: AtomicBool,
    capture_revision: AtomicU64,
    last_published_sync_revision: AtomicU64,
}

/// Change publication shared by the runtime and the hooks it installs.
struct Changes {
    events: broadcast::Sender<EventData>,
    store: Store,
    modules: Weak<copypaste_modules::ModuleHost>,
    host: Arc<dyn RuntimeHost>,
    deadlines: Arc<deadlines::Deadlines>,
    clipboard: features::ClipboardOwner,
    library: Mutex<LibraryChanges>,
    progress: Mutex<HashMap<String, ProgressSlot>>,
}

/// Library changes waiting for the coalesced `Library` event.
#[derive(Default)]
struct LibraryChanges {
    kinds: LibraryKinds,
    /// A peer or the cloud applied versions; diff the revision to learn what.
    received: bool,
    scheduled: bool,
    baseline: Option<LibraryRevision>,
}

/// The last emission of one operation's progress and the update held back.
struct ProgressSlot {
    emitted_at: i64,
    held: Option<OperationProgress>,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Changes {
    /// Send one event when anyone listens; the count is read only then.
    fn send(&self, event: EventKind, fill: impl FnOnce(&mut EventData)) {
        if self.events.receiver_count() == 0 {
            return;
        }
        let mut data = EventData::change(event, self.store.count().unwrap_or(0));
        fill(&mut data);
        let _ = self.events.send(data);
    }

    /// Versions arrived from a peer or the cloud. The coalesced flush
    /// re-checks library kinds and sweeps deadlines, since a synced expiry
    /// may have arrived; a long pull costs one sweep per window, not per row.
    fn received(&self) {
        self.library_changed(LibraryKinds::default(), true);
    }

    fn library_changed(&self, kinds: LibraryKinds, received: bool) {
        let mut library = locked(&self.library);
        library.kinds = library.kinds.union(kinds);
        library.received |= received;
        if std::mem::replace(&mut library.scheduled, true) {
            return;
        }
        drop(library);
        self.deadlines.schedule(
            deadlines::DeadlineKey::new("library_event", ""),
            self.deadlines.now().saturating_add(LIBRARY_COALESCE_MS),
            Box::new(|runtime: &Runtime| runtime.flush_library()),
        );
    }

    fn publish(&self, event: EventKind, captured_item_id: Option<&str>) {
        if event == EventKind::Items {
            if let Some(modules) = self.modules.upgrade() {
                modules.history_changed();
            }
        }
        // No subscriber is the ordinary case, and `item_count` costs a
        // `SELECT COUNT(*)`, so nothing is built for nobody.
        if self.events.receiver_count() == 0 {
            return;
        }
        let _ = self.events.send(EventData {
            captured: captured_item_id.is_some(),
            captured_item_id: captured_item_id.map(str::to_owned),
            ..EventData::change(event, self.store.count().unwrap_or(0))
        });
    }

    /// A received item reached the system clipboard: copy feedback without
    /// treating the receive as local activity.
    fn received_copy(&self, item_id: &str) {
        self.clipboard.changed(Some(item_id));
        self.publish(EventKind::Items, Some(item_id));
        self.host.copy_feedback();
    }
}

/// Outcome for one SMS inbox row. Android advances its durable inbox cursor
/// only for terminal outcomes, so a temporary host or storage failure leaves
/// the row available for the next existing inbox scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmsCaptureOutcome {
    Captured,
    Ignored,
    Retry,
}

/// Optional application identity captured by a native host with the payload.
pub struct CaptureSourceApp {
    pub bundle_id: String,
    pub name: Option<String>,
    pub icon: Option<copypaste_core::SourceAppIconMetadata>,
}

impl Runtime {
    pub fn set_inference_launcher(
        &self,
        launcher: Arc<dyn copypaste_modules::InferenceLauncher>,
    ) -> Result<(), copypaste_modules::ModuleError> {
        self.modules.set_inference_launcher(launcher)
    }

    /// Opens only application-owned paths. The platform host must initialize
    /// its keystore before calling this constructor.
    pub fn open(data_dir: &Path, device_name: &str, port: u16) -> Result<Self, RuntimeError> {
        Self::open_with_clipboard(
            data_dir,
            device_name,
            port,
            Arc::new(host::UnavailableClipboard),
        )
    }

    pub fn open_with_clipboard(
        data_dir: &Path,
        device_name: &str,
        port: u16,
        clipboard: Arc<dyn ClipboardWriter>,
    ) -> Result<Self, RuntimeError> {
        std::fs::create_dir_all(data_dir).map_err(|_| RuntimeError::Storage)?;
        let keyring =
            Arc::new(Keyring::load_or_create(data_dir).map_err(|_| RuntimeError::Keyring)?);
        Self::open_with_keyring(
            data_dir,
            device_name,
            port,
            Arc::new(host::InProcessHost(clipboard)),
            keyring,
        )
    }

    /// Continues construction after the host has selected a keyring, so tests
    /// can inject a deterministic key without touching the platform keychain.
    fn open_with_keyring(
        data_dir: &Path,
        device_name: &str,
        port: u16,
        host: Arc<dyn RuntimeHost>,
        keyring: Arc<Keyring>,
    ) -> Result<Self, RuntimeError> {
        let store = Store::open(&data_dir.join("history.db"), &keyring.db_key())
            .map_err(|_| RuntimeError::Storage)?;
        copypaste_core::file_transfer::upgrade_legacy(&store, &keyring.item_key())
            .map_err(|_| RuntimeError::Storage)?;
        let meta = Meta::open(&store, device_name).map_err(|_| RuntimeError::Storage)?;
        let settings = Arc::new(Settings::load(&store));
        let peers = copypaste_core::peer_store::open(
            &store,
            &keyring,
            &data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
        )
        .map_err(|_| RuntimeError::PeerStore)?;
        Self::assemble(RuntimeParts {
            data_dir: data_dir.to_path_buf(),
            store,
            keyring,
            meta,
            settings,
            peers,
            port,
            host,
        })
    }

    /// Compose the runtime over opened parts. Spawns nothing: the host starts
    /// the listener and the background loops it wants.
    pub fn assemble(parts: RuntimeParts) -> Result<Self, RuntimeError> {
        let RuntimeParts {
            data_dir,
            store,
            keyring,
            meta,
            settings,
            peers,
            port,
            host,
        } = parts;
        let discovery = Discovery::dormant(&meta.device_name(), port)
            .or_else(|error| {
                tracing::warn!(%error, "could not start discovery; peers must be given an address");
                Discovery::dormant("CopyPaste device", port)
            })
            .ok();
        let p2p = P2p::new(Node::new(
            peers,
            discovery,
            port,
            settings.get().lan_visibility,
        ));
        let modules = Arc::new(copypaste_modules::ModuleHost::new(&data_dir));
        let events = broadcast::channel(EVENT_BUFFER).0;
        let deadlines = Arc::new(deadlines::Deadlines::default());
        let changes = Arc::new(Changes {
            events: events.clone(),
            store: store.clone(),
            modules: Arc::downgrade(&modules),
            host: Arc::clone(&host),
            deadlines: Arc::clone(&deadlines),
            clipboard: features::ClipboardOwner::default(),
            library: Mutex::default(),
            progress: Mutex::default(),
        });
        let instant_clipboard = Arc::new(copypaste_core::sync::InstantClipboard::default());
        let source = Arc::new(Self::source(
            &store,
            &keyring,
            &meta,
            &settings,
            &modules,
            &instant_clipboard,
            &host,
            &changes,
        ));
        p2p.node()
            .prepare_group(source.as_ref())
            .map_err(|_| RuntimeError::Storage)?;
        let search_events = events.clone();
        let search_store = store.clone();
        modules.bind_search(
            store.clone(),
            Arc::new(move || {
                let _ = search_events.send(EventData::change(
                    EventKind::Items,
                    search_store.count().unwrap_or(0),
                ));
            }),
        );
        let shutdown = watch::channel(false).0;
        Self::install_module_services(
            &store, &meta, &settings, &p2p, &modules, &source, &changes, &shutdown,
        )?;
        Ok(Self {
            store,
            keyring,
            meta,
            p2p,
            settings,
            modules,
            spoilers: Default::default(),
            instant_clipboard,
            file_downloads: Default::default(),
            source,
            host,
            changes,
            deadlines,
            compiled_rules: Mutex::default(),
            paste_queue: features::PasteQueue::default(),
            ai: features::AiState::default(),
            runtime_events: broadcast::channel(EVENT_BUFFER).0,
            shutdown,
            listener_started: AtomicBool::new(false),
            module_sync_started: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
            capture_revision: AtomicU64::new(0),
            last_published_sync_revision: AtomicU64::new(u64::MAX),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn source(
        store: &Store,
        keyring: &Arc<Keyring>,
        meta: &Meta,
        settings: &Arc<Settings>,
        modules: &Arc<copypaste_modules::ModuleHost>,
        instant_clipboard: &Arc<copypaste_core::sync::InstantClipboard>,
        host: &Arc<dyn RuntimeHost>,
        changes: &Arc<Changes>,
    ) -> StoreSource {
        let source_settings = Arc::clone(settings);
        let versions = Arc::downgrade(modules);
        let (gate, clip_store, clip_keyring, clip_settings, writer, received) = (
            Arc::clone(instant_clipboard),
            store.clone(),
            Arc::clone(keyring),
            Arc::clone(settings),
            Arc::clone(host),
            Arc::clone(changes),
        );
        StoreSource::with_retention_settings(
            store.clone(),
            Arc::clone(keyring),
            meta.device_id().to_owned(),
            meta.device_name(),
            move || source_settings.config(),
        )
        // A peer's row carries the sender's stamp, routinely older than the
        // cloud upload floor; pull the floor back or it is never forwarded.
        .on_applied(move |stamp| {
            if let Some(modules) = versions.upgrade() {
                modules.note_version(stamp);
            }
        })
        // Shares the capture authority, so a settings change cannot race the
        // native write.
        .on_clip_received(move |row| {
            clip_settings.with_capture_authority(|config, _| {
                let applied = gate.apply(
                    &clip_store,
                    &clip_keyring,
                    row,
                    || config,
                    |payload| {
                        writer.write_received(
                            &row.id,
                            payload,
                            &row.content_type,
                            row.clipboard_privacy().secret,
                        )
                    },
                );
                if applied {
                    received.received_copy(&row.id);
                }
            });
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn install_module_services(
        store: &Store,
        meta: &Meta,
        settings: &Arc<Settings>,
        p2p: &P2p,
        modules: &copypaste_modules::ModuleHost,
        source: &Arc<StoreSource>,
        changes: &Arc<Changes>,
        shutdown: &watch::Sender<bool>,
    ) -> Result<(), RuntimeError> {
        let running = shutdown.subscribe();
        let (enabled_settings, enabled_node) = (Arc::clone(settings), Arc::clone(p2p.node()));
        let (local_node, local_driver, local_changes) = (
            Arc::clone(p2p.node()),
            Arc::clone(&p2p.driver),
            Arc::clone(changes),
        );
        let (export_node, export_source) = (Arc::clone(p2p.node()), Arc::clone(source));
        let (import_node, import_driver, import_changes) = (
            Arc::clone(p2p.node()),
            Arc::clone(&p2p.driver),
            Arc::clone(changes),
        );
        let revision_node = Arc::clone(p2p.node());
        modules
            .set_sync_services(Arc::new(
                copypaste_modules::SyncServices::new(
                    store.clone(),
                    copypaste_sync::store::StoreView::new(
                        source.without_version_hook(),
                        meta.device_id().to_owned(),
                    ),
                    move || {
                        !*running.borrow()
                            && enabled_settings.get().sync_enabled
                            && enabled_node.peers().group_active()
                    },
                    // A cloud row is remote to this device but local to its
                    // peers: relay it without counting it as local activity.
                    move |stamp| {
                        local_node.cursors().note_local(stamp);
                        local_driver.wake();
                        local_changes.publish(EventKind::Items, None);
                        local_changes.received();
                    },
                )
                .with_group(
                    move || {
                        export_node
                            .export_group(export_source.as_ref())
                            .map_err(|e| e.to_string())
                    },
                    move |device_id, json| {
                        if import_node
                            .import_cloud_group(device_id, json)
                            .or_else(|error| match error {
                                copypaste_p2p::group::GroupImportError::Rejected => Ok(false),
                                error => Err(error),
                            })
                            .map_err(|e| e.to_string())?
                        {
                            import_driver.wake();
                            import_changes.publish(EventKind::Peers, None);
                        }
                        Ok(())
                    },
                    move || revision_node.peers().group_revision(),
                ),
            ))
            .map_err(|_| RuntimeError::Storage)
    }

    /// The runtime half of `status`, completed by the host's platform probes.
    ///
    /// Never fails: an unreadable count is reported as zero, because a caller
    /// may be probing precisely because the database is unhappy.
    pub fn status_data(&self) -> copypaste_ipc::StatusData {
        let item_count = self.store.count().unwrap_or_else(|error| {
            tracing::warn!(?error, "could not count items for status");
            0
        });
        let (revision, config, private_mode_epoch, settings_health, running, platform) = loop {
            let revision = self.capture_revision.load(Ordering::Acquire);
            let (config, epoch, health) = {
                let settings = self.settings.get();
                (
                    settings.clone(),
                    settings.private_mode_epoch(),
                    settings.health().cloned(),
                )
            };
            let running = self.capture_running.load(Ordering::Acquire);
            let platform = self.host.platform_status();
            if revision == self.capture_revision.load(Ordering::Acquire) {
                break (revision, config, epoch, health, running, platform);
            }
        };
        let mut capabilities = platform.capabilities;
        apply_capture_policy_capabilities(&mut capabilities, &config);
        if let Some(capture) = capabilities.get_mut(&PlatformCapability::ClipboardCapture) {
            if !running {
                capture.state = CapabilityState::Error;
                capture.guarantee = CapabilityGuarantee::None;
                capture.reason = Some("Capture loop is stopped.".into());
            } else if config.private_mode {
                capture.reason = Some("Capture is paused by private mode.".into());
            }
        }
        let device_name = self.meta.device_name();
        let listen_addr = self.p2p.listen_addr();
        copypaste_ipc::StatusData {
            platform_capabilities: capabilities,
            runtime_watch: true,
            capture_watch: true,
            capture_revision: revision,
            sync_status: self.p2p.node().sync_status(config.sync_enabled),
            device_details: Some(p2p_contract::local_device_details(
                &device_name,
                listen_addr.as_deref(),
            )),
            device_name,
            device_id: Some(self.meta.device_id().to_owned()),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: copypaste_ipc::PROTOCOL_VERSION,
            listen_addr,
            item_count,
            capture_running: running,
            clipboard_available: platform.clipboard_available,
            clipboard_backend: platform.clipboard_backend,
            private_mode: config.private_mode,
            private_mode_epoch,
            counters: platform.counters,
            // Carried on `status` so a client cannot render the settings
            // screen before it learns they are not the user's own values.
            settings_health,
        }
    }

    /// Module workers have their own lifecycle, independent of the peer port.
    /// Returns the worker the first time and `None` once it is running.
    pub fn start_module_sync(&self) -> Option<tokio::task::JoinHandle<()>> {
        if self.module_sync_started.swap(true, Ordering::AcqRel) {
            return None;
        }
        let modules = Arc::clone(&self.modules);
        let shutdown = self.shutdown.subscribe();
        Some(tokio::spawn(async move {
            modules.run_sync(shutdown).await;
        }))
    }

    /// Bind the peer port and start serving, forwarding and scheduling peer
    /// sync until [`Runtime::shutdown`]. Idempotent while the listener runs.
    pub async fn start_listener(self: &Arc<Self>) -> Result<(), RuntimeError> {
        self.start_module_sync();
        self.start_deadlines();
        if self.listener_started.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let listener = match self
            .p2p
            .node()
            .bind_listener()
            .and_then(tokio::net::TcpListener::from_std)
        {
            Ok(listener) => listener,
            Err(_) => {
                self.listener_started.store(false, Ordering::Release);
                return Err(RuntimeError::Listener);
            }
        };
        let runtime = Arc::clone(self);
        let shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            Arc::clone(&runtime).listen(listener, shutdown).await;
            runtime.listener_started.store(false, Ordering::Release);
        });
        tokio::spawn(self.forward_changes(self.shutdown.subscribe()));
        self.p2p
            .sync_enabled_changed(self.settings.get().sync_enabled);
        tokio::spawn(Arc::clone(self).run_peer_sync(self.shutdown.subscribe()));
        Ok(())
    }

    /// Accept peer connections until shutdown, serving them from this history.
    pub async fn listen(
        self: Arc<Self>,
        listener: tokio::net::TcpListener,
        shutdown: watch::Receiver<bool>,
    ) {
        let node = Arc::clone(self.p2p.node());
        let source = Arc::clone(&self.source);
        let session = Arc::clone(&self);
        let probe = self;
        copypaste_p2p::node::listen(
            node,
            listener,
            source,
            move |_, outcome| {
                session.remember_device(outcome);
                if outcome.stats.received > 0 {
                    session.note_received_change();
                }
            },
            move |sync_status_changed: bool| {
                if sync_status_changed {
                    probe.note_sync_status_changed();
                } else {
                    probe.note_peers_changed();
                }
            },
            shutdown,
        )
        .await;
    }

    /// Forward pairing, sync-status and group transitions that originate in
    /// the node, including an inbound handshake no request started. The
    /// receivers are subscribed before this returns, so no transition between
    /// spawning and the first poll is lost.
    pub fn forward_changes(
        self: &Arc<Self>,
        mut shutdown: watch::Receiver<bool>,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut pairing = self.p2p.node().subscribe_pairing_changes();
        let mut sync = self.p2p.node().subscribe_sync_changes();
        let mut group = self.p2p.peers().subscribe_group_changes();
        let runtime = Arc::clone(self);
        async move {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return,
                    changed = group.changed() => {
                        if changed.is_err() { return; }
                        runtime.modules.wake_sync();
                        runtime.p2p.wake();
                        runtime.note_peers_changed();
                    },
                    changed = sync.changed() => {
                        if changed.is_err() { return; }
                        runtime.note_sync_status_changed();
                    },
                    changed = pairing.changed() => {
                        if changed.is_err() { return; }
                        runtime.note_peers_changed();
                    },
                }
            }
        }
    }

    /// Outgoing peer sync on the shared cadence until shutdown.
    pub async fn run_peer_sync(self: Arc<Self>, shutdown: watch::Receiver<bool>) {
        let driver = Arc::clone(&self.p2p.driver);
        driver
            .run(Arc::new(peer_sync::RuntimePeerHost(self)), shutdown)
            .await;
    }

    /// One scheduled pass under an already held round permit.
    #[cfg(test)]
    async fn run_peer_round(
        self: &Arc<Self>,
        permit: copypaste_core::sync::RoundGuard,
        shutdown: watch::Receiver<bool>,
    ) {
        self.p2p
            .driver
            .run_round(
                Arc::new(peer_sync::RuntimePeerHost(Arc::clone(self))),
                shutdown,
                permit,
            )
            .await;
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EventData> {
        self.changes.events.subscribe()
    }

    pub fn subscribe_runtime_events(&self) -> broadcast::Receiver<copypaste_ipc::RuntimeEventData> {
        self.runtime_events.subscribe()
    }

    pub fn runtime_event_sender(&self) -> broadcast::Sender<copypaste_ipc::RuntimeEventData> {
        self.runtime_events.clone()
    }

    /// History changed on this device since `floor_ms`. Pulls both transport
    /// floors back to it and wakes them. The split from
    /// [`Runtime::note_remote_change`] keeps a received row from ringing back.
    pub fn note_local_change_since(&self, floor_ms: i64) {
        self.p2p.node().note_local_version(floor_ms);
        self.changes.publish(EventKind::Items, None);
        self.p2p.wake();
        self.modules.note_version(floor_ms);
    }

    pub fn note_local_change(&self) {
        self.note_local_change_since(now_ms());
    }

    /// A local capture: a local change plus the bit that lets the surface
    /// owning notifications tell it from a delete, a pin or an import.
    pub fn note_capture(&self, floor_ms: i64, item_id: &str) {
        self.instant_clipboard.note_local(&self.store, floor_ms);
        self.p2p.node().note_local_version(floor_ms);
        self.changes.publish(EventKind::Items, Some(item_id));
        self.p2p.wake();
        self.modules.note_version(floor_ms);
        self.on_captured(item_id);
    }

    /// History changed because a peer or the cloud delivered something, or
    /// only its presentation changed.
    pub fn note_remote_change(&self) {
        self.changes.publish(EventKind::Items, None);
    }

    /// A peer or the cloud applied versions here: history changed, library
    /// records may have, and a synced deadline may have arrived.
    pub fn note_received_change(&self) {
        self.note_remote_change();
        self.changes.received();
    }

    /// A host saw the clipboard change without capturing it (paused or
    /// excluded), so a pending restore must not overwrite it.
    pub fn note_clipboard_changed(&self) {
        self.changes.clipboard.changed(None);
    }

    /// A store deadline (a clip expiry, a purge time) was written; the
    /// deadline task re-arms when it is the earliest.
    pub fn note_store_deadline(&self, at_ms: i64) {
        self.deadlines.note_store_deadline(at_ms);
    }

    /// A committed local capture, for the features that follow captures.
    fn on_captured(&self, item_id: &str) {
        self.changes.clipboard.changed(Some(item_id));
        self.paste_queue_captured(item_id);
    }

    /// History was cleared: the paste queue and the host surfaces that show
    /// history (Android widgets) are emptied with it.
    pub(crate) fn on_history_cleared(&self) {
        self.paste_queue_history_cleared();
        self.host.history_cleared();
    }

    /// A paired device was removed: host surfaces drop what they show from it.
    pub(crate) fn on_device_removed(&self, device_id: &str) {
        self.host.device_removed(device_id);
    }

    /// Library records of `kinds` changed locally. Emits one coalesced
    /// `Library` event and drops compiled rules when rules changed.
    pub fn on_org_changed(&self, kinds: LibraryKinds) {
        self.changes.library_changed(kinds, false);
    }

    fn flush_library(&self) {
        let (mut kinds, received, baseline) = {
            let mut library = locked(&self.changes.library);
            library.scheduled = false;
            (
                std::mem::take(&mut library.kinds),
                std::mem::take(&mut library.received),
                library.baseline,
            )
        };
        if received {
            self.deadlines.request_sweep();
        }
        match self.store.library_revision() {
            Ok(current) => {
                if received {
                    kinds = kinds.union(
                        baseline.map_or(LibraryKinds::all(), |base| library_diff(&base, &current)),
                    );
                }
                locked(&self.changes.library).baseline = Some(current);
            }
            Err(_) if received => kinds = LibraryKinds::all(),
            Err(_) => {}
        }
        if kinds == LibraryKinds::default() {
            return;
        }
        if kinds.intersects(LibraryKinds::RULES) {
            *locked(&self.compiled_rules) = None;
        }
        self.changes
            .send(EventKind::Library, |data| data.library = Some(kinds));
    }

    /// The paste queue moved to `revision`.
    pub fn note_paste_queue_changed(&self, revision: u64) {
        self.changes.send(EventKind::PasteQueue, |data| {
            data.paste_queue_revision = Some(revision);
        });
    }

    /// An AI tool approval is pending or a session's authorization changed.
    pub fn note_ai_approval(&self) {
        self.changes.send(EventKind::AiApproval, |_| {});
    }

    /// A long operation advanced. Coalesced per request to one event per
    /// 250 ms; the final update is never held back.
    pub fn note_progress(&self, progress: OperationProgress) {
        let now = self.deadlines.now();
        let mut slots = locked(&self.changes.progress);
        if slots.len() >= MAX_PROGRESS_OPERATIONS && !slots.contains_key(&progress.request_id) {
            slots.clear();
        }
        let request_id = progress.request_id.clone();
        let finished = progress.done >= progress.total;
        let slot = slots.entry(request_id.clone()).or_insert(ProgressSlot {
            emitted_at: i64::MIN,
            held: None,
        });
        let due = slot.emitted_at.saturating_add(PROGRESS_COALESCE_MS);
        if !finished && now < due {
            let first_held = slot.held.replace(progress).is_none();
            drop(slots);
            if first_held {
                let request = request_id.clone();
                self.deadlines.schedule(
                    deadlines::DeadlineKey::new("progress_event", request_id),
                    due,
                    Box::new(move |runtime: &Runtime| runtime.flush_progress(&request)),
                );
            }
            return;
        }
        if finished {
            slots.remove(&request_id);
        } else {
            slot.emitted_at = now;
            slot.held = None;
        }
        drop(slots);
        if finished {
            self.deadlines
                .cancel(&deadlines::DeadlineKey::new("progress_event", request_id));
        }
        self.changes
            .send(EventKind::Progress, |data| data.operation = Some(progress));
    }

    fn flush_progress(&self, request_id: &str) {
        let now = self.deadlines.now();
        let held = locked(&self.changes.progress)
            .get_mut(request_id)
            .and_then(|slot| {
                slot.emitted_at = now;
                slot.held.take()
            });
        if let Some(held) = held {
            self.changes
                .send(EventKind::Progress, |data| data.operation = Some(held));
        }
    }

    /// The content rules compiled for the current rules revision; `build`
    /// runs only when the revision moved since the last call.
    pub fn compiled_rules(
        &self,
        build: impl FnOnce(&Store) -> Result<CompiledRuleSet, copypaste_core::StoreError>,
    ) -> Result<Arc<CompiledRuleSet>, copypaste_core::StoreError> {
        let revision = self.store.resource_revisions()?.rules;
        let mut cached = locked(&self.compiled_rules);
        if let Some((cached_revision, rules)) = cached.as_ref() {
            if *cached_revision == revision {
                return Ok(Arc::clone(rules));
            }
        }
        let rules = Arc::new(build(&self.store)?);
        *cached = Some((revision, Arc::clone(&rules)));
        Ok(rules)
    }

    pub fn note_peers_changed(&self) {
        self.changes.publish(EventKind::Peers, None);
    }

    /// Publish the sync status once per revision, and only when subscribed.
    pub fn note_sync_status_changed(&self) {
        if self.changes.events.receiver_count() == 0 {
            return;
        }
        let sync_status = self
            .p2p
            .node()
            .sync_status(self.settings.get().sync_enabled);
        if self
            .last_published_sync_revision
            .swap(sync_status.revision, Ordering::AcqRel)
            == sync_status.revision
        {
            return;
        }
        let _ = self.changes.events.send(EventData {
            sync_status: Some(sync_status),
            ..EventData::change(EventKind::Peers, self.store.count().unwrap_or(0))
        });
    }

    pub fn capture_revision(&self) -> u64 {
        self.capture_revision.load(Ordering::Acquire)
    }

    pub fn capture_running(&self) -> bool {
        self.capture_running.load(Ordering::Acquire)
    }

    /// Advance the capture revision and publish status, only when subscribed.
    /// Status includes no clipboard payload.
    pub fn note_capture_state_changed(&self) {
        self.capture_revision.fetch_add(1, Ordering::AcqRel);
        if self.runtime_events.receiver_count() == 0 {
            return;
        }
        let _ = self.runtime_events.send(copypaste_ipc::RuntimeEventData {
            file_operation: None,
            change: None,
            progress: None,
            capture: Some(self.status_data()),
        });
    }

    /// Records text captured by an in-process platform host.
    ///
    /// Android owns the permission and clipboard-read mechanics. The shared
    /// runtime remains the only owner of encryption, deduplication, retention,
    /// exclusions and History change events.
    pub fn capture_text(&self, content: &str) -> Result<(), RuntimeError> {
        self.capture_text_with_policy(content, false).map(|_| ())
    }

    /// Records text from an explicit Android Share or Process Text action.
    pub fn capture_explicit_text(&self, content: &str) -> Result<(), RuntimeError> {
        self.capture_text_with_policy(content, true).map(|_| ())
    }

    /// Captures text and returns the id of the history item that holds it.
    fn capture_text_with_policy(
        &self,
        content: &str,
        explicit: bool,
    ) -> Result<String, RuntimeError> {
        let host = self
            .capture_admission()
            .open_host(if explicit {
                CaptureKind::Explicit
            } else {
                CaptureKind::Implicit
            })
            .ok_or(RuntimeError::CaptureRefused)?;
        let result = self
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .ok_or(RuntimeError::CaptureRefused)
            .and_then(|token| {
                let result = self.capture_text_item(token, content).map(|(id, _)| id);
                self.capture_admission().abandon(token);
                result
            });
        let _ = self.capture_admission().revoke_host(host);
        result
    }

    /// Returns the privacy the capture was committed with, so a native host
    /// can hide its own preview of a secret exactly as the app does.
    pub fn capture_text_operation(
        &self,
        token: u64,
        content: &str,
    ) -> Result<copypaste_ipc::ClipboardPrivacy, RuntimeError> {
        self.capture_text_item(token, content)
            .map(|(_, privacy)| privacy)
    }

    fn capture_text_item(
        &self,
        token: u64,
        content: &str,
    ) -> Result<(String, copypaste_ipc::ClipboardPrivacy), RuntimeError> {
        // Detection only marks the item; producer markers alone gate capture.
        let detected =
            copypaste_core::classify_credentials(copypaste_ipc::content_type::TEXT, content)
                .unwrap_or_default();
        let mut permit = self
            .capture_admission()
            .acquire(token, CaptureScope::Commit)
            .ok_or(RuntimeError::CaptureRefused)?;
        let settings = &permit.config;
        let privacy_metadata =
            (!permit.privacy.is_empty()).then_some(copypaste_core::PayloadMetadata {
                secret_override: None,
                privacy: permit.privacy,
                ..Default::default()
            });
        let ingested =
            copypaste_core::ingest_into_with_capture_source_metadata_with_current_retention(
                &self.store,
                &self.keyring,
                content,
                copypaste_ipc::content_type::TEXT,
                now_ms(),
                None,
                None,
                privacy_metadata.as_ref(),
                settings,
                || self.settings.config(),
            )
            .map_err(|_| RuntimeError::Capture)?;
        let privacy = permit.privacy.union(detected);
        let item = ingested.into_item();
        self.note_capture(item.created_at, &item.id);
        permit.committed();
        Ok((item.id, privacy))
    }

    pub fn has_sms_module(&self) -> Result<bool, copypaste_modules::ModuleError> {
        self.modules.has_sms_handler()
    }

    /// The SMS body is invocation-only. Only the recognized code enters the
    /// encrypted History and the normal sync source. Publication holds the
    /// same admission scope used by native clipboard capture.
    pub fn capture_sms_operation(&self, token: u64, text: &str) -> SmsCaptureOutcome {
        let mut read = self.capture_admission().acquire(token, CaptureScope::Read);
        if read.is_none() {
            return if self.sms_capture_paused() {
                SmsCaptureOutcome::Ignored
            } else {
                SmsCaptureOutcome::Retry
            };
        }
        match self.modules.dispatch_sms(text, |code| {
            drop(read.take());
            self.publish_sms_code(token, code)
                .map_err(|_| copypaste_modules::ModuleError::State)?;
            Ok(())
        }) {
            Ok(true) => SmsCaptureOutcome::Captured,
            // No unique authentication code is a terminal result. Disabled
            // handlers also arrive here after the module lifecycle closes.
            Ok(false) => SmsCaptureOutcome::Ignored,
            Err(_) if self.sms_capture_paused() => SmsCaptureOutcome::Ignored,
            Err(_) => SmsCaptureOutcome::Retry,
        }
    }

    /// SMS monitoring deliberately discards messages received while private
    /// mode is on. Other admission failures are transient and must be retried.
    #[must_use]
    pub fn sms_capture_paused(&self) -> bool {
        self.settings.config().private_mode
    }

    fn publish_sms_code(&self, token: u64, code: &str) -> Result<(), RuntimeError> {
        let (item_id, _) = self.capture_text_item(token, code)?;
        let completion = self
            .capture_admission()
            .acquire(token, CaptureScope::Completion)
            .ok_or(RuntimeError::CaptureRefused)?;
        self.host
            .write(
                &item_id,
                &ClipboardPayload::Text(code.to_owned().into()),
                copypaste_ipc::content_type::TEXT,
                WriteOptions::copy(completion.privacy.secret),
            )
            .map_err(|_| RuntimeError::Capture)?;
        self.changes.clipboard.changed(Some(&item_id));
        Ok(())
    }

    /// Records an image or file captured by an in-process platform host.
    pub fn capture_binary(
        &self,
        bytes: &[u8],
        content_type: &str,
        filename: Option<&str>,
        source_reference: Option<&str>,
    ) -> Result<(), RuntimeError> {
        self.capture_binary_with_policy(bytes, content_type, filename, source_reference, false)
    }

    /// Records a binary value from an explicit Android Share action.
    pub fn capture_explicit_binary(
        &self,
        bytes: &[u8],
        content_type: &str,
        filename: Option<&str>,
        source_reference: Option<&str>,
    ) -> Result<(), RuntimeError> {
        self.capture_binary_with_policy(bytes, content_type, filename, source_reference, true)
    }

    fn capture_binary_with_policy(
        &self,
        bytes: &[u8],
        content_type: &str,
        filename: Option<&str>,
        source_reference: Option<&str>,
        explicit: bool,
    ) -> Result<(), RuntimeError> {
        let host = self
            .capture_admission()
            .open_host(if explicit {
                CaptureKind::Explicit
            } else {
                CaptureKind::Implicit
            })
            .ok_or(RuntimeError::CaptureRefused)?;
        let result = self
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .ok_or(RuntimeError::CaptureRefused)
            .and_then(|token| {
                let result = self.capture_binary_operation(
                    token,
                    bytes,
                    content_type,
                    filename,
                    source_reference,
                    None,
                );
                self.capture_admission().abandon(token);
                result
            });
        let _ = self.capture_admission().revoke_host(host);
        result
    }

    pub fn capture_binary_operation(
        &self,
        token: u64,
        bytes: &[u8],
        content_type: &str,
        filename: Option<&str>,
        source_reference: Option<&str>,
        source_app: Option<&CaptureSourceApp>,
    ) -> Result<(), RuntimeError> {
        if !content_type.starts_with("image/") {
            return self.capture_file_operation(
                token,
                bytes.len() as u64,
                filename.ok_or(RuntimeError::Capture)?,
                content_type,
                source_reference.ok_or(RuntimeError::Capture)?,
                source_app,
            );
        }
        let mut permit = self
            .capture_admission()
            .acquire(token, CaptureScope::Commit)
            .ok_or(RuntimeError::CaptureRefused)?;
        let settings = &permit.config;
        let metadata = copypaste_core::PayloadMetadata {
            credentials: false,
            secret_override: None,
            privacy: permit.privacy,
            file: None,
            source_app_icon: source_app.and_then(|source| source.icon.clone()),
        };
        let metadata = (!metadata.privacy.is_empty()
            || metadata.file.is_some()
            || metadata.source_app_icon.is_some())
        .then_some(metadata);
        let ingested = copypaste_core::ingest_binary_into_with_capture_source_metadata(
            &self.store,
            &self.keyring,
            bytes,
            content_type,
            now_ms(),
            source_app.map(|source| source.bundle_id.as_str()),
            source_app.and_then(|source| source.name.as_deref()),
            metadata.as_ref(),
            settings,
        )
        .map_err(|_| RuntimeError::Capture)?;
        let item = ingested.into_item();
        self.note_capture(item.created_at, &item.id);
        permit.committed();
        Ok(())
    }

    pub fn capture_file_operation(
        &self,
        token: u64,
        size_bytes: u64,
        filename: &str,
        mime_type: &str,
        reference: &str,
        source_app: Option<&CaptureSourceApp>,
    ) -> Result<(), RuntimeError> {
        let mut permit = self
            .capture_admission()
            .acquire(token, CaptureScope::Commit)
            .ok_or(RuntimeError::CaptureRefused)?;
        let metadata =
            copypaste_core::FileMetadata::with_source_reference(filename, mime_type, reference)
                .ok_or(RuntimeError::Capture)?;
        let context = copypaste_core::PayloadMetadata {
            privacy: permit.privacy,
            source_app_icon: source_app.and_then(|source| source.icon.clone()),
            ..Default::default()
        };
        let reference = copypaste_core::file_reference::FileReference::prepare(
            reference,
            permit.config.capture_limit_bytes("file"),
            Some(size_bytes),
        )
        .map_err(|_| RuntimeError::Capture)?;
        let row = reference
            .publish(
                &self.store,
                &self.keyring.item_key(),
                metadata,
                now_ms(),
                source_app.map(|source| source.bundle_id.as_str()),
                source_app.and_then(|source| source.name.as_deref()),
                Some(&context),
                &permit.config,
                false,
            )
            .map_err(|_| RuntimeError::Capture)?;
        let item = row.into_item();
        self.note_capture(item.created_at, &item.id);
        permit.committed();
        Ok(())
    }

    pub fn set_capture_running(&self, running: bool) {
        if self.capture_running.swap(running, Ordering::AcqRel) != running {
            self.note_capture_state_changed();
        }
    }

    /// Returns a policy snapshot for status presentation, never read authority.
    #[must_use]
    pub fn implicit_capture_allowed(&self) -> bool {
        let settings = self.settings.get();
        !settings.private_mode && settings.excluded_app_bundle_ids.is_empty()
    }

    pub fn capture_admission(&self) -> &CaptureAdmission {
        &self.settings.capture
    }

    #[must_use]
    pub fn notify_on_copy_enabled(&self) -> bool {
        self.settings.get().notify_on_copy
    }

    #[must_use]
    pub fn notification_preview_enabled(&self) -> bool {
        self.settings.get().notification_preview
    }

    #[must_use]
    pub fn sound_on_copy_enabled(&self) -> bool {
        self.settings.get().sound_on_copy
    }

    /// Permanently closes capture and the runtime's own workers. Idempotent;
    /// call from a worker when capture scopes may be active.
    pub fn shutdown(&self) {
        self.file_downloads.cancel_all();
        let _ = self.capture_admission().shutdown();
        let _ = self.shutdown.send(true);
    }
}

/// The library kinds whose change markers differ between two revisions.
fn library_diff(before: &LibraryRevision, after: &LibraryRevision) -> LibraryKinds {
    [
        (
            before.snippets != after.snippets,
            LibraryKinds::SNIPPETS.union(LibraryKinds::COLLECTIONS),
        ),
        (before.rules != after.rules, LibraryKinds::RULES),
        (before.pipelines != after.pipelines, LibraryKinds::PIPELINES),
        (
            before.preferences != after.preferences,
            LibraryKinds::PREFERENCES,
        ),
        (
            before.set_members != after.set_members,
            LibraryKinds::SET_MEMBERS,
        ),
        (before.grants != after.grants, LibraryKinds::GRANTS),
    ]
    .into_iter()
    .filter(|(changed, _)| *changed)
    .fold(LibraryKinds::default(), |kinds, (_, kind)| {
        kinds.union(kind)
    })
}

/// Source exclusions need authenticated attribution: without an active source
/// adapter, capture stops rather than recording what it cannot attribute.
fn apply_capture_policy_capabilities(
    capabilities: &mut BTreeMap<PlatformCapability, CapabilityStatus>,
    settings: &copypaste_ipc::ConfigData,
) {
    if settings.excluded_app_bundle_ids.is_empty() {
        return;
    }
    let source_is_unsupported = capabilities
        .get(&PlatformCapability::SourceAttribution)
        .is_some_and(|source| source.state == CapabilityState::Unsupported);
    if !source_is_unsupported {
        return;
    }
    if let Some(capture) = capabilities.get_mut(&PlatformCapability::ClipboardCapture) {
        if capture.state == CapabilityState::Available {
            capture.state = CapabilityState::NeedsCompanion;
            capture.guarantee = CapabilityGuarantee::None;
            capture.reason = Some(
                "Source exclusions stop capture until an authenticated source adapter is active."
                    .into(),
            );
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("the platform keyring is unavailable")]
    Keyring,
    #[error("the history store is unavailable")]
    Storage,
    #[error("the paired-device store is unavailable")]
    PeerStore,
    #[error("the peer listener is unavailable")]
    Listener,
    #[error("the clipboard capture was refused")]
    CaptureRefused,
    #[error("the clipboard capture could not be stored")]
    Capture,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use copypaste_core::ClipboardWriteError;
    use copypaste_ipc::{ErrorCode, Item, Method, Response, ResponseData};
    use std::sync::Mutex;
    use std::time::Duration;

    #[tokio::test]
    async fn capture_watch_reports_runtime_and_private_transitions() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_runtime_events();
        runtime.set_capture_running(true);
        let first = events.try_recv().unwrap().capture.unwrap();
        assert!(first.capture_running);
        assert_eq!(first.capture_revision, 1);
        runtime.set_capture_running(true);
        assert!(events.try_recv().is_err());
        let response = runtime
            .request(1, Method::SetPrivateMode { enabled: true })
            .await;
        assert!(response.ok);
        let second = events.try_recv().unwrap().capture.unwrap();
        assert!(second.private_mode);
        assert_eq!(second.private_mode_epoch, 1);
        assert_eq!(second.capture_revision, 2);
    }

    #[tokio::test]
    async fn nearby_request_uses_typed_errors_and_honors_the_sync_switch() {
        let (runtime, _dir) = fixture();
        let invalid = runtime
            .request(
                1,
                Method::PairNearby {
                    addr: "invalid".into(),
                },
            )
            .await;
        assert_eq!(invalid.error_code, Some(ErrorCode::PairingAddress));
        assert!(
            runtime
                .request(
                    2,
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            sync_enabled: Some(false),
                            ..Default::default()
                        }
                    }
                )
                .await
                .ok
        );
        for method in [
            Method::PairNearby {
                addr: "invalid".into(),
            },
            Method::PairCreateInvite,
            Method::PairJoin {
                code: "invalid".into(),
                addr: "invalid".into(),
            },
            Method::PairConfirm { accept: true },
        ] {
            assert_eq!(
                runtime.request(3, method).await.error_code,
                Some(ErrorCode::NotReady)
            );
        }
    }

    impl Runtime {
        fn item_value(&self, row: copypaste_core::StoredItem, preview: bool) -> Option<Item> {
            let origin = self.meta.origin_of(&row);
            copypaste_core::history_item::open_item(
                row,
                &origin,
                self.meta.device_id(),
                &self.keyring.item_key(),
                preview,
            )
            .ok()
            .map(|(item, _)| item)
        }

        fn page(
            &self,
            rows: Vec<copypaste_core::StoredItem>,
            _: Option<String>,
        ) -> copypaste_ipc::ItemPage {
            self.decrypt_page(rows)
        }

        fn save_file(&self, id: u64, item_id: &str, destination: &str) -> Response {
            self.request_blocking(
                id,
                Method::SaveFile {
                    id: item_id.into(),
                    dest_path: destination.into(),
                    download_token: None,
                },
            )
        }
    }

    pub(crate) fn fixture() -> (Arc<Runtime>, tempfile::TempDir) {
        fixture_with(Arc::new(host::UnavailableClipboard))
    }

    fn fixture_with(clipboard: Arc<dyn ClipboardWriter>) -> (Arc<Runtime>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "fixture phone",
            0,
            Arc::new(host::InProcessHost(clipboard)),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        (Arc::new(runtime), dir)
    }

    #[derive(Default)]
    struct RecordingClipboard(Mutex<Vec<String>>, Mutex<Vec<WriteOptions>>);
    impl ClipboardWriter for RecordingClipboard {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _content_type: &str,
            options: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            self.1.lock().unwrap().push(options);
            self.0.lock().unwrap().push(match payload {
                ClipboardPayload::Text(value) => format!("text:{}", value.as_str()),
                ClipboardPayload::Image { content_type, .. } => format!("image:{content_type}"),
                ClipboardPayload::File { metadata, .. } => format!(
                    "file:{}",
                    metadata
                        .as_ref()
                        .and_then(|value| value.source_reference.as_deref())
                        .unwrap_or_default()
                ),
                ClipboardPayload::Unsupported { .. } => {
                    return Err(ClipboardWriteError::UnsupportedContent);
                }
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn manual_and_derived_copies_preserve_native_sensitivity() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        seed_text(&runtime, "secret-source", "synthetic secret");
        runtime
            .store
            .set_secret(&runtime.keyring, "secret-source", true)
            .unwrap();
        assert!(runtime.copy(1, "secret-source", false).ok);
        assert!(runtime.copy(2, "secret-source", true).ok);
        assert!(
            runtime
                .request(
                    3,
                    Method::CopyText {
                        id: "secret-source".into(),
                        text: "derived secret".into(),
                    }
                )
                .await
                .ok
        );
        runtime
            .store
            .set_secret(&runtime.keyring, "secret-source", false)
            .unwrap();
        assert!(runtime.copy(4, "secret-source", false).ok);
        assert_eq!(
            *clipboard.1.lock().unwrap(),
            [true, true, true, false].map(WriteOptions::copy),
            "history copies keep their sensitivity and are never transient"
        );
    }

    #[derive(Default)]
    struct ReceivingClipboard {
        writes: RecordingClipboard,
        acknowledgements: Mutex<usize>,
        concealed: Mutex<Vec<bool>>,
    }

    impl ClipboardWriter for ReceivingClipboard {
        fn write(
            &self,
            payload: &ClipboardPayload,
            kind: &str,
            options: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            self.writes.write(payload, kind, options)
        }

        fn write_received(
            &self,
            payload: &ClipboardPayload,
            kind: &str,
            secret: bool,
        ) -> Result<(), ClipboardWriteError> {
            self.write(payload, kind, WriteOptions::copy(secret))?;
            *self.acknowledgements.lock().unwrap() += 1;
            self.concealed.lock().unwrap().push(secret);
            Ok(())
        }
    }

    #[tokio::test]
    async fn explicit_file_import_publishes_each_named_record_without_capture_admission() {
        let (runtime, dir) = fixture();
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"%PDF original").unwrap();
        let mut events = runtime.subscribe_events();
        for name in ["a.pdf", "b.pdf"] {
            let response = runtime
                .request(
                    1,
                    Method::ImportFile {
                        path: source.to_string_lossy().into_owned(),
                        filename: name.to_owned(),
                        mime_type: "application/pdf".to_owned(),
                        source_reference: Some("content://documents/source.pdf".to_owned()),
                    },
                )
                .await;
            assert!(response.ok, "{response:?}");
            let event = events.recv().await.unwrap();
            assert_eq!(event.event, EventKind::Items);
            assert!(!event.captured);
        }
        assert_eq!(runtime.store.count().unwrap(), 2);
        for row in runtime.store.list(10, 0).unwrap() {
            let Some(item) = runtime.item_value(row, false) else {
                panic!("expected readable item");
            };
            let file = item.file_details.unwrap();
            assert_eq!(
                file.source_reference.as_deref(),
                Some("content://documents/source.pdf")
            );
            assert!(!file.source_available);
            let saved = dir.path().join(format!("{}.saved", item.id));
            assert!(runtime.save_file(2, &item.id, &saved.to_string_lossy()).ok);
            assert_eq!(std::fs::read(saved).unwrap(), b"%PDF original");
        }
    }

    #[tokio::test]
    async fn spoiler_conceal_of_an_empty_session_does_not_reload_history() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_events();
        let response = runtime
            .request(
                1,
                Method::SetSpoilerRevealed {
                    id: None,
                    revealed: false,
                },
            )
            .await;
        assert!(response.ok);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn spoiler_visibility_is_shared_locally_and_reset_by_a_secret_choice() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "private", "private body");
        assert!(
            runtime
                .request(
                    1,
                    Method::SetSecret {
                        id: "private".into(),
                        secret: true
                    }
                )
                .await
                .ok
        );
        let mut first = runtime.subscribe_events();
        let mut second = runtime.subscribe_events();
        let revealed = runtime
            .request(
                2,
                Method::SetSpoilerRevealed {
                    id: Some("private".into()),
                    revealed: true,
                },
            )
            .await;
        assert!(revealed.ok);
        assert!(
            matches!(revealed.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids == ["private"])
        );
        for receiver in [&mut first, &mut second] {
            let event = receiver.try_recv().unwrap();
            assert_eq!(event.event, EventKind::Items);
            assert!(!event.captured);
        }
        let snapshot = runtime.request(3, Method::GetSpoilerState).await;
        assert!(
            matches!(snapshot.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids == ["private"])
        );
        assert!(
            runtime
                .request(
                    4,
                    Method::SetSecret {
                        id: "private".into(),
                        secret: true
                    }
                )
                .await
                .ok
        );
        let snapshot = runtime.request(5, Method::GetSpoilerState).await;
        assert!(
            matches!(snapshot.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids.is_empty())
        );
        assert!(
            runtime
                .request(
                    6,
                    Method::SetSpoilerRevealed {
                        id: Some("private".into()),
                        revealed: true
                    }
                )
                .await
                .ok
        );
        let concealed = runtime
            .request(
                7,
                Method::SetSpoilerRevealed {
                    id: None,
                    revealed: false,
                },
            )
            .await;
        assert!(
            matches!(concealed.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids.is_empty())
        );
        assert!(
            !runtime
                .request(
                    8,
                    Method::SetSpoilerRevealed {
                        id: Some("missing".into()),
                        revealed: true
                    }
                )
                .await
                .ok
        );
    }

    #[tokio::test]
    async fn secret_choice_persists_and_publishes_an_item_event_without_copying() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "privacy-toggle", "toggle text");
        let mut events = runtime.subscribe_events();
        for secret in [true, false] {
            let response = runtime
                .request(
                    10,
                    Method::SetSecret {
                        id: "privacy-toggle".into(),
                        secret,
                    },
                )
                .await;
            assert!(response.ok, "{response:?}");
            let Some(ResponseData::Item(item)) = response.data else {
                panic!("expected item");
            };
            assert_eq!(item.privacy.secret, secret);
            assert_eq!(
                runtime
                    .store
                    .get("privacy-toggle")
                    .unwrap()
                    .unwrap()
                    .clipboard_privacy()
                    .secret,
                secret
            );
            let event = events.recv().await.unwrap();
            assert_eq!(event.event, EventKind::Items);
            assert!(!event.captured);
        }
        assert_eq!(
            runtime
                .request(
                    11,
                    Method::SetSecret {
                        id: "missing".into(),
                        secret: true
                    }
                )
                .await
                .error_code,
            Some(ErrorCode::NotFound)
        );
    }

    #[test]
    fn detected_credentials_are_captured_with_a_hidden_preview() {
        let (runtime, _dir) = fixture();
        assert!(runtime.settings.config().skip_secret);
        let content = "-----BEGIN OPENSSH PRIVATE KEY-----\nsynthetic";
        runtime.capture_text(content).unwrap();
        let row = runtime.store.list(10, 0).unwrap().pop().unwrap();
        let preview = runtime.item_value(row, true).unwrap();
        assert!(preview.privacy.secret);
        assert!(preview.content.is_empty());
        assert_eq!(
            preview.semantic_kind,
            Some(copypaste_ipc::SemanticKind::Credentials)
        );

        // A native host learns the detected secret so its own preview hides it.
        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::Implicit)
            .unwrap();
        let token = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        let privacy = runtime
            .capture_text_operation(token, "-----BEGIN RSA PRIVATE KEY-----\nother")
            .unwrap();
        assert!(privacy.secret);
    }

    fn seed_text(runtime: &Runtime, id: &str, value: &str) {
        let (nonce, content_ciphertext) =
            copypaste_core::encrypt(value.as_bytes(), &runtime.keyring.item_key(), id).unwrap();
        runtime
            .store
            .insert(copypaste_core::NewItem {
                id: id.into(),
                content_ciphertext,
                nonce,
                content_type: copypaste_ipc::content_type::TEXT.into(),
                content_hash: copypaste_core::compute_content_hash(value.as_bytes()),
                search_text: Some(value.into()),
                created_at: 1,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
    }

    #[tokio::test]
    async fn oversized_authenticated_text_has_the_same_full_body_refusal_as_desktop() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        let id = "oversized-legacy";
        seed_text(
            &runtime,
            id,
            &"x".repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1),
        );
        let before = runtime.store.get(id).unwrap().unwrap().content_ciphertext;
        for method in [
            Method::Get { id: id.into() },
            Method::Copy { id: id.into() },
            Method::CopyPlainText { id: id.into() },
        ] {
            let response = runtime.request(1, method).await;
            assert_eq!(response.error_code, Some(ErrorCode::ContentTooLarge));
            assert!(!response.ok);
        }
        let row = runtime.store.get(id).unwrap().unwrap();
        assert_eq!(row.content_ciphertext, before);
        let preview = runtime.item_value(row, true).unwrap();
        assert!(preview.truncated);
        assert!(preview.content.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);
        assert!(clipboard.0.lock().unwrap().is_empty());
    }

    #[test]
    fn email_previews_require_an_explicit_secret_hint_to_hide_content() {
        let (runtime, _dir) = fixture();
        let text = "person.99@example.com";
        seed_text(&runtime, "email-preview", text);
        let stored = runtime.store.get("email-preview").unwrap().unwrap();
        for (metadata, secret) in [
            ("unrecognized old metadata", false),
            (
                r#"{"privacy":{"secret":true},"source_app_icon":"broken"}"#,
                true,
            ),
        ] {
            let mut row = stored.clone();
            row.payload_metadata = Some(metadata.into());
            for preview in [true, false] {
                let item = runtime.item_value(row.clone(), preview).unwrap();
                assert_eq!(item.privacy.secret, secret);
                if secret && preview {
                    assert!(item.content.is_empty());
                    assert!(item.semantic_kind.is_none());
                } else {
                    assert_eq!(item.content, text);
                    assert_eq!(item.semantic_kind, Some(copypaste_ipc::SemanticKind::Email));
                }
            }
        }
    }

    #[tokio::test]
    async fn status_exposes_the_stable_local_device_id() {
        let (runtime, _dir) = fixture();
        let response = runtime.request(1, Method::Status).await;

        match response.data {
            Some(ResponseData::Status(status)) => {
                assert_eq!(status.device_id.as_deref(), Some(runtime.meta.device_id()));
                assert!(status.clipboard_available);
            }
            other => panic!("expected status response, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn history_facets_include_the_typed_local_device() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "local-item", "local text");

        let response = runtime.request(2, Method::HistoryFacets).await;
        let facets = match response.data {
            Some(ResponseData::HistoryFacets(facets)) => facets,
            other => panic!("expected history facets, got {other:?}"),
        };
        assert_eq!(facets.origin_devices.len(), 1);
        assert_eq!(
            facets.origin_devices[0].device_class,
            runtime.meta.device_class()
        );
    }

    #[tokio::test]
    async fn history_query_uses_the_local_facet_identity_for_local_captures() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "local-item", "local text");
        runtime
            .source
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "remote-item",
                content: "remote text",
                binary_content: None,
                payload_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT,
                created_at: 2,
                deleted: false,
                content_hash: None,
                origin_device_id: "remote-device",
                app_bundle_id: None,
                app_name: None,
            })
            .expect("the remote row is stored");

        let local_facet = match runtime.request(1, Method::HistoryFacets).await.data {
            Some(ResponseData::HistoryFacets(facets)) => facets
                .origin_devices
                .into_iter()
                .find(|facet| facet.id == runtime.meta.device_id())
                .expect("the local device facet"),
            other => panic!("{other:?}"),
        };
        let page = match runtime
            .request(
                2,
                Method::HistoryQuery {
                    query: copypaste_ipc::HistoryQuery {
                        origin_device_id: Some(local_facet.id),
                        ..Default::default()
                    },
                    limit: 10,
                    cursor: None,
                },
            )
            .await
            .data
        {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "local-item");
        assert_eq!(page.items[0].origin_device_id, runtime.meta.device_id());
    }

    #[test]
    fn an_all_local_page_uses_local_origin_metadata() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "local-item-a", "local text a");
        seed_text(&runtime, "local-item-b", "local text b");

        let page = runtime.page(runtime.store.list(10, 0).unwrap(), None);
        assert_eq!(page.items.len(), 2);
        assert!(page.items.iter().all(|item| {
            item.origin_device_id == runtime.meta.device_id()
                && item.origin_device_name.as_deref() == Some(runtime.meta.device_name().as_str())
                && item.origin_device_class == runtime.meta.device_class()
        }));
    }

    #[test]
    fn a_remote_item_uses_persisted_peer_metadata_instead_of_local_identity() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "local-item", "local text");
        runtime
            .source
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "remote-item",
                content: "remote text",
                binary_content: None,
                payload_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT,
                created_at: 2,
                deleted: false,
                content_hash: None,
                origin_device_id: "remote-device",
                app_bundle_id: None,
                app_name: None,
            })
            .expect("the remote row is stored");

        runtime.remember_device(&copypaste_p2p::sync::SyncOutcome {
            stats: copypaste_p2p::sync::SyncStats::default(),
            peer_device_id: "remote-device".into(),
            peer_device_name: "Remote phone".into(),
            peer_profile: Some(copypaste_p2p::DeviceProfile {
                device_class: copypaste_ipc::DeviceClass::Phone,
                ..Default::default()
            }),
            peer_listen_addr: None,
            cursor: copypaste_p2p::sync::SyncCursor::default(),
            applied_floor: None,
        });

        let item = match runtime.get(1, "remote-item").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.origin_device_id, "remote-device");
        assert_eq!(item.origin_device_name.as_deref(), Some("Remote phone"));
        assert_eq!(item.origin_device_class, copypaste_ipc::DeviceClass::Phone);
        assert_ne!(
            item.origin_device_name.as_deref(),
            Some(runtime.meta.device_name().as_str())
        );

        let page = runtime.page(runtime.store.list(10, 0).unwrap(), None);
        let remote = page
            .items
            .iter()
            .find(|candidate| candidate.id == "remote-item")
            .expect("the remote item is listed");
        assert_eq!(remote.origin_device_name.as_deref(), Some("Remote phone"));
        assert_eq!(
            remote.origin_device_class,
            copypaste_ipc::DeviceClass::Phone
        );
        let local = page
            .items
            .iter()
            .find(|candidate| candidate.id == "local-item")
            .expect("the local item is listed");
        assert_eq!(
            local.origin_device_name.as_deref(),
            Some(runtime.meta.device_name().as_str())
        );
    }

    #[tokio::test]
    async fn selected_settings_are_persisted_and_reported_by_status() {
        let (runtime, _dir) = fixture();
        let response = runtime
            .request(
                2,
                Method::SetConfig {
                    patch: copypaste_ipc::ConfigPatch {
                        retention_days: Some(30),
                        storage_quota_bytes: Some(5 * 1024 * 1024 * 1024),
                        lan_visibility: Some(false),
                        sync_enabled: Some(false),
                        notify_on_copy: Some(true),
                        sound_on_copy: Some(true),
                        ..Default::default()
                    },
                },
            )
            .await;
        let config = match response.data {
            Some(ResponseData::Config(applied)) => applied.config,
            other => panic!("expected config response, got {other:?}"),
        };
        assert_eq!(config.retention_days, 30);
        assert!(!config.lan_visibility);
        assert!(!config.sync_enabled);
        assert!(config.notify_on_copy && config.sound_on_copy);
        assert!(runtime.notify_on_copy_enabled());
        assert!(runtime.sound_on_copy_enabled());

        let private = runtime
            .request(3, Method::SetPrivateMode { enabled: true })
            .await;
        assert!(matches!(
            private.data,
            Some(ResponseData::PrivateMode(copypaste_ipc::PrivateModeData {
                private_mode: true,
                private_mode_epoch: 1
            }))
        ));
        let status = runtime.request(4, Method::Status).await;
        match status.data {
            Some(ResponseData::Status(status)) => {
                assert!(status.private_mode);
                assert_eq!(status.private_mode_epoch, 1);
            }
            other => panic!("expected status response, got {other:?}"),
        }
    }

    /// Local captures store no origin, so the local device filter must count
    /// them exactly as the list shows them.
    #[tokio::test]
    async fn history_counts_answer_in_process_with_the_local_device_filter() {
        let (runtime, _dir) = fixture();
        for id in ["first", "second", "third"] {
            seed_text(&runtime, id, id);
        }
        assert!(runtime.store.set_pinned("third", true).unwrap());
        let local = copypaste_ipc::HistoryQuery {
            origin_device_id: Some(runtime.meta.device_id().to_owned()),
            ..Default::default()
        };
        let ranges = vec![copypaste_ipc::HistoryCountRange {
            from_ms: 0,
            before_ms: 2,
        }];
        let counted = runtime
            .request(
                1,
                Method::HistoryCounts {
                    query: local.clone(),
                    ranges: ranges.clone(),
                },
            )
            .await;
        assert!(matches!(
            counted.data,
            Some(ResponseData::HistoryCounts(Some(counts)))
                if counts.pinned == 1 && counts.ranges == [2]
        ));

        let searching = runtime
            .request(
                2,
                Method::HistoryCounts {
                    query: copypaste_ipc::HistoryQuery {
                        search: Some("first".into()),
                        ..local
                    },
                    ranges,
                },
            )
            .await;
        assert!(matches!(
            searching.data,
            Some(ResponseData::HistoryCounts(None))
        ));
    }

    /// The readout rides the settings replies, and a reply that tightened
    /// retention reports the history as the sweep left it.
    #[tokio::test]
    async fn settings_replies_report_quota_usage_through_pruning() {
        fn usage(response: Response) -> copypaste_ipc::StorageUsage {
            match response.data {
                Some(ResponseData::Config(applied)) => {
                    applied.storage_usage.expect("usage is reported")
                }
                other => panic!("expected config response, got {other:?}"),
            }
        }
        let (runtime, _dir) = fixture();
        assert_eq!(
            usage(runtime.request(1, Method::GetConfig).await),
            copypaste_ipc::StorageUsage {
                used_bytes: 0,
                oldest_unpinned_at: None,
            }
        );

        for n in 0..12 {
            seed_text(&runtime, &format!("clip-{n:02}"), &format!("text {n:02}"));
        }
        let clip_bytes = runtime
            .store
            .get("clip-00")
            .unwrap()
            .unwrap()
            .content_ciphertext
            .len() as u64;
        assert_eq!(
            usage(runtime.request(2, Method::GetConfig).await),
            copypaste_ipc::StorageUsage {
                used_bytes: 12 * clip_bytes,
                oldest_unpinned_at: Some(1),
            }
        );

        let tightened = runtime
            .request(
                3,
                Method::SetConfig {
                    patch: copypaste_ipc::ConfigPatch {
                        history_limit: Some(10),
                        ..Default::default()
                    },
                },
            )
            .await;
        assert_eq!(usage(tightened).used_bytes, 10 * clip_bytes);
        assert_eq!(
            usage(runtime.request(4, Method::GetConfig).await).used_bytes,
            10 * clip_bytes
        );
    }

    #[tokio::test]
    async fn ordinary_configuration_changes_publish_one_watch_revision() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_runtime_events();
        let patch = copypaste_ipc::ConfigPatch {
            notify_on_copy: Some(true),
            ..Default::default()
        };
        assert!(
            runtime
                .request(
                    1,
                    Method::SetConfig {
                        patch: patch.clone()
                    }
                )
                .await
                .ok
        );
        assert!(events.try_recv().unwrap().capture.is_some());
        assert!(events.try_recv().is_err());
        assert!(runtime.request(2, Method::SetConfig { patch }).await.ok);
        assert!(events.try_recv().is_err());
    }

    #[tokio::test]
    async fn export_backup_and_confirmed_restore_use_the_shared_store() {
        let (runtime, dir) = fixture();
        seed_text(&runtime, "first", "first text");

        let exported = runtime.request(5, Method::Export { limit: 0 }).await;
        match exported.data {
            Some(ResponseData::Export(data)) => {
                assert_eq!(data.items.len(), 1);
                assert_eq!(data.items[0].content, "first text");
            }
            other => panic!("expected export response, got {other:?}"),
        }

        let backup_path = dir.path().join("history.copypaste-backup");
        assert!(
            runtime
                .request(
                    6,
                    Method::Backup {
                        dest_path: backup_path.to_string_lossy().into_owned(),
                    },
                )
                .await
                .ok
        );
        seed_text(&runtime, "second", "second text");
        assert_eq!(runtime.store.count().unwrap(), 2);

        assert!(
            runtime
                .request(
                    7,
                    Method::Restore {
                        src_path: backup_path.to_string_lossy().into_owned(),
                        confirm: true,
                    },
                )
                .await
                .ok
        );
        assert_eq!(runtime.store.count().unwrap(), 1);
        assert!(runtime.store.get("first").unwrap().is_some());
        assert!(runtime.store.get("second").unwrap().is_none());
    }

    #[test]
    fn streamed_export_and_backup_emit_frames_then_their_reply() {
        let (runtime, dir) = fixture();
        seed_text(&runtime, "first", "first text");

        let mut frames = Vec::new();
        let reply = runtime.request_stream(8, Method::ExportStream, &mut |frame| {
            frames.push(frame);
            true
        });
        let Some(ResponseData::Export(summary)) = reply.data else {
            panic!("expected an export summary, got {reply:?}")
        };
        assert!(summary.items.is_empty());
        let Some(ResponseData::ExportChunk(chunk)) = &frames.last().unwrap().data else {
            panic!("expected export chunks")
        };
        assert_eq!(chunk.progress.processed, chunk.progress.total);
        assert!(frames.iter().all(|frame| frame.id == 8));

        let dest = dir.path().join("streamed.copypaste-backup");
        let method = Method::BackupStream {
            dest_path: dest.to_string_lossy().into_owned(),
        };
        let mut progress = Vec::new();
        let reply = runtime.request_stream(9, method.clone(), &mut |frame| {
            progress.push(frame);
            true
        });
        assert!(
            matches!(reply.data, Some(ResponseData::Backup(_))),
            "{reply:?}"
        );
        assert!(matches!(
            progress.last().and_then(|frame| frame.data.as_ref()),
            Some(ResponseData::Progress(p)) if p.processed == p.total
        ));

        std::fs::remove_file(&dest).unwrap();
        let abandoned = runtime.request_stream(10, method, &mut |_| false);
        assert!(!abandoned.ok);
        assert!(!dest.exists());
        assert!(!runtime.request_blocking(11, Method::ExportStream).ok);
    }

    #[test]
    fn sms_code_uses_encrypted_history_events_and_clipboard_once() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        let mut events = runtime.subscribe_events();
        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::ModuleEvent)
            .unwrap();
        let token = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        runtime.publish_sms_code(token, "007123").unwrap();
        let event = events.try_recv().unwrap();
        let id = event.captured_item_id.unwrap();
        let item = match runtime.get(0, &id).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.content, "007123");
        assert_eq!(*clipboard.0.lock().unwrap(), ["text:007123"]);
        assert!(runtime.publish_sms_code(token, "999999").is_err());
        assert_eq!(runtime.store.count().unwrap(), 1);
        runtime.capture_admission().abandon(token);
        runtime.capture_admission().revoke_host(host).unwrap();
    }

    #[test]
    fn no_sms_module_or_revoked_privacy_scope_never_publishes_message_bodies() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::ModuleEvent)
            .unwrap();
        let token = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        assert_eq!(
            runtime.capture_sms_operation(token, "Your code is 007123"),
            SmsCaptureOutcome::Ignored
        );
        assert_eq!(runtime.store.count().unwrap(), 0);
        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                private_mode: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert!(runtime.publish_sms_code(token, "007123").is_err());
        assert!(runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .is_none());
        assert!(clipboard.0.lock().unwrap().is_empty());
        assert_eq!(runtime.store.count().unwrap(), 0);
        runtime.capture_admission().revoke_host(host).unwrap();
    }

    #[test]
    fn sms_capture_retries_operational_refusal_but_discards_private_mode_messages() {
        let (runtime, _dir) = fixture();
        assert_eq!(
            runtime.capture_sms_operation(999, "Your code is 007123"),
            SmsCaptureOutcome::Retry
        );

        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::ModuleEvent)
            .unwrap();
        let token = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                private_mode: Some(true),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            runtime.capture_sms_operation(token, "Your code is 007123"),
            SmsCaptureOutcome::Ignored
        );
        runtime.capture_admission().revoke_host(host).unwrap();
    }

    #[test]
    fn copy_and_plain_copy_use_the_injected_typed_port() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        seed_text(&runtime, "text-item", "trusted text");
        seed_text(&runtime, "z-newer", "another clip");
        let mut events = runtime.subscribe_events();
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, "z-newer");
        assert!(runtime.copy(1, "text-item", false).ok);
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, "text-item");
        let first_stamp = runtime.store.get("text-item").unwrap().unwrap().created_at;
        let event = events.try_recv().unwrap();
        assert_eq!(event.event, EventKind::Items);
        assert!(!event.captured);
        assert!(runtime.copy(2, "z-newer", false).ok);
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, "z-newer");
        assert!(runtime.copy(2, "text-item", true).ok);
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, "text-item");
        assert!(runtime.store.get("text-item").unwrap().unwrap().created_at > first_stamp);
        assert_eq!(runtime.store.count().unwrap(), 2);
        assert_eq!(
            *clipboard.0.lock().unwrap(),
            vec![
                "text:trusted text",
                "text:another clip",
                "text:trusted text"
            ]
        );
    }

    #[test]
    fn a_failed_copy_does_not_promote_history_or_emit_a_change() {
        struct RefusingClipboard;
        impl ClipboardWriter for RefusingClipboard {
            fn write(
                &self,
                _: &ClipboardPayload,
                _: &str,
                _: WriteOptions,
            ) -> Result<(), ClipboardWriteError> {
                Err(ClipboardWriteError::Failed)
            }
        }
        let (runtime, _dir) = fixture_with(Arc::new(RefusingClipboard));
        seed_text(&runtime, "text-item", "trusted text");
        let mut events = runtime.subscribe_events();
        assert!(!runtime.copy(1, "text-item", false).ok);
        assert_eq!(
            runtime.store.get("text-item").unwrap().unwrap().created_at,
            1
        );
        assert!(events.try_recv().is_err());
    }

    #[tokio::test]
    async fn selection_copy_promotes_its_source_only_after_a_successful_native_write() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        seed_text(&runtime, "source", "full original text");
        seed_text(&runtime, "z-newer", "newer text");
        let original = runtime.store.get("source").unwrap().unwrap();
        let mut events = runtime.subscribe_events();
        assert!(
            runtime
                .request(
                    1,
                    Method::CopyText {
                        id: "source".into(),
                        text: "original".into()
                    }
                )
                .await
                .ok
        );
        assert_eq!(*clipboard.0.lock().unwrap(), ["text:original"]);
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, "source");
        let copied = runtime.store.get("source").unwrap().unwrap();
        assert_eq!(copied.content_ciphertext, original.content_ciphertext);
        assert_eq!(runtime.store.count().unwrap(), 2);
        assert_eq!(events.try_recv().unwrap().event, EventKind::Items);
        let stamp = copied.created_at;
        assert!(copypaste_core::history_actions::copy_text(
            &runtime.store,
            "source",
            "refused",
            |_| Err(ClipboardWriteError::Failed)
        )
        .is_err());
        assert_eq!(
            runtime.store.get("source").unwrap().unwrap().created_at,
            stamp
        );
        assert!(
            !runtime
                .request(
                    2,
                    Method::CopyText {
                        id: "missing".into(),
                        text: "never written".into()
                    }
                )
                .await
                .ok
        );
        assert_eq!(clipboard.0.lock().unwrap().len(), 1);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn the_opened_runtime_applies_received_content_through_its_platform_writer() {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(ReceivingClipboard::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "receiver",
            0,
            Arc::new(host::InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        let incoming = copypaste_core::RemoteVersion {
            item_id: "incoming",
            content: "from another device",
            binary_content: None,
            payload_metadata: None,
            content_type: "text",
            created_at: 100,
            deleted: false,
            content_hash: None,
            origin_device_id: "sender",
            app_bundle_id: None,
            app_name: None,
        };
        assert!(runtime.source.apply_version(&incoming).unwrap());
        assert_eq!(
            *clipboard.writes.0.lock().unwrap(),
            ["text:from another device"]
        );
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
        assert_eq!(*clipboard.concealed.lock().unwrap(), [false]);
        assert!(!runtime.source.apply_version(&incoming).unwrap());
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                instant_clipboard: Some(false),
                ..Default::default()
            })
            .unwrap();
        assert!(runtime
            .source
            .apply_version(&copypaste_core::RemoteVersion {
                created_at: 200,
                ..incoming
            })
            .unwrap());
        assert_eq!(clipboard.writes.0.lock().unwrap().len(), 1);
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
        assert_eq!(
            runtime.store.get("incoming").unwrap().unwrap().created_at,
            200
        );
        assert!(runtime.copy(1, "incoming", false).ok);
        assert_eq!(clipboard.writes.0.lock().unwrap().len(), 2);
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
    }

    #[test]
    fn received_secrets_reach_the_clipboard_with_concealed_native_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(ReceivingClipboard::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "receiver",
            0,
            Arc::new(host::InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        let incoming = copypaste_core::RemoteVersion {
            item_id: "secret-incoming",
            content: "synthetic confidential text",
            binary_content: None,
            payload_metadata: Some(r#"{"privacy":{"secret":true}}"#),
            content_type: "text",
            created_at: 100,
            deleted: false,
            content_hash: None,
            origin_device_id: "sender",
            app_bundle_id: None,
            app_name: None,
        };
        assert!(runtime.source.apply_version(&incoming).unwrap());
        assert_eq!(
            *clipboard.writes.0.lock().unwrap(),
            ["text:synthetic confidential text"]
        );
        assert_eq!(*clipboard.concealed.lock().unwrap(), [true]);
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
        assert!(!runtime.source.apply_version(&incoming).unwrap());
        assert_eq!(*clipboard.acknowledgements.lock().unwrap(), 1);
        assert!(runtime.store.search("confidential", 10).unwrap().is_empty());
    }

    #[test]
    fn item_response_carries_semantic_kind_and_color_swatch() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "color-item", "oklch(50% 0.1 30)");

        let item = match runtime.get(1, "color-item").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.semantic_kind, Some(copypaste_ipc::SemanticKind::Color));
        assert!(item.color_rgba.is_some());
    }

    #[tokio::test]
    async fn queued_image_preview_leaves_status_responsive_and_cancels_cleanly() {
        use std::time::Duration;

        let (runtime, _dir) = fixture();
        let permit = copypaste_core::image_preview::acquire_preview_source().await;
        let method = Method::ImagePreview {
            id: "missing".into(),
            max_edge: None,
            bounds: None,
        };
        let mut preview = Box::pin(runtime.request(1, method));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut preview)
                .await
                .is_err()
        );
        let status =
            tokio::time::timeout(Duration::from_secs(2), runtime.request(2, Method::Status))
                .await
                .expect("queued preview must not block status");
        assert!(status.ok);
        drop(preview);
        drop(permit);
        let permit = tokio::time::timeout(
            Duration::from_secs(2),
            copypaste_core::image_preview::acquire_preview_source(),
        )
        .await
        .expect("cancelled preview must not consume a permit");
        drop(permit);
    }

    #[tokio::test]
    async fn image_preview_decodes_image_and_recovers_after_missing_item() {
        let (runtime, _dir) = fixture();
        let source = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        let row = copypaste_core::ingest_binary_into_with_capture_context(
            &runtime.store,
            &runtime.keyring,
            &source,
            copypaste_ipc::content_type::IMAGE_PNG,
            1,
            None,
            None,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();
        let missing = Method::ImagePreview {
            id: "missing".into(),
            max_edge: None,
            bounds: None,
        };
        assert!(!runtime.request(1, missing).await.ok);
        let method = Method::ImagePreview {
            id: row.id,
            max_edge: None,
            bounds: None,
        };
        let preview = match runtime.request(2, method).await.data {
            Some(ResponseData::ImagePreview(preview)) => preview,
            other => panic!("{other:?}"),
        };
        assert_eq!((preview.width, preview.height), (1, 1));
        assert_eq!(
            &STANDARD.decode(preview.png_base64).unwrap()[..8],
            b"\x89PNG\r\n\x1a\n"
        );
    }

    #[test]
    fn selected_image_carries_original_metadata_but_list_preview_does_not() {
        let (runtime, _dir) = fixture();
        let source = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        let row = copypaste_core::ingest_binary_into_with_capture_context(
            &runtime.store,
            &runtime.keyring,
            &source,
            copypaste_ipc::content_type::IMAGE_PNG,
            1,
            None,
            None,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();

        let listed = runtime.item_value(row.clone(), true).unwrap();
        assert!(listed.image_details.is_none());

        let detail = runtime.item_value(row, false).unwrap();
        assert_eq!(detail.origin_device_class, runtime.meta.device_class());
        let metadata = detail.image_details.expect("selected image metadata");
        assert_eq!((metadata.width, metadata.height), (1, 1));
        assert_eq!(metadata.size_bytes, source.len() as u64);
    }

    #[test]
    fn platform_capture_writes_history_and_emits_captured_events() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_events();

        runtime.capture_text("copied on Android").unwrap();
        let text_event = events.try_recv().unwrap();
        assert!(text_event.captured);
        let text_id = text_event.captured_item_id.expect("captured text ID");
        assert_eq!(
            runtime.store.get(&text_id).unwrap().unwrap().content_type,
            copypaste_ipc::content_type::TEXT
        );
        assert_eq!(text_event.item_count, 1);

        runtime
            .capture_binary(
                &[1, 2, 3],
                "application/pdf",
                Some("paper.pdf"),
                Some("content://documents/paper.pdf"),
            )
            .unwrap();
        let file_event = events.try_recv().unwrap();
        let file_id = file_event
            .captured_item_id
            .as_ref()
            .expect("captured file ID");
        assert_ne!(file_id, &text_id);
        assert!(file_event.captured);
        assert_eq!(file_event.item_count, 2);
    }

    #[tokio::test]
    async fn in_process_file_save_emits_token_progress() {
        let (runtime, dir) = fixture();
        let source = dir.path().join("original.bin");
        let bytes = vec![3; copypaste_core::file_transfer::CHUNK_BYTES * 2 + 1];
        std::fs::write(&source, &bytes).unwrap();
        let row = copypaste_core::file_import::import_file(
            &runtime.store,
            &runtime.keyring,
            &source,
            "original.bin",
            "application/octet-stream",
            source.to_str(),
            &runtime.settings.config(),
        )
        .unwrap();
        let destination = dir.path().join("saved.bin");
        let mut progress = runtime.subscribe_runtime_events();
        let mut legacy = runtime.subscribe_events();
        let result = runtime
            .request(
                1,
                Method::SaveFile {
                    id: row.id,
                    dest_path: destination.to_string_lossy().into_owned(),
                    download_token: Some("token".into()),
                },
            )
            .await;
        assert!(result.ok, "{:?}", result.error);
        let mut last = 0;
        while let Ok(event) = progress.try_recv() {
            let event = event.progress.unwrap();
            assert_eq!(event.token, "token");
            assert!(event.written > last);
            assert_eq!(event.total, bytes.len() as u64);
            last = event.written;
        }
        assert_eq!(last, bytes.len() as u64);
        assert!(matches!(
            legacy.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(std::fs::read(destination).unwrap(), bytes);
    }

    #[test]
    fn image_export_keeps_original_bytes_and_never_overwrites_a_destination() {
        let (runtime, dir) = fixture();
        let original = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        runtime
            .capture_binary(&original, "image/png", None, None)
            .unwrap();
        let row = runtime.store.list(1, 0).unwrap().pop().unwrap();
        let destination = dir.path().join("ocr-input.png");
        assert!(
            runtime
                .save_file(1, &row.id, &destination.to_string_lossy())
                .ok
        );
        assert_eq!(std::fs::read(&destination).unwrap(), original);
        assert!(
            !runtime
                .save_file(2, &row.id, &destination.to_string_lossy())
                .ok
        );
        assert_eq!(std::fs::read(&destination).unwrap(), original);
        assert_eq!(runtime.store.count().unwrap(), 1);

        seed_text(&runtime, "text-export", "text");
        let text_destination = dir.path().join("text.png");
        assert!(
            !runtime
                .save_file(3, "text-export", &text_destination.to_string_lossy())
                .ok
        );
        assert!(!text_destination.exists());
    }

    #[test]
    fn screenshot_capture_saves_available_source_fields_and_allows_missing_attribution() {
        let png = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=").unwrap();
        for source in [
            None,
            Some(CaptureSourceApp {
                bundle_id: "com.example.editor".into(),
                name: None,
                icon: None,
            }),
            Some(CaptureSourceApp {
                bundle_id: "com.example.editor".into(),
                name: Some("Editor".into()),
                icon: copypaste_core::SourceAppIconMetadata::new(&png, 1, 1),
            }),
        ] {
            let (runtime, _dir) = fixture();
            let host = runtime
                .capture_admission()
                .open_host(CaptureKind::Implicit)
                .unwrap();
            let token = runtime
                .capture_admission()
                .begin(host, Arc::new(|| {}))
                .unwrap();
            runtime
                .capture_binary_operation(token, &png, "image/png", None, None, source.as_ref())
                .unwrap();
            let row = runtime.store.list(1, 0).unwrap().pop().unwrap();
            assert_eq!(
                row.app_bundle_id.as_deref(),
                source.as_ref().map(|source| source.bundle_id.as_str())
            );
            assert_eq!(
                row.app_name.as_deref(),
                source.as_ref().and_then(|source| source.name.as_deref())
            );
            assert_eq!(&*runtime.source.open_bytes(&row).unwrap(), &png);
            let item = match runtime.get(1, &row.id).data {
                Some(ResponseData::Item(item)) => item,
                other => panic!("{other:?}"),
            };
            assert_eq!(
                item.source_app_icon_id.is_some(),
                source.as_ref().is_some_and(|source| source.icon.is_some())
            );
            if let Some(icon_id) = item.source_app_icon_id {
                assert_eq!(
                    runtime
                        .store
                        .source_app_icon_by_id(&icon_id)
                        .unwrap()
                        .unwrap()
                        .png()
                        .unwrap(),
                    png
                );
            }
        }
    }

    #[test]
    fn a_captured_file_keeps_its_uri_and_copy_writes_that_reference() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, dir) = fixture_with(clipboard.clone());
        runtime
            .capture_binary(
                b"file bytes",
                "application/pdf",
                Some("paper.pdf"),
                Some("content://documents/paper.pdf"),
            )
            .unwrap();
        let row = runtime.store.list(1, 0).unwrap().pop().unwrap();
        let item = match runtime.get(1, &row.id).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.content_class, copypaste_ipc::ContentClass::File);
        assert_eq!(item.content, "content://documents/paper.pdf");
        assert_eq!(
            item.file_details
                .as_ref()
                .and_then(|details| details.source_reference.as_deref()),
            Some("content://documents/paper.pdf")
        );
        assert!(runtime.copy(2, &row.id, false).ok);
        assert_eq!(
            *clipboard.0.lock().unwrap(),
            vec!["file:content://documents/paper.pdf"]
        );
        let destination = dir.path().join("paper.pdf");
        assert!(!runtime.store.has_file_transfer(&row.id).unwrap());
        assert!(runtime.store.file_reference(&row.id).unwrap().is_some());
        // This fixture has no provider resolver: capture retained only the URI,
        // so it cannot recover a hidden copy of the supplied compatibility bytes.
        assert!(
            !runtime
                .save_file(3, &row.id, &destination.to_string_lossy())
                .ok
        );
        assert!(!destination.exists());
    }

    #[test]
    fn a_received_file_copy_does_not_write_the_senders_uri() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        runtime
            .capture_binary(
                b"file bytes",
                "application/pdf",
                Some("paper.pdf"),
                Some("content://sender/paper.pdf"),
            )
            .unwrap();
        let source = runtime.store.list(1, 0).unwrap().pop().unwrap();
        let ClipboardPayload::File { bytes: offer, .. } =
            ClipboardPayload::open(&source, &runtime.keyring.item_key()).unwrap()
        else {
            panic!("expected file offer")
        };
        assert!(runtime
            .source
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "received-paper",
                content: "",
                binary_content: Some(&offer),
                payload_metadata: source.payload_metadata.as_deref(),
                content_type: copypaste_ipc::content_type::FILE,
                created_at: copypaste_core::now_ms() + 1,
                deleted: false,
                content_hash: None,
                origin_device_id: "sender",
                app_bundle_id: None,
                app_name: None,
            })
            .unwrap());
        clipboard.0.lock().unwrap().clear();
        assert!(!runtime.copy(1, "received-paper", false).ok);
        assert!(clipboard.0.lock().unwrap().is_empty());
    }

    #[test]
    fn encrypted_commit_wins_before_pause_and_pause_wins_before_new_commit() {
        use std::sync::{mpsc, Barrier};
        for kind in [CaptureKind::Implicit, CaptureKind::Explicit] {
            for binary in [false, true] {
                for duplicate in [false, true] {
                    let (runtime, _dir) = fixture();
                    let mut events = runtime.subscribe_events();
                    let capture = |runtime: &Runtime, token| {
                        if binary {
                            runtime.capture_binary_operation(
                                token,
                                b"barrier file",
                                "application/pdf",
                                Some("paper.pdf"),
                                Some("content://documents/paper.pdf"),
                                None,
                            )
                        } else {
                            runtime
                                .capture_text_operation(token, "barrier text")
                                .map(|_| ())
                        }
                    };
                    let host = runtime.capture_admission().open_host(kind).unwrap();
                    if duplicate {
                        let token = runtime
                            .capture_admission()
                            .begin(host, Arc::new(|| {}))
                            .unwrap();
                        capture(&runtime, token).unwrap();
                        runtime.capture_admission().abandon(token);
                        assert!(events.try_recv().unwrap().captured);
                    }
                    let entered = Arc::new(Barrier::new(2));
                    let release = Arc::new(Barrier::new(2));
                    let commit_entered = Arc::clone(&entered);
                    let commit_release = Arc::clone(&release);
                    runtime.capture_admission().before_next_commit(move || {
                        commit_entered.wait();
                        commit_release.wait();
                    });
                    let invalidated = Arc::new(Barrier::new(2));
                    let cancelled = Arc::clone(&invalidated);
                    let token = runtime
                        .capture_admission()
                        .begin(
                            host,
                            Arc::new(move || {
                                cancelled.wait();
                            }),
                        )
                        .unwrap();
                    let worker_runtime = Arc::clone(&runtime);
                    let worker = std::thread::spawn(move || {
                        if binary {
                            worker_runtime.capture_binary_operation(
                                token,
                                b"barrier file",
                                "application/pdf",
                                Some("paper.pdf"),
                                Some("content://documents/paper.pdf"),
                                None,
                            )
                        } else {
                            worker_runtime
                                .capture_text_operation(token, "barrier text")
                                .map(|_| ())
                        }
                    });
                    entered.wait();
                    let pause_runtime = Arc::clone(&runtime);
                    let (done_tx, done_rx) = mpsc::channel();
                    let pause = std::thread::spawn(move || {
                        pause_runtime
                            .settings
                            .apply(&copypaste_ipc::ConfigPatch {
                                private_mode: Some(true),
                                ..Default::default()
                            })
                            .unwrap();
                        done_tx.send(()).unwrap();
                    });
                    invalidated.wait();
                    assert!(done_rx.try_recv().is_err());
                    assert_eq!(
                        runtime.store.count().unwrap(),
                        usize::from(duplicate) as u64
                    );
                    release.wait();
                    worker.join().unwrap().unwrap();
                    done_rx.recv().unwrap();
                    pause.join().unwrap();
                    assert_eq!(runtime.store.count().unwrap(), 1);
                    assert!(events.try_recv().unwrap().captured);
                    let stored = runtime.store.list(1, 0).unwrap().pop().unwrap();
                    assert!(capture(&runtime, token).is_err());
                    assert_eq!(
                        runtime.store.list(1, 0).unwrap()[0].created_at,
                        stored.created_at
                    );
                    assert!(events.try_recv().is_err());
                    assert!(runtime
                        .capture_admission()
                        .acquire(token, CaptureScope::Completion)
                        .is_none());
                }
            }
        }
    }

    #[test]
    fn token_failures_and_lowered_live_caps_emit_no_success_or_duplicate_bump() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_events();
        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::Explicit)
            .unwrap();
        let failed = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        assert!(runtime
            .capture_binary_operation(failed, b"file", "application/pdf", None, None, None)
            .is_err());
        assert!(runtime
            .capture_admission()
            .acquire(failed, CaptureScope::Completion)
            .is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(runtime.store.count().unwrap(), 0);
        let old = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        let large = "x".repeat(copypaste_ipc::MIN_TEXT_SIZE_BYTES as usize + 1);
        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                max_text_size_bytes: Some(copypaste_ipc::MIN_TEXT_SIZE_BYTES),
                ..Default::default()
            })
            .unwrap();
        assert!(runtime.capture_text_operation(old, &large).is_err());
        let new = runtime
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        assert!(runtime.capture_text_operation(new, &large).is_err());
        assert!(runtime
            .capture_admission()
            .acquire(new, CaptureScope::Completion)
            .is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(runtime.store.count().unwrap(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocked_read_keeps_settings_pending_without_blocking_the_reactor() {
        let (runtime, _dir) = fixture();
        let host = runtime
            .capture_admission()
            .open_host(CaptureKind::Explicit)
            .unwrap();
        let invalidated = Arc::new(tokio::sync::Notify::new());
        let cancelled = Arc::clone(&invalidated);
        let token = runtime
            .capture_admission()
            .begin(host, Arc::new(move || cancelled.notify_one()))
            .unwrap();
        let read = runtime
            .capture_admission()
            .acquire(token, CaptureScope::Read)
            .unwrap();
        let worker_runtime = Arc::clone(&runtime);
        let pause = tokio::spawn(async move {
            worker_runtime
                .request(5, Method::SetPrivateMode { enabled: true })
                .await
        });
        invalidated.notified().await;
        assert!(!pause.is_finished());
        assert!(!runtime.settings.config().private_mode);
        let heartbeat = tokio::spawn(async {
            tokio::task::yield_now().await;
            true
        });
        assert!(heartbeat.await.unwrap());
        drop(read);
        assert!(pause.await.unwrap().ok);
        assert!(runtime.settings.config().private_mode);
        assert!(runtime.capture_text_operation(token, "stale").is_err());
    }

    #[test]
    fn activity_host_replacement_reuses_runtime_without_reviving_old_operations() {
        let (runtime, _dir) = fixture();
        let mut events = runtime.subscribe_events();
        let old_host = runtime
            .capture_admission()
            .open_host(CaptureKind::Implicit)
            .unwrap();
        let old = runtime
            .capture_admission()
            .begin(old_host, Arc::new(|| {}))
            .unwrap();
        runtime.capture_admission().revoke_host(old_host).unwrap();
        runtime.capture_admission().drain_host(old_host).unwrap();
        assert!(runtime.capture_text_operation(old, "old activity").is_err());
        runtime.capture_text("reopened activity").unwrap();
        assert_eq!(runtime.store.count().unwrap(), 1);
        assert!(events.try_recv().unwrap().captured);
        runtime.shutdown();
        assert!(runtime.capture_text("after true shutdown").is_err());
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn platform_capture_fails_closed_without_source_app_attribution() {
        let (runtime, _dir) = fixture();
        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["com.example.secret".into()]),
                ..Default::default()
            })
            .unwrap();

        assert!(!runtime.implicit_capture_allowed());
        assert!(matches!(
            runtime.capture_text("must not be captured"),
            Err(RuntimeError::CaptureRefused)
        ));
        assert_eq!(runtime.store.count().unwrap(), 0);

        runtime.capture_explicit_text("shared by the user").unwrap();
        assert_eq!(runtime.store.count().unwrap(), 1);

        runtime
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                private_mode: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert!(matches!(
            runtime.capture_explicit_text("private"),
            Err(RuntimeError::CaptureRefused)
        ));
        assert_eq!(runtime.store.count().unwrap(), 1);
    }

    #[tokio::test]
    async fn local_text_and_image_capture_sync_without_a_manual_request() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        receiver.start_listener().await.unwrap();
        sender.start_listener().await.unwrap();
        let receiver_address: std::net::SocketAddr =
            receiver.p2p.node().listen_addr().unwrap().parse().unwrap();
        let sender_address: std::net::SocketAddr =
            sender.p2p.node().listen_addr().unwrap().parse().unwrap();
        let token = copypaste_p2p::PairingToken::generate();
        for (runtime, address) in [(&receiver, sender_address), (&sender, receiver_address)] {
            runtime
                .p2p
                .node()
                .peers()
                .upsert(copypaste_p2p::Peer {
                    pairing_id: token.pairing_id(),
                    device_id: None,
                    name: "automatic peer".into(),
                    psk: token.psk(),
                    last_addr: Some(address),
                    last_seen_ms: 0,
                    profile: None,
                    profile_observed_at_ms: 0,
                })
                .unwrap();
        }
        let running_manual_round = sender.p2p.driver.rounds.enter().await;
        sender.capture_text("automatic clipboard text").unwrap();
        let png = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=").unwrap();
        let source = CaptureSourceApp {
            bundle_id: "com.example.editor".into(),
            name: Some("Editor".into()),
            icon: copypaste_core::SourceAppIconMetadata::new(&png, 1, 1),
        };
        let host = sender
            .capture_admission()
            .open_host(CaptureKind::Implicit)
            .unwrap();
        let capture = sender
            .capture_admission()
            .begin(host, Arc::new(|| {}))
            .unwrap();
        sender
            .capture_binary_operation(capture, &png, "image/png", None, None, Some(&source))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(running_manual_round);
        tokio::time::timeout(Duration::from_secs(2), async {
            while receiver.store.count().unwrap() != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let row = receiver
            .store
            .list(2, 0)
            .unwrap()
            .into_iter()
            .find(|row| row.content_type == "image/png")
            .unwrap();
        assert_eq!(&*receiver.source.open_bytes(&row).unwrap(), &png);
        assert_eq!(row.app_bundle_id.as_deref(), Some("com.example.editor"));
        assert_eq!(row.app_name.as_deref(), Some("Editor"));
        assert_eq!(
            receiver
                .store
                .source_app_icon_by_id(row.source_icon_id.as_deref().unwrap())
                .unwrap()
                .unwrap()
                .png()
                .unwrap(),
            png
        );
        assert_eq!(
            receiver.store.count().unwrap(),
            sender.store.count().unwrap()
        );
        receiver.shutdown();
        sender.shutdown();
    }

    #[tokio::test]
    async fn inbound_loopback_merge_emits_items_and_peers() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        seed_text(&sender, "remote-item", "from peer");
        receiver.start_listener().await.unwrap();
        receiver.p2p.driver.cycle.set_enabled(false);
        sender.start_listener().await.unwrap();
        sender.p2p.driver.cycle.set_enabled(false);
        let remote_addr = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(address) = receiver.p2p.node().listen_addr() {
                    break address;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let token = copypaste_p2p::PairingToken::generate();
        let pairing_id = token.pairing_id();
        for (runtime, address) in [(&receiver, None), (&sender, remote_addr.parse().ok())] {
            runtime
                .p2p
                .node()
                .peers()
                .upsert(copypaste_p2p::Peer {
                    pairing_id: pairing_id.clone(),
                    device_id: None,
                    name: "loopback".into(),
                    psk: token.psk(),
                    last_addr: address,
                    last_seen_ms: 0,
                    profile: None,
                    profile_observed_at_ms: 0,
                })
                .unwrap();
        }
        let mut events = receiver.subscribe_events();
        let peer = sender.p2p.node().peers().get(&pairing_id).unwrap();
        sender
            .p2p
            .node()
            .sync_one(&peer, sender.source.as_ref())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            let mut items = false;
            let mut peers = false;
            while !items || !peers {
                let event = events.recv().await.unwrap();
                if event.sync_status.is_some() {
                    assert_eq!(event.event, EventKind::Peers);
                } else {
                    items |= event.event == EventKind::Items;
                    peers |= event.event == EventKind::Peers;
                }
            }
        })
        .await
        .unwrap();
        assert!(receiver.store.get("remote-item").unwrap().is_some());
        receiver.shutdown();
        sender.shutdown();
    }

    #[tokio::test]
    async fn inbound_probe_emits_one_peers_event() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        receiver.start_listener().await.unwrap();
        receiver.p2p.driver.cycle.set_enabled(false);
        let remote_addr = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(address) = receiver.p2p.node().listen_addr() {
                    break address;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let token = copypaste_p2p::PairingToken::generate();
        let pairing_id = token.pairing_id();
        receiver
            .p2p
            .node()
            .peers()
            .upsert(copypaste_p2p::Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "sender".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        let peer = copypaste_p2p::Peer {
            pairing_id: pairing_id.clone(),
            device_id: None,
            name: "receiver".into(),
            psk: token.psk(),
            last_addr: remote_addr.parse().ok(),
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        let mut events = receiver.subscribe_events();
        sender.p2p.node().probe_one(&peer).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap()
                .unwrap()
                .event,
            EventKind::Peers
        );
        let observed = receiver.p2p.node().peers().get(&pairing_id).unwrap();
        assert!(observed.profile.is_some());
        assert!(observed.profile_observed_at_ms > 0);
        assert!(observed.last_seen_ms > 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(25), events.recv())
                .await
                .is_err()
        );
        receiver.shutdown();
    }

    #[tokio::test]
    async fn listener_starts_and_shutdown_quiesces_it() {
        let (runtime, _dir) = fixture();
        runtime.start_listener().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while runtime.p2p.listen_addr().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        runtime.shutdown();
        tokio::time::timeout(Duration::from_secs(1), async {
            while runtime.listener_started.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn item_and_peer_events_reach_independent_watchers() {
        let (runtime, _dir) = fixture();
        let mut first = runtime.subscribe_events();
        let mut second = runtime.subscribe_events();
        runtime.note_remote_change();
        assert!(matches!(
            first.recv().await.unwrap().event,
            EventKind::Items
        ));
        assert!(matches!(
            second.recv().await.unwrap().event,
            EventKind::Items
        ));
        drop(first);
        runtime.note_peers_changed();
        assert!(matches!(
            second.recv().await.unwrap().event,
            EventKind::Peers
        ));
    }

    #[test]
    fn known_device_data_appears_only_after_confirmation() {
        let (state, _dir) = fixture();
        let token = copypaste_p2p::PairingToken::generate();
        let pairing_id = token.pairing_id();
        state
            .p2p
            .peers()
            .upsert(copypaste_p2p::peers::Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        let progress = |phase| {
            state.pairing_progress(
                1,
                copypaste_p2p::PairingStatus {
                    pairing_id: Some(pairing_id.clone()),
                    role: Some(copypaste_p2p::PairingRole::Initiator),
                    phase,
                    sas: None,
                    peer: None,
                    error: None,
                    expires_in_ms: None,
                },
            )
        };

        let awaiting = progress(copypaste_p2p::PairingPhase::AwaitingConfirmation);
        assert!(matches!(
            awaiting.data,
            Some(ResponseData::PairingProgress(
                copypaste_ipc::PairingProgressData {
                    known_device: None,
                    ..
                }
            ))
        ));
        let confirmed = progress(copypaste_p2p::PairingPhase::Confirmed);
        assert!(matches!(
            confirmed.data,
            Some(ResponseData::PairingProgress(copypaste_ipc::PairingProgressData {
                known_device: Some(copypaste_ipc::PeerInfo { pairing_id: id, .. }),
                ..
            })) if id == pairing_id
        ));
    }

    #[test]
    fn source_exclusions_report_capture_blocked_without_attribution() {
        use copypaste_ipc::{
            CapabilityGuarantee, CapabilityState, CapabilityStatus, PlatformCapability,
        };
        use std::collections::BTreeMap;

        let mut capabilities = BTreeMap::from([
            (
                PlatformCapability::ClipboardCapture,
                CapabilityStatus {
                    state: CapabilityState::Available,
                    backend: Some("linux-wayland-data-control".into()),
                    reason: None,
                    permissions: Vec::new(),
                    guarantee: CapabilityGuarantee::Verified,
                    mime_types: Vec::new(),
                    max_bytes: None,
                },
            ),
            (
                PlatformCapability::SourceAttribution,
                CapabilityStatus {
                    state: CapabilityState::Unsupported,
                    backend: Some("linux-wayland-data-control".into()),
                    reason: None,
                    permissions: Vec::new(),
                    guarantee: CapabilityGuarantee::None,
                    mime_types: Vec::new(),
                    max_bytes: None,
                },
            ),
        ]);
        let settings = copypaste_ipc::ConfigData {
            excluded_app_bundle_ids: vec!["org.example.Private".into()],
            ..Default::default()
        };
        apply_capture_policy_capabilities(&mut capabilities, &settings);
        let capture = &capabilities[&PlatformCapability::ClipboardCapture];
        assert_eq!(capture.state, CapabilityState::NeedsCompanion);
        assert_eq!(capture.guarantee, CapabilityGuarantee::None);
        assert_eq!(
            capture.backend.as_deref(),
            Some("linux-wayland-data-control")
        );
    }

    #[tokio::test]
    async fn library_changes_coalesce_into_one_event_and_clearing_history_reaches_the_host() {
        #[derive(Default)]
        struct ClearingClipboard(Mutex<usize>);
        impl ClipboardWriter for ClearingClipboard {
            fn write(
                &self,
                _: &ClipboardPayload,
                _: &str,
                _: WriteOptions,
            ) -> Result<(), ClipboardWriteError> {
                Ok(())
            }
            fn history_cleared(&self) {
                *self.0.lock().unwrap() += 1;
            }
        }
        let clipboard = Arc::new(ClearingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        let task = runtime.start_deadlines().unwrap();
        let mut events = runtime.subscribe_events();
        runtime.on_org_changed(LibraryKinds::SNIPPETS);
        runtime.on_org_changed(LibraryKinds::RULES);
        runtime.on_org_changed(LibraryKinds::SNIPPETS);
        tokio::time::sleep(Duration::from_millis(900)).await;
        let event = events.try_recv().unwrap();
        assert_eq!(event.event, EventKind::Library);
        assert_eq!(
            event.library,
            Some(LibraryKinds::SNIPPETS.union(LibraryKinds::RULES))
        );
        assert!(events.try_recv().is_err(), "one event for the burst");

        assert!(
            runtime
                .request_blocking(1, Method::DeleteAll { through: None })
                .ok
        );
        assert_eq!(*clipboard.0.lock().unwrap(), 1);
        runtime.shutdown();
        task.await.unwrap();
    }
}
