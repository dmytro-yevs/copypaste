//! Parsing, the gates, and routing one request to a handler.
//!
//! Dispatch is a `match` on `copypaste_ipc::Method`. There is no string matching
//! anywhere in this file: the compiler enumerates the operations, so adding one
//! to the enum fails to build here until it is handled. string dispatch cannot prove
//! stringly-typed verbs through a chain of fall-through `match` arms spread over
//! 21 files, and a typo produced `unknown method` at runtime.

use std::io::Read;
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use serde::Deserialize;
use serde_json::value::RawValue;

use copypaste_ipc::{ErrorCode, Method, Request, Response, PROTOCOL_VERSION};
use tracing::{debug, error};

use super::listener::WireLease;
use super::messages::{MSG_INTERNAL, MSG_MALFORMED, MSG_NOT_READY};
use super::{config, dbadmin, items, transfer};
use crate::AppState;

/// A pre-handler rejection and whether the rejected request was a subscription.
pub(super) struct GateRejection {
    pub(super) response: Box<Response>,
    pub(super) was_watch: bool,
}

/// One line to a `Request`, or the reply that says why not.
///
/// Split from [`gate`] so that the caller's `Shutdown` fast path can branch on
/// the parsed value instead of parsing the line a second time to find the
/// method. The rejection is boxed: a `Response` carries a whole payload
/// variant, and an error type that large would be paid for on the success path
/// too.
#[derive(Deserialize)]
struct RequestEnvelope<'a> {
    id: u64,
    protocol_version: u32,
    #[serde(borrow)]
    method: &'a RawValue,
    #[serde(default, borrow)]
    params: Option<&'a RawValue>,
}

fn decode_request(line: &str) -> Result<Request, serde_json::Error> {
    let envelope: RequestEnvelope<'_> = serde_json::from_str(line)?;
    // Put the tag before its content regardless of the client's field order.
    // Reader chains borrow the input; they do not copy the potentially large
    // params into a temporary JSON tree or another complete wire buffer.
    let prefix = b"{\"method\":".as_slice();
    let separator = b",\"params\":".as_slice();
    let suffix = b"}".as_slice();
    let params = envelope.params.map(RawValue::get).unwrap_or("null");
    let reader = prefix
        .chain(envelope.method.get().as_bytes())
        .chain(separator)
        .chain(params.as_bytes())
        .chain(suffix);
    let method = serde_json::from_reader(reader)?;
    Ok(Request {
        id: envelope.id,
        protocol_version: envelope.protocol_version,
        method,
    })
}

pub(super) fn parse(line: &str) -> Result<Request, GateRejection> {
    decode_request(line).map_err(|e| {
        debug!(error = %e, "could not parse a request");
        GateRejection {
            response: Box::new(Response::err(
                recover_id(line),
                ErrorCode::InvalidRequest,
                MSG_MALFORMED,
            )),
            was_watch: false,
        }
    })
}

/// Everything that can reject a parsed request before a handler sees it.
pub(super) fn gate(state: &AppState, request: &Request) -> Option<GateRejection> {
    if let Some(rejection) = protocol_gate(request) {
        return Some(GateRejection {
            response: Box::new(rejection),
            was_watch: matches!(request.method, Method::Watch | Method::WatchRuntime),
        });
    }
    if state.is_draining() && !matches!(request.method, Method::Status) {
        return Some(GateRejection {
            response: Box::new(Response::err(
                request.id,
                ErrorCode::NotReady,
                MSG_NOT_READY,
            )),
            was_watch: matches!(request.method, Method::Watch | Method::WatchRuntime),
        });
    }
    if requires_ready(&request.method) && !state.is_ready() {
        return Some(GateRejection {
            response: Box::new(Response::err(
                request.id,
                ErrorCode::NotReady,
                MSG_NOT_READY,
            )),
            was_watch: matches!(request.method, Method::Watch | Method::WatchRuntime),
        });
    }
    None
}

/// Best-effort id recovery for a request that did not deserialise.
///
/// A client matches replies by id, so an unechoed id looks like a lost request
/// rather than a rejected one. If the line is not JSON at all there is nothing
/// to recover and `0` is used.
fn recover_id(line: &str) -> u64 {
    // Ignore all other values without materializing them; only the last id
    // matters, matching JSON object handling in the previous Value parser.
    struct IdVisitor;
    impl<'de> serde::de::Visitor<'de> for IdVisitor {
        type Value = u64;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a request object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<u64, M::Error> {
            let mut id = 0;
            while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                if key == "id" {
                    let raw = map.next_value::<&RawValue>()?;
                    id = serde_json::from_str(raw.get()).unwrap_or(0);
                } else {
                    map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            Ok(id)
        }
    }
    use serde::de::Deserializer;
    let mut deserializer = serde_json::Deserializer::from_str(line);
    deserializer
        .deserialize_map(IdVisitor)
        .and_then(|id| {
            deserializer.end()?;
            Ok(id)
        })
        .unwrap_or(0)
}

/// v2 speaks exactly one protocol version, so anything else is a hard mismatch:
/// clients must surface an upgrade prompt, not retry (manifest 04 §6.2).
fn protocol_gate(request: &Request) -> Option<Response> {
    if request.protocol_version == PROTOCOL_VERSION {
        return None;
    }
    Some(Response::err(
        request.id,
        ErrorCode::ProtocolMismatch,
        format!(
            "unsupported protocol version {} (daemon speaks {})",
            request.protocol_version, PROTOCOL_VERSION
        ),
    ))
}

/// Which operations need a usable database.
///
/// `Status` is deliberately exempt (manifest 04 §6.1): it is how a client — and
/// the next daemon deciding whether this socket is stale — asks what state the
/// daemon is in, so it must answer before readiness. No `_` arm: a new method
/// has to be classified here.
fn requires_ready(method: &Method) -> bool {
    match method {
        Method::Status => false,
        Method::Modules { .. } | Method::FileOperations { .. } | Method::ImportFileReference { .. } | Method::DownloadedFile { .. } | Method::StartFileDownload { .. } => true,
        // Settings live in the database, so reading them needs it open. There
        // is no degraded mode in v2 — a daemon whose database will not open
        // does not finish starting — so nothing is lost by gating them, and
        // `Status` remains the one question answerable before readiness.
        | Method::GetConfig
        | Method::SetConfig { .. }
        | Method::GetPrivateMode
        | Method::SetPrivateMode { .. }
        // Discovery is a network table held in memory, and a client asking
        // "what is on the LAN" has nothing to do with the database.
        | Method::Discovered
        | Method::Rescan => false,
        Method::List { .. }
        | Method::Search { .. }
        | Method::HistoryQuery { .. }
        | Method::HistoryFacets
        | Method::Copy { .. }
        | Method::CopyPlainText { .. }
        | Method::Get { .. }
        | Method::ImagePreview { .. }
        | Method::SourceAppIcon { .. }
        | Method::SaveFile { .. }
        | Method::CancelFileDownload { .. }
        | Method::Add { .. }
        | Method::ImportFile { .. }
        | Method::Delete { .. }
        | Method::DeleteAll { .. }
        | Method::HistoryCeiling
        | Method::Pin { .. }
        | Method::SetSecret { .. }
        | Method::GetSpoilerState
        | Method::SetSpoilerRevealed { .. }
        | Method::ReorderPinned { .. }
        | Method::SetDeviceName { .. }
        // Peer operations read and write the same history, and pairing needs
        // the device identity that comes out of the same database.
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
        // Cloud operations touch the same history and keep their cursors in
        // the same database, so they need it open too. `CloudStatus` is the
        // exception, for the same reason `Status` is: it is how a client finds
        // out what state the daemon is in.
        | Method::CloudSignIn { .. }
        | Method::CloudSignUp { .. }
        | Method::CloudSetEndpoint { .. }
        | Method::CloudSignOut
        | Method::CloudSyncNow
        // Transfer and database administration all read or write history.
        // `Restore` is manifest 04's "recovery escape hatch" and is gated here
        // anyway, deliberately: the degraded
        // mode to escape from, and v2 does not.
        | Method::Export { .. }
        | Method::Import { .. }
        | Method::Backup { .. }
        | Method::Restore { .. } => true,
        Method::CloudStatus => false,
        // Both are intercepted in `server::listener` before dispatch — one
        // changes what the connection is, the other ends the process — and
        // neither reaches here. Listed rather than left to a `_` so a new
        // method is still a compile error.
        //
        // `Shutdown` would be exempt anyway: a daemon whose database will not
        // open is exactly the one a user needs to be able to stop.
        Method::Watch | Method::WatchRuntime => true,
        Method::Shutdown => false,
    }
}

// Admission weights are work-class estimates based on the page,
// content and frame limits, not measurements or a bound on all process memory.
// One unit is MiB. The separate pool leaves control/status requests responsive.
const BULK_PAYLOAD_UNITS: usize = 96;
const INTERACTIVE_PAYLOAD_UNITS: usize = 32;
const MIB: usize = 1024 * 1024;

fn payload_pool(interactive: bool) -> Arc<Semaphore> {
    static BULK: OnceLock<Arc<Semaphore>> = OnceLock::new();
    static INTERACTIVE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    if interactive {
        Arc::clone(INTERACTIVE.get_or_init(|| Arc::new(Semaphore::new(INTERACTIVE_PAYLOAD_UNITS))))
    } else {
        Arc::clone(BULK.get_or_init(|| Arc::new(Semaphore::new(BULK_PAYLOAD_UNITS))))
    }
}

pub(super) fn interactive(method: &Method) -> bool {
    matches!(
        method,
        Method::Copy { .. }
            | Method::CopyPlainText { .. }
            | Method::Add { .. }
            | Method::Delete { .. }
            | Method::Pin { .. }
            | Method::SetSecret { .. }
    )
}

fn blocking_weight(method: &Method) -> u32 {
    match method {
        Method::Export { .. } | Method::Import { .. } | Method::ImportFile { .. } => {
            (2 * copypaste_ipc::MAX_FRAME_BYTES / MIB) as u32
        }
        method if interactive(method) => (4 * copypaste_ipc::MAX_CONTENT_BYTES / MIB) as u32,
        Method::Get { .. } => (8 * copypaste_ipc::MAX_CONTENT_BYTES / MIB) as u32,
        Method::List { .. }
        | Method::Search { .. }
        | Method::HistoryQuery { .. }
        | Method::HistoryFacets => (4 * copypaste_ipc::MAX_PAGE_CONTENT_BYTES / MIB) as u32,
        Method::SourceAppIcon { .. } => 8,
        _ => 0,
    }
}

pub(super) struct AdmittedResponse {
    response: Response,
    _lease: Option<OwnedSemaphorePermit>,
    _wire_lease: Option<WireLease>,
}
impl std::ops::Deref for AdmittedResponse {
    type Target = Response;
    fn deref(&self) -> &Response {
        &self.response
    }
}

async fn blocking_work<F>(
    pool: Arc<Semaphore>,
    units: u32,
    wire_lease: Option<WireLease>,
    work: F,
) -> Result<AdmittedResponse, tokio::task::JoinError>
where
    F: FnOnce() -> Response + Send + 'static,
{
    let lease = if units == 0 {
        None
    } else {
        Some(
            pool.acquire_many_owned(units)
                .await
                .expect("payload admission is never closed"),
        )
    };
    // The closure owns the lease even if the awaiting connection is cancelled.
    // It returns the lease with the response, retaining it through the write.
    tokio::task::spawn_blocking(move || AdmittedResponse {
        response: work(),
        _lease: lease,
        _wire_lease: wire_lease,
    })
    .await
}

/// Route one request.
///
/// Two kinds of work, and they belong on different threads. The peer and cloud
/// operations are network I/O — a TCP connect, a Noise handshake, an HTTPS
/// round trip — so they run on the reactor and `await` like any other socket
/// work. Everything else is blocking (SQLite, AEAD, the pasteboard) and takes
/// one `spawn_blocking` hop, exactly as it did before sync existed.
pub(super) async fn dispatch_request(
    state: &Arc<AppState>,
    request: Request,
    wire_lease: Option<WireLease>,
) -> AdmittedResponse {
    let id = request.id;
    let response = match request.method {
        Method::StartFileDownload {
            id: item_id,
            dest_path,
            token,
        } => {
            if item_id.len() > 128 || token.len() > 128 || dest_path.len() > 4096 {
                Response::err(
                    id,
                    ErrorCode::InvalidRequest,
                    "The download request contains invalid file metadata.",
                )
            } else {
                match state.file_downloads.reserve_background() {
                    Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
                    Ok(slot) => {
                        let state = Arc::clone(state);
                        tokio::spawn(async move {
                            let _slot = slot;
                            let settings = state.settings.get().clone();
                            let _ = state
                                .file_downloads
                                .save_with_progress(
                                    &state.store,
                                    &state.keyring,
                                    state.p2p.node(),
                                    state.meta.device_id(),
                                    &settings,
                                    copypaste_core::file_download::FileDownloadRequest {
                                        id: &item_id,
                                        destination: std::path::Path::new(&dest_path),
                                        token: Some(&token),
                                    },
                                    Arc::new(|_, _| {}),
                                )
                                .await;
                        });
                        Response::ok(id, copypaste_ipc::ResponseData::Empty {})
                    }
                }
            }
        }
        Method::CancelFileDownload { id: item_id } => {
            state.file_downloads.cancel(&item_id);
            Response::ok(id, copypaste_ipc::ResponseData::Empty {})
        }
        Method::SaveFile {
            id: item_id,
            dest_path,
            download_token,
        } => {
            let is_offer = state.store.get(&item_id).ok().flatten().is_some_and(|row| {
                copypaste_core::file_transfer::row_offer(&row, &state.keyring.item_key())
                    .ok()
                    .flatten()
                    .is_some()
            });
            if is_offer {
                let settings = state.settings.get().clone();
                let sender = state.runtime_event_sender();
                let progress_token = download_token.clone();
                let last_percent = std::sync::atomic::AtomicU64::new(u64::MAX);
                match state
                    .file_downloads
                    .save_with_progress(
                        &state.store,
                        &state.keyring,
                        state.p2p.node(),
                        state.meta.device_id(),
                        &settings,
                        copypaste_core::file_download::FileDownloadRequest {
                            id: &item_id,
                            destination: std::path::Path::new(&dest_path),
                            token: download_token.as_deref(),
                        },
                        Arc::new(move |written, total| {
                            if progress_token.is_none() || sender.receiver_count() == 0 {
                                return;
                            }
                            let percent = if total == 0 {
                                100
                            } else {
                                ((u128::from(written) * 100) / u128::from(total)).min(100) as u64
                            };
                            if last_percent.swap(percent, std::sync::atomic::Ordering::Relaxed)
                                != percent
                            {
                                if let Some(token) = progress_token.as_ref() {
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
                            }
                        }),
                    )
                    .await
                {
                    Ok(()) => Response::ok(id, copypaste_ipc::ResponseData::Empty {}),
                    Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
                }
            } else {
                let state = Arc::clone(state);
                return blocking_work(payload_pool(false), 0, wire_lease, move || {
                    items::save_file(&state, id, &item_id, &dest_path)
                })
                .await
                .unwrap_or_else(|_| AdmittedResponse {
                    response: Response::err(id, ErrorCode::Internal, MSG_INTERNAL),
                    _lease: None,
                    _wire_lease: None,
                });
            }
        }
        Method::ImagePreview {
            id: item_id,
            max_edge,
            bounds,
        } => {
            let permit = copypaste_core::image_preview::acquire_preview_source().await;
            let state = Arc::clone(state);
            return blocking_work(payload_pool(false), 0, wire_lease, move || {
                let _permit = permit;
                items::image_preview(&state, id, &item_id, max_edge, bounds)
            })
            .await
            .unwrap_or_else(|e| {
                error!(error = %e, "request handler did not complete");
                AdmittedResponse {
                    response: Response::err(id, ErrorCode::Internal, MSG_INTERNAL),
                    _lease: None,
                    _wire_lease: None,
                }
            });
        }
        Method::Modules { operation } => match state.modules.request(operation).await {
            Ok(json) => Response::ok(id, copypaste_ipc::ResponseData::Modules { json }),
            Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
        },
        Method::PairCreateInvite => crate::p2p::handlers::pair_create_invite(state, id).await,
        Method::PairNearby { addr } => crate::p2p::handlers::pair_nearby(state, id, &addr).await,
        Method::PairJoin { code, addr } => {
            crate::p2p::handlers::pair_join(state, id, &code, &addr).await
        }
        Method::PairProgress => crate::p2p::handlers::pair_get_progress(state, id).await,
        Method::PairConfirm { accept } => {
            crate::p2p::handlers::pair_confirm(state, id, accept).await
        }
        Method::PairCancel => crate::p2p::handlers::pair_cancel(state, id).await,
        Method::Unpair { pairing_id } => crate::p2p::handlers::unpair(state, id, &pairing_id).await,
        Method::Revoke { pairing_id } => crate::p2p::handlers::revoke(state, id, &pairing_id).await,
        Method::Peers => crate::p2p::handlers::peers(state, id).await,
        Method::SyncNow { pairing_id } => {
            crate::p2p::handlers::sync_now(state, id, pairing_id.as_deref()).await
        }
        Method::Discovered => crate::p2p::handlers::discovered(state, id).await,
        Method::Rescan => crate::p2p::handlers::rescan(state, id).await,
        method @ (Method::CloudSignIn { .. }
        | Method::CloudSignUp { .. }
        | Method::CloudSetEndpoint { .. }
        | Method::CloudSignOut
        | Method::CloudStatus
        | Method::CloudSyncNow) => state.modules.cloud_request(id, method).await,
        // Not `_`: a method added to the enum lands here and then fails to
        // build inside `dispatch_store`, which is where it has to be handled.
        method => {
            let units = blocking_weight(&method);
            let pool = payload_pool(interactive(&method));
            let state = Arc::clone(state);
            return match blocking_work(pool, units, wire_lease, move || {
                dispatch_store(&state, id, method)
            })
            .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!(error = %e, "request handler did not complete");
                    AdmittedResponse {
                        response: Response::err(id, ErrorCode::Internal, MSG_INTERNAL),
                        _lease: None,
                        _wire_lease: None,
                    }
                }
            };
        }
    };
    AdmittedResponse {
        response,
        _lease: None,
        _wire_lease: wire_lease,
    }
}

/// The blocking half of [`dispatch_request`]. Exhaustive over `Method` by design.
pub(crate) fn dispatch_store(state: &AppState, id: u64, method: Method) -> Response {
    match method {
        Method::CancelFileDownload { id: item_id } => {
            state.file_downloads.cancel(&item_id);
            Response::ok(id, copypaste_ipc::ResponseData::Empty {})
        }
        Method::Status => items::status(state, id),
        Method::Modules { .. } => unreachable!("module operations use their shared blocking host"),
        Method::SetDeviceName { name } => items::set_device_name(state, id, &name),
        Method::List { limit, cursor } => items::list(state, id, limit, cursor.as_deref()),
        Method::Search { query, limit } => items::search(state, id, &query, limit),
        Method::HistoryQuery {
            query,
            limit,
            cursor,
        } => items::history_query(state, id, &query, limit, cursor.as_deref()),
        Method::HistoryFacets => items::history_facets(state, id),
        Method::GetSpoilerState => {
            let (revision, revealed_ids) = state.spoilers.snapshot();
            Response::ok(
                id,
                copypaste_ipc::ResponseData::SpoilerState {
                    revision,
                    revealed_ids,
                },
            )
        }
        Method::SetSpoilerRevealed {
            id: item_id,
            revealed,
        } => items::set_spoiler_revealed(state, id, item_id.as_deref(), revealed),
        Method::Copy { id: item_id } => items::copy(state, id, &item_id),
        Method::CopyPlainText { id: item_id } => items::copy_plain_text(state, id, &item_id),
        Method::Get { id: item_id } => items::get(state, id, &item_id),
        Method::ImagePreview {
            id: item_id,
            max_edge,
            bounds,
        } => items::image_preview(state, id, &item_id, max_edge, bounds),
        Method::SourceAppIcon { id: item_id } => items::source_app_icon(state, id, &item_id),
        Method::SaveFile {
            id: item_id,
            dest_path,
            ..
        } => items::save_file(state, id, &item_id, &dest_path),
        Method::FileOperations {
            clear,
            before,
            id: operation_id,
        } => match state
            .store
            .file_operations(clear, before.as_deref(), operation_id.as_deref())
        {
            Ok(records) => Response::ok(id, copypaste_ipc::ResponseData::FileOperations(records)),
            Err(_) => Response::err(
                id,
                ErrorCode::Internal,
                "File operations could not be loaded.",
            ),
        },
        Method::DownloadedFile { id: item_id } => {
            let path = state
                .store
                .downloaded_file(&item_id)
                .ok()
                .flatten()
                .filter(|path| std::path::Path::new(path).is_file());
            Response::ok(id, copypaste_ipc::ResponseData::DownloadedFile(path))
        }
        Method::StartFileDownload { .. } => Response::err(
            id,
            ErrorCode::InvalidRequest,
            "Use the asynchronous download dispatcher.",
        ),
        Method::ImportFileReference {
            reference,
            filename,
            mime_type,
            size_hint,
        } => match copypaste_core::file_reference::import(
            &state.store,
            &state.keyring,
            &reference,
            &filename,
            &mime_type,
            &state.settings.get(),
            size_hint,
        ) {
            Ok(_) => {
                state.note_local_change();
                Response::ok(id, copypaste_ipc::ResponseData::Empty {})
            }
            Err(error) => Response::err(id, ErrorCode::InvalidRequest, error.to_string()),
        },
        Method::Add { content } => items::add(state, id, &content),
        Method::ImportFile {
            path,
            filename,
            mime_type,
            source_reference,
        } => items::import_file(
            state,
            id,
            &path,
            &filename,
            &mime_type,
            source_reference.as_deref(),
        ),
        Method::Delete { id: item_id } => items::delete(state, id, &item_id),
        Method::DeleteAll { through } => items::delete_all(state, id, through),
        Method::HistoryCeiling => items::history_ceiling(state, id),
        Method::Pin {
            id: item_id,
            pinned,
        } => items::pin(state, id, &item_id, pinned),
        Method::SetSecret {
            id: item_id,
            secret,
        } => items::set_secret(state, id, &item_id, secret),
        Method::ReorderPinned { ids } => items::reorder_pinned(state, id, &ids),
        Method::Export { limit } => transfer::export(state, id, limit),
        Method::Import { items } => transfer::import(state, id, items),
        Method::Backup { dest_path } => dbadmin::backup(state, id, &dest_path),
        Method::Restore { src_path, confirm } => dbadmin::restore(state, id, &src_path, confirm),
        Method::GetConfig => config::get(state, id),
        Method::SetConfig { patch } => config::set(state, id, &patch),
        Method::GetPrivateMode => config::private_mode(state, id),
        Method::SetPrivateMode { enabled } => config::set_private_mode(state, id, enabled),
        // Unreachable: `dispatch` takes these first. Spelled out rather than
        // left to a `_` so that adding a method to the enum is still a compile
        // error here, which is the whole point of dispatching on a type.
        Method::PairCreateInvite
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
        | Method::CloudSyncNow
        | Method::Watch
        | Method::WatchRuntime
        | Method::Shutdown => {
            error!("a network operation reached the blocking dispatcher");
            Response::err(id, ErrorCode::Internal, MSG_INTERNAL)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_decoder_preserves_wire_formats_and_field_order() {
        for line in [
            r#"{"id":1,"protocol_version":6,"method":"status"}"#,
            r#"{"params":{"content":"hello"},"method":"add","protocol_version":6,"id":2}"#,
            r#"{"method":"list","id":3,"unknown":[1,2,{"x":[4]}],"params":{"limit":5},"protocol_version":6}"#,
            r#"{"id":4,"protocol_version":6,"method":"watch","params":null}"#,
        ] {
            let expected: Request = serde_json::from_str(line).unwrap();
            let actual = decode_request(line).unwrap();
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{line}"
            );
        }
    }

    #[test]
    fn malformed_id_recovery_skips_nested_values_without_materializing_them() {
        let line = r#"{"id":1,"ignored":[{"a":[1,2,3]}],"id":42,"method":"missing"}"#;
        assert_eq!(recover_id(line), 42);
        assert_eq!(recover_id(r#"{"id":"bad"}"#), 0);
    }

    #[test]
    fn control_requests_do_not_enter_payload_pool() {
        assert_eq!(blocking_weight(&Method::Status), 0);
        assert_eq!(blocking_weight(&Method::GetConfig), 0);
        assert_eq!(
            blocking_weight(&Method::CancelFileDownload { id: "x".into() }),
            0
        );
        assert!(blocking_weight(&Method::Get { id: "x".into() }) > 0);
        assert!(
            blocking_weight(&Method::List {
                limit: 10,
                cursor: None
            }) > 0
        );
    }

    #[tokio::test]
    async fn control_work_runs_while_payload_pool_is_full() {
        let pool = Arc::new(Semaphore::new(1));
        let occupied = Arc::clone(&pool).acquire_owned().await.unwrap();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            blocking_work(Arc::clone(&pool), 0, None, || {
                Response::ok(1, copypaste_ipc::ResponseData::Empty {})
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(response.ok);
        drop(occupied);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stalled_export_does_not_block_copy_or_status() {
        use std::time::Duration;
        let (state, _dir) = crate::testutil::test_state("ipc-bulk-interactive");
        let copy_id = crate::testutil::add(&state, "copy me");
        crate::testutil::add(&state, &"x".repeat(copypaste_ipc::MAX_CONTENT_BYTES));
        let export_method = Method::Export { limit: 0 };
        let copy_method = Method::Copy { id: copy_id };
        assert!(!interactive(&export_method));
        assert!(interactive(&copy_method));
        let bulk = Arc::new(Semaphore::new(blocking_weight(&export_method) as usize));
        let actions = Arc::new(Semaphore::new(INTERACTIVE_PAYLOAD_UNITS));
        let export_state = Arc::clone(&state);
        let export = blocking_work(
            Arc::clone(&bulk),
            blocking_weight(&export_method),
            None,
            move || dispatch_store(&export_state, 1, export_method),
        )
        .await
        .unwrap();
        assert!(export.ok);
        let (server, client) = copypaste_ipc::transport::Stream::pair().unwrap();
        let (_, mut writer) = server.into_split();
        let mut stalled =
            tokio::spawn(async move { super::super::listener::send(&mut writer, &export).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut stalled)
                .await
                .is_err()
        );
        assert_eq!(bulk.available_permits(), 0);
        let copy_state = Arc::clone(&state);
        let copy_units = blocking_weight(&copy_method);
        let copy = tokio::time::timeout(
            Duration::from_secs(2),
            blocking_work(Arc::clone(&actions), copy_units, None, move || {
                dispatch_store(&copy_state, 2, copy_method)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(copy.ok, "{:?}", copy.error);
        let status_state = Arc::clone(&state);
        let status = tokio::time::timeout(
            Duration::from_secs(2),
            blocking_work(Arc::clone(&bulk), 0, None, move || {
                dispatch_store(&status_state, 3, Method::Status)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(status.ok);
        stalled.abort();
        let _ = stalled.await;
        drop(client);
        assert_eq!(
            bulk.available_permits(),
            2 * copypaste_ipc::MAX_FRAME_BYTES / MIB
        );
    }

    #[tokio::test]
    async fn blocking_payload_permit_survives_connection_cancellation() {
        let pool = Arc::new(Semaphore::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_pool = Arc::clone(&pool);
        static WIRE_USED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let wire = WireLease::reserve(&WIRE_USED, 100, 80).unwrap();
        let task = tokio::spawn(blocking_work(Arc::clone(&pool), 1, Some(wire), move || {
            let payload = "x".repeat(MIB);
            assert_eq!(worker_pool.available_permits(), 0);
            let _ = started_tx.send(());
            release_rx.recv().unwrap();
            Response::ok(1, copypaste_ipc::ResponseData::Modules { json: payload })
        }));
        started_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        assert_eq!(
            pool.available_permits(),
            0,
            "running closure must retain the lease"
        );
        assert_eq!(WIRE_USED.load(std::sync::atomic::Ordering::Acquire), 80);
        let queued = tokio::spawn(blocking_work(Arc::clone(&pool), 1, None, || {
            Response::ok(2, copypaste_ipc::ResponseData::Empty {})
        }));
        tokio::task::yield_now().await;
        queued.abort();
        let _ = queued.await;
        assert_eq!(
            pool.available_permits(),
            0,
            "cancelled waiter must not release the running closure's lease"
        );
        release_tx.send(()).unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        let permit = tokio::time::timeout_at(deadline, Arc::clone(&pool).acquire_owned())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(pool.available_permits(), 1);
        // Response fields drop separately. The semaphore wakes this task
        // before the blocking thread necessarily drops the following wire lease.
        tokio::time::timeout_at(deadline, async {
            while WIRE_USED.load(std::sync::atomic::Ordering::Acquire) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("completed response must release its wire lease");
        assert_eq!(WIRE_USED.load(std::sync::atomic::Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn blocking_response_retains_its_permit_until_dropped() {
        let pool = Arc::new(Semaphore::new(1));
        let result = blocking_work(Arc::clone(&pool), 1, None, || {
            Response::ok(1, copypaste_ipc::ResponseData::Empty {})
        })
        .await
        .unwrap();
        assert_eq!(pool.available_permits(), 0);
        drop(result);
        assert_eq!(pool.available_permits(), 1);
    }

    #[test]
    fn mismatched_protocol_version_is_rejected() {
        let request = Request {
            id: 7,
            protocol_version: PROTOCOL_VERSION + 1,
            method: Method::Status,
        };
        let response = protocol_gate(&request).expect("a mismatched version must be rejected");
        assert_eq!(response.id, 7);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::ProtocolMismatch));
    }

    #[tokio::test]
    async fn queued_image_preview_leaves_status_responsive_and_cancels_cleanly() {
        use std::time::Duration;

        let (state, _dir) = crate::testutil::test_state("preview-admission-status");
        let permit = copypaste_core::image_preview::acquire_preview_source().await;
        let request = Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::ImagePreview {
                id: "missing".into(),
                max_edge: None,
                bounds: None,
            },
        };
        let mut preview = Box::pin(dispatch_request(&state, request, None));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut preview)
                .await
                .is_err()
        );
        let status = Request {
            id: 2,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        };
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            dispatch_request(&state, status, None),
        )
        .await
        .expect("queued preview must not block status");
        assert!(response.ok);
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

    #[test]
    fn matching_protocol_version_passes_the_gate() {
        assert!(protocol_gate(&Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        })
        .is_none());
    }

    #[test]
    fn only_status_answers_before_readiness() {
        assert!(!requires_ready(&Method::Status));
        assert!(requires_ready(&Method::PairCreateInvite));
        assert!(requires_ready(&Method::PairJoin {
            code: "x".into(),
            addr: "127.0.0.1:1".into()
        }));
        assert!(requires_ready(&Method::PairProgress));
        assert!(requires_ready(&Method::PairConfirm { accept: true }));
        assert!(requires_ready(&Method::PairCancel));
        assert!(requires_ready(&Method::Unpair {
            pairing_id: "x".into()
        }));
        assert!(requires_ready(&Method::Revoke {
            pairing_id: "x".into()
        }));
        assert!(requires_ready(&Method::Peers));
        assert!(requires_ready(&Method::SyncNow { pairing_id: None }));
        assert!(!requires_ready(&Method::CloudStatus));
        assert!(requires_ready(&Method::CloudSyncNow));
        assert!(requires_ready(&Method::CloudSignOut));
        assert!(requires_ready(&Method::CloudSignIn {
            email: "a@example.com".into(),
            password: "pw".into(),
            passphrase: "a long passphrase".into(),
        }));
        assert!(requires_ready(&Method::CloudSignUp {
            email: "a@example.com".into(),
            password: "pw".into(),
            passphrase: "a long passphrase".into(),
        }));
        assert!(requires_ready(&Method::CloudSetEndpoint {
            url: "https://example.supabase.co".into(),
            anon_key: "anon".into(),
        }));
        assert!(requires_ready(&Method::List {
            limit: 10,
            cursor: None
        }));
        assert!(requires_ready(&Method::Search {
            query: "x".into(),
            limit: 10
        }));
        assert!(requires_ready(&Method::Copy { id: "x".into() }));
        assert!(requires_ready(&Method::CopyPlainText { id: "x".into() }));
        assert!(requires_ready(&Method::Get { id: "x".into() }));
        assert!(requires_ready(&Method::ImagePreview {
            id: "x".into(),
            max_edge: None,
            bounds: None,
        }));
        assert!(requires_ready(&Method::Add {
            content: "x".into()
        }));
        assert!(requires_ready(&Method::Delete { id: "x".into() }));
        assert!(requires_ready(&Method::DeleteAll { through: None }));
        assert!(requires_ready(&Method::Pin {
            id: "x".into(),
            pinned: true
        }));
        assert!(requires_ready(&Method::ReorderPinned { ids: Vec::new() }));
        // The one a user reaches for when nothing else answers.
        assert!(!requires_ready(&Method::Shutdown));
        assert!(requires_ready(&Method::Watch));
        assert!(requires_ready(&Method::Export { limit: 0 }));
        assert!(requires_ready(&Method::Import { items: Vec::new() }));
        assert!(requires_ready(&Method::Backup {
            dest_path: "x".into()
        }));
        assert!(requires_ready(&Method::Restore {
            src_path: "x".into(),
            confirm: true
        }));
        assert!(!requires_ready(&Method::Discovered));
        assert!(!requires_ready(&Method::Rescan));
        assert!(!requires_ready(&Method::GetConfig));
        assert!(!requires_ready(&Method::SetConfig {
            patch: copypaste_ipc::ConfigPatch::default()
        }));
        assert!(!requires_ready(&Method::GetPrivateMode));
        assert!(!requires_ready(&Method::SetPrivateMode { enabled: true }));
    }

    #[test]
    fn a_parse_failure_still_echoes_the_request_id() {
        // Valid JSON, unknown method: the id is recoverable.
        assert_eq!(recover_id(r#"{"id":42,"method":"nope"}"#), 42);
        // Not JSON at all: nothing to recover.
        assert_eq!(recover_id("{\"id\":42,"), 0);
        assert_eq!(recover_id("garbage"), 0);
    }
}
