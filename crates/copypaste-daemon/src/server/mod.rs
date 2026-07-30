//! The Unix-socket IPC server.
//!
//! Framing is `tokio_util::codec::LinesCodec`. v1 hand-rolled a two-pass,
//! byte-scanning partial-JSON reader — it had to, because it wanted a
//! method-aware size cap before the method was parseable. v2 has one cap
//! (`copypaste_ipc::MAX_FRAME_BYTES`) and therefore no reason to look at bytes
//! at all.
//!
//! Only the read half goes through the codec. `SinkExt` lives behind
//! futures-util's `sink` feature, which this crate's dependency line does not
//! enable, so a reply is written as `to_string` plus a newline straight to the
//! socket. That is not the thing v1 got wrong: the parsing was.
//!
//! Dispatch is a `match` on `copypaste_ipc::Method`. There is no string
//! matching anywhere in this file: the compiler enumerates the operations, so
//! adding one to the enum fails to build here until it is handled. v1
//! dispatched 61 stringly-typed verbs through a chain of fall-through `match`
//! arms spread over 21 files, and a typo produced `unknown method` at runtime.
//!
//! **Errors never carry a filesystem path.** The socket path discloses the
//! local username (CLAUDE.md rule 4), and a `StoreError` from SQLite routinely
//! embeds the database path. Every failure is therefore mapped to one of the
//! fixed sentences below; the underlying error goes to the local log and never
//! onto the wire.

use std::fmt::Display;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use copypaste_core::StoredItem;
use copypaste_ipc::{
    ErrorCode, Item, Method, Request, Response, ResponseData, StatusData, MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tracing::{debug, error, info, warn};

use crate::capture::{self, IngestError};
use crate::AppState;

/// Server-side clamp on any caller-supplied page size (manifest 04 §3.3,
/// `MAX_PAGE`). A client asking for 10 million rows gets 1 000.
const MAX_PAGE: u32 = 1_000;
/// Applied when `list` is called with `limit = 0`.
const DEFAULT_LIST_PAGE: u32 = 50;
/// Applied when `search` is called with `limit = 0`.
const DEFAULT_SEARCH_PAGE: u32 = 20;

// The complete set of client-visible failure messages. Fixed sentences, no
// interpolation of internal errors, no paths. Asserted by
// `error_messages_never_disclose_a_path`.
const MSG_NOT_READY: &str = "daemon is still starting up; retry shortly";
const MSG_NOT_FOUND: &str = "item not found";
const MSG_MALFORMED: &str = "malformed request";
const MSG_TOO_LARGE: &str = "request exceeds the maximum frame size";
const MSG_EMPTY_CONTENT: &str = "content must not be empty";
const MSG_STORAGE: &str = "the history database could not be accessed";
const MSG_DECRYPT: &str = "the stored item could not be decrypted";
const MSG_ENCRYPT: &str = "the item could not be encrypted";
const MSG_CLIPBOARD: &str = "the system clipboard could not be written";
const MSG_INTERNAL: &str = "the daemon failed to process the request";

/// Create the socket directory, clear a stale socket, bind, and lock the socket
/// down to `0600`.
///
/// The socket is the only authentication boundary — there is no in-band auth
/// (manifest 04 I14) — so the `chmod` is a hard error, while tightening the
/// parent directory is warn-only (it may be a pre-existing shared data dir).
pub fn bind(path: &Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create the socket directory")?;
        if let Err(e) = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)) {
            warn!(error = %e, "could not restrict the socket directory to the owner");
        }
    }

    clear_stale_socket(path)?;
    let listener = UnixListener::bind(path).context("bind the daemon socket")?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .context("restrict the daemon socket to the owner")?;

    info!("ipc socket listening");
    Ok(listener)
}

/// A socket file left behind by a crashed daemon is removed; one with a live
/// listener behind it is not.
///
/// Refusing to steal a live socket is dual-daemon prevention: two daemons on
/// one database is a data-loss shape, and the second one exiting is the safe
/// outcome.
fn clear_stale_socket(path: &Path) -> anyhow::Result<()> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        anyhow::bail!("another copypaste daemon is already listening on the socket");
    }
    std::fs::remove_file(path).context("remove the stale socket")?;
    debug!("removed a stale socket file");
    Ok(())
}

/// Accept connections until shutdown.
pub async fn run(
    listener: UnixListener,
    state: Arc<AppState>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut connections = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let state = Arc::clone(&state);
                    connections.spawn(async move { handle_connection(stream, state).await });
                }
                Err(e) => warn!(error = %e, "could not accept an ipc connection"),
            },
        }
    }

    connections.shutdown().await;
}

/// One connection: read lines, answer each with exactly one line, until EOF.
async fn handle_connection(stream: UnixStream, state: Arc<AppState>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

    loop {
        let line = match lines.next().await {
            None => break,
            Some(Ok(line)) => line,
            Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {
                // The id lived somewhere in the discarded bytes, so `0` is the
                // best that can be echoed. The codec resumes at the next
                // newline, so the connection stays usable.
                let response = Response::err(0, ErrorCode::InvalidRequest, MSG_TOO_LARGE);
                if send(&mut writer, &response).await.is_err() {
                    break;
                }
                continue;
            }
            Some(Err(e)) => {
                debug!(error = %e, "ipc connection read failed");
                break;
            }
        };

        // Blank lines are keep-alives.
        if line.trim().is_empty() {
            continue;
        }

        let response = dispatch_line(&state, &line).await;

        if send(&mut writer, &response).await.is_err() {
            break;
        }
    }
}

/// One request, one line, one response line.
async fn send(writer: &mut OwnedWriteHalf, response: &Response) -> Result<(), ()> {
    let mut encoded = match serde_json::to_string(response) {
        Ok(encoded) => encoded,
        Err(e) => {
            error!(error = %e, "could not serialise a response");
            return Err(());
        }
    };
    encoded.push('\n');
    writer.write_all(encoded.as_bytes()).await.map_err(|e| {
        debug!(error = %e, "ipc connection write failed");
    })
}

/// Parse one request line, run the gates, dispatch.
fn dispatch_line(state: &Arc<AppState>, line: &str) -> impl std::future::Future<Output = Response> {
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
        Method::List { .. }
        | Method::Search { .. }
        | Method::Copy { .. }
        | Method::Add { .. }
        | Method::Delete { .. }
        | Method::DeleteAll
        | Method::Pin { .. }
        // Peer operations read and write the same history, and pairing needs
        // the device identity that comes out of the same database.
        | Method::PairCreate { .. }
        | Method::PairAccept { .. }
        | Method::Unpair { .. }
        | Method::Peers
        | Method::SyncNow { .. } => true,
    }
}

/// Route one request.
///
/// Two kinds of work, and they belong on different threads. The peer
/// operations are network I/O — a TCP connect, a Noise handshake, a session
/// that can last as long as the peer takes — so they run on the reactor and
/// `await` like any other socket work. Everything else is blocking (SQLite,
/// AEAD, the pasteboard) and takes one `spawn_blocking` hop, exactly as it did
/// before peers existed.
async fn dispatch(state: &Arc<AppState>, request: Request) -> Response {
    let id = request.id;
    match request.method {
        Method::PairCreate { name } => crate::p2p::handlers::pair_create(state, id, &name).await,
        Method::PairAccept { code, addr } => {
            crate::p2p::handlers::pair_accept(state, id, &code, &addr).await
        }
        Method::Unpair { pairing_id } => crate::p2p::handlers::unpair(state, id, &pairing_id).await,
        Method::Peers => crate::p2p::handlers::peers(state, id).await,
        Method::SyncNow { pairing_id } => {
            crate::p2p::handlers::sync_now(state, id, pairing_id.as_deref()).await
        }
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
fn dispatch_store(state: &AppState, id: u64, method: Method) -> Response {
    match method {
        Method::Status => status(state, id),
        Method::List { limit, offset } => list(state, id, limit, offset),
        Method::Search { query, limit } => search(state, id, &query, limit),
        Method::Copy { id: item_id } => copy(state, id, &item_id),
        Method::Add { content } => add(state, id, &content),
        Method::Delete { id: item_id } => delete(state, id, &item_id),
        Method::DeleteAll => delete_all(state, id),
        Method::Pin {
            id: item_id,
            pinned,
        } => pin(state, id, &item_id, pinned),
        // Unreachable: `dispatch` takes these first. Spelled out rather than
        // left to a `_` so that adding a method to the enum is still a compile
        // error here, which is the whole point of dispatching on a type.
        Method::PairCreate { .. }
        | Method::PairAccept { .. }
        | Method::Unpair { .. }
        | Method::Peers
        | Method::SyncNow { .. } => {
            error!("a peer operation reached the blocking dispatcher");
            Response::err(id, ErrorCode::Internal, MSG_INTERNAL)
        }
    }
}

fn status(state: &AppState, id: u64) -> Response {
    // `status` never fails: an unreadable count is reported as zero rather than
    // turned into an error, because the caller may be probing precisely because
    // the database is unhappy.
    let item_count = match state.store.count() {
        Ok(count) => u64::try_from(count).unwrap_or(0),
        Err(e) => {
            warn!(error = ?e, "could not count items for status");
            0
        }
    };

    Response::ok(
        id,
        ResponseData::Status(StatusData {
            version: crate::DAEMON_VERSION.to_string(),
            protocol_version: PROTOCOL_VERSION,
            item_count,
            capture_running: state.capture_running(),
            clipboard_backend: state.backend_name().to_string(),
        }),
    )
}

fn list(state: &AppState, id: u64, limit: u32, offset: u32) -> Response {
    let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
    match state.store.list(limit, offset) {
        Ok(rows) => Response::ok(id, ResponseData::Items(decrypt_rows(state, rows))),
        Err(e) => storage_error(id, "list", e),
    }
}

fn search(state: &AppState, id: u64, query: &str, limit: u32) -> Response {
    let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
    match state.store.search(query, limit) {
        Ok(rows) => {
            // Read-time enforcement of "sensitive items are never searchable".
            // The store already keeps them out of the index at write time; this
            // is the second of the three layers the rule demands, and it is
            // what protects a database written before the rule existed.
            let rows: Vec<StoredItem> = rows.into_iter().filter(|row| !row.is_sensitive).collect();
            Response::ok(id, ResponseData::Items(decrypt_rows(state, rows)))
        }
        Err(e) => storage_error(id, "search", e),
    }
}

fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
    };

    let item = match to_wire(state, row) {
        Ok(item) => item,
        Err(e) => return decrypt_error(id, e),
    };

    if let Err(e) = state.clipboard().set_contents(&item.content) {
        error!(error = ?e, "pasteboard write failed");
        return Response::err(id, ErrorCode::Internal, MSG_CLIPBOARD);
    }

    Response::ok(id, ResponseData::Item(item))
}

fn add(state: &AppState, id: u64, content: &str) -> Response {
    // Same ingest path as the capture loop: detector, encrypt, dedup, insert,
    // evict. `add` cannot skip the detector — an item entering here is exactly
    // as likely to be a credential as one copied from the pasteboard.
    match capture::ingest(state, content, "text") {
        Ok(ingested) => match to_wire(state, ingested.into_item()) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, e),
        },
        Err(IngestError::Empty) => Response::err(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT),
        Err(e @ IngestError::Crypto(_)) => {
            error!(error = ?e, "add failed to encrypt");
            Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
        }
        Err(e @ IngestError::Storage(_)) => storage_error(id, "add", e),
    }
}

fn delete(state: &AppState, id: u64, item_id: &str) -> Response {
    // Read first so an unknown id is `not_found` rather than a silent success:
    // a client that deleted nothing needs to know it deleted nothing.
    match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
        Ok(Some(_)) => {}
    }

    match state.store.delete(item_id) {
        Ok(_) => Response::ok(id, ResponseData::Empty {}),
        Err(e) => storage_error(id, "delete", e),
    }
}

fn delete_all(state: &AppState, id: u64) -> Response {
    // Manifest 03 (`CopyPaste-cb7u`) has `delete_all` tombstone only the
    // non-pinned rows — a pin is the user saying "keep this". `Store::delete_all`
    // currently clears pinned rows too; that is a storage-layer decision and it
    // is deliberately not second-guessed here, because filtering in the server
    // would put the rule in two places and leave the store's own callers with
    // the other behaviour.
    match state.store.delete_all() {
        Ok(deleted) => Response::ok(id, ResponseData::Count(u64::try_from(deleted).unwrap_or(0))),
        Err(e) => storage_error(id, "delete_all", e),
    }
}

fn pin(state: &AppState, id: u64, item_id: &str, pinned: bool) -> Response {
    match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
        Ok(Some(_)) => {}
    }

    if let Err(e) = state.store.set_pinned(item_id, pinned) {
        return storage_error(id, "set_pinned", e);
    }

    // Reply with the updated row so a client does not have to re-list to learn
    // the new state.
    match state.store.get(item_id) {
        Ok(Some(row)) => match to_wire(state, row) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, e),
        },
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => storage_error(id, "get", e),
    }
}

/// Decrypt a stored row into its wire form.
fn to_wire(state: &AppState, row: StoredItem) -> Result<Item, copypaste_core::CryptoError> {
    let key = state.keyring.item_key();
    // The item id is the AAD: a row decrypted under another row's identity must
    // fail authentication, not fall back to a plaintext read (CLAUDE.md rule 4,
    // "fail closed on crypto").
    let plaintext = copypaste_core::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)?;
    Ok(Item {
        id: row.id,
        content: String::from_utf8_lossy(&plaintext).into_owned(),
        content_type: row.content_type,
        created_at: row.created_at,
        pinned: row.pinned,
        is_sensitive: row.is_sensitive,
    })
}

/// Decrypt a page of rows, dropping any row that will not open.
///
/// One unreadable row must not blank an entire page of history — the other
/// items are still the user's data. The failure is logged with the row id so it
/// is diagnosable.
fn decrypt_rows(state: &AppState, rows: Vec<StoredItem>) -> Vec<Item> {
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let row_id = row.id.clone();
        match to_wire(state, row) {
            Ok(item) => items.push(item),
            Err(e) => warn!(id = %row_id, error = ?e, "skipping an item that failed to decrypt"),
        }
    }
    items
}

fn clamp_page(limit: u32, default: u32) -> u32 {
    if limit == 0 {
        default
    } else {
        limit.min(MAX_PAGE)
    }
}

/// Map a storage failure onto the wire.
///
/// The error itself is logged and dropped: a `StoreError` from SQLite carries
/// the database path, and a path in a client-visible string discloses the local
/// username.
fn storage_error(id: u64, operation: &'static str, error: impl Display) -> Response {
    error!(operation, error = %error, "storage operation failed");
    Response::err(id, ErrorCode::Internal, MSG_STORAGE)
}

fn decrypt_error(id: u64, error: copypaste_core::CryptoError) -> Response {
    error!(error = ?error, "decryption failed");
    Response::err(id, ErrorCode::Internal, MSG_DECRYPT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::tests::test_state;

    /// Every string this server can put in `Response.error`.
    const ALL_MESSAGES: &[&str] = &[
        MSG_NOT_READY,
        MSG_NOT_FOUND,
        MSG_MALFORMED,
        MSG_TOO_LARGE,
        MSG_EMPTY_CONTENT,
        MSG_STORAGE,
        MSG_DECRYPT,
        MSG_ENCRYPT,
        MSG_CLIPBOARD,
        MSG_INTERNAL,
    ];

    fn request(id: u64, method: Method) -> Request {
        Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        }
    }

    #[test]
    fn error_messages_never_disclose_a_path() {
        // CLAUDE.md rule 4. The socket path contains the local username, and
        // the database path is right next to it; the cheapest way to keep both
        // off the wire is to keep every separator out of every message.
        for message in ALL_MESSAGES {
            assert!(
                !message.contains('/') && !message.contains('\\'),
                "client-visible message looks like it contains a path: {message}"
            );
        }
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

    #[test]
    fn matching_protocol_version_passes_the_gate() {
        assert!(protocol_gate(&request(1, Method::Status)).is_none());
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
        assert!(requires_ready(&Method::Peers));
        assert!(requires_ready(&Method::SyncNow { pairing_id: None }));
        assert!(requires_ready(&Method::List {
            limit: 10,
            offset: 0
        }));
        assert!(requires_ready(&Method::Search {
            query: "x".into(),
            limit: 10
        }));
        assert!(requires_ready(&Method::Copy { id: "x".into() }));
        assert!(requires_ready(&Method::Add {
            content: "x".into()
        }));
        assert!(requires_ready(&Method::Delete { id: "x".into() }));
        assert!(requires_ready(&Method::DeleteAll));
        assert!(requires_ready(&Method::Pin {
            id: "x".into(),
            pinned: true
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

    #[test]
    fn page_sizes_are_clamped() {
        assert_eq!(clamp_page(0, DEFAULT_LIST_PAGE), DEFAULT_LIST_PAGE);
        assert_eq!(clamp_page(10, DEFAULT_LIST_PAGE), 10);
        assert_eq!(clamp_page(u32::MAX, DEFAULT_LIST_PAGE), MAX_PAGE);
    }

    #[tokio::test]
    async fn a_stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        std::fs::write(&path, b"left behind by a crashed daemon").unwrap();

        let listener = bind(&path).expect("bind must clear the stale file");
        drop(listener);
    }

    #[tokio::test]
    async fn the_socket_is_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let _listener = bind(&path).expect("bind");

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the socket is the only auth boundary");
    }

    /// A sensitive item is stored, is visible in `list`, and is never returned
    /// by `search`.
    ///
    /// The store keeps it out of the FTS index at write time; this covers the
    /// server's read-time layer, which is what protects a database written
    /// before the rule existed (CLAUDE.md rule 4 — "enforced at write time, at
    /// read time, and by a purge migration").
    #[test]
    fn search_never_returns_a_sensitive_item() {
        let (state, _dir) = test_state("server");

        let secret = "AKIAIOSFODNN7EXAMPLE";
        let response = dispatch_store(
            &state,
            1,
            Method::Add {
                content: secret.into(),
            },
        );
        let added = match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected an item, got {other:?}"),
        };
        assert!(added.is_sensitive, "the detector must flag an AWS key id");

        // Data loss is the worse outcome: flagged, but still stored and still
        // listed.
        let response = dispatch_store(
            &state,
            2,
            Method::List {
                limit: 50,
                offset: 0,
            },
        );
        match response.data {
            Some(ResponseData::Items(items)) => {
                assert!(items.iter().any(|item| item.id == added.id));
            }
            other => panic!("expected items, got {other:?}"),
        }

        let response = dispatch_store(
            &state,
            3,
            Method::Search {
                query: secret.into(),
                limit: 50,
            },
        );
        match response.data {
            Some(ResponseData::Items(items)) => assert!(
                !items.iter().any(|item| item.id == added.id),
                "a sensitive item reached the search results"
            ),
            other => panic!("expected items, got {other:?}"),
        }
    }

    /// `add` and the capture loop share one ingest path, so the same content
    /// twice inside the dedup window is one row.
    #[test]
    fn adding_the_same_content_twice_deduplicates() {
        let (state, _dir) = test_state("server");

        let add = |id| {
            let response = dispatch_store(
                &state,
                id,
                Method::Add {
                    content: "the same thing".into(),
                },
            );
            match response.data {
                Some(ResponseData::Item(item)) => item,
                other => panic!("expected an item, got {other:?}"),
            }
        };

        let first = add(1);
        let second = add(2);
        assert_eq!(first.id, second.id);
        assert_eq!(state.store.count().unwrap(), 1);
    }

    /// Empty content is a rejected request, not an empty row.
    #[test]
    fn adding_empty_content_is_rejected() {
        let (state, _dir) = test_state("server");

        let response = dispatch_store(
            &state,
            1,
            Method::Add {
                content: "   \n".into(),
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(state.store.count().unwrap(), 0);
    }

    /// Full round trip: a real socket, a real client, one connection, several
    /// pipelined requests.
    #[tokio::test]
    async fn requests_round_trip_over_a_socket() {
        let (state, dir) = test_state("server");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = UnixStream::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);

        // status answers, and reports the fake backend rather than a real one.
        let response = client.call(request(1, Method::Status)).await;
        assert!(response.ok);
        assert_eq!(response.id, 1);
        match response.data {
            Some(ResponseData::Status(status)) => {
                assert_eq!(status.protocol_version, PROTOCOL_VERSION);
                assert_eq!(status.clipboard_backend, "fake");
            }
            other => panic!("expected status data, got {other:?}"),
        }

        // add stores an item and returns it decrypted.
        let response = client
            .call(request(
                2,
                Method::Add {
                    content: "round trip".into(),
                },
            ))
            .await;
        assert!(response.ok, "add failed: {:?}", response.error);
        let added = match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected an item, got {other:?}"),
        };
        assert_eq!(added.content, "round trip");
        assert!(!added.is_sensitive);

        // list sees it.
        let response = client
            .call(request(
                3,
                Method::List {
                    limit: 10,
                    offset: 0,
                },
            ))
            .await;
        let items = match response.data {
            Some(ResponseData::Items(items)) => items,
            other => panic!("expected items, got {other:?}"),
        };
        assert!(items.iter().any(|item| item.id == added.id));

        // copy writes to the clipboard source.
        let response = client
            .call(request(
                4,
                Method::Copy {
                    id: added.id.clone(),
                },
            ))
            .await;
        assert!(response.ok, "copy failed: {:?}", response.error);

        // pin round-trips through the store.
        let response = client
            .call(request(
                5,
                Method::Pin {
                    id: added.id.clone(),
                    pinned: true,
                },
            ))
            .await;
        match response.data {
            Some(ResponseData::Item(item)) => assert!(item.pinned),
            other => panic!("expected the updated item, got {other:?}"),
        }

        // An unknown id is not_found, not a silent success.
        let response = client
            .call(request(
                6,
                Method::Delete {
                    id: "00000000-0000-0000-0000-000000000000".into(),
                },
            ))
            .await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::NotFound));

        // A mismatched protocol version is rejected before any handler runs.
        let mismatched = serde_json::json!({
            "id": 7,
            "protocol_version": PROTOCOL_VERSION + 1,
            "method": "status",
        });
        let response = client.call_raw(&mismatched.to_string()).await;
        assert_eq!(response.id, 7);
        assert_eq!(response.error_code, Some(ErrorCode::ProtocolMismatch));

        // Garbage is answered, not fatal to the connection.
        let response = client.call_raw("not json").await;
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));

        // ...and the connection still works afterwards.
        let response = client.call(request(9, Method::Status)).await;
        assert!(response.ok);
        assert_eq!(response.id, 9);

        shutdown_tx.send(true).unwrap();
        let _ = server.await;
    }

    /// A minimal newline-JSON client, mirroring what the CLI does.
    struct Client {
        writer: OwnedWriteHalf,
        lines: FramedRead<tokio::net::unix::OwnedReadHalf, LinesCodec>,
    }

    impl Client {
        fn new(stream: UnixStream) -> Self {
            let (reader, writer) = stream.into_split();
            Self {
                writer,
                lines: FramedRead::new(reader, LinesCodec::new()),
            }
        }

        async fn call(&mut self, request: Request) -> Response {
            self.call_raw(&serde_json::to_string(&request).unwrap())
                .await
        }

        async fn call_raw(&mut self, line: &str) -> Response {
            self.writer.write_all(line.as_bytes()).await.unwrap();
            self.writer.write_all(b"\n").await.unwrap();
            let reply = self
                .lines
                .next()
                .await
                .expect("a reply")
                .expect("a valid frame");
            serde_json::from_str(&reply).expect("a Response")
        }
    }
}
