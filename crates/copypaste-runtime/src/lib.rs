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
#[cfg(test)]
use copypaste_p2p::peers::PeerStore;
use copypaste_p2p::Node;

pub mod capture_admission;
mod peer_sync;
mod settings;
use capture_admission::{CaptureAdmission, CaptureKind, CaptureScope};

use settings::{RuntimeSettings, SettingsError};

/// Storage and direct peer networking shared by daemon and in-process hosts.
pub struct Runtime {
    peer_sync: Arc<peer_sync::PeerSyncDriver>,
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
    module_sync_started: AtomicBool,
    clipboard: Arc<dyn ClipboardWriter>,
    instant_clipboard: Arc<copypaste_core::sync::InstantClipboard>,
}

pub trait ClipboardWriter: Send + Sync {
    fn write(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
    ) -> Result<(), ClipboardWriteError>;
}

#[derive(Clone)]
struct ItemOrigin {
    device_id: String,
    device_name: Option<String>,
    device_class: copypaste_ipc::DeviceClass,
}
struct UnavailableClipboard;
impl ClipboardWriter for UnavailableClipboard {
    fn write(&self, _: &ClipboardPayload, _: &str) -> Result<(), ClipboardWriteError> {
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
        let peers = copypaste_core::peer_store::open(
            &store,
            &keyring,
            &data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
        )
        .map_err(|_| RuntimeError::PeerStore)?;
        let discovery = Discovery::dormant(&identity.device_name, port).ok();
        let node = Arc::new(Node::new(
            peers,
            discovery,
            port,
            settings.config().lan_visibility,
        ));
        let modules = Arc::new(copypaste_modules::ModuleHost::new(data_dir));
        let module_versions = Arc::downgrade(&modules);
        let source_settings = Arc::clone(&settings);
        let instant_clipboard = Arc::new(copypaste_core::sync::InstantClipboard::default());
        let clipboard_gate = Arc::clone(&instant_clipboard);
        let clipboard_store = store.clone();
        let clipboard_keyring = Arc::clone(&keyring);
        let clipboard_settings = Arc::clone(&settings);
        let clipboard_writer = Arc::clone(&clipboard);
        let source = Arc::new(
            StoreSource::with_retention_settings(
                store.clone(),
                Arc::clone(&keyring),
                identity.device_id.clone(),
                identity.device_name.clone(),
                move || source_settings.config(),
            )
            .on_applied(move |stamp| {
                if let Some(modules) = module_versions.upgrade() {
                    modules.note_version(stamp);
                }
            })
            .on_clip_received(move |row| {
                let _ = clipboard_settings.with_clipboard_settings(|settings| {
                    clipboard_gate.apply(
                        &clipboard_store,
                        &clipboard_keyring,
                        row,
                        || settings,
                        |payload| clipboard_writer.write(payload, &row.content_type),
                    );
                });
            }),
        );
        let (events, _) = tokio::sync::broadcast::channel(64);
        let modules = Arc::new(copypaste_modules::ModuleHost::new(data_dir));
        let changed_events = events.clone();
        let changed_store = store.clone();
        modules.bind_search(
            store.clone(),
            Arc::new(move || {
                let _ = changed_events.send(EventData {
                    sync_status: None,
                    event: EventKind::Items,
                    item_count: changed_store.count().unwrap_or(0),
                    captured: false,
                    captured_item_id: None,
                });
            }),
        );
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let module_shutdown = shutdown.subscribe();
        let module_settings = Arc::clone(&settings);
        let module_events = events.clone();
        let module_store = store.clone();
        let module_node = Arc::clone(&node);
        let module_peers = peer_sync::PeerSyncDriver::new();
        let module_relay = Arc::clone(&module_peers);
        modules
            .set_sync_services(Arc::new(copypaste_modules::SyncServices::new(
                store.clone(),
                copypaste_sync::store::StoreView::new(
                    source.without_version_hook(),
                    identity.device_id.clone(),
                ),
                move || !*module_shutdown.borrow() && module_settings.config().sync_enabled,
                move |stamp| {
                    module_node.cursors().note_local(stamp);
                    module_relay.wake();
                    let _ = module_events.send(EventData {
                        sync_status: None,
                        event: EventKind::Items,
                        item_count: module_store.count().unwrap_or(0),
                        captured: false,
                        captured_item_id: None,
                    });
                },
            )))
            .map_err(|_| RuntimeError::Storage)?;
        let device_class = copypaste_p2p::DeviceProfile::current().device_class;
        Ok(Self {
            peer_sync: peer_sync::PeerSyncDriver::new(),
            modules,
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
            module_sync_started: AtomicBool::new(false),
            clipboard,
            instant_clipboard,
        })
    }

    /// Dispatches the stable desktop IPC contract in-process.
    ///
    /// The platform host supplies clipboard writing, but history and P2P never
    /// take a daemon shortcut: they use the same encrypted store, merge source
    /// and Noise node as the desktop service.
    pub async fn request(&self, id: u64, method: Method) -> Response {
        self.start_module_sync();
        let item_mutation = matches!(
            &method,
            Method::Delete { .. }
                | Method::DeleteAll { .. }
                | Method::Pin { .. }
                | Method::ReorderPinned { .. }
                | Method::Restore { .. }
                | Method::ImportFile { .. }
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
                        sync_status: self.node.sync_status(settings.config.sync_enabled),
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
                match self.modules.query_history(
                    &self.store,
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
            Method::ImportFile {
                path,
                filename,
                mime_type,
                source_reference,
            } => {
                match copypaste_core::file_import::import_file(
                    &self.store,
                    &self.keyring,
                    Path::new(&path),
                    &filename,
                    &mime_type,
                    source_reference.as_deref(),
                    &self.settings.config(),
                ) {
                    Ok(_) => {
                        self.node.note_local_version(now_ms());
                        self.peer_sync.wake();
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
                }
            }
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
            method @ (Method::CloudStatus
            | Method::CloudSignIn { .. }
            | Method::CloudSignUp { .. }
            | Method::CloudSetEndpoint { .. }
            | Method::CloudSignOut
            | Method::CloudSyncNow) => self.modules.cloud_request(id, method).await,
            _ => Response::err(
                id,
                ErrorCode::InvalidRequest,
                "This in-process operation is not wired yet.",
            ),
        };
        if response.ok && item_mutation {
            self.modules.note_version(0);
            self.emit(EventKind::Items);
        }
        if response.ok && peer_mutation {
            self.emit(EventKind::Peers);
        }
        response
    }

    /// Module workers have their own lifecycle, independent of the peer TCP port.
    pub fn start_module_sync(&self) {
        if self.module_sync_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let modules = Arc::clone(&self.modules);
        let shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            modules.run_sync(shutdown).await;
        });
    }

    pub async fn start_listener(self: &Arc<Self>) -> Result<(), RuntimeError> {
        self.start_module_sync();
        if self.listener_started.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let listener = match self.node.bind_listener() {
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
                move || probe_callback_runtime.emit(EventKind::Peers),
                shutdown,
            )
            .await;
            runtime.listener_started.store(false, Ordering::Release);
        });
        let runtime = Arc::clone(self);
        let mut pairing_changes = self.node.subscribe_pairing_changes();
        let mut sync_changes = self.node.subscribe_sync_changes();
        let mut shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return,
                    changed = pairing_changes.changed() => if changed.is_err() { return } else {
                        runtime.peer_sync.wake();
                        runtime.emit(EventKind::Peers);
                    },
                    changed = sync_changes.changed() => if changed.is_err() { return } else {
                        let _ = runtime.events.send(EventData {
                            sync_status: Some(runtime.node.sync_status(runtime.settings.config().sync_enabled)),
                            event: EventKind::Peers,
                            item_count: runtime.store.count().unwrap_or(0),
                            captured: false,
                            captured_item_id: None,
                        });
                    },
                }
            }
        });
        self.peer_sync
            .set_enabled(self.settings.config().sync_enabled);
        tokio::spawn(peer_sync::run(Arc::clone(self)));
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
        let privacy_metadata =
            (!permit.privacy.is_empty()).then_some(copypaste_core::PayloadMetadata {
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
        self.emit_capture(ingested.into_item());
        permit.committed();
        Ok(())
    }

    pub fn has_sms_module(&self) -> bool {
        self.modules.has_sms_handler().unwrap_or(false)
    }

    /// The SMS body is invocation-only. Only the recognized code enters the
    /// encrypted History and the normal sync source. Publication holds the
    /// same admission scope used by native clipboard capture.
    pub fn capture_sms_operation(&self, token: u64, text: &str) -> Result<bool, RuntimeError> {
        let mut read = Some(
            self.capture_admission()
                .acquire(token, CaptureScope::Read)
                .ok_or(RuntimeError::CaptureRefused)?,
        );
        self.modules
            .dispatch_sms(text, |code| {
                drop(read.take());
                self.publish_sms_code(token, code)
                    .map_err(|_| copypaste_modules::ModuleError::State)?;
                Ok(())
            })
            .map_err(|_| RuntimeError::Capture)
    }

    fn publish_sms_code(&self, token: u64, code: &str) -> Result<(), RuntimeError> {
        self.capture_text_operation(token, code)?;
        let _completion = self
            .capture_admission()
            .acquire(token, CaptureScope::Completion)
            .ok_or(RuntimeError::CaptureRefused)?;
        self.clipboard
            .write(
                &ClipboardPayload::Text(code.to_owned().into()),
                copypaste_ipc::content_type::TEXT,
            )
            .map_err(|_| RuntimeError::Capture)
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
        let metadata = copypaste_core::PayloadMetadata {
            privacy: permit.privacy,
            file: metadata,
            source_app_icon: None,
        };
        let metadata =
            (!metadata.privacy.is_empty() || metadata.file.is_some()).then_some(metadata);
        let ingested = copypaste_core::ingest_binary_into_with_capture_source_metadata(
            &self.store,
            &self.keyring,
            bytes,
            stored_type,
            now_ms(),
            None,
            None,
            metadata.as_ref(),
            settings,
        )
        .map_err(|_| RuntimeError::Capture)?;
        self.emit_capture(ingested.into_item());
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
        if event == EventKind::Items {
            self.modules.history_changed();
        }
        let _ = self.events.send(EventData {
            sync_status: None,
            event,
            item_count: self.store.count().unwrap_or(0),
            captured: false,
            captured_item_id: None,
        });
    }

    fn emit_capture(&self, item: copypaste_core::StoredItem) {
        self.modules.history_changed();
        self.instant_clipboard
            .note_local(&self.store, item.created_at);
        self.node.note_local_version(item.created_at);
        self.modules.note_version(item.created_at);
        self.peer_sync.wake();
        let _ = self.events.send(EventData {
            sync_status: None,
            event: EventKind::Items,
            item_count: self.store.count().unwrap_or(0),
            captured: true,
            captured_item_id: Some(item.id),
        });
    }

    async fn apply_settings(
        &self,
        patch: copypaste_ipc::ConfigPatch,
        reconcile_retention: bool,
    ) -> Result<settings::SettingsApplied, SettingsError> {
        let settings = Arc::clone(&self.settings);
        let node = Arc::clone(&self.node);
        let peer_sync = Arc::clone(&self.peer_sync);
        let modules = Arc::clone(&self.modules);
        let store = self.store.clone();
        let events = self.events.clone();
        tokio::task::spawn_blocking(move || {
            let applied = settings.apply_with_effects(&patch, |applied| {
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
                if applied.before.sync_enabled != applied.config.sync_enabled {
                    peer_sync.set_enabled(applied.config.sync_enabled);
                    let _ = events.send(EventData {
                        sync_status: Some(node.sync_status(applied.config.sync_enabled)),
                        event: EventKind::Peers,
                        item_count: store.count().unwrap_or(0),
                        captured: false,
                        captured_item_id: None,
                    });
                }
                if removed > 0 {
                    let _ = events.send(EventData {
                        sync_status: None,
                        event: EventKind::Items,
                        item_count: store.count().unwrap_or(0),
                        captured: false,
                        captured_item_id: None,
                    });
                }
            })?;
            if applied.before.sync_enabled != applied.config.sync_enabled {
                modules.sync_enabled_changed(applied.config.sync_enabled);
            }
            Ok(applied)
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
        match self.modules.search(
            &self.store,
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
        match self.store.source_app_icon_by_id(item_id) {
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
        if !self.settings.config().sync_enabled {
            return Response::err(request_id, ErrorCode::NotReady, "Sync is turned off.");
        }
        let _round = self.peer_sync.rounds.enter().await;
        if !self.settings.config().sync_enabled {
            return Response::err(request_id, ErrorCode::NotReady, "Sync is turned off.");
        }
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
            let cancel = self.peer_sync.cycle.cancel_token();
            let outcome = tokio::select! {
                _ = cancel.cancelled() => Err(copypaste_p2p::NodeError::Session),
                result = self.node.sync_one_in_cycle(peer, self.source.as_ref(), &self.peer_sync.cycle) => result,
            };
            if let Ok(outcome) = &outcome {
                self.remember_device(outcome);
                if outcome.stats.received > 0 {
                    self.emit(EventKind::Items);
                }
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
        match self.clipboard.write(
            &payload,
            if plain_text {
                copypaste_ipc::content_type::TEXT
            } else {
                &row.content_type
            },
        ) {
            Ok(()) => {
                self.instant_clipboard.note_local(&self.store, now_ms());
                self.item(request_id, item_id)
            }
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
            Ok(payload)
                if matches!(
                    &payload,
                    ClipboardPayload::Image { .. } | ClipboardPayload::File { .. }
                ) =>
            {
                payload
            }
            Ok(_) => {
                return Response::err(
                    request_id,
                    ErrorCode::UnsupportedContent,
                    "This clip is not an image or file.",
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
        let privacy = row.clipboard_privacy();
        let origin_device_id = origin.device_id.clone();
        let origin_device_name = origin.device_name.clone();
        let origin_device_class = origin.device_class;
        let payload = match copypaste_core::ClipboardPayload::open(&row, &self.keyring.item_key()) {
            Ok(payload) => payload,
            Err(_) => return None,
        };
        let (content, truncated) = if preview && privacy.secret {
            (String::new(), false)
        } else if preview {
            payload.display_preview()
        } else {
            (payload.display_text(), false)
        };
        let semantic = payload
            .plain_text()
            .and_then(|text| copypaste_core::classify_semantic(&row.content_type, text));
        let too_large_to_sync = payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES;
        let imported_image_metadata = row
            .payload_metadata
            .as_deref()
            .and_then(|value| copypaste_core::PayloadMetadata::from_json(value, &row.content_type))
            .and_then(|value| value.file);
        let file_details = match &payload {
            ClipboardPayload::File { bytes, .. } | ClipboardPayload::Image { bytes, .. }
                if imported_image_metadata.is_some()
                    || matches!(&payload, ClipboardPayload::File { .. }) =>
            {
                let metadata = match &payload {
                    ClipboardPayload::File { metadata, .. } => metadata,
                    _ => &imported_image_metadata,
                };
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
            ClipboardPayload::File { .. }
            | ClipboardPayload::Text(_)
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
            privacy,
            id: row.id,
            content,
            content_type: row.content_type.clone(),
            content_class: copypaste_ipc::content_type::classify(&row.content_type),
            semantic_kind: semantic
                .filter(|_| !privacy.secret || !preview)
                .map(|classification| classification.kind),
            color_rgba: semantic
                .filter(|_| !privacy.secret || !preview)
                .and_then(|classification| classification.color_rgba),
            created_at: row.created_at,
            pinned: row.pinned,
            file_details: if preview && privacy.secret {
                None
            } else {
                file_details
            },
            image_details,
            origin_device_id,
            origin_device_name,
            origin_device_class,
            source_app_bundle_id: row.app_bundle_id,
            source_app_name: row.app_name,
            source_app_icon_id: row.source_icon_id,
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
            PeerStore::open(&dir.path().join("peers.json"), &keyring.peer_store_key()).unwrap(),
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
                peer_sync: peer_sync::PeerSyncDriver::new(),
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
                module_sync_started: AtomicBool::new(false),
                clipboard,
                instant_clipboard: Arc::default(),
            }),
            dir,
        )
    }

    #[derive(Default)]
    struct RecordingClipboard(Mutex<Vec<String>>);
    impl ClipboardWriter for RecordingClipboard {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _content_type: &str,
        ) -> Result<(), ClipboardWriteError> {
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
    async fn explicit_file_import_publishes_each_named_record_without_capture_admission() {
        let (runtime, dir) = fixture();
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"%PDF original").unwrap();
        let mut events = runtime.events.subscribe();
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
        let item = match runtime.item(0, &id).data {
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
        assert!(!runtime
            .capture_sms_operation(token, "Your code is 007123")
            .unwrap());
        assert_eq!(runtime.store.count().unwrap(), 0);
        let transition = runtime.capture_admission().transition().unwrap();
        let mut paused = runtime.settings.config();
        paused.private_mode = true;
        transition.publish(paused).unwrap();
        drop(transition);
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
    fn the_opened_runtime_applies_received_content_through_its_platform_writer() {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(RecordingClipboard::default());
        let runtime =
            Runtime::open_with_clipboard(dir.path(), "receiver", 0, clipboard.clone()).unwrap();
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
        assert_eq!(*clipboard.0.lock().unwrap(), ["text:from another device"]);
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
        assert_eq!(clipboard.0.lock().unwrap().len(), 1);
        assert_eq!(
            runtime.store.get("incoming").unwrap().unwrap().created_at,
            200
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
    async fn local_text_and_image_capture_sync_without_a_manual_request() {
        let (receiver, _receiver_dir) = fixture();
        let (sender, _sender_dir) = fixture();
        receiver.start_listener().await.unwrap();
        sender.start_listener().await.unwrap();
        let receiver_address: std::net::SocketAddr =
            receiver.node.listen_addr().unwrap().parse().unwrap();
        let sender_address: std::net::SocketAddr =
            sender.node.listen_addr().unwrap().parse().unwrap();
        let token = copypaste_p2p::PairingToken::generate();
        for (runtime, address) in [(&receiver, sender_address), (&sender, receiver_address)] {
            runtime
                .node
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
        let running_manual_round = sender.peer_sync.rounds.enter().await;
        sender.capture_text("automatic clipboard text").unwrap();
        let png = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=").unwrap();
        sender
            .capture_binary(&png, "image/png", None, None)
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
        receiver.peer_sync.cycle.set_enabled(false);
        sender.start_listener().await.unwrap();
        sender.peer_sync.cycle.set_enabled(false);
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
        receiver.peer_sync.cycle.set_enabled(false);
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
