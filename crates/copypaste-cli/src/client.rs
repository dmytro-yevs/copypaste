//! The daemon client: one Unix socket, one newline-delimited JSON request, one
//! typed response.
//!
//! Framing is [`tokio_util::codec::LinesCodec`], not a byte-scanning read loop —
//! the same codec the daemon uses, so there is exactly one framing
//! implementation on each side of the socket.
//!
//! Responses are deserialised straight into [`Response`] / [`ResponseData`].
//! v1's CLI walked `serde_json::Value` with 138 `.as_*()` calls that silently
//! defaulted on a missing field; there is no `serde_json::Value` anywhere in
//! this crate.

use crate::error::CliError;
use copypaste_ipc::{
    socket_path, BackupData, CloudStatusData, CloudSyncData, ConfigApplied, DiscoveredData,
    ErrorCode, EventData, ExportData, ImportData, Item, ItemPage, Method, PairingData, PeerInfo,
    Request, Response, ResponseData, StatusData, SyncResult, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LinesCodec};

/// Port manifest 04 §6.2: `not_ready` is retried with a fixed 500 ms backoff,
/// at most three times, reconnecting each time. The cap is deliberately tight —
/// a persistently degraded daemon returns this code forever, and hanging is a
/// worse answer than "try again in a few seconds".
const NOT_READY_MAX_RETRIES: u32 = 3;
const NOT_READY_BACKOFF: Duration = Duration::from_millis(500);

/// A CLI invocation is one command, but a `not_ready` retry reconnects, so ids
/// still have to differ within the process.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

struct Connection {
    framed: Framed<UnixStream, LinesCodec>,
}

impl Connection {
    async fn open(path: &Path) -> Result<Self, CliError> {
        // The path is never surfaced to the user: it discloses the local
        // username (CLAUDE.md rule 4). Every failure here collapses to one
        // pathless variant.
        let stream = UnixStream::connect(path)
            .await
            .map_err(|_| CliError::DaemonUnreachable)?;
        Ok(Self {
            framed: Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES)),
        })
    }

    async fn call(&mut self, method: Method) -> Result<Response, CliError> {
        let id = next_id();
        let request = Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        };
        let line = serde_json::to_string(&request)
            .map_err(|e| CliError::local(format!("could not encode the request: {e}")))?;

        self.framed
            .send(line)
            .await
            .map_err(|_| CliError::DaemonUnreachable)?;

        let line = match self.framed.next().await {
            Some(Ok(line)) => line,
            // A dropped connection with no reply is the daemon's read-timeout
            // behaviour, and is indistinguishable from it having gone away.
            Some(Err(_)) | None => return Err(CliError::DaemonUnreachable),
        };

        let response: Response = serde_json::from_str(&line).map_err(|e| {
            CliError::local(format!("could not understand the daemon's reply: {e}"))
        })?;

        if response.id != id {
            return Err(CliError::local(format!(
                "the daemon answered request {} with a reply for request {}",
                id, response.id
            )));
        }
        Ok(response)
    }
}

/// Send one request to the running daemon, honouring the `not_ready` retry
/// contract.
pub async fn request(method: Method) -> Result<Response, CliError> {
    request_at(&socket_path(), method).await
}

async fn request_at(path: &Path, method: Method) -> Result<Response, CliError> {
    let mut attempts = 0;
    loop {
        let mut connection = Connection::open(path).await?;
        let response = connection.call(method.clone()).await?;

        if response.error_code == Some(ErrorCode::NotReady) && attempts < NOT_READY_MAX_RETRIES {
            attempts += 1;
            // Each retry reconnects: the daemon may close the connection after
            // an error response.
            drop(connection);
            tokio::time::sleep(NOT_READY_BACKOFF).await;
            continue;
        }
        return Ok(response);
    }
}

/// Subscribe to the daemon's change stream and hand each event to `on_event`.
///
/// Returns when the daemon stops or the callback asks to stop. The connection
/// is deliberately not the one [`request`] uses: a subscription lives for as
/// long as the client wants it to, and mixing it with request/response would
/// make replies and events ambiguous on one stream.
pub async fn watch(mut on_event: impl FnMut(EventData) -> bool) -> Result<(), CliError> {
    let mut connection = Connection::open(&socket_path()).await?;
    // The ack proves the daemon subscribed before answering, so nothing that
    // happens after this line is missed.
    let ack = connection.call(Method::Watch).await?;
    into_data(ack)?;

    loop {
        let line = match connection.framed.next().await {
            Some(Ok(line)) => line,
            Some(Err(_)) | None => return Err(CliError::DaemonUnreachable),
        };
        let response: Response = serde_json::from_str(&line).map_err(|e| {
            CliError::local(format!("could not understand the daemon's reply: {e}"))
        })?;
        match into_data(response)? {
            Some(ResponseData::Event(event)) => {
                if !on_event(event) {
                    return Ok(());
                }
            }
            _ => return Err(shape_error("a change event")),
        }
    }
}

/// Split a response into success payload or typed failure.
///
/// Branches on `ok` and `error_code`, never on the `error` string
/// (port manifest 04, I9).
pub fn into_data(response: Response) -> Result<Option<ResponseData>, CliError> {
    if response.ok {
        return Ok(response.data);
    }
    Err(CliError::Daemon {
        code: response.error_code,
        message: response
            .error
            .unwrap_or_else(|| "the daemon reported a failure but gave no reason".to_string()),
    })
}

fn shape_error(expected: &str) -> CliError {
    CliError::local(format!(
        "the daemon replied with something other than {expected}; \
         the daemon and this CLI may be different versions"
    ))
}

/// A response that must carry a page of items.
pub fn expect_page(data: Option<ResponseData>) -> Result<ItemPage, CliError> {
    match data {
        Some(ResponseData::Page(page)) => Ok(page),
        _ => Err(shape_error("a page of items")),
    }
}

/// A response that must carry an export.
pub fn expect_export(data: Option<ResponseData>) -> Result<ExportData, CliError> {
    match data {
        Some(ResponseData::Export(export)) => Ok(export),
        _ => Err(shape_error("an export")),
    }
}

/// A response that must carry import counts.
pub fn expect_import(data: Option<ResponseData>) -> Result<ImportData, CliError> {
    match data {
        Some(ResponseData::Import(result)) => Ok(result),
        _ => Err(shape_error("import results")),
    }
}

/// A response that must carry a completed backup.
pub fn expect_backup(data: Option<ResponseData>) -> Result<BackupData, CliError> {
    match data {
        Some(ResponseData::Backup(backup)) => Ok(backup),
        _ => Err(shape_error("a backup result")),
    }
}

/// A response that must carry discovered devices.
pub fn expect_discovered(data: Option<ResponseData>) -> Result<DiscoveredData, CliError> {
    match data {
        Some(ResponseData::Discovered(found)) => Ok(found),
        _ => Err(shape_error("discovered devices")),
    }
}

/// A response that must carry the daemon's settings.
pub fn expect_config(data: Option<ResponseData>) -> Result<ConfigApplied, CliError> {
    match data {
        Some(ResponseData::Config(applied)) => Ok(applied),
        _ => Err(shape_error("daemon settings")),
    }
}

/// A response that must carry daemon status.
pub fn expect_status(data: Option<ResponseData>) -> Result<StatusData, CliError> {
    match data {
        Some(ResponseData::Status(status)) => Ok(status),
        _ => Err(shape_error("daemon status")),
    }
}

/// The item a mutation returned, when it returned one.
///
/// `add` may answer with the stored item or with an acknowledgement; both are
/// valid, so this is an option rather than a requirement.
pub fn optional_item(data: &Option<ResponseData>) -> Option<&Item> {
    match data {
        Some(ResponseData::Item(item)) => Some(item),
        _ => None,
    }
}

/// The count a mutation returned, when it returned one.
pub fn optional_count(data: &Option<ResponseData>) -> Option<u64> {
    match data {
        Some(ResponseData::Count(count)) => Some(*count),
        _ => None,
    }
}

/// A response that must carry a freshly minted pairing.
pub fn expect_pairing(data: Option<ResponseData>) -> Result<PairingData, CliError> {
    match data {
        Some(ResponseData::Pairing(pairing)) => Ok(pairing),
        _ => Err(shape_error("a pairing")),
    }
}

/// A response that must carry a list of peers.
pub fn expect_peers(data: Option<ResponseData>) -> Result<Vec<PeerInfo>, CliError> {
    match data {
        Some(ResponseData::Peers(peers)) => Ok(peers),
        _ => Err(shape_error("a list of peers")),
    }
}

/// A response that must carry cloud sync status.
pub fn expect_cloud_status(data: Option<ResponseData>) -> Result<CloudStatusData, CliError> {
    match data {
        Some(ResponseData::CloudStatus(status)) => Ok(status),
        _ => Err(shape_error("cloud sync status")),
    }
}

/// A response that must carry the counts from one cloud round.
pub fn expect_cloud_sync(data: Option<ResponseData>) -> Result<CloudSyncData, CliError> {
    match data {
        Some(ResponseData::CloudSync(stats)) => Ok(stats),
        _ => Err(shape_error("cloud sync results")),
    }
}

/// A response that must carry per-peer sync results. Empty lists as above.
pub fn expect_sync(data: Option<ResponseData>) -> Result<Vec<SyncResult>, CliError> {
    match data {
        Some(ResponseData::Sync(results)) => Ok(results),
        // `Peers` is the first array-shaped variant, so an *empty* JSON array
        // decodes as one before it ever reaches `Sync`. That is a property of
        // the untagged wire encoding, not a daemon bug, so "no peers to sync
        // with" is accepted here rather than reported as a shape error.
        Some(ResponseData::Peers(peers)) if peers.is_empty() => Ok(Vec::new()),
        _ => Err(shape_error("sync results")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::net::UnixListener;

    /// A stub listener that answers a fixed script of replies, one per
    /// connection. `None` means "accept, read the request, then hang up",
    /// which is what the daemon does on a read timeout.
    ///
    /// This exercises the real codec on both sides, so the framing is covered
    /// without waiting for the daemon crate.
    struct StubDaemon {
        path: PathBuf,
    }

    impl StubDaemon {
        fn start(name: &str, replies: Vec<Option<String>>) -> Self {
            let path = std::env::temp_dir().join(format!(
                "copypaste-cli-test-{}-{name}.sock",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path).expect("bind stub socket");
            tokio::spawn(async move {
                for reply in replies {
                    let Ok((stream, _)) = listener.accept().await else {
                        return;
                    };
                    let mut framed = Framed::new(stream, LinesCodec::new());
                    let Some(Ok(line)) = framed.next().await else {
                        return;
                    };
                    let Some(reply) = reply else { continue };
                    // Echo the request id back, as the protocol requires.
                    let request: Request = serde_json::from_str(&line).expect("valid request");
                    let reply = reply.replace("{id}", &request.id.to_string());
                    let _ = framed.send(reply).await;
                }
            });
            Self { path }
        }
    }

    impl Drop for StubDaemon {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[tokio::test]
    async fn a_full_request_response_round_trip_yields_typed_items() {
        let stub = StubDaemon::start(
            "roundtrip",
            vec![Some(
                r#"{"id":{id},"ok":true,"data":{"items":[{"id":"a","content":"hi","content_type":"text/plain","created_at":5,"pinned":false,"is_sensitive":false}],"skipped_undecryptable":0}}"#
                    .to_string(),
            )],
        );

        let response = request_at(
            &stub.path,
            Method::List {
                limit: 1,
                offset: 0,
            },
        )
        .await
        .expect("round trip");
        let page = expect_page(into_data(response).unwrap()).unwrap();
        assert_eq!(page.items[0].content, "hi");
    }

    #[tokio::test]
    async fn a_missing_socket_is_reported_as_an_unreachable_daemon() {
        let missing = std::env::temp_dir().join("copypaste-cli-test-absent.sock");
        let _ = std::fs::remove_file(&missing);
        let err = request_at(&missing, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_UNREACHABLE);
        assert!(!err.user_message().contains('/'), "{err}");
    }

    #[tokio::test]
    async fn hanging_up_without_answering_is_treated_as_unreachable() {
        let stub = StubDaemon::start("hangup", vec![None]);
        let err = request_at(&stub.path, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_UNREACHABLE);
    }

    #[tokio::test]
    async fn a_reply_for_a_different_request_is_rejected() {
        let stub = StubDaemon::start(
            "idmismatch",
            vec![Some(r#"{"id":999999,"ok":true,"data":{}}"#.to_string())],
        );
        let err = request_at(&stub.path, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("reply for request"), "{err}");
    }

    #[tokio::test]
    async fn not_ready_is_retried_on_a_fresh_connection() {
        let stub = StubDaemon::start(
            "notready",
            vec![
                Some(
                    r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#
                        .to_string(),
                ),
                Some(
                    r#"{"id":{id},"ok":true,"data":{"version":"2.0.0","protocol_version":1,"item_count":0,"capture_running":true,"clipboard_backend":"fake"}}"#
                        .to_string(),
                ),
            ],
        );

        let response = request_at(&stub.path, Method::Status).await.expect("retry");
        let status = expect_status(into_data(response).unwrap()).unwrap();
        assert_eq!(status.item_count, 0);
    }

    #[tokio::test]
    async fn not_ready_gives_up_rather_than_retrying_forever() {
        let reply = r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#
            .to_string();
        let stub = StubDaemon::start(
            "notready-exhausted",
            vec![Some(reply.clone()); NOT_READY_MAX_RETRIES as usize + 1],
        );

        let response = request_at(&stub.path, Method::Status).await.expect("reply");
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("starting up"), "{err}");
    }

    #[test]
    fn requests_serialise_to_the_documented_envelope() {
        let request = Request {
            id: 7,
            protocol_version: PROTOCOL_VERSION,
            method: Method::List {
                limit: 50,
                offset: 0,
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""method":"list""#), "{json}");
        assert!(
            json.contains(r#""params":{"limit":50,"offset":0}"#),
            "{json}"
        );
        assert!(!json.contains('\n'), "a request must be one line: {json}");
    }

    #[test]
    fn unit_methods_serialise_without_params() {
        let json = serde_json::to_string(&Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        })
        .unwrap();
        assert!(json.contains(r#""method":"status""#), "{json}");
    }

    #[test]
    fn pin_carries_the_desired_state() {
        let json = serde_json::to_string(&Method::Pin {
            id: "abc".into(),
            pinned: false,
        })
        .unwrap();
        assert!(json.contains(r#""pinned":false"#), "{json}");
    }

    #[test]
    fn item_pages_deserialise_into_typed_items_and_a_skip_count() {
        let line = r#"{"id":1,"ok":true,"data":{"items":[{"id":"a","content":"hi",
            "content_type":"text/plain","created_at":5,"pinned":true,"is_sensitive":false}],
            "skipped_undecryptable":3}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let page = expect_page(into_data(response).unwrap()).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].content, "hi");
        assert!(page.items[0].pinned);
        assert_eq!(page.skipped_undecryptable, 3, "the skip count was dropped");
    }

    #[test]
    fn an_empty_page_is_not_a_shape_error() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":true,"data":{"items":[],"skipped_undecryptable":0}}"#,
        )
        .unwrap();
        assert!(expect_page(into_data(response).unwrap())
            .unwrap()
            .items
            .is_empty());
    }

    /// The untagged decoder tries variants in order, and an export's payload is
    /// a superset of a page's. `Export` is declared first for exactly this
    /// reason, and this is the test that fails if the order is changed.
    #[test]
    fn an_export_does_not_decode_as_a_page() {
        let line = r#"{"id":1,"ok":true,"data":{"items":[],"skipped_non_text":1,
            "skipped_sensitive":2,"skipped_undecryptable":3}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let export = expect_export(into_data(response).unwrap()).unwrap();
        assert_eq!(
            (
                export.skipped_non_text,
                export.skipped_sensitive,
                export.skipped_undecryptable
            ),
            (1, 2, 3)
        );
    }

    #[test]
    fn discovered_devices_read_back_as_devices() {
        let line = r#"{"id":1,"ok":true,"data":{"devices":[{"pairing_id":"abc","name":"phone",
            "addr":"192.168.1.9:47654","last_seen_ms":5,"paired":false}]}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let found = expect_discovered(into_data(response).unwrap()).unwrap();
        assert_eq!(found.devices.len(), 1);
        assert!(!found.devices[0].paired);
    }

    #[test]
    fn settings_read_back_with_their_restart_list() {
        let line = r#"{"id":1,"ok":true,"data":{"config":{"poll_interval_ms":250,
            "history_limit":10000,"retention_days":0,"dedup_window_secs":60,
            "max_item_bytes":4194304,"sensitive_ttl_secs":0,"excluded_app_bundle_ids":[],
            "lan_visibility":true,"sync_enabled":true},"restart_required":["lan_visibility"]}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let applied = expect_config(into_data(response).unwrap()).unwrap();
        assert_eq!(applied.config.poll_interval_ms, 250);
        assert_eq!(applied.restart_required, ["lan_visibility"]);
    }

    #[test]
    fn status_deserialises_into_typed_status() {
        let line = r#"{"id":1,"ok":true,"data":{"version":"2.0.0","protocol_version":1,
            "item_count":3,"capture_running":true,"clipboard_backend":"macos"}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let status = expect_status(into_data(response).unwrap()).unwrap();
        assert_eq!(status.item_count, 3);
        assert_eq!(status.clipboard_backend, "macos");
    }

    #[test]
    fn a_count_payload_reads_back_as_a_count() {
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":9}"#).unwrap();
        let data = into_data(response).unwrap();
        assert_eq!(optional_count(&data), Some(9));
    }

    #[test]
    fn an_empty_payload_carries_neither_item_nor_count() {
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":{}}"#).unwrap();
        let data = into_data(response).unwrap();
        assert!(optional_item(&data).is_none());
        assert!(optional_count(&data).is_none());
    }

    #[test]
    fn a_failure_response_becomes_a_typed_daemon_error() {
        let line = r#"{"id":1,"ok":false,"error":"no such item","error_code":"not_found"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_NOT_FOUND);
    }

    #[test]
    fn an_untagged_failure_still_becomes_an_error() {
        let line = r#"{"id":1,"ok":false,"error":"unknown method"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
    }

    #[test]
    fn a_wrong_shape_is_reported_rather_than_silently_defaulted() {
        // v1 would have produced an empty list here via unwrap_or_default.
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":{}}"#).unwrap();
        assert!(expect_page(into_data(response).unwrap()).is_err());
    }

    #[test]
    fn a_pairing_payload_reads_back_as_a_pairing() {
        let line = r#"{"id":1,"ok":true,"data":{"code":"ABCD-EFGH",
            "pairing_id":"0123456789abcdef","listen_addr":"192.168.1.24:47654"}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let pairing = expect_pairing(into_data(response).unwrap()).unwrap();
        assert_eq!(pairing.code, "ABCD-EFGH");
        assert_eq!(pairing.listen_addr.as_deref(), Some("192.168.1.24:47654"));
    }

    #[test]
    fn a_peer_list_reads_back_as_peers_and_not_as_items() {
        let line = r#"{"id":1,"ok":true,"data":[{"pairing_id":"abc","name":"phone",
            "last_addr":null,"last_seen_ms":0,"online":true}]}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let peers = expect_peers(into_data(response).unwrap()).unwrap();
        assert_eq!(peers.len(), 1);
        assert!(peers[0].online);
        assert!(peers[0].last_addr.is_none());
    }

    #[test]
    fn sync_results_read_back_as_sync_results() {
        let line = r#"{"id":1,"ok":true,"data":[{"pairing_id":"abc","name":"phone",
            "sent":2,"received":3,"error":null}]}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let results = expect_sync(into_data(response).unwrap()).unwrap();
        assert_eq!(results[0].sent, 2);
        assert_eq!(results[0].received, 3);
        assert!(results[0].error.is_none());
    }

    /// An empty JSON array cannot be told apart from an empty item list in an
    /// untagged union, so "no peers" must not read as a protocol error.
    #[test]
    fn an_empty_list_is_no_peers_and_no_sync_results() {
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":[]}"#).unwrap();
        let data = into_data(response).unwrap();
        assert!(expect_peers(data.clone()).unwrap().is_empty());
        assert!(expect_sync(data).unwrap().is_empty());
    }

    #[test]
    fn a_wrongly_shaped_peer_reply_is_still_reported() {
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":9}"#).unwrap();
        assert!(expect_peers(into_data(response).unwrap()).is_err());
    }

    #[test]
    fn peer_methods_serialise_to_the_documented_envelope() {
        let json = serde_json::to_string(&Method::PairAccept {
            code: "ABCD".into(),
            addr: "127.0.0.1:47654".into(),
        })
        .unwrap();
        assert!(json.contains(r#""method":"pair_accept""#), "{json}");
        assert!(json.contains(r#""code":"ABCD""#), "{json}");

        let json = serde_json::to_string(&Method::SyncNow { pairing_id: None }).unwrap();
        assert!(json.contains(r#""method":"sync_now""#), "{json}");
    }

    #[test]
    fn protocol_mismatch_maps_to_the_version_message() {
        let line = r#"{"id":1,"ok":false,"error":"unsupported protocol version 4",
            "error_code":"protocol_mismatch"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let err = into_data(response).unwrap_err();
        assert!(err.user_message().contains("versions differ"));
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
    }
}
