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

use copypaste_ipc::{ErrorCode, Method, OpGates, Request, Response, PROTOCOL_VERSION};
use tracing::{debug, error};

use super::listener::WireLease;
use super::messages::{MSG_FORBIDDEN, MSG_INTERNAL, MSG_MALFORMED, MSG_NOT_READY};
use crate::AppState;
use copypaste_runtime::Route;

/// What the daemon knows about a connected peer, settled once per connection.
///
/// `is_app` is true only for a peer verified as the signed CopyPaste app.
/// Until that check exists no IPC peer is the app, so every privileged
/// operation is refused.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CallerIdentity {
    pub(crate) is_app: bool,
}

/// Whether a request turns its connection into a subscription.
pub(super) fn is_watch(method: &Method) -> bool {
    matches!(
        method,
        Method::Watch | Method::WatchRuntime | Method::WatchEvents { .. }
    )
}

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
pub(super) fn gate(
    state: &AppState,
    request: &Request,
    caller: CallerIdentity,
) -> Option<GateRejection> {
    let reject = |response| {
        Some(GateRejection {
            response: Box::new(response),
            was_watch: is_watch(&request.method),
        })
    };
    if let Some(rejection) = protocol_gate(request) {
        return reject(rejection);
    }
    if state.is_draining() && !matches!(request.method, Method::Status) {
        return reject(Response::err(
            request.id,
            ErrorCode::NotReady,
            MSG_NOT_READY,
        ));
    }
    if let Some(rejection) = privileged_gate(request, caller) {
        return reject(rejection);
    }
    if requires_ready(&request.method) && !state.is_ready() {
        return reject(Response::err(
            request.id,
            ErrorCode::NotReady,
            MSG_NOT_READY,
        ));
    }
    None
}

/// Approvals, grant administration, policy changes and un-hiding are served
/// only to the CopyPaste app, so an agent cannot approve its own request.
fn privileged_gate(request: &Request, caller: CallerIdentity) -> Option<Response> {
    (request.method.privileged() && !caller.is_app)
        .then(|| Response::err(request.id, ErrorCode::Forbidden, MSG_FORBIDDEN))
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
        | Method::HistoryTags { .. }
        | Method::MutateHistoryTags { .. }
        | Method::Copy { .. }
        | Method::CopyPlainText { .. }
        | Method::CopyText { .. }
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
        Method::Watch | Method::WatchRuntime | Method::WatchEvents { .. } => true,
        Method::Shutdown => false,
        Method::Snippets { .. }
        | Method::Clips { .. }
        | Method::PasteQueue { .. }
        | Method::Transforms { .. }
        | Method::Rules { .. }
        | Method::Ai { .. } => method.feature_op().is_some_and(OpGates::requires_ready),
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
            | Method::CopyText { .. }
            | Method::Add { .. }
            | Method::Delete { .. }
            | Method::Pin { .. }
            | Method::SetSecret { .. }
    ) || method.feature_op().is_some_and(OpGates::interactive)
}

fn blocking_weight(method: &Method) -> u32 {
    if let Some(op) = method.feature_op() {
        return op.blocking_weight();
    }
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

/// Route one request to the shared runtime.
///
/// The runtime's [`Route`] decides the thread. Reactor work (peer, cloud,
/// module and file-offer operations) is awaited here. Everything else is
/// blocking (SQLite, AEAD, the pasteboard) and takes one admitted
/// `spawn_blocking` hop.
pub(super) async fn dispatch_request(
    state: &Arc<AppState>,
    request: Request,
    wire_lease: Option<WireLease>,
) -> AdmittedResponse {
    let id = request.id;
    let method = request.method;
    let admission = match state.runtime.route(&method).await {
        Route::Reactor => {
            return AdmittedResponse {
                response: state.runtime.request(id, method).await,
                _lease: None,
                _wire_lease: wire_lease,
            };
        }
        Route::Blocking(admission) => admission,
    };
    let units = blocking_weight(&method);
    let pool = payload_pool(interactive(&method));
    let state = Arc::clone(state);
    match blocking_work(pool, units, wire_lease, move || {
        let _admission = admission;
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
    }
}

/// The blocking half of [`dispatch_request`].
pub(crate) fn dispatch_store(state: &AppState, id: u64, method: Method) -> Response {
    state.request_blocking(id, method)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_decoder_preserves_wire_formats_and_field_order() {
        for line in [
            r#"{"id":1,"protocol_version":7,"method":"status"}"#,
            r#"{"params":{"content":"hello"},"method":"add","protocol_version":7,"id":2}"#,
            r#"{"method":"list","id":3,"unknown":[1,2,{"x":[4]}],"params":{"limit":5},"protocol_version":7}"#,
            r#"{"id":4,"protocol_version":7,"method":"watch","params":null}"#,
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
    fn privileged_operations_are_refused_unless_the_caller_is_the_app() {
        let request = |method| Request {
            id: 9,
            protocol_version: PROTOCOL_VERSION,
            method,
        };
        let revoke = request(Method::Ai {
            op: copypaste_ipc::ai::AiOp::RevokeGrant { id: "g".into() },
        });
        let refused = privileged_gate(&revoke, CallerIdentity::default()).expect("refused");
        assert_eq!(refused.id, 9);
        assert_eq!(refused.error_code, Some(ErrorCode::Forbidden));
        assert!(privileged_gate(&revoke, CallerIdentity { is_app: true }).is_none());
        let grants = request(Method::Ai {
            op: copypaste_ipc::ai::AiOp::Grants,
        });
        assert!(privileged_gate(&grants, CallerIdentity::default()).is_none());
    }

    #[test]
    fn feature_operations_take_their_own_gate_answers() {
        let deliver = Method::PasteQueue {
            op: copypaste_ipc::paste_queue::PasteQueueOp::Deliver {
                target: copypaste_ipc::paste_queue::QueueTarget::Next,
                plain_text: None,
                mode: copypaste_ipc::paste_queue::QueueDeliveryMode::Clipboard,
            },
        };
        assert!(interactive(&deliver));
        assert!(blocking_weight(&deliver) as usize <= INTERACTIVE_PAYLOAD_UNITS);
        let catalogue = Method::Transforms {
            op: copypaste_ipc::transforms::TransformOp::Catalogue {
                locale: "en".into(),
            },
        };
        assert!(!requires_ready(&catalogue));
        assert!(requires_ready(&Method::WatchEvents { kinds: Vec::new() }));
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
