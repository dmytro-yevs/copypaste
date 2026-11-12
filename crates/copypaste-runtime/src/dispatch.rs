//! The one dispatch of the private IPC contract, for every platform.
//!
//! [`Runtime::request`] is the in-process entry point. A host that keeps its
//! own thread pools asks [`Runtime::route`] where a method runs: reactor
//! methods go to [`Runtime::request`], every other method to
//! [`Runtime::request_blocking`] on a blocking worker. Both answer from the
//! same handlers; no method has a second implementation.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_core::{
    now_ms, p2p_contract, ClipboardPayload, HistoryCursor, IngestError, ItemCursor, StoreError,
    StoredItem,
};
use copypaste_ipc::{
    clamp_page, ConfigApplied, ConfigData, ConfigPatch, ErrorCode, ExportItem, HistoryQuery, Item,
    ItemPage, Method, PrivateModeData, Response, ResponseData, SyncResult, DEFAULT_LIST_PAGE,
    DEFAULT_SEARCH_PAGE, MAX_PAGE_CONTENT_BYTES,
};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::{NodeError, PairingPhase, PairingStatus};

use crate::messages::*;
use crate::settings::{SettingsError, SettingsTransition};
use crate::Runtime;

/// Ceiling on one `reorder_pinned` request. The frame cap bounds the bytes;
/// this bounds the work one write transaction may hold. It is above any
/// plausible pinned section, because the history cap itself is 10 000 items.
pub const MAX_REORDER_IDS: usize = 10_000;

/// Where a host runs one request.
pub enum Route {
    /// Await [`Runtime::request`]: the method awaits network I/O or a shared
    /// admission.
    Reactor,
    /// Call [`Runtime::request_blocking`] on a blocking worker, holding the
    /// admission until it returns.
    Blocking(BlockingAdmission),
}

/// The shared admission one blocking request holds while it runs.
pub struct BlockingAdmission {
    _preview: Option<tokio::sync::SemaphorePermit<'static>>,
}

impl Runtime {
    /// Decide where `method` runs and take its blocking admission.
    pub async fn route(&self, method: &Method) -> Route {
        let offer = matches!(method, Method::SaveFile { id, .. } if self.is_file_offer(id));
        if offer || Self::runs_on_reactor(method) {
            return Route::Reactor;
        }
        let preview = match method {
            Method::ImagePreview { .. } => {
                Some(copypaste_core::image_preview::acquire_preview_source().await)
            }
            _ => None,
        };
        Route::Blocking(BlockingAdmission { _preview: preview })
    }

    fn runs_on_reactor(method: &Method) -> bool {
        matches!(
            method,
            Method::Modules { .. }
                | Method::StartFileDownload { .. }
                | Method::CancelFileDownload { .. }
                | Method::PairCreateInvite
                | Method::PairNearby { .. }
                | Method::PairJoin { .. }
                | Method::PairProgress
                | Method::PairConfirm { .. }
                | Method::PairCancel
                | Method::Unpair { .. }
                | Method::Revoke { .. }
                | Method::Peers
                | Method::SyncNow { .. }
                | Method::Discovered
                | Method::Rescan
                | Method::CloudSignIn { .. }
                | Method::CloudSignUp { .. }
                | Method::CloudSetEndpoint { .. }
                | Method::CloudSignOut
                | Method::CloudStatus
                | Method::CloudSyncNow
        )
    }

    /// Dispatch one request in-process.
    pub async fn request(self: &Arc<Self>, id: u64, method: Method) -> Response {
        self.start_module_sync();
        let admission = match self.route(&method).await {
            Route::Reactor => return self.request_async(id, method).await,
            Route::Blocking(admission) => admission,
        };
        if matches!(
            method,
            Method::SetConfig { .. } | Method::SetPrivateMode { .. }
        ) {
            // A settings change waits for admitted native writes; keep that
            // wait off the reactor.
            let runtime = Arc::clone(self);
            return tokio::task::spawn_blocking(move || runtime.request_blocking(id, method))
                .await
                .unwrap_or_else(|_| {
                    Response::err(id, ErrorCode::Internal, SettingsError::Store.to_string())
                });
        }
        let response = self.request_blocking(id, method);
        drop(admission);
        response
    }

    async fn request_async(&self, id: u64, method: Method) -> Response {
        match method {
            Method::SaveFile {
                id: item_id,
                dest_path,
                download_token,
            } => {
                self.save_offer(id, &item_id, &dest_path, download_token)
                    .await
            }
            Method::Modules { operation } => match self.modules.request(operation).await {
                Ok(json) => Response::ok(id, ResponseData::Modules { json }),
                Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
            },
            Method::StartFileDownload {
                id: item_id,
                dest_path,
                token,
            } => self.start_file_download(id, item_id, dest_path, token),
            Method::CancelFileDownload { id: item_id } => {
                self.file_downloads.cancel(&item_id);
                Response::ok(id, ResponseData::Empty {})
            }
            Method::PairCreateInvite => self.pair_create_invite(id),
            Method::PairNearby { addr } => {
                if !self.settings.get().sync_enabled {
                    return sync_disabled(id);
                }
                match self
                    .p2p
                    .node()
                    .pair_nearby(&addr, self.source.as_ref())
                    .await
                {
                    Ok(status) => self.pairing_progress(id, status),
                    Err(error) => failed(id, error),
                }
            }
            Method::PairJoin { code, addr } => {
                if !self.settings.get().sync_enabled {
                    return sync_disabled(id);
                }
                match self
                    .p2p
                    .node()
                    .pair_join(&code, &addr, self.source.as_ref())
                    .await
                {
                    Ok(status) => self.pairing_progress(id, status),
                    Err(error) => failed(id, error),
                }
            }
            Method::PairProgress => self.pairing_progress(id, self.p2p.node().pair_progress()),
            Method::PairConfirm { accept } => {
                if accept && !self.settings.get().sync_enabled {
                    self.p2p.node().pair_cancel();
                    return sync_disabled(id);
                }
                match self.p2p.node().pair_confirm(accept) {
                    Ok(status) => self.pairing_progress(id, status),
                    Err(error) => failed(id, error),
                }
            }
            Method::PairCancel => self.pairing_progress(id, self.p2p.node().pair_cancel()),
            Method::Unpair { pairing_id } => match self.p2p.node().unpair(&pairing_id) {
                Ok(true) => {
                    self.note_peers_changed();
                    tracing::info!(%pairing_id, "unpaired a device");
                    Response::ok(id, ResponseData::Empty {})
                }
                Ok(false) => failed(id, NodeError::NoPeer),
                Err(error) => failed(id, error),
            },
            // Succeeds even when no peer was removed: barring an id this
            // device has not seen yet is what revoking a lost device needs.
            Method::Revoke { pairing_id } => match self.p2p.node().revoke(&pairing_id, now_ms()) {
                Ok(removed) => {
                    self.note_peers_changed();
                    tracing::info!(%pairing_id, removed, "revoked a pairing");
                    Response::ok(id, ResponseData::Empty {})
                }
                Err(error) => {
                    tracing::warn!(%error, "could not revoke a pairing");
                    failed(id, NodeError::PeerStore)
                }
            },
            Method::Peers => {
                let peers = self.p2p.peers().list();
                Response::ok(
                    id,
                    ResponseData::Peers(peers.iter().map(|peer| self.peer_info(peer)).collect()),
                )
            }
            Method::SyncNow { pairing_id } => self.sync_now(id, pairing_id.as_deref()).await,
            Method::Discovered => Response::ok(id, ResponseData::Discovered(self.discovered())),
            Method::Rescan => {
                self.p2p.node().clear_discovery_candidate_cooldowns();
                self.p2p.node().republish();
                Response::ok(id, ResponseData::Discovered(self.discovered()))
            }
            method @ (Method::CloudSignIn { .. }
            | Method::CloudSignUp { .. }
            | Method::CloudSetEndpoint { .. }
            | Method::CloudSignOut
            | Method::CloudStatus
            | Method::CloudSyncNow) => self.modules.cloud_request(id, method).await,
            method => self.request_blocking(id, method),
        }
    }

    /// The blocking half of the dispatch: SQLite, AEAD and the native
    /// clipboard. Exhaustive over `Method` by design.
    pub fn request_blocking(&self, id: u64, method: Method) -> Response {
        match method {
            Method::Status => Response::ok(id, ResponseData::Status(self.status_data())),
            Method::SetDeviceName { name } => self.set_device_name(id, &name),
            Method::List { limit, cursor } => self.list(id, limit, cursor.as_deref()),
            Method::Search { query, limit } => self.search(id, &query, limit),
            Method::HistoryQuery {
                query,
                limit,
                cursor,
            } => self.history_query(id, &query, limit, cursor.as_deref()),
            Method::HistoryFacets => match self.store.history_facets(
                self.meta.device_id(),
                &self.meta.device_name(),
                self.meta.device_class(),
            ) {
                Ok(facets) => Response::ok(id, ResponseData::HistoryFacets(facets)),
                Err(error) => storage_error(id, "history_facets", &error),
            },
            Method::HistoryTags { clip_ids } => match self.store.history_tags(&clip_ids) {
                Ok(tags) => Response::ok(id, ResponseData::HistoryTags(tags)),
                Err(_) => Response::err(id, ErrorCode::InvalidRequest, "Tags could not be loaded."),
            },
            Method::MutateHistoryTags { mutation } => {
                let started = now_ms();
                match self
                    .store
                    .mutate_history_tags(&self.keyring.item_key(), &mutation)
                {
                    Ok(()) => {
                        self.note_local_change_since(started);
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(_) => Response::err(
                        id,
                        ErrorCode::InvalidRequest,
                        "The tag change is invalid. Use a unique name of up to 64 characters and select pinned clips.",
                    ),
                }
            }
            Method::GetSpoilerState => {
                let (revision, revealed_ids) = self.spoilers.snapshot();
                Response::ok(
                    id,
                    ResponseData::SpoilerState {
                        revision,
                        revealed_ids,
                    },
                )
            }
            Method::SetSpoilerRevealed {
                id: item_id,
                revealed,
            } => self.set_spoiler_revealed(id, item_id.as_deref(), revealed),
            Method::Copy { id: item_id } => self.copy(id, &item_id, false),
            Method::CopyPlainText { id: item_id } => self.copy(id, &item_id, true),
            Method::CopyText { id: item_id, text } => self.copy_text(id, &item_id, &text),
            Method::Get { id: item_id } => self.get(id, &item_id),
            Method::ImagePreview {
                id: item_id,
                max_edge,
                bounds,
            } => self.image_preview(id, &item_id, max_edge, bounds),
            Method::SourceAppIcon { id: item_id } => self.source_app_icon(id, &item_id),
            Method::SaveFile {
                id: item_id,
                dest_path,
                ..
            } => match history_actions::save_file(
                &self.store,
                &self.keyring.item_key(),
                &item_id,
                &dest_path,
                self.settings.get().max_file_size_bytes,
            ) {
                Ok(()) => Response::ok(id, ResponseData::Empty {}),
                Err(error) => error.response(id),
            },
            Method::FileOperations {
                clear,
                before,
                id: operation_id,
            } => match self
                .store
                .file_operations(clear, before.as_deref(), operation_id.as_deref())
            {
                Ok(records) => Response::ok(id, ResponseData::FileOperations(records)),
                Err(_) => Response::err(
                    id,
                    ErrorCode::Internal,
                    "File operations could not be loaded.",
                ),
            },
            Method::DownloadedFile { id: item_id } => {
                let path = self
                    .store
                    .downloaded_file(&item_id)
                    .ok()
                    .flatten()
                    .filter(|path| Path::new(path).is_file());
                Response::ok(id, ResponseData::DownloadedFile(path))
            }
            Method::ImportFileReference {
                reference,
                filename,
                mime_type,
                size_hint,
            } => {
                let started = now_ms();
                match copypaste_core::file_reference::import(
                    &self.store,
                    &self.keyring,
                    &reference,
                    &filename,
                    &mime_type,
                    &self.settings.get(),
                    size_hint,
                ) {
                    Ok(_) => {
                        self.note_local_change_since(started);
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
                }
            }
            Method::Add { content } => self.add(id, &content),
            Method::ImportFile {
                path,
                filename,
                mime_type,
                source_reference,
            } => {
                let started = now_ms();
                match copypaste_core::file_import::import_file(
                    &self.store,
                    &self.keyring,
                    Path::new(&path),
                    &filename,
                    &mime_type,
                    source_reference.as_deref(),
                    &self.settings.get(),
                ) {
                    Ok(_) => {
                        self.note_local_change_since(started);
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
                }
            }
            Method::Delete { id: item_id } => self.delete(id, &item_id),
            Method::DeleteAll { through } => self.delete_all(id, through),
            Method::HistoryCeiling => match self.store.max_rowid() {
                Ok(ceiling) => Response::ok(id, ResponseData::Count(ceiling.max(0) as u64)),
                Err(error) => storage_error(id, "history_ceiling", &error),
            },
            Method::Pin {
                id: item_id,
                pinned,
            } => self.pin(id, &item_id, pinned),
            Method::SetSecret {
                id: item_id,
                secret,
            } => {
                let started = now_ms();
                match self.store.set_secret(&self.keyring, &item_id, secret) {
                    Ok(true) => {
                        self.spoilers.set(Some(&item_id), false);
                        self.note_local_change_since(started);
                        self.mutated_item(id, &item_id)
                    }
                    Ok(false) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
                    Err(error) => storage_error(id, "set_secret", &error),
                }
            }
            Method::ReorderPinned { ids } => self.reorder_pinned(id, &ids),
            Method::Export { limit } => {
                match copypaste_core::transfer::export(&self.store, &self.keyring, limit) {
                    Ok(data) => Response::ok(id, ResponseData::Export(data)),
                    Err(copypaste_core::transfer::ExportError::ContentTooLarge) => {
                        Response::err(id, ErrorCode::ContentTooLarge, MSG_CONTENT_TOO_LARGE)
                    }
                    Err(copypaste_core::transfer::ExportError::Store(error)) => {
                        storage_error(id, "export", &error)
                    }
                }
            }
            Method::Import { items } => self.import(id, items),
            Method::Backup { dest_path } => {
                match copypaste_core::history_backup::backup(&self.store, &dest_path) {
                    Ok(size_bytes) => Response::ok(
                        id,
                        ResponseData::Backup(copypaste_ipc::BackupData { size_bytes }),
                    ),
                    Err(error) => error.response(id),
                }
            }
            Method::Restore { src_path, confirm } => {
                let started = now_ms();
                match copypaste_core::history_backup::restore(
                    &self.store,
                    &self.keyring.db_key(),
                    &src_path,
                    confirm,
                ) {
                    Ok(floor) => {
                        self.note_local_change_since(
                            floor.map_or(started, |oldest| oldest.min(started)),
                        );
                        Response::ok(id, ResponseData::Empty {})
                    }
                    Err(error) => error.response(id),
                }
            }
            Method::GetConfig => Response::ok(
                id,
                ResponseData::Config(ConfigApplied {
                    config: self.settings.config(),
                    restart_required: Vec::new(),
                }),
            ),
            Method::SetConfig { patch } => self.set_config(id, &patch),
            Method::GetPrivateMode => {
                let settings = self.settings.get();
                Response::ok(
                    id,
                    ResponseData::PrivateMode(PrivateModeData {
                        private_mode: settings.private_mode,
                        private_mode_epoch: settings.private_mode_epoch(),
                    }),
                )
            }
            Method::SetPrivateMode { enabled } => self.set_private_mode(id, enabled),
            Method::Snippets { op } => self.snippets(id, op),
            Method::Clips { op } => self.clips(id, op),
            Method::PasteQueue { op } => self.paste_queue(id, op),
            Method::Transforms { op } => self.transforms(id, op),
            Method::Rules { op } => self.rules(id, op),
            Method::Ai { op } => self.ai(id, op),
            Method::Watch
            | Method::WatchRuntime
            | Method::WatchEvents { .. }
            | Method::Shutdown => Response::err(id, ErrorCode::InvalidRequest, MSG_TRANSPORT_ONLY),
            // Spelled out rather than left to a `_`, so a method added to the
            // enum is a compile error here until it is classified.
            Method::Modules { .. }
            | Method::StartFileDownload { .. }
            | Method::CancelFileDownload { .. }
            | Method::PairCreateInvite
            | Method::PairJoin { .. }
            | Method::PairNearby { .. }
            | Method::PairProgress
            | Method::PairConfirm { .. }
            | Method::PairCancel
            | Method::Unpair { .. }
            | Method::Revoke { .. }
            | Method::Peers
            | Method::SyncNow { .. }
            | Method::Discovered
            | Method::Rescan
            | Method::CloudSignIn { .. }
            | Method::CloudSignUp { .. }
            | Method::CloudSetEndpoint { .. }
            | Method::CloudSignOut
            | Method::CloudStatus
            | Method::CloudSyncNow => {
                tracing::error!("a reactor operation reached the blocking dispatcher");
                Response::err(id, ErrorCode::Internal, MSG_STORAGE)
            }
        }
    }

    /// Whether `item_id` is a remote file offer that `SaveFile` must download.
    fn is_file_offer(&self, item_id: &str) -> bool {
        self.store.get(item_id).ok().flatten().is_some_and(|row| {
            copypaste_core::file_transfer::row_offer(&row, &self.keyring.item_key())
                .ok()
                .flatten()
                .is_some()
        })
    }

    /// Download a remote file offer, reporting progress to runtime watchers.
    async fn save_offer(
        &self,
        id: u64,
        item_id: &str,
        dest_path: &str,
        download_token: Option<String>,
    ) -> Response {
        let settings = self.settings.config();
        let sender = self.runtime_events.clone();
        let progress_token = download_token.clone();
        let last_percent = std::sync::atomic::AtomicU64::new(u64::MAX);
        match self
            .file_downloads
            .save_with_progress(
                &self.store,
                &self.keyring,
                self.p2p.node(),
                self.meta.device_id(),
                &settings,
                copypaste_core::file_download::FileDownloadRequest {
                    id: item_id,
                    destination: Path::new(dest_path),
                    token: download_token.as_deref(),
                },
                Arc::new(move |written, total| {
                    let Some(token) = progress_token.as_ref() else {
                        return;
                    };
                    if sender.receiver_count() == 0 {
                        return;
                    }
                    let percent = if total == 0 {
                        100
                    } else {
                        ((u128::from(written) * 100) / u128::from(total)).min(100) as u64
                    };
                    if last_percent.swap(percent, std::sync::atomic::Ordering::Relaxed) != percent {
                        let _ = sender.send(copypaste_ipc::RuntimeEventData {
                            file_operation: None,
                            change: None,
                            capture: None,
                            progress: Some(copypaste_ipc::FileProgress {
                                token: token.clone(),
                                written,
                                total,
                            }),
                        });
                    }
                }),
            )
            .await
        {
            Ok(()) => Response::ok(id, ResponseData::Empty {}),
            Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
        }
    }

    fn start_file_download(
        &self,
        id: u64,
        item_id: String,
        dest_path: String,
        token: String,
    ) -> Response {
        if item_id.len() > 128 || token.len() > 128 || dest_path.len() > 4096 {
            return Response::err(id, ErrorCode::InvalidRequest, MSG_DOWNLOAD_METADATA);
        }
        let slot = match self.file_downloads.reserve_background() {
            Ok(slot) => slot,
            Err(error) => return Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
        };
        let downloads = Arc::clone(&self.file_downloads);
        let store = self.store.clone();
        let keyring = Arc::clone(&self.keyring);
        let node = Arc::clone(self.p2p.node());
        let device = self.meta.device_id().to_owned();
        let settings = self.settings.config();
        tokio::spawn(async move {
            let _slot = slot;
            let _ = downloads
                .save_with_progress(
                    &store,
                    &keyring,
                    &node,
                    &device,
                    &settings,
                    copypaste_core::file_download::FileDownloadRequest {
                        id: &item_id,
                        destination: Path::new(&dest_path),
                        token: Some(&token),
                    },
                    Arc::new(|_, _| {}),
                )
                .await;
        });
        Response::ok(id, ResponseData::Empty {})
    }

    fn set_device_name(&self, id: u64, name: &str) -> Response {
        match self.meta.set_device_name(name) {
            Ok(()) => {
                self.meta
                    .publish_device_name(|name| self.p2p.node().set_device_name(name));
                self.note_peers_changed();
                Response::ok(id, ResponseData::Empty {})
            }
            Err(error) => storage_error(id, "rename device", &error),
        }
    }

    fn list(&self, id: u64, limit: u32, cursor: Option<&str>) -> Response {
        let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
        // Never "start from the top": a load-more that silently restarted
        // would repeat the whole history.
        let Ok(after) = cursor.map(ItemCursor::parse).transpose() else {
            return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_CURSOR);
        };
        match self.store.list_previews_bounded(
            after.as_ref(),
            limit,
            MAX_PAGE_CONTENT_BYTES,
            &self.keyring.item_key(),
            &self.keyring.preview_key(),
        ) {
            Ok(page) => Response::ok(
                id,
                ResponseData::Page(
                    self.preview_page(page.items, page.next.map(|cursor| cursor.token())),
                ),
            ),
            Err(error) => storage_error(id, "list", &error),
        }
    }

    fn search(&self, id: u64, query: &str, limit: u32) -> Response {
        let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
        match self
            .modules
            .search(&self.store, query, limit, MAX_PAGE_CONTENT_BYTES)
        {
            Ok(rows) => Response::ok(id, ResponseData::Page(self.decrypt_page(rows))),
            Err(error) => storage_error(id, "search", &error),
        }
    }

    /// The cursor is authenticated to the exact filter and order, so changing
    /// a filter cannot silently resume an unrelated list.
    fn history_query(
        &self,
        id: u64,
        query: &HistoryQuery,
        limit: u32,
        cursor: Option<&str>,
    ) -> Response {
        let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
        let after = match cursor
            .map(|token| HistoryCursor::parse_for(token, query))
            .transpose()
        {
            Ok(after) => after,
            Err(StoreError::InvalidCursor) => {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_CURSOR)
            }
            Err(_) => return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_HISTORY_QUERY),
        };
        match self.modules.query_history_previews(
            &self.store,
            query,
            after.as_ref(),
            limit,
            MAX_PAGE_CONTENT_BYTES,
            Some(self.meta.device_id()),
            (&self.keyring.item_key(), &self.keyring.preview_key()),
        ) {
            Ok(page) => Response::ok(
                id,
                ResponseData::Page(
                    self.preview_page(page.items, page.next.map(|cursor| cursor.token())),
                ),
            ),
            Err(StoreError::InvalidCursor) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_CURSOR)
            }
            Err(StoreError::InvalidHistoryQuery) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_HISTORY_QUERY)
            }
            Err(error) => storage_error(id, "history_query", &error),
        }
    }

    /// Produce a small PNG only when a history row needs to draw an image.
    /// The caller holds the shared preview-source admission.
    fn image_preview(
        &self,
        id: u64,
        item_id: &str,
        max_edge: Option<u32>,
        bounds: Option<copypaste_ipc::ImagePreviewBounds>,
    ) -> Response {
        // Release the encrypted row and decrypted source before encoding.
        let thumbnail = {
            let row = match self.store.get(item_id) {
                Ok(Some(row)) => row,
                Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
                Err(error) => return storage_error(id, "image_preview", &error),
            };
            if !matches!(
                copypaste_ipc::content_type::classify(&row.content_type),
                copypaste_ipc::ContentClass::Image
            ) {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_IMAGE_PREVIEW);
            }
            let bytes = match copypaste_core::open_binary(
                &row.content_ciphertext,
                &self.keyring.item_key(),
                &row.id,
            ) {
                Ok(bytes) => bytes,
                Err(error) => return decrypt_error(id, &error),
            };
            let budget = self.settings.get().max_decoded_image_mb;
            match copypaste_core::thumbnail_png(
                &bytes,
                budget,
                max_edge,
                bounds.map(|bounds| (bounds.width, bounds.height)),
            ) {
                Ok(thumbnail) => thumbnail,
                Err(error) => {
                    tracing::warn!(id = %row.id, ?error, "image preview unavailable");
                    return Response::err(id, ErrorCode::InvalidRequest, MSG_IMAGE_PREVIEW);
                }
            }
        };
        Response::ok(
            id,
            ResponseData::ImagePreview(copypaste_ipc::ImagePreview {
                png_base64: STANDARD.encode(thumbnail.png),
                width: thumbnail.width,
                height: thumbnail.height,
            }),
        )
    }

    fn source_app_icon(&self, id: u64, item_id: &str) -> Response {
        match self.store.source_app_icon_by_id(item_id) {
            Ok(Some(icon)) => Response::ok(
                id,
                ResponseData::SourceAppIcon(copypaste_ipc::ImagePreview {
                    png_base64: icon.png_base64,
                    width: icon.width,
                    height: icon.height,
                }),
            ),
            Ok(None) => Response::ok(id, ResponseData::Empty {}),
            Err(error) => storage_error(id, "source_app_icon", &error),
        }
    }

    /// The same ingest path as capture: encrypt, dedup, insert and evict.
    fn add(&self, id: u64, content: &str) -> Response {
        let settings = self.settings.config();
        let ingested =
            copypaste_core::ingest::ingest_into_with_capture_source_with_current_retention(
                &self.store,
                &self.keyring,
                content,
                copypaste_ipc::content_type::TEXT,
                now_ms(),
                None,
                None,
                &settings,
                || self.settings.config(),
            );
        match ingested {
            Ok(ingested) => match self.to_wire(ingested.into_item()) {
                Ok((item, _)) => {
                    self.note_local_change_since(item.created_at);
                    Response::ok(id, ResponseData::Item(item))
                }
                Err(error) => decrypt_error(id, &error),
            },
            Err(IngestError::Empty) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT)
            }
            Err(IngestError::TooLarge) => Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_BIG),
            Err(error @ (IngestError::Crypto(_) | IngestError::InvalidMetadata)) => {
                tracing::error!(?error, "add failed to encrypt");
                Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
            }
            Err(IngestError::Storage(error)) => storage_error(id, "add", &error),
        }
    }

    fn import(&self, id: u64, items: Vec<ExportItem>) -> Response {
        let settings = self.settings.config();
        match copypaste_core::transfer::import_with_current_retention_outcome(
            &self.store,
            &self.keyring,
            &settings,
            || self.settings.config(),
            items,
        ) {
            Ok(outcome) => {
                // Imported stamps predate the transport floors; pull them back
                // to the oldest committed row or the rows are never offered.
                if let Some(oldest) = outcome.oldest_changed_at {
                    self.note_local_change_since(oldest);
                }
                Response::ok(id, ResponseData::Import(outcome.data))
            }
            Err(copypaste_core::ImportError::Empty) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_EMPTY)
            }
            Err(copypaste_core::ImportError::TooMany) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_TOO_MANY)
            }
            Err(copypaste_core::ImportError::UnsupportedContentType) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_NON_TEXT)
            }
            Err(copypaste_core::ImportError::Partial {
                oldest_inserted_at,
                source,
                ..
            }) => {
                // Announce the durable rows even though the request fails.
                self.note_local_change_since(oldest_inserted_at);
                match *source {
                    copypaste_core::ImportError::Crypto(error) => {
                        tracing::warn!(
                            ?error,
                            "import partially completed before encryption failed"
                        );
                        Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
                    }
                    copypaste_core::ImportError::Storage(error) => {
                        storage_error(id, "import", &error)
                    }
                    _ => unreachable!("only fatal import errors can be partial"),
                }
            }
            Err(error @ copypaste_core::ImportError::Crypto(_)) => {
                tracing::warn!(?error, "import failed to encrypt an item");
                Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
            }
            Err(copypaste_core::ImportError::Storage(error)) => storage_error(id, "import", &error),
        }
    }

    /// One item, decrypted, with nothing else touched: reading an item must
    /// not publish it to the clipboard as a side effect.
    pub(crate) fn get(&self, id: u64, item_id: &str) -> Response {
        match self.store.get(item_id) {
            Ok(Some(row)) => match self.to_wire(row) {
                Ok((_, payload)) if history_actions::oversized_text(&payload) => {
                    Response::err(id, ErrorCode::ContentTooLarge, MSG_CONTENT_TOO_LARGE)
                }
                Ok((item, _)) => Response::ok(id, ResponseData::Item(item)),
                Err(error) => decrypt_error(id, &error),
            },
            Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
            Err(error) => storage_error(id, "get", &error),
        }
    }

    /// The row a mutation just changed. The change is already applied, so an
    /// oversized body answers with a bounded preview rather than an error.
    fn mutated_item(&self, id: u64, item_id: &str) -> Response {
        match self.store.get(item_id) {
            Ok(Some(row)) => match self.to_wire(row) {
                Ok((mut item, payload)) => {
                    if history_actions::oversized_text(&payload) {
                        item.truncated = copypaste_ipc::limits::bound_preview(&mut item.content);
                    }
                    Response::ok(id, ResponseData::Item(item))
                }
                Err(error) => decrypt_error(id, &error),
            },
            Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
            Err(error) => storage_error(id, "get", &error),
        }
    }

    fn delete(&self, id: u64, item_id: &str) -> Response {
        // Read first so an unknown id is `not_found` rather than a silent success.
        match self.store.get(item_id) {
            Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
            Err(error) => return storage_error(id, "get", &error),
            Ok(Some(_)) => {}
        }
        let started = now_ms();
        match self.store.delete(item_id) {
            Ok(_) => {
                self.note_local_change_since(started);
                Response::ok(id, ResponseData::Empty {})
            }
            Err(error) => storage_error(id, "delete", &error),
        }
    }

    /// Pinned rows survive; the predicate lives in the store, not here.
    fn delete_all(&self, id: u64, through: Option<i64>) -> Response {
        let started = now_ms();
        let result = match through {
            Some(through) => self.store.delete_all_through(through),
            None => self.store.delete_all(),
        };
        match result {
            Ok(deleted) => {
                if deleted > 0 {
                    self.note_local_change_since(started);
                }
                Response::ok(id, ResponseData::Count(deleted))
            }
            Err(error) => storage_error(id, "delete_all", &error),
        }
    }

    fn pin(&self, id: u64, item_id: &str, pinned: bool) -> Response {
        match self.store.get(item_id) {
            Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
            Err(error) => return storage_error(id, "get", &error),
            Ok(Some(_)) => {}
        }
        let started = now_ms();
        if let Err(error) = self
            .store
            .set_pinned_synced(item_id, pinned, &self.keyring.item_key())
        {
            return storage_error(id, "set_pinned", &error);
        }
        self.note_local_change_since(started);
        self.mutated_item(id, item_id)
    }

    /// The whole ordering in one transaction. Ids the caller no longer owns
    /// are ignored by the store, so a concurrent peer delete does not lose the
    /// user's reorder. Answers with the count of rows renumbered.
    fn reorder_pinned(&self, id: u64, ids: &[String]) -> Response {
        if ids.len() > MAX_REORDER_IDS {
            return Response::err(id, ErrorCode::InvalidRequest, MSG_REORDER_TOO_MANY);
        }
        let started = now_ms();
        match self
            .store
            .reorder_pinned_synced(ids, &self.keyring.item_key())
        {
            Ok(renumbered) => {
                if renumbered > 0 {
                    self.note_local_change_since(started);
                }
                Response::ok(id, ResponseData::Count(renumbered))
            }
            Err(error) => storage_error(id, "reorder_pinned", &error),
        }
    }

    fn set_spoiler_revealed(&self, id: u64, item_id: Option<&str>, revealed: bool) -> Response {
        if revealed {
            let Some(item_id) = item_id else {
                return Response::err(id, ErrorCode::InvalidRequest, "A clip is required.");
            };
            match self.store.get(item_id) {
                Ok(Some(item)) if item.clipboard_privacy().secret => {}
                Ok(_) => {
                    return Response::err(id, ErrorCode::NotFound, "The secret clip was not found.")
                }
                Err(error) => return storage_error(id, "reveal", &error),
            }
        }
        let (revision, revealed_ids, changed) = self.spoilers.set_with_change(item_id, revealed);
        if changed {
            self.note_remote_change();
        }
        Response::ok(
            id,
            ResponseData::SpoilerState {
                revision,
                revealed_ids,
            },
        )
    }

    pub(crate) fn copy(&self, id: u64, item_id: &str, plain_text: bool) -> Response {
        let key = self.keyring.item_key();
        let result = (|| {
            let opened = history_actions::open(&self.store, &key, item_id)?;
            let origin = self.meta.origin_of(opened.row());
            let prepared = history_actions::prepare_copy(
                &self.store,
                &key,
                opened,
                CopyOptions {
                    origin: &origin,
                    local_device_id: self.meta.device_id(),
                    plain_text,
                    allow_content_uri: self.host.writes_content_uris(),
                    max_file_size_bytes: self.settings.get().max_file_size_bytes,
                },
            )?;
            self.host
                .write(
                    &prepared.item.id,
                    prepared.payload(),
                    &prepared.content_type,
                    crate::host::WriteOptions::copy(prepared.item.privacy.secret),
                )
                .map_err(copypaste_core::history_error::HistoryError::from)?;
            self.host.copy_feedback();
            let item = history_actions::finish_copy(&self.store, prepared)?;
            self.instant_clipboard
                .note_local(&self.store, item.created_at);
            self.note_local_change_since(item.created_at);
            Ok::<_, copypaste_core::history_error::HistoryError>(item)
        })();
        match result {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(error) => error.response(id),
        }
    }

    fn copy_text(&self, id: u64, item_id: &str, text: &str) -> Response {
        match history_actions::copy_text_classified(
            &self.store,
            item_id,
            text,
            |payload, secret| {
                let options = crate::host::WriteOptions::copy(secret);
                self.host
                    .write(item_id, payload, copypaste_ipc::content_type::TEXT, options)
                    .inspect(|()| self.host.copy_feedback())
            },
        ) {
            Ok(stamp) => {
                self.instant_clipboard.note_local(&self.store, stamp);
                self.note_local_change_since(stamp);
                Response::ok(id, ResponseData::Empty {})
            }
            Err(error) => error.response(id),
        }
    }

    fn set_config(&self, id: u64, patch: &ConfigPatch) -> Response {
        self.set_config_with(id, patch, |_| {})
    }

    /// `SetConfig` with an extra effect run first inside the transition, so a
    /// host's tests can observe that transitions stay serialised while
    /// effects run.
    #[cfg(feature = "test-support")]
    pub fn set_config_with_effects(
        &self,
        id: u64,
        patch: &ConfigPatch,
        effects: impl FnOnce(&SettingsTransition),
    ) -> Response {
        self.set_config_with(id, patch, effects)
    }

    fn set_config_with(
        &self,
        id: u64,
        patch: &ConfigPatch,
        effects: impl FnOnce(&SettingsTransition),
    ) -> Response {
        let before = self.settings_fingerprint();
        match self.settings.apply_with_effects(patch, |transition| {
            effects(transition);
            self.host.settings_applied(transition);
            self.apply_runtime_effects(transition);
        }) {
            Ok(applied) => {
                if self.settings_fingerprint() != before {
                    self.note_capture_state_changed();
                }
                Response::ok(
                    id,
                    ResponseData::Config(ConfigApplied {
                        config: applied.config,
                        restart_required: ConfigData::restart_required_by(patch)
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    }),
                )
            }
            Err(error) => settings_error(id, &error),
        }
    }

    fn set_private_mode(&self, id: u64, enabled: bool) -> Response {
        let before = self.settings_fingerprint();
        match self.settings.apply_with_effects(
            &ConfigPatch {
                private_mode: Some(enabled),
                ..Default::default()
            },
            |transition| self.host.settings_applied(transition),
        ) {
            Ok(applied) => {
                if self.settings_fingerprint() != before {
                    self.note_capture_state_changed();
                }
                Response::ok(
                    id,
                    ResponseData::PrivateMode(PrivateModeData {
                        private_mode: applied.config.private_mode,
                        private_mode_epoch: applied.private_mode_epoch,
                    }),
                )
            }
            Err(error) => settings_error(id, &error),
        }
    }

    fn settings_fingerprint(&self) -> (ConfigData, u64, Option<copypaste_ipc::SettingsHealth>) {
        let settings = self.settings.get();
        (
            settings.clone(),
            settings.private_mode_epoch(),
            settings.health().cloned(),
        )
    }

    fn apply_runtime_effects(&self, transition: &SettingsTransition) {
        let removed = copypaste_core::retention::reconcile_policy(
            &self.store,
            || self.settings.config(),
            transition.should_enforce_retention(),
        );
        if removed > 0 {
            // Retention is local housekeeping, not a sync deletion, so it
            // only wakes watchers.
            self.note_remote_change();
        }
        if transition.lan_visibility_changed() {
            self.p2p
                .node()
                .set_lan_visibility(transition.config().lan_visibility);
        }
        if let Some(enabled) = transition.sync_enabled_changed() {
            self.p2p.sync_enabled_changed(enabled);
            self.modules.sync_enabled_changed(enabled);
            self.note_sync_status_changed();
        }
    }

    fn pair_create_invite(&self, id: u64) -> Response {
        if !self.settings.get().sync_enabled {
            return sync_disabled(id);
        }
        match self.p2p.node().pair_create_invite() {
            Ok(invite) => Response::ok(
                id,
                ResponseData::PairingInvite(copypaste_ipc::PairingInviteData {
                    code: invite.code,
                    pairing_id: invite.pairing_id,
                    listen_addr: invite.listen_addr,
                    expires_in_secs: invite.expires_in_secs,
                }),
            ),
            Err(error) => failed(id, error),
        }
    }

    /// A confirmed pairing records the peer's name and answers with the
    /// device it now knows, so no client has to re-list peers to show it.
    pub(crate) fn pairing_progress(&self, id: u64, status: PairingStatus) -> Response {
        let known_device = if status.phase == PairingPhase::Confirmed {
            if let Some(peer) = &status.peer {
                if let Err(error) = self.meta.record_device_name(&peer.device_id, &peer.name) {
                    tracing::warn!(%error, "could not record a paired device name");
                }
            }
            status
                .pairing_id
                .as_deref()
                .and_then(|pairing_id| self.p2p.peers().get(pairing_id))
                .map(|peer| self.peer_info(&peer))
        } else {
            None
        };
        Response::ok(
            id,
            ResponseData::PairingProgress(p2p_contract::pairing_progress(status, known_device)),
        )
    }

    fn peer_info(&self, peer: &Peer) -> copypaste_ipc::PeerInfo {
        let node = self.p2p.node();
        let discovered = node.find(&peer.pairing_id);
        let authenticated = node.authenticated_profile(&peer.pairing_id);
        let reachability = node.authenticated_reachability(&peer.pairing_id);
        p2p_contract::peer_info(
            peer,
            discovered.as_ref(),
            authenticated.as_ref(),
            reachability.as_ref(),
        )
    }

    /// LAN devices, paired or not. `paired` is resolved locally, because
    /// anyone on the LAN can claim any pairing id.
    fn discovered(&self) -> copypaste_ipc::DiscoveredData {
        let node = self.p2p.node();
        copypaste_ipc::DiscoveredData {
            devices: node
                .seen()
                .into_iter()
                .map(|found| {
                    let paired = node.is_paired_discovery(&found);
                    p2p_contract::discovered_device(found, paired)
                })
                .collect(),
        }
    }

    /// Sync with one peer, or with every known peer. One peer failing is that
    /// peer's `error` field; only a named peer that does not exist fails the
    /// whole request.
    async fn sync_now(&self, id: u64, pairing_id: Option<&str>) -> Response {
        if !self.settings.get().sync_enabled {
            return sync_disabled(id);
        }
        let targets = match pairing_id {
            Some(pairing_id) => match self.p2p.peers().get(pairing_id) {
                Some(peer) => vec![peer],
                None => return failed(id, NodeError::NoPeer),
            },
            None => self.p2p.peers().list(),
        };
        // Queued behind a running scheduled pass rather than dialling the
        // same peers beside it.
        let _permit = self.p2p.begin_round().await;
        if !self.settings.get().sync_enabled {
            return sync_disabled(id);
        }
        let cycle = self.p2p.sync_cycle();
        let mut results = Vec::with_capacity(targets.len());
        for peer in &targets {
            results.push(self.sync_one(peer, &cycle).await);
        }
        if results.iter().any(|result| result.received > 0) {
            self.note_remote_change();
        }
        Response::ok(id, ResponseData::Sync(results))
    }

    /// One peer, start to finish. A failure is a field, never an `Err`.
    pub(crate) async fn sync_one(
        &self,
        peer: &Peer,
        cycle: &copypaste_p2p::node::SyncCycle,
    ) -> SyncResult {
        let started = Instant::now();
        let cancel = cycle.cancel_token();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(NodeError::Session),
            result = self.p2p.node().sync_one_in_cycle(peer, self.source.as_ref(), cycle) => result,
        };
        let duration = started.elapsed();
        if let Ok(outcome) = &result {
            self.remember_device(outcome);
        }
        p2p_contract::sync_result(peer, result, duration)
    }

    /// Persist what the device on the other end calls itself. Best-effort: a
    /// failure costs a label, never a session that already exchanged rows.
    pub(crate) fn remember_device(&self, outcome: &copypaste_p2p::sync::SyncOutcome) {
        if let Err(error) = self
            .meta
            .record_device_name(&outcome.peer_device_id, &outcome.peer_device_name)
        {
            tracing::warn!(?error, "could not record a peer device name");
        }
        if let Some(profile) = &outcome.peer_profile {
            if let Err(error) = self
                .meta
                .record_device_class(&outcome.peer_device_id, profile.device_class)
            {
                tracing::warn!(?error, "could not record a peer device class");
            }
        }
    }

    pub(crate) fn to_wire(
        &self,
        row: StoredItem,
    ) -> Result<(Item, ClipboardPayload), copypaste_core::CryptoError> {
        let origin = self.meta.origin_of(&row);
        copypaste_core::history_item::open_item(
            row,
            &origin,
            self.meta.device_id(),
            &self.keyring.item_key(),
            false,
        )
    }

    /// A page of bounded previews. An unreadable row is counted, never
    /// allowed to blank the rest of the page.
    fn preview_page(
        &self,
        entries: Vec<copypaste_core::HistoryPreviewRow>,
        next_cursor: Option<String>,
    ) -> ItemPage {
        let (rows, previews): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .map(|entry| (entry.row, entry.preview))
            .unzip();
        let origins = self.meta.origins_for(&rows);
        let mut page = ItemPage {
            items: Vec::with_capacity(rows.len()),
            skipped_undecryptable: 0,
            next_cursor,
        };
        let pinned: Vec<String> = rows
            .iter()
            .filter(|row| row.pinned)
            .map(|row| row.id.clone())
            .collect();
        let mut tags = if pinned.is_empty() {
            Default::default()
        } else {
            self.store.item_tag_ids(&pinned).unwrap_or_else(|error| {
                tracing::warn!(?error, "showing a history page without tags");
                Default::default()
            })
        };
        for (row, preview) in rows.into_iter().zip(previews) {
            match preview {
                Some(preview) => {
                    let mut item =
                        preview.into_item(&row, &origins[&row.id], self.meta.device_id());
                    item.tag_ids = tags.remove(&row.id).unwrap_or_default();
                    page.items.push(item);
                }
                None => page.skipped_undecryptable += 1,
            }
        }
        page
    }

    pub(crate) fn decrypt_page(&self, rows: Vec<StoredItem>) -> ItemPage {
        let origins = self.meta.origins_for(&rows);
        // One key derivation for the page, scoped no wider than its plaintexts.
        let key = self.keyring.item_key();
        let mut page = ItemPage {
            items: Vec::with_capacity(rows.len()),
            skipped_undecryptable: 0,
            next_cursor: None,
        };
        for row in rows {
            let row_id = row.id.clone();
            match copypaste_core::history_item::open_item(
                row,
                &origins[&row_id],
                self.meta.device_id(),
                &key,
                true,
            ) {
                Ok((item, _)) => page.items.push(item),
                Err(error) => {
                    tracing::warn!(id = %row_id, ?error, "skipping an item that failed to decrypt");
                    page.skipped_undecryptable += 1;
                }
            }
        }
        page
    }
}

fn sync_disabled(id: u64) -> Response {
    Response::err(id, ErrorCode::NotReady, MSG_SYNC_OFF)
}

/// Only the error taxonomy is shared; the node owns the sentences.
fn failed(id: u64, error: NodeError) -> Response {
    Response::err(id, p2p_contract::node_error_code(&error), error.to_string())
}

/// `ConfigError` names a field and a bound and never a path, so it is passed
/// through: it is the one message here a user can act on.
fn settings_error(id: u64, error: &SettingsError) -> Response {
    match error {
        SettingsError::Invalid(_) => {
            Response::err(id, ErrorCode::InvalidRequest, error.to_string())
        }
        SettingsError::Store => Response::err(id, ErrorCode::Internal, error.to_string()),
    }
}
