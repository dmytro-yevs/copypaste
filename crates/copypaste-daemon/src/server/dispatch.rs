//! Parsing, the gates, and routing one request to a handler.
//!
//! Dispatch is a `match` on `copypaste_ipc::Method`. There is no string matching
//! anywhere in this file: the compiler enumerates the operations, so adding one
//! to the enum fails to build here until it is handled. v1 dispatched 61
//! stringly-typed verbs through a chain of fall-through `match` arms spread over
//! 21 files, and a typo produced `unknown method` at runtime.

use std::sync::Arc;

use copypaste_ipc::{ErrorCode, Method, Request, Response, PROTOCOL_VERSION};
use tracing::{debug, error};

use super::messages::{MSG_INTERNAL, MSG_MALFORMED, MSG_NOT_READY};
use super::{config, dbadmin, items, transfer};
use crate::AppState;

/// Parse one request line, run the gates, dispatch.
pub(crate) fn dispatch_line(
    state: &Arc<AppState>,
    line: &str,
) -> impl std::future::Future<Output = Response> {
    let parsed = parse_and_gate(state, line);
    let state = Arc::clone(state);
    async move {
        match parsed {
            Err(rejection) => *rejection,
            Ok(request) => dispatch(&state, request).await,
        }
    }
}

/// Everything that can reject a request before a handler sees it.
///
/// The rejection is boxed: a `Response` carries a whole payload variant, and an
/// error type that large would be paid for on the success path too.
fn parse_and_gate(state: &AppState, line: &str) -> Result<Request, Box<Response>> {
    let request: Request = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(e) => {
            debug!(error = %e, "could not parse a request");
            return Err(Box::new(Response::err(
                recover_id(line),
                ErrorCode::InvalidRequest,
                MSG_MALFORMED,
            )));
        }
    };

    if let Some(rejection) = protocol_gate(&request) {
        return Err(Box::new(rejection));
    }
    if requires_ready(&request.method) && !state.is_ready() {
        return Err(Box::new(Response::err(
            request.id,
            ErrorCode::NotReady,
            MSG_NOT_READY,
        )));
    }

    Ok(request)
}

/// Best-effort id recovery for a request that did not deserialise.
///
/// A client matches replies by id, so an unechoed id looks like a lost request
/// rather than a rejected one. If the line is not JSON at all there is nothing
/// to recover and `0` is used.
fn recover_id(line: &str) -> u64 {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| value.get("id").and_then(serde_json::Value::as_u64))
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
        | Method::Copy { .. }
        | Method::CopyPlainText { .. }
        | Method::Get { .. }
        | Method::Add { .. }
        | Method::Delete { .. }
        | Method::DeleteAll
        | Method::Pin { .. }
        | Method::ReorderPinned { .. }
        // Peer operations read and write the same history, and pairing needs
        // the device identity that comes out of the same database.
        | Method::PairCreate { .. }
        | Method::PairAccept { .. }
        | Method::Unpair { .. }
        | Method::Revoke { .. }
        | Method::Peers
        | Method::SyncNow { .. }
        // Cloud operations touch the same history and keep their cursors in
        // the same database, so they need it open too. `CloudStatus` is the
        // exception, for the same reason `Status` is: it is how a client finds
        // out what state the daemon is in.
        | Method::CloudSignIn { .. }
        | Method::CloudSignOut
        | Method::CloudSyncNow
        // Transfer and database administration all read or write history.
        // `Restore` is manifest 04's "recovery escape hatch" and is gated here
        // anyway, deliberately: v1 needed it ungated because it had a degraded
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
        Method::Watch | Method::Shutdown => false,
    }
}

/// Route one request.
///
/// Two kinds of work, and they belong on different threads. The peer and cloud
/// operations are network I/O — a TCP connect, a Noise handshake, an HTTPS
/// round trip — so they run on the reactor and `await` like any other socket
/// work. Everything else is blocking (SQLite, AEAD, the pasteboard) and takes
/// one `spawn_blocking` hop, exactly as it did before sync existed.
async fn dispatch(state: &Arc<AppState>, request: Request) -> Response {
    let id = request.id;
    match request.method {
        Method::PairCreate { name } => crate::p2p::handlers::pair_create(state, id, &name).await,
        Method::PairAccept { code, addr } => {
            crate::p2p::handlers::pair_accept(state, id, &code, &addr).await
        }
        Method::Unpair { pairing_id } => crate::p2p::handlers::unpair(state, id, &pairing_id).await,
        Method::Revoke { pairing_id } => crate::p2p::handlers::revoke(state, id, &pairing_id).await,
        Method::Peers => crate::p2p::handlers::peers(state, id).await,
        Method::SyncNow { pairing_id } => {
            crate::p2p::handlers::sync_now(state, id, pairing_id.as_deref()).await
        }
        Method::Discovered => crate::p2p::handlers::discovered(state, id).await,
        Method::Rescan => crate::p2p::handlers::rescan(state, id).await,
        Method::CloudSignIn {
            email,
            password,
            passphrase,
        } => crate::cloud::handlers::sign_in(state, id, &email, &password, &passphrase).await,
        Method::CloudSignOut => crate::cloud::handlers::sign_out(state, id).await,
        Method::CloudStatus => crate::cloud::handlers::status(state, id).await,
        Method::CloudSyncNow => crate::cloud::handlers::sync_now(state, id).await,
        // Not `_`: a method added to the enum lands here and then fails to
        // build inside `dispatch_store`, which is where it has to be handled.
        method => {
            let state = Arc::clone(state);
            match tokio::task::spawn_blocking(move || dispatch_store(&state, id, method)).await {
                Ok(response) => response,
                Err(e) => {
                    error!(error = %e, "request handler did not complete");
                    Response::err(id, ErrorCode::Internal, MSG_INTERNAL)
                }
            }
        }
    }
}

/// The blocking half of [`dispatch`]. Exhaustive over `Method` by design.
pub(crate) fn dispatch_store(state: &AppState, id: u64, method: Method) -> Response {
    match method {
        Method::Status => items::status(state, id),
        Method::List { limit, cursor } => items::list(state, id, limit, cursor.as_deref()),
        Method::Search { query, limit } => items::search(state, id, &query, limit),
        Method::Copy { id: item_id } => items::copy(state, id, &item_id),
        Method::CopyPlainText { id: item_id } => items::copy_plain_text(state, id, &item_id),
        Method::Get { id: item_id } => items::get(state, id, &item_id),
        Method::Add { content } => items::add(state, id, &content),
        Method::Delete { id: item_id } => items::delete(state, id, &item_id),
        Method::DeleteAll => items::delete_all(state, id),
        Method::Pin {
            id: item_id,
            pinned,
        } => items::pin(state, id, &item_id, pinned),
        Method::ReorderPinned { ids } => items::reorder_pinned(state, id, &ids),
        Method::Export {
            limit,
            include_sensitive,
        } => transfer::export(state, id, limit, include_sensitive),
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
        Method::PairCreate { .. }
        | Method::PairAccept { .. }
        | Method::Unpair { .. }
        | Method::Revoke { .. }
        | Method::Peers
        | Method::SyncNow { .. }
        | Method::Discovered
        | Method::Rescan
        | Method::CloudSignIn { .. }
        | Method::CloudSignOut
        | Method::CloudStatus
        | Method::CloudSyncNow
        | Method::Watch
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
        assert!(requires_ready(&Method::PairCreate { name: "x".into() }));
        assert!(requires_ready(&Method::PairAccept {
            code: "x".into(),
            addr: "127.0.0.1:1".into()
        }));
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
        assert!(requires_ready(&Method::Add {
            content: "x".into()
        }));
        assert!(requires_ready(&Method::Delete { id: "x".into() }));
        assert!(requires_ready(&Method::DeleteAll));
        assert!(requires_ready(&Method::Pin {
            id: "x".into(),
            pinned: true
        }));
        assert!(requires_ready(&Method::ReorderPinned { ids: Vec::new() }));
        // The one a user reaches for when nothing else answers.
        assert!(!requires_ready(&Method::Shutdown));
        assert!(requires_ready(&Method::Export {
            limit: 0,
            include_sensitive: false
        }));
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
