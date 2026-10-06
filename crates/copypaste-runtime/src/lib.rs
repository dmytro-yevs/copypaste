//! In-process composition of CopyPaste storage and peer-sync services.
//!
//! Platform hosts own native clipboard capture and lifecycle. This crate owns
//! no cryptography, merge rules, or handshake protocol; those remain in
//! `copypaste-core` and `copypaste-p2p`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_core::{
    now_ms, p2p_contract, ClipboardPayload, ClipboardWriteError, Keyring, Store, StoreSource,
};
use copypaste_ipc::{
    ErrorCode, EventData, EventKind, Item, ItemPage, Method, Response, ResponseData,
};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;
use copypaste_p2p::Node;

pub mod capture_admission;
mod settings;
use capture_admission::{CaptureAdmission, CaptureKind, CaptureScope};

use settings::{RuntimeSettings, SettingsError};

/// Storage and direct peer networking shared by daemon and in-process hosts.
pub struct Runtime {
    modules: Arc<copypaste_modules::ModuleHost>,
    pub store: Store,
    pub keyring: Arc<Keyring>,
    pub source: Arc<StoreSource>,
    pub node: Arc<Node>,
    pub device_id: String,
    pub device_name: String,
    pub device_class: copypaste_ipc::DeviceClass,
    settings: Arc<RuntimeSettings>,
    events: tokio::sync::broadcast::Sender<EventData>,
    shutdown: tokio::sync::watch::Sender<bool>,
    listener_started: AtomicBool,
    capture_running: AtomicBool,
    clipboard: Arc<dyn ClipboardWriter>,
}

pub trait ClipboardWriter: Send + Sync {
    fn write(&self, payload: &ClipboardPayload) -> Result<(), ClipboardWriteError>;
}

#[derive(Clone)]
struct ItemOrigin {
    device_id: String,
    device_name: Option<String>,
    device_class: copypaste_ipc::DeviceClass,
}
struct UnavailableClipboard;
impl ClipboardWriter for UnavailableClipboard {
    fn write(&self, _: &ClipboardPayload) -> Result<(), ClipboardWriteError> {
        Err(ClipboardWriteError::Failed)
    }
}

impl Runtime {
    /// Opens only application-owned paths. The platform host must initialize
    /// its keystore before calling this constructor.
    pub fn open(data_dir: &Path, device_name: &str, port: u16) -> Result<Self, RuntimeError> {
        Self::open_with_clipboard(data_dir, device_name, port, Arc::new(UnavailableClipboard))
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
        let store = Store::open(&data_dir.join("history.db"), &keyring.db_key())
            .map_err(|_| RuntimeError::Storage)?;
        let identity = store
            .device_identity(device_name)
            .map_err(|_| RuntimeError::Storage)?;
        let settings = Arc::new(RuntimeSettings::load(&store));
        let peers = PeerStore::open(&data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME))
            .map_err(|_| RuntimeError::PeerStore)?;
        let discovery = Discovery::dormant(&identity.device_name, port).ok();
        let node = Arc::new(Node::new(
            peers,
            discovery,
            port,
            settings.config().lan_visibility,
        ));
        let source_settings = Arc::clone(&settings);
        let source = Arc::new(StoreSource::with_retention_settings(
            store.clone(),
            Arc::clone(&keyring),
            identity.device_id.clone(),
            identity.device_name.clone(),
            move || source_settings.config(),
        ));
        let (events, _) = tokio::sync::broadcast::channel(64);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let device_class = copypaste_p2p::DeviceProfile::current().device_class;
        Ok(Self {
            modules: Arc::new(copypaste_modules::ModuleHost::new(data_dir)),
            store,
            keyring,
            source,
            node,
            device_id: identity.device_id,
            device_name: identity.device_name,
            device_class,
            settings,
            events,
            shutdown,
            listener_started: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
            clipboard,
        })
    }

    /// Dispatches the stable desktop IPC contract in-process.
    ///
    /// The platform host supplies clipboard writing, but history and P2P never
    /// take a daemon shortcut: they use the same encrypted store, merge source
    /// and Noise node as the desktop service.
    pub async fn request(&self, id: u64, method: Method) -> Response {
        let item_mutation = matches!(
            &method,
            Method::Delete { .. }
                | Method::DeleteAll { .. }
                | Method::Pin { .. }
                | Method::ReorderPinned { .. }
                | Method::Restore { .. }
        );
        let peer_mutation = matches!(
            &method,
            Method::SetDeviceName { .. }
                | Method::PairCreateInvite
                | Method::PairConfirm { .. }
                | Method::PairCancel
                | Method::PairJoin { .. }
                | Method::Unpair { .. }
                | Method::Revoke { .. }
        );
        let response = match method {
            Method::Modules { operation } => match self.modules.request(operation).await {
                Ok(json) => Response::ok(id, ResponseData::Modules { json }),
                Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
            },
            Method::Status => {
                let settings = self.settings.snapshot();
                Response::ok(
                    id,
                    ResponseData::Status(copypaste_ipc::StatusData {
                        device_details: Some(p2p_contract::local_device_details(
                            &self.device_name,
                            self.node.listen_addr().as_deref(),
                        )),
                        device_name: self.device_name.clone(),
                        device_id: Some(self.device_id.clone()),
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                        protocol_version: copypaste_ipc::PROTOCOL_VERSION,
                        listen_addr: self.node.listen_addr(),
                        item_count: self.store.count().unwrap_or(0),
                        capture_running: self.capture_running.load(Ordering::Acquire),
                        clipboard_backend: "android".to_owned(),
                        private_mode: settings.config.private_mode,
                        private_mode_epoch: settings.private_mode_epoch,
                        counters: Default::default(),
                        settings_health: settings.health,
                    }),
                )
            }
            Method::List { limit, cursor } => self.list(id, limit, cursor),
            Method::Search { query, limit } => self.search(id, &query, limit),
            Method::HistoryFacets => match self.store.history_facets(
                &self.device_id,
                &self.device_name,
                self.device_class,
            ) {
                Ok(facets) => Response::ok(id, ResponseData::HistoryFacets(facets)),
                Err(_) => Response::err(
                    id,
                    ErrorCode::Internal,
                    "The history filters are unavailable.",
                ),
            },
            Method::HistoryQuery {
                query,
                limit,
                cursor,
            } => {
                let cursor = match cursor
                    .map(|token| copypaste_core::HistoryCursor::parse_for(&token, &query))
                    .transpose()
                {
                    Ok(value) => value,
                    Err(_) => {
                        return Response::err(
                            id,
                            ErrorCode::InvalidRequest,
                            "The history cursor is invalid.",
                        );
                    }
                };
                match self.store.query_history_bounded_for_device(
                    &query,
                    cursor.as_ref(),
                    limit.clamp(1, 1000),
                    copypaste_ipc::MAX_CONTENT_BYTES,
                    Some(&self.device_id),
                ) {
                    Ok(page) => Response::ok(
                        id,
                        ResponseData::Page(
                            self.page(page.items, page.next.map(|cursor| cursor.token())),
                        ),
                    ),
                    Err(_) => Response::err(
                        id,
                        ErrorCode::InvalidRequest,
                        "The history query is invalid.",
                    ),
                }
            }
            Method::Get { id: item_id } => self.item(id, &item_id),
            Method::ImagePreview {
                id: item_id,
                max_edge,
                bounds,
            } => self.image_preview(id, &item_id, max_edge, bounds),
            Method::SourceAppIcon { id: item_id } => self.source_icon(id, &item_id),
            Method::SaveFile {
                id: item_id,
                dest_path,
            } => self.save_file(id, &item_id, &dest_path),
            Method::Delete { id: item_id } => match self.store.delete(&item_id) {
                Ok(true) => Response::ok(id, ResponseData::Empty {}),
                Ok(false) => Response::err(id, ErrorCode::NotFound, "The clip was not found."),
                Err(_) => {
                    Response::err(id, ErrorCode::Internal, "The history store is unavailable.")
                }
            },
            Method::Pin {
                id: item_id,
                pinned,
            } => match self.store.set_pinned(&item_id, pinned) {
                Ok(true) => self.item(id, &item_id),
                Ok(false) => Response::err(id, ErrorCode::NotFound, "The clip was not found."),
                Err(_) => {
                    Response::err(id, ErrorCode::Internal, "The history store is unavailable.")
                }
            },
            Method::DeleteAll { through } => match through {
                Some(through) => self.store.delete_all_through(through),
                None => self.store.delete_all(),
            }
            .map(|count| Response::ok(id, ResponseData::Count(count)))
            .unwrap_or_else(|_| {
                Response::err(id, ErrorCode::Internal, "The history store is unavailable.")
            }),
            Method::ReorderPinned { ids } => match self.store.reorder_pinned(&ids) {
                Ok(count) => Response::ok(id, ResponseData::Count(count)),
                Err(_) => Response::err(
                    id,
                    ErrorCode::InvalidRequest,
                    "The pinned order is invalid.",
                ),
            },
            Method::SetDeviceName { name } => {
                match self.store.set_device_name(&self.device_id, &name) {
                    Ok(name) => {
                        self.node.set_device_name(&name);
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(_) => {
                        Response::err(id, ErrorCode::InvalidRequest, "The device name is invalid.")
                    }
                }
            }
            Method::Peers => {
                let peers = self.node.peers().list();
                let events = self.events.clone();
                let store = self.store.clone();
                self.node
                    .refresh_reachability(peers.iter().cloned(), move || {
                        let _ = events.send(EventData {
                            event: EventKind::Peers,
                            item_count: store.count().unwrap_or(0),
                            captured: false,
                            captured_item_id: None,
                        });
                    });
                Response::ok(
                    id,
                    ResponseData::Peers(
                        peers
                            .iter()
                            .map(|peer| {
                                let found = self.node.find(&peer.pairing_id);
                                let authenticated =
                                    self.node.authenticated_profile(&peer.pairing_id);
                                let reachability =
                                    self.node.authenticated_reachability(&peer.pairing_id);
                                p2p_contract::peer_info(
                                    peer,
                                    found.as_ref(),
                                    authenticated.as_ref(),
                                    reachability.as_ref(),
                                )
                            })
                            .collect(),
                    ),
                )
            }
            Method::Discovered | Method::Rescan => {
                if matches!(method, Method::Rescan) {
                    self.node.clear_discovery_candidate_cooldowns();
                    self.node.republish();
                }
                Response::ok(
                    id,
                    ResponseData::Discovered(copypaste_ipc::DiscoveredData {
                        devices: self
                            .node
                            .seen()
                            .into_iter()
                            .map(|found| {
                                let paired = found
                                    .pairing_ids
                                    .iter()
                                    .any(|pairing_id| self.node.peers().get(pairing_id).is_some());
                                p2p_contract::discovered_device(found, paired)
                            })
                            .collect(),
                    }),
                )
            }
            Method::PairCreateInvite => match self.node.pair_create_invite() {
                Ok(invite) => Response::ok(
                    id,
                    ResponseData::PairingInvite(copypaste_ipc::PairingInviteData {
                        code: invite.code,
                        pairing_id: invite.pairing_id,
                        listen_addr: invite.listen_addr,
                        expires_in_secs: invite.expires_in_secs,
                    }),
                ),
                Err(error) => self.node_error(id, error),
            },
            Method::PairProgress => self.pair_progress(id, self.node.pair_progress()),
            Method::PairConfirm { accept } => match self.node.pair_confirm(accept) {
                Ok(status) => self.pair_progress(id, status),
                Err(error) => self.node_error(id, error),
            },
            Method::PairCancel => self.pair_progress(id, self.node.pair_cancel()),
            Method::Unpair { pairing_id } => match self.node.unpair(&pairing_id) {
                Ok(true) => Response::ok(id, ResponseData::Empty {}),
                Ok(false) => self.node_error(id, copypaste_p2p::NodeError::NoPeer),
                Err(error) => self.node_error(id, error),
            },
            Method::Revoke { pairing_id } => match self.node.revoke(&pairing_id, now_ms()) {
                Ok(_) => Response::ok(id, ResponseData::Empty {}),
                Err(_) => Response::err(
                    id,
                    ErrorCode::PeerFailed,
                    "The paired-device store is unavailable.",
                ),
            },
            Method::SyncNow { pairing_id } => self.sync_now(id, pairing_id).await,
            Method::PairJoin { code, addr } => match self
                .node
                .pair_join(&code, &addr, self.source.as_ref())
                .await
            {
                Ok(status) => self.pair_progress(id, status),
                Err(error) => self.node_error(id, error),
            },
            Method::Copy { id: item_id } => self.copy(id, &item_id, false),
            Method::CopyPlainText { id: item_id } => self.copy(id, &item_id, true),
            Method::HistoryCeiling => match self.store.max_rowid() {
                Ok(value) => Response::ok(id, ResponseData::Count(value.max(0) as u64)),
                Err(_) => {
                    Response::err(id, ErrorCode::Internal, "The history store is unavailable.")
                }
            },
            Method::GetConfig => Response::ok(
                id,
                ResponseData::Config(copypaste_ipc::ConfigApplied {
                    config: self.settings.config(),
                    restart_required: Vec::new(),
                }),
            ),
            Method::SetConfig { patch } => self.apply_config(id, patch).await,
            Method::GetPrivateMode => {
                let settings = self.settings.snapshot();
                Response::ok(
                    id,
                    ResponseData::PrivateMode(copypaste_ipc::PrivateModeData {
                        private_mode: settings.config.private_mode,
                        private_mode_epoch: settings.private_mode_epoch,
                    }),
                )
            }
            Method::SetPrivateMode { enabled } => self.set_private_mode(id, enabled).await,
            Method::Export { limit } => self.export(id, limit),
            Method::Backup { dest_path } => self.backup(id, &dest_path),
            Method::Restore { src_path, confirm } => self.restore(id, &src_path, confirm),
            Method::CloudStatus => Response::ok(
                id,
                ResponseData::CloudStatus(copypaste_ipc::CloudStatusData {
                    configured: false,
                    signed_in: false,
                    key_ready: false,
                    email: None,
                    last_sync_ms: None,
                    last_error: None,
                    poll_interval_secs: 0,
                    unreadable_uploads: 0,
                }),
            ),
            Method::CloudSignIn { .. }
            | Method::CloudSignUp { .. }
            | Method::CloudSetEndpoint { .. }
            | Method::CloudSignOut
            | Method::CloudSyncNow => Response::err(
                id,
                ErrorCode::InvalidRequest,
                "Cloud sync is not configured on this device.",
            ),
            _ => Response::err(
                id,
                ErrorCode::InvalidRequest,
                "This in-process operation is not wired yet.",
            ),
        };
        if response.ok && item_mutation {
            self.emit(EventKind::Items);
        }
        if response.ok && peer_mutation {
            self.emit(EventKind::Peers);
        }
        response
    }

    pub async fn start_listener(self: &Arc<Self>) -> Result<(), RuntimeError> {
        if self.listener_started.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let listener = match copypaste_p2p::node::bind(self.node.port()) {
            Ok(listener) => listener,
            Err(_) => {
                self.listener_started.store(false, Ordering::Release);
                return Err(RuntimeError::Listener);
            }
        };
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(_) => {
                self.listener_started.store(false, Ordering::Release);
                return Err(RuntimeError::Listener);
            }
        };
        let runtime = Arc::clone(self);
        let callback_runtime = Arc::clone(self);
        let probe_callback_runtime = Arc::clone(self);
        let shutdown = self.shutdown.subscribe();
        let node = Arc::clone(&self.node);
        let source = Arc::clone(&self.source);
        tokio::spawn(async move {
            copypaste_p2p::node::listen(
                node,
                listener,
                source,
                move |_, outcome| {
                    callback_runtime.remember_device(outcome);
                    if outcome.stats.received > 0 {
                        callback_runtime.emit(EventKind::Items);
                    }
                    callback_runtime.emit(EventKind::Peers);
                },
                move |_| probe_callback_runtime.emit(EventKind::Peers),
                shutdown,
            )
            .await;
            runtime.listener_started.store(false, Ordering::Release);
        });
        let runtime = Arc::clone(self);
        let mut pairing_changes = self.node.subscribe_pairing_changes();
        let mut shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! { _ = shutdown.changed() => return, changed = pairing_changes.changed() => if changed.is_err() { return } else { runtime.emit(EventKind::Peers); } }
            }
        });
        Ok(())
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<EventData> {
        self.events.subscribe()
    }

    /// Records text captured by an in-process platform host.
    ///
    /// Android owns the permission and clipboard-read mechanics. The shared
    /// runtime remains the only owner of encryption, deduplication, retention,
    /// exclusions and History change events.
    pub fn capture_text(&self, content: &str) -> Result<(), RuntimeError> {
        self.capture_text_with_policy(content, false)
    }

    /// Records text from an explicit Android Share or Process Text action.
    pub fn capture_explicit_text(&self, content: &str) -> Result<(), RuntimeError> {
        self.capture_text_with_policy(content, true)
    }

    fn capture_text_with_policy(&self, content: &str, explicit: bool) -> Result<(), RuntimeError> {
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
                let result = self.capture_text_operation(token, content);
                self.capture_admission().abandon(token);
                result
            });
        let _ = self.capture_admission().revoke_host(host);
        result
    }

    pub fn capture_text_operation(&self, token: u64, content: &str) -> Result<(), RuntimeError> {
        let mut permit = self
            .capture_admission()
            .acquire(token, CaptureScope::Commit)
            .ok_or(RuntimeError::CaptureRefused)?;
        let settings = &permit.config;
        let ingested = copypaste_core::ingest_into_with_capture_source_with_current_retention(
            &self.store,
            &self.keyring,
            content,
            copypaste_ipc::content_type::TEXT,
            now_ms(),
            None,
            None,
            &settings,
            || self.settings.config(),
        )
        .map_err(|_| RuntimeError::Capture)?;
        self.emit_capture(ingested.into_item().id);
        permit.committed();
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
    ) -> Result<(), RuntimeError> {
        let mut permit = self
            .capture_admission()
            .acquire(token, CaptureScope::Commit)
            .ok_or(RuntimeError::CaptureRefused)?;
        let settings = &permit.config;
        let (stored_type, metadata) = if content_type.starts_with("image/") {
            (content_type, None)
        } else {
            let filename = filename.ok_or(RuntimeError::Capture)?;
            let source_reference = source_reference.ok_or(RuntimeError::Capture)?;
            let metadata = copypaste_core::FileMetadata::with_source_reference(
                filename,
                content_type,
                source_reference,
            )
            .ok_or(RuntimeError::Capture)?;
            (copypaste_ipc::content_type::FILE, Some(metadata))
        };
        let ingested = copypaste_core::ingest_binary_into_with_capture_source(
            &self.store,
            &self.keyring,
            bytes,
            stored_type,
            now_ms(),
            None,
            None,
            metadata.as_ref(),
            &settings,
        )
        .map_err(|_| RuntimeError::Capture)?;
        self.emit_capture(ingested.into_item().id);
        permit.committed();
        Ok(())
    }

    pub fn set_capture_running(&self, running: bool) {
        self.capture_running.store(running, Ordering::Release);
    }

    /// Returns a policy snapshot for status presentation, never read authority.
    #[must_use]
    pub fn implicit_capture_allowed(&self) -> bool {
        let settings = self.settings.config();
        !settings.private_mode && settings.excluded_app_bundle_ids.is_empty()
    }

    pub fn capture_admission(&self) -> &CaptureAdmission {
        &self.settings.capture
    }

    #[must_use]
    pub fn notify_on_copy_enabled(&self) -> bool {
        self.settings.config().notify_on_copy
    }

    #[must_use]
    pub fn notification_preview_enabled(&self) -> bool {
        self.settings.config().notification_preview
    }

    #[must_use]
    pub fn sound_on_copy_enabled(&self) -> bool {
        self.settings.config().sound_on_copy
    }

    /// Permanently closes capture. Call from a worker when scopes may be active.
    pub fn shutdown(&self) {
        let _ = self.capture_admission().shutdown();
        let _ = self.shutdown.send(true);
    }

    fn emit(&self, event: EventKind) {
        let _ = self.events.send(EventData {
            event,
            item_count: self.store.count().unwrap_or(0),
            captured: false,
            captured_item_id: None,
        });
    }

    fn emit_capture(&self, item_id: String) {
        let _ = self.events.send(EventData {
            event: EventKind::Items,
            item_count: self.store.count().unwrap_or(0),
            captured: true,
            captured_item_id: Some(item_id),
        });
    }

    async fn apply_settings(
        &self,
        patch: copypaste_ipc::ConfigPatch,
        reconcile_retention: bool,
    ) -> Result<settings::SettingsApplied, SettingsError> {
        let settings = Arc::clone(&self.settings);
        let node = Arc::clone(&self.node);
        let store = self.store.clone();
        let events = self.events.clone();
        tokio::task::spawn_blocking(move || {
            settings.apply_with_effects(&patch, |applied| {
                let removed = if reconcile_retention {
                    let enforce = copypaste_core::retention::policy_tightened(
                        &applied.before,
                        &applied.config,
                    );
                    copypaste_core::retention::reconcile_policy(
                        &store,
                        || settings.config(),
                        enforce,
                    )
                } else {
                    0
                };
                if applied.before.lan_visibility != applied.config.lan_visibility {
                    node.set_lan_visibility(applied.config.lan_visibility);
                }
                if removed > 0 {
                    let _ = events.send(EventData {
                        event: EventKind::Items,
                        item_count: store.count().unwrap_or(0),
                        captured: false,
                        captured_item_id: None,
                    });
                }
            })
        })
        .await
        .map_err(|_| SettingsError::Store)?
    }

    async fn apply_config(&self, request_id: u64, patch: copypaste_ipc::ConfigPatch) -> Response {
        match self.apply_settings(patch, true).await {
            Ok(applied) => Response::ok(
                request_id,
                ResponseData::Config(copypaste_ipc::ConfigApplied {
                    config: applied.config,
                    restart_required: Vec::new(),
                }),
            ),
            Err(SettingsError::Invalid(error)) => {
                Response::err(request_id, ErrorCode::InvalidRequest, error.to_string())
            }
            Err(SettingsError::Store) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The settings could not be saved.",
            ),
        }
    }

    async fn set_private_mode(&self, request_id: u64, enabled: bool) -> Response {
        match self
            .apply_settings(
                copypaste_ipc::ConfigPatch {
                    private_mode: Some(enabled),
                    ..Default::default()
                },
                false,
            )
            .await
        {
            Ok(applied) => Response::ok(
                request_id,
                ResponseData::PrivateMode(copypaste_ipc::PrivateModeData {
                    private_mode: applied.config.private_mode,
                    private_mode_epoch: applied.private_mode_epoch,
                }),
            ),
            Err(SettingsError::Invalid(error)) => {
                Response::err(request_id, ErrorCode::InvalidRequest, error.to_string())
            }
            Err(SettingsError::Store) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The settings could not be saved.",
            ),
        }
    }

    fn export(&self, request_id: u64, limit: u32) -> Response {
        match copypaste_core::transfer::export(&self.store, &self.keyring, limit) {
            Ok(export) => Response::ok(request_id, ResponseData::Export(export)),
            Err(copypaste_core::transfer::ExportError::ContentTooLarge) => Response::err(
                request_id,
                ErrorCode::ContentTooLarge,
                "The text history is too large to export in one file.",
            ),
            Err(copypaste_core::transfer::ExportError::Store(_)) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The history store is unavailable.",
            ),
        }
    }

    fn backup(&self, request_id: u64, raw_path: &str) -> Response {
        let path = std::path::Path::new(raw_path.trim());
        if raw_path.trim().is_empty()
            || path.exists()
            || !path.parent().is_some_and(std::path::Path::is_dir)
        {
            return Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "Choose a new backup file in an existing folder.",
            );
        }
        if self.store.backup_to(path).is_err() {
            let _ = std::fs::remove_file(path);
            return Response::err(
                request_id,
                ErrorCode::Internal,
                "The encrypted backup could not be created.",
            );
        }
        restrict_backup(path);
        let size_bytes = std::fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        Response::ok(
            request_id,
            ResponseData::Backup(copypaste_ipc::BackupData { size_bytes }),
        )
    }

    fn restore(&self, request_id: u64, raw_path: &str, confirm: bool) -> Response {
        if !confirm {
            return Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "Restoring history requires confirmation.",
            );
        }
        let path = std::path::Path::new(raw_path.trim());
        if raw_path.trim().is_empty() || !path.is_file() {
            return Response::err(
                request_id,
                ErrorCode::NotFound,
                "The backup file was not found.",
            );
        }
        match self.store.restore_from(path, &self.keyring.db_key()) {
            Ok(()) => {
                if let Ok(Some(oldest)) = self.store.oldest_version_ms() {
                    self.node.cursors().note_local(oldest);
                }
                Response::ok(request_id, ResponseData::Empty {})
            }
            Err(copypaste_core::RestoreError::InvalidBackup(_)) => Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "That file is not a valid backup for this device.",
            ),
            Err(copypaste_core::RestoreError::Failed(_)) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The history could not be restored; the current history is unchanged.",
            ),
        }
    }

    fn list(&self, id: u64, limit: u32, cursor: Option<String>) -> Response {
        let cursor = match cursor
            .map(|value| copypaste_core::ItemCursor::parse(&value))
            .transpose()
        {
            Ok(value) => value,
            Err(_) => {
                return Response::err(
                    id,
                    ErrorCode::InvalidRequest,
                    "The history cursor is invalid.",
                );
            }
        };
        match self.store.list_from_bounded(
            cursor.as_ref(),
            limit.clamp(1, 1000),
            copypaste_ipc::MAX_CONTENT_BYTES,
        ) {
            Ok(page) => Response::ok(
                id,
                ResponseData::Page(self.page(page.items, page.next.map(|value| value.token()))),
            ),
            Err(_) => Response::err(id, ErrorCode::Internal, "The history store is unavailable."),
        }
    }

    fn search(&self, id: u64, query: &str, limit: u32) -> Response {
        match self.store.search_bounded(
            query,
            limit.clamp(1, 1000),
            copypaste_ipc::MAX_CONTENT_BYTES,
        ) {
            Ok(rows) => Response::ok(id, ResponseData::Page(self.page(rows, None))),
            Err(_) => Response::err(
                id,
                ErrorCode::InvalidRequest,
                "The history query is invalid.",
            ),
        }
    }

    fn page(&self, rows: Vec<copypaste_core::StoredItem>, next_cursor: Option<String>) -> ItemPage {
        let origins = self.origins_for(&rows);
        let mut page = ItemPage {
            items: Vec::with_capacity(rows.len()),
            skipped_undecryptable: 0,
            next_cursor,
        };
        for row in rows {
            let origin = origins
                .get(&row.id)
                .expect("every row has an origin resolved before decryption");
            match self.item_value_with_origin(row, true, origin) {
                Some(item) => page.items.push(item),
                None => page.skipped_undecryptable += 1,
            }
        }
        page
    }

    fn image_preview(
        &self,
        request_id: u64,
        item_id: &str,
        max_edge: Option<u32>,
        bounds: Option<copypaste_ipc::ImagePreviewBounds>,
    ) -> Response {
        let Ok(Some(row)) = self.store.get(item_id) else {
            return Response::err(request_id, ErrorCode::NotFound, "The clip was not found.");
        };
        if !matches!(
            copypaste_ipc::content_type::classify(&row.content_type),
            copypaste_ipc::ContentClass::Image
        ) {
            return Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "This clip is not an image.",
            );
        }
        let Ok(payload) = ClipboardPayload::open(&row, &self.keyring.item_key()) else {
            return Response::err(
                request_id,
                ErrorCode::Internal,
                "The clip could not be decrypted.",
            );
        };
        let ClipboardPayload::Image { bytes, .. } = payload else {
            unreachable!("content class was checked")
        };
        match copypaste_core::thumbnail_png(
            &bytes,
            copypaste_ipc::ConfigData::default().max_decoded_image_mb,
            max_edge,
            bounds.map(|bounds| (bounds.width, bounds.height)),
        ) {
            Ok(image) => Response::ok(
                request_id,
                ResponseData::ImagePreview(copypaste_ipc::ImagePreview {
                    png_base64: STANDARD.encode(image.png),
                    width: image.width,
                    height: image.height,
                }),
            ),
            Err(_) => Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "The image preview is unavailable.",
            ),
        }
    }

    fn source_icon(&self, request_id: u64, item_id: &str) -> Response {
        match self.store.source_app_icon_metadata(item_id) {
            Ok(Some(icon)) => Response::ok(
                request_id,
                ResponseData::SourceAppIcon(copypaste_ipc::ImagePreview {
                    png_base64: icon.png_base64,
                    width: icon.width,
                    height: icon.height,
                }),
            ),
            Ok(None) => Response::ok(request_id, ResponseData::Empty {}),
            Err(_) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The history store is unavailable.",
            ),
        }
    }

    fn pair_progress(&self, request_id: u64, status: copypaste_p2p::PairingStatus) -> Response {
        Response::ok(
            request_id,
            ResponseData::PairingProgress(p2p_contract::pairing_progress(status, None)),
        )
    }

    fn node_error(&self, request_id: u64, error: copypaste_p2p::NodeError) -> Response {
        Response::err(
            request_id,
            p2p_contract::node_error_code(&error),
            error.to_string(),
        )
    }

    async fn sync_now(&self, request_id: u64, pairing_id: Option<String>) -> Response {
        let peers = match pairing_id {
            Some(id) => match self.node.peers().get(&id) {
                Some(peer) => vec![peer],
                None => return self.node_error(request_id, copypaste_p2p::NodeError::NoPeer),
            },
            None => self.node.peers().list(),
        };
        let mut results = Vec::with_capacity(peers.len());
        for peer in &peers {
            let started = std::time::Instant::now();
            let outcome = self.node.sync_one(peer, self.source.as_ref()).await;
            if let Ok(outcome) = &outcome {
                self.remember_device(outcome);
            }
            results.push(p2p_contract::sync_result(peer, outcome, started.elapsed()));
        }
        Response::ok(request_id, ResponseData::Sync(results))
    }

    /// Persist authenticated peer metadata separately from the item merge.
    ///
    /// The item origin remains immutable for merge ordering. Device labels and
    /// form factors are cosmetic metadata, so a successful session may update
    /// them without changing any version that arrived through it.
    fn remember_device(&self, outcome: &copypaste_p2p::sync::SyncOutcome) {
        if let Err(error) = self
            .store
            .record_device_name(&outcome.peer_device_id, &outcome.peer_device_name)
        {
            tracing::warn!(?error, "could not record a peer device name");
        }
        if let Some(profile) = &outcome.peer_profile {
            if let Err(error) = self
                .store
                .record_device_class(&outcome.peer_device_id, profile.device_class)
            {
                tracing::warn!(?error, "could not record a peer device class");
            }
        }
    }

    fn item(&self, request_id: u64, item_id: &str) -> Response {
        let row = match self.store.get(item_id) {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Response::err(request_id, ErrorCode::NotFound, "The clip was not found.");
            }
            Err(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::Internal,
                    "The history store is unavailable.",
                );
            }
        };
        let Some(item) = self.item_value(row, false) else {
            return Response::err(
                request_id,
                ErrorCode::Internal,
                "The clip could not be decrypted.",
            );
        };
        Response::ok(request_id, ResponseData::Item(item))
    }

    fn copy(&self, request_id: u64, item_id: &str, plain_text: bool) -> Response {
        let row = match self.store.get(item_id) {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Response::err(request_id, ErrorCode::NotFound, "The clip was not found.");
            }
            Err(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::Internal,
                    "The history store is unavailable.",
                );
            }
        };
        let payload = match ClipboardPayload::open(&row, &self.keyring.item_key()) {
            Ok(value) => value,
            Err(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::Internal,
                    "The clip could not be decrypted.",
                );
            }
        };
        if plain_text && payload.plain_text().is_none() {
            return Response::err(
                request_id,
                ErrorCode::UnsupportedContent,
                "This clip cannot be pasted as plain text.",
            );
        }
        match self.clipboard.write(&payload) {
            Ok(()) => self.item(request_id, item_id),
            Err(ClipboardWriteError::UnsupportedContent) => Response::err(
                request_id,
                ErrorCode::UnsupportedContent,
                "This clipboard cannot write that content type.",
            ),
            Err(ClipboardWriteError::Failed) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The system clipboard could not be written.",
            ),
        }
    }

    fn save_file(&self, request_id: u64, item_id: &str, raw_destination: &str) -> Response {
        let destination = raw_destination.trim();
        if destination.is_empty() {
            return Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "A destination is required.",
            );
        }
        let path = std::path::Path::new(destination);
        if path.exists() {
            return Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "A file already exists at that destination.",
            );
        }
        if path.parent().is_none_or(|parent| !parent.is_dir()) {
            return Response::err(
                request_id,
                ErrorCode::NotFound,
                "The destination folder was not found.",
            );
        }
        let row = match self.store.get(item_id) {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Response::err(request_id, ErrorCode::NotFound, "The clip was not found.");
            }
            Err(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::Internal,
                    "The history store is unavailable.",
                );
            }
        };
        let payload = match ClipboardPayload::open(&row, &self.keyring.item_key()) {
            Ok(payload) if matches!(&payload, ClipboardPayload::File { .. }) => payload,
            Ok(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::UnsupportedContent,
                    "This clip is not a file.",
                );
            }
            Err(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::Internal,
                    "The clip could not be decrypted.",
                );
            }
        };
        match payload.save_file_to(path) {
            Ok(()) => Response::ok(request_id, ResponseData::Empty {}),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Response::err(
                request_id,
                ErrorCode::InvalidRequest,
                "A file already exists at that destination.",
            ),
            Err(_) => Response::err(
                request_id,
                ErrorCode::Internal,
                "The file could not be saved.",
            ),
        }
    }

    fn item_value(&self, row: copypaste_core::StoredItem, preview: bool) -> Option<Item> {
        let origins = self.origins_for(std::slice::from_ref(&row));
        let origin = origins
            .get(&row.id)
            .expect("the single row has an origin resolved before decryption");
        self.item_value_with_origin(row, preview, origin)
    }

    /// Resolve one immutable origin per row with two bounded metadata reads.
    ///
    /// A history page contains up to 1,000 rows. Looking up every name and
    /// class while serialising each item would turn a list into an N+1 query.
    fn origins_for(&self, rows: &[copypaste_core::StoredItem]) -> HashMap<String, ItemOrigin> {
        let device_ids = rows
            .iter()
            .map(|row| {
                if row.origin_device_id.is_empty() {
                    self.device_id.clone()
                } else {
                    row.origin_device_id.clone()
                }
            })
            .collect::<Vec<_>>();
        let remote_device_ids = device_ids
            .iter()
            .filter(|device_id| device_id.as_str() != self.device_id.as_str())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let names = if remote_device_ids.is_empty() {
            HashMap::new()
        } else {
            match self.store.device_names(&remote_device_ids) {
                Ok(names) => names,
                Err(error) => {
                    tracing::warn!(?error, "could not resolve origin device names");
                    HashMap::new()
                }
            }
        };
        let classes = if remote_device_ids.is_empty() {
            HashMap::new()
        } else {
            match self.store.device_classes(&remote_device_ids) {
                Ok(classes) => classes,
                Err(error) => {
                    tracing::warn!(?error, "could not resolve origin device classes");
                    HashMap::new()
                }
            }
        };

        rows.iter()
            .zip(device_ids)
            .map(|(row, device_id)| {
                let origin = if device_id == self.device_id {
                    ItemOrigin {
                        device_id,
                        device_name: Some(self.device_name.clone()),
                        device_class: self.device_class,
                    }
                } else {
                    ItemOrigin {
                        device_name: names.get(&device_id).cloned(),
                        device_class: classes
                            .get(&device_id)
                            .copied()
                            .unwrap_or(copypaste_ipc::DeviceClass::Unknown),
                        device_id,
                    }
                };
                (row.id.clone(), origin)
            })
            .collect()
    }

    fn item_value_with_origin(
        &self,
        row: copypaste_core::StoredItem,
        preview: bool,
        origin: &ItemOrigin,
    ) -> Option<Item> {
        let origin_device_id = origin.device_id.clone();
        let origin_device_name = origin.device_name.clone();
        let origin_device_class = origin.device_class;
        let payload = match copypaste_core::ClipboardPayload::open(&row, &self.keyring.item_key()) {
            Ok(payload) => payload,
            Err(_) => return None,
        };
        let (content, truncated) = if preview {
            payload.display_preview()
        } else {
            (payload.display_text(), false)
        };
        let semantic = payload
            .plain_text()
            .and_then(|text| copypaste_core::classify_semantic(&row.content_type, text));
        let too_large_to_sync = payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES;
        let file_details = match &payload {
            ClipboardPayload::File { bytes, metadata } => {
                let source_reference = metadata
                    .as_ref()
                    .and_then(|metadata| metadata.source_reference.clone());
                Some(copypaste_ipc::FileDetails {
                    filename: metadata.as_ref().map(|metadata| metadata.filename.clone()),
                    mime_type: metadata.as_ref().map(|metadata| metadata.mime_type.clone()),
                    source_available: origin_device_id == self.device_id
                        && source_reference.as_deref().is_some_and(|reference| {
                            !reference.starts_with("content://")
                                && std::path::Path::new(reference).is_file()
                        }),
                    source_reference,
                    size_bytes: bytes.len() as u64,
                    file_count: 1,
                })
            }
            ClipboardPayload::Text(_)
            | ClipboardPayload::Image { .. }
            | ClipboardPayload::Unsupported { .. } => None,
        };
        let image_details = if preview {
            None
        } else {
            match &payload {
                ClipboardPayload::Image { bytes, .. } => copypaste_core::image_metadata(bytes)
                    .ok()
                    .map(|metadata| copypaste_ipc::ImageDetails {
                        width: metadata.width,
                        height: metadata.height,
                        size_bytes: metadata.size_bytes,
                    }),
                ClipboardPayload::Text(_)
                | ClipboardPayload::File { .. }
                | ClipboardPayload::Unsupported { .. } => None,
            }
        };
        Some(Item {
            id: row.id,
            content,
            content_type: row.content_type.clone(),
            content_class: copypaste_ipc::content_type::classify(&row.content_type),
            semantic_kind: semantic.map(|classification| classification.kind),
            color_rgba: semantic.and_then(|classification| classification.color_rgba),
            created_at: row.created_at,
            pinned: row.pinned,
            file_details,
            image_details,
            origin_device_id,
            origin_device_name,
            origin_device_class,
            source_app_bundle_id: row.app_bundle_id,
            source_app_name: row.app_name,
            too_large_to_sync,
            truncated,
        })
    }
}

#[cfg(unix)]
fn restrict_backup(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_backup(_path: &std::path::Path) {}

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
    use std::sync::Mutex;
    use std::time::Duration;

    fn fixture() -> (Arc<Runtime>, tempfile::TempDir) {
        fixture_with(Arc::new(UnavailableClipboard))
    }

    fn fixture_with(clipboard: Arc<dyn ClipboardWriter>) -> (Arc<Runtime>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let keyring = Arc::new(Keyring::from_secret(&[7; 32]));
        let store = Store::open(&dir.path().join("history.db"), &keyring.db_key()).unwrap();
        let identity = store.device_identity("fixture phone").unwrap();
        let settings = Arc::new(RuntimeSettings::load(&store));
        let node = Arc::new(Node::new(
            PeerStore::open(&dir.path().join("peers.json")).unwrap(),
            None,
            0,
            true,
        ));
        let source_settings = Arc::clone(&settings);
        let source = Arc::new(StoreSource::with_retention_settings(
            store.clone(),
            Arc::clone(&keyring),
            identity.device_id.clone(),
            identity.device_name.clone(),
            move || source_settings.config(),
        ));
        let (events, _) = tokio::sync::broadcast::channel(8);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        (
            Arc::new(Runtime {
                modules: Arc::new(copypaste_modules::ModuleHost::new(dir.path())),
                store,
                keyring,
                source,
                node,
                device_id: identity.device_id,
                device_name: identity.device_name,
                device_class: copypaste_p2p::DeviceProfile::current().device_class,
                settings,
                events,
                shutdown,
                listener_started: AtomicBool::new(false),
                capture_running: AtomicBool::new(false),
                clipboard,
            }),
            dir,
        )
    }

    #[derive(Default)]
    struct RecordingClipboard(Mutex<Vec<String>>);
    impl ClipboardWriter for RecordingClipboard {
        fn write(&self, payload: &ClipboardPayload) -> Result<(), ClipboardWriteError> {
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
    async fn status_exposes_the_stable_local_device_id() {
        let (runtime, _dir) = fixture();
        let response = runtime.request(1, Method::Status).await;

        match response.data {
            Some(ResponseData::Status(status)) => {
                assert_eq!(
                    status.device_id.as_deref(),
                    Some(runtime.device_id.as_str())
                );
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
        assert_eq!(facets.origin_devices[0].device_class, runtime.device_class);
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
                .find(|facet| facet.id == runtime.device_id)
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
        assert_eq!(page.items[0].origin_device_id, runtime.device_id);
    }

    #[test]
    fn an_all_local_page_uses_local_origin_metadata() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "local-item-a", "local text a");
        seed_text(&runtime, "local-item-b", "local text b");

        let page = runtime.page(runtime.store.list(10, 0).unwrap(), None);
        assert_eq!(page.items.len(), 2);
        assert!(page.items.iter().all(|item| {
            item.origin_device_id == runtime.device_id
                && item.origin_device_name.as_deref() == Some(runtime.device_name.as_str())
                && item.origin_device_class == runtime.device_class
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

        let item = match runtime.item(1, "remote-item").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.origin_device_id, "remote-device");
        assert_eq!(item.origin_device_name.as_deref(), Some("Remote phone"));
        assert_eq!(item.origin_device_class, copypaste_ipc::DeviceClass::Phone);
        assert_ne!(
            item.origin_device_name.as_deref(),
            Some(runtime.device_name.as_str())
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
            Some(runtime.device_name.as_str())
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
    fn copy_and_plain_copy_use_the_injected_typed_port() {
        let clipboard = Arc::new(RecordingClipboard::default());
        let (runtime, _dir) = fixture_with(clipboard.clone());
        seed_text(&runtime, "text-item", "trusted text");
        assert!(runtime.copy(1, "text-item", false).ok);
        assert!(runtime.copy(2, "text-item", true).ok);
        assert_eq!(
            *clipboard.0.lock().unwrap(),
            vec!["text:trusted text", "text:trusted text"]
        );
    }

    #[test]
    fn item_response_carries_semantic_kind_and_color_swatch() {
        let (runtime, _dir) = fixture();
        seed_text(&runtime, "color-item", "oklch(50% 0.1 30)");

        let item = match runtime.item(1, "color-item").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(item.semantic_kind, Some(copypaste_ipc::SemanticKind::Color));
        assert!(item.color_rgba.is_some());
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
        assert_eq!(detail.origin_device_class, runtime.device_class);
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
        let item = match runtime.item(1, &row.id).data {
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
        assert!(
            runtime
                .save_file(3, &row.id, &destination.to_string_lossy())
                .ok
        );
        assert_eq!(std::fs::read(destination).unwrap(), b"file bytes");
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
                            )
                        } else {
                            runtime.capture_text_operation(token, "barrier text")
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
                            )
                        } else {
                            worker_runtime.capture_text_operation(token, "barrier text")
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
            .capture_binary_operation(failed, b"file", "application/pdf", None, None)
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
    async fn inbound_loopback_merge_emits_items_and_peers() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        seed_text(&sender, "remote-item", "from peer");
        receiver.start_listener().await.unwrap();
        sender.start_listener().await.unwrap();
        let remote_addr = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(address) = receiver.node.listen_addr() {
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
                .node
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
        let peer = sender.node.peers().get(&pairing_id).unwrap();
        sender
            .node
            .sync_one(&peer, sender.source.as_ref())
            .await
            .unwrap();
        let first = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            (first.event, second.event),
            (EventKind::Items, EventKind::Peers) | (EventKind::Peers, EventKind::Items)
        ));
        assert!(receiver.store.get("remote-item").unwrap().is_some());
        receiver.shutdown();
        sender.shutdown();
    }

    #[tokio::test]
    async fn inbound_probe_emits_one_peers_event() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        receiver.start_listener().await.unwrap();
        let remote_addr = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(address) = receiver.node.listen_addr() {
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
            .node
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
            pairing_id,
            device_id: None,
            name: "receiver".into(),
            psk: token.psk(),
            last_addr: remote_addr.parse().ok(),
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        let mut events = receiver.subscribe_events();
        sender.node.probe_one(&peer).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap()
                .unwrap()
                .event,
            EventKind::Peers
        );
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
            while runtime.node.listen_addr().is_none() {
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
        runtime.emit(EventKind::Items);
        assert!(matches!(
            first.recv().await.unwrap().event,
            EventKind::Items
        ));
        assert!(matches!(
            second.recv().await.unwrap().event,
            EventKind::Items
        ));
        drop(first);
        runtime.emit(EventKind::Peers);
        assert!(matches!(
            second.recv().await.unwrap().event,
            EventKind::Peers
        ));
    }
}
