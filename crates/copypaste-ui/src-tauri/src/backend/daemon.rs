//! The desktop backend: IPC to the running daemon over its `0600` Unix socket.
//!
//! Deliberately thin. It owns no state, caches nothing and retries nothing —
//! React Query does all three on the frontend, and a second policy here could
//! only disagree with it. Every call is one connection, one newline-delimited
//! JSON request, one typed reply.
//!
//! Three things it does not do, on purpose:
//!
//! * **It does not redefine the wire types.** [`copypaste_ipc::Method`] and
//!   friends go straight onto the socket. v1 had three models of this contract;
//!   there is one.
//! * **It does not frame bytes by hand.** Framing is
//!   [`tokio_util::codec::LinesCodec`], the same codec the daemon and the CLI
//!   use (CLAUDE.md rule 1).
//! * **It never puts a path in an error.** Failures become
//!   [`BackendError`], which scrubs on the way in.
//!
//! # Why it connects per call
//!
//! The daemon may not be running when the UI starts, may be restarted under it,
//! and may be upgraded mid-session. A held-open socket would buy one connect
//! per request and cost a reconnect state machine, a liveness probe and a
//! decision about what to do with in-flight requests when the peer goes away.
//! Connecting per call makes "the daemon went away" the same code path as "the
//! daemon was never there", which is also what the user sees.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use copypaste_ipc::{
    BackupData, ConfigApplied, ConfigPatch, DiscoveredDevice, EventData, ExportData, ExportItem,
    ImagePreview, ImportData, Item, MAX_FRAME_BYTES, Method, PROTOCOL_VERSION, PairingData,
    PeerInfo, Request, Response, ResponseData, StatusData, SyncResult, socket_path,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{self, Receiver};
use tokio_util::codec::{Framed, LinesCodec};

use super::{Backend, BackendError, Page, Result};

/// Ids only have to be unique within this process; the daemon echoes them back
/// so a reply can be matched to its request.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Why `reorder_pinned` refuses. See the method for the full argument.
const MSG_NO_REORDER: &str = "Reordering pinned items isn't available yet. Pinned items keep the order they \
     were pinned in.";

/// Talks to the daemon. Holds nothing, so it is trivially `Send + Sync`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DaemonBackend;

impl DaemonBackend {
    pub fn new() -> Self {
        Self
    }

    /// Send one request and return its payload.
    async fn call(&self, method: Method) -> Result<Option<ResponseData>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        let stream = UnixStream::connect(socket_path())
            .await
            .map_err(|_| BackendError::Unreachable)?;
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

        let line = serde_json::to_string(&Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        })
        .map_err(|_| BackendError::Internal("Could not encode the request.".into()))?;

        framed
            .send(line)
            .await
            .map_err(|_| BackendError::Unreachable)?;

        let line = match framed.next().await {
            Some(Ok(line)) => line,
            // A hang-up with no reply is the daemon's read-timeout behaviour
            // and is indistinguishable from it having gone away.
            Some(Err(_)) | None => return Err(BackendError::Unreachable),
        };

        let response: Response = serde_json::from_str(&line).map_err(|_| {
            BackendError::Internal("Could not understand the daemon's reply.".into())
        })?;

        if response.id != id {
            return Err(BackendError::Internal(
                "The daemon answered a different request.".into(),
            ));
        }

        into_data(response)
    }
}

/// Split a reply into payload or failure.
///
/// Branches on `error_code`, never on the `error` string (manifest 04, I9).
pub(super) fn into_data(response: Response) -> Result<Option<ResponseData>> {
    if response.ok {
        return Ok(response.data);
    }
    Err(BackendError::from_code(
        response.error_code,
        response.raw_error_code.as_deref(),
        response.error.as_deref(),
    ))
}

fn expect_page(data: Option<ResponseData>) -> Result<Page> {
    match data {
        Some(ResponseData::Page(page)) => Ok(Page::from(page)),
        _ => Err(BackendError::wrong_shape("a page of items")),
    }
}

fn expect_discovered(data: Option<ResponseData>) -> Result<Vec<DiscoveredDevice>> {
    match data {
        Some(ResponseData::Discovered(found)) => Ok(found.devices),
        _ => Err(BackendError::wrong_shape("a list of nearby devices")),
    }
}

fn expect_item(data: Option<ResponseData>) -> Result<Item> {
    match data {
        Some(ResponseData::Item(item)) => Ok(item),
        _ => Err(BackendError::wrong_shape("an item")),
    }
}

fn expect_image_preview(data: Option<ResponseData>) -> Result<ImagePreview> {
    match data {
        Some(ResponseData::ImagePreview(preview)) => Ok(preview),
        _ => Err(BackendError::wrong_shape("an image preview")),
    }
}

fn expect_status(data: Option<ResponseData>) -> Result<StatusData> {
    match data {
        Some(ResponseData::Status(status)) => Ok(status),
        _ => Err(BackendError::wrong_shape("daemon status")),
    }
}

fn expect_pairing(data: Option<ResponseData>) -> Result<PairingData> {
    match data {
        Some(ResponseData::Pairing(pairing)) => Ok(pairing),
        _ => Err(BackendError::wrong_shape("a pairing code")),
    }
}

fn expect_peers(data: Option<ResponseData>) -> Result<Vec<PeerInfo>> {
    match data {
        Some(ResponseData::Peers(peers)) => Ok(peers),
        _ => Err(BackendError::wrong_shape("a list of devices")),
    }
}

fn expect_sync(data: Option<ResponseData>) -> Result<Vec<SyncResult>> {
    match data {
        Some(ResponseData::Sync(results)) => Ok(results),
        _ => Err(BackendError::wrong_shape("a sync report")),
    }
}

fn expect_config(data: Option<ResponseData>) -> Result<ConfigApplied> {
    match data {
        Some(ResponseData::Config(applied)) => Ok(applied),
        _ => Err(BackendError::wrong_shape("the service's settings")),
    }
}

fn expect_export(data: Option<ResponseData>) -> Result<ExportData> {
    match data {
        Some(ResponseData::Export(export)) => Ok(export),
        _ => Err(BackendError::wrong_shape("an export")),
    }
}

fn expect_import(data: Option<ResponseData>) -> Result<ImportData> {
    match data {
        Some(ResponseData::Import(result)) => Ok(result),
        _ => Err(BackendError::wrong_shape("an import report")),
    }
}

fn expect_backup(data: Option<ResponseData>) -> Result<BackupData> {
    match data {
        Some(ResponseData::Backup(backup)) => Ok(backup),
        _ => Err(BackendError::wrong_shape("a backup report")),
    }
}

/// The fallback keeps an acknowledged clear from becoming a shape error:
/// `clear` reporting "0 deleted" is right when the daemon omitted a count.
fn expect_count(data: Option<ResponseData>) -> Result<u64> {
    match data {
        Some(ResponseData::Count(count)) => Ok(count),
        Some(ResponseData::Empty {}) | None => Ok(0),
        _ => Err(BackendError::wrong_shape("a count")),
    }
}

impl Backend for DaemonBackend {
    async fn list(&self, limit: u32, cursor: Option<&str>) -> Result<Page> {
        expect_page(
            self.call(Method::List {
                limit,
                cursor: cursor.map(str::to_string),
            })
            .await?,
        )
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Page> {
        expect_page(
            self.call(Method::Search {
                query: query.to_string(),
                limit,
            })
            .await?,
        )
    }

    async fn add(&self, content: &str) -> Result<Item> {
        expect_item(
            self.call(Method::Add {
                content: content.to_string(),
            })
            .await?,
        )
    }

    /// `Method::Get`, never `Method::Copy`.
    ///
    /// Its only caller is the reveal gesture — a user asking to *look* at a
    /// secret. Routing that through `Copy` would publish the password to every
    /// app on the machine that reads the pasteboard, as a side effect of a
    /// gesture that promised only to show it.
    async fn get(&self, id: &str) -> Result<Item> {
        expect_item(self.call(Method::Get { id: id.to_string() }).await?)
    }

    async fn image_preview(&self, id: &str) -> Result<ImagePreview> {
        expect_image_preview(
            self.call(Method::ImagePreview { id: id.to_string() })
                .await?,
        )
    }

    async fn copy(&self, id: &str) -> Result<Item> {
        expect_item(self.call(Method::Copy { id: id.to_string() }).await?)
    }

    async fn copy_as_plain_text(&self, id: &str) -> Result<Item> {
        expect_item(
            self.call(Method::CopyPlainText { id: id.to_string() })
                .await?,
        )
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.call(Method::Delete { id: id.to_string() }).await?;
        Ok(())
    }

    async fn clear(&self) -> Result<u64> {
        expect_count(self.call(Method::DeleteAll).await?)
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<Item> {
        expect_item(
            self.call(Method::Pin {
                id: id.to_string(),
                pinned,
            })
            .await?,
        )
    }

    /// Blocked on a wire verb that does not exist.
    ///
    /// `copypaste_ipc::Method` has no reorder, so there is nothing to send.
    /// The alternative — unpin and re-pin every item in the wanted order —
    /// would work by accident of `set_pinned` appending at `MAX(pin_order) + 1`
    /// and is not worth having: it is N round trips, it is not atomic, and a
    /// failure halfway leaves the pinned section in an order the user never
    /// asked for. Refusing keeps the gap visible (parity finding 19).
    async fn reorder_pinned(&self, _ids: &[String]) -> Result<()> {
        Err(BackendError::Unsupported(MSG_NO_REORDER))
    }

    async fn status(&self) -> Result<StatusData> {
        expect_status(self.call(Method::Status).await?)
    }

    async fn pair_create(&self, name: &str) -> Result<PairingData> {
        expect_pairing(
            self.call(Method::PairCreate {
                name: name.to_string(),
            })
            .await?,
        )
    }

    async fn pair_accept(&self, code: &str, addr: &str) -> Result<Vec<PeerInfo>> {
        expect_peers(
            self.call(Method::PairAccept {
                code: code.to_string(),
                addr: addr.to_string(),
            })
            .await?,
        )
    }

    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        expect_peers(self.call(Method::Peers).await?)
    }

    async fn unpair(&self, pairing_id: &str) -> Result<()> {
        self.call(Method::Unpair {
            pairing_id: pairing_id.to_string(),
        })
        .await?;
        Ok(())
    }

    async fn revoke(&self, pairing_id: &str) -> Result<()> {
        self.call(Method::Revoke {
            pairing_id: pairing_id.to_string(),
        })
        .await?;
        Ok(())
    }

    async fn sync(&self, pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        expect_sync(
            self.call(Method::SyncNow {
                pairing_id: pairing_id.map(str::to_string),
            })
            .await?,
        )
    }

    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>> {
        expect_discovered(self.call(Method::Discovered).await?)
    }

    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>> {
        expect_discovered(self.call(Method::Rescan).await?)
    }

    async fn get_config(&self) -> Result<ConfigApplied> {
        expect_config(self.call(Method::GetConfig).await?)
    }

    async fn set_config(&self, patch: ConfigPatch) -> Result<ConfigApplied> {
        expect_config(self.call(Method::SetConfig { patch }).await?)
    }

    async fn export(&self, limit: u32, include_sensitive: bool) -> Result<ExportData> {
        expect_export(
            self.call(Method::Export {
                limit,
                include_sensitive,
            })
            .await?,
        )
    }

    async fn import(&self, items: Vec<ExportItem>) -> Result<ImportData> {
        expect_import(self.call(Method::Import { items }).await?)
    }

    /// `to_string_lossy` rather than a refusal on non-UTF-8.
    ///
    /// The path came from the platform's own save panel, so a byte sequence
    /// this cannot render is a path the user picked in a file manager that
    /// could. The daemon does the real check — it refuses an empty or
    /// non-absolute path, and refuses to overwrite — and a mangled name fails
    /// there with a sentence rather than here with a silent nothing.
    async fn backup(&self, dest: &Path) -> Result<BackupData> {
        expect_backup(
            self.call(Method::Backup {
                dest_path: dest.to_string_lossy().into_owned(),
            })
            .await?,
        )
    }

    /// `confirm: true` unconditionally.
    ///
    /// The wire flag exists so a scripted `copypaste restore` cannot replace a
    /// history by accident. Here the confirmation has already happened — the
    /// user answered a dialog naming what is about to be lost — so passing
    /// `false` would only produce a second refusal with nothing left to ask.
    async fn restore(&self, src: &Path) -> Result<()> {
        self.call(Method::Restore {
            src_path: src.to_string_lossy().into_owned(),
            confirm: true,
        })
        .await?;
        Ok(())
    }

    /// The one call that keeps its connection.
    ///
    /// Every other method connects, asks and hangs up — see the module docs.
    /// This one cannot: `Method::Watch` turns the connection *into* the
    /// subscription, so the socket is the thing being held. The reader task
    /// owns it and ends when the receiver is dropped, which is what makes the
    /// lifetime the caller's rather than a background task's.
    async fn watch(&self) -> Result<Receiver<EventData>> {
        let stream = UnixStream::connect(socket_path())
            .await
            .map_err(|_| BackendError::Unreachable)?;
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let line = serde_json::to_string(&Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Watch,
        })
        .map_err(|_| BackendError::Internal("Could not encode the request.".into()))?;
        framed
            .send(line)
            .await
            .map_err(|_| BackendError::Unreachable)?;

        // The acknowledgement is read here rather than in the task, so a
        // daemon that refuses `watch` — an older one, or one at its watcher
        // cap — is an error the caller can fall back from instead of a
        // subscription that silently never delivers anything.
        match framed.next().await {
            Some(Ok(reply)) => {
                let response: Response = serde_json::from_str(&reply).map_err(|_| {
                    BackendError::Internal("Could not understand the daemon\'s reply.".into())
                })?;
                into_data(response)?;
            }
            Some(Err(_)) | None => return Err(BackendError::Unreachable),
        }

        // Depth 1: events say only *that* something changed, so a backlog of
        // them is one event repeated. A full channel means the consumer is
        // still handling the last change, and the next poll or the next event
        // covers whatever it missed.
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            while let Some(Ok(line)) = framed.next().await {
                let Ok(response) = serde_json::from_str::<Response>(&line) else {
                    continue;
                };
                if let Ok(Some(ResponseData::Event(event))) = into_data(response) {
                    // `send` failing means the receiver was dropped: the
                    // screen went away, so hang up rather than hold the socket.
                    if tx.send(event).await.is_err() {
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Response {
        serde_json::from_str(json).expect("valid response JSON")
    }

    #[test]
    fn a_daemon_supplied_path_never_reaches_the_frontend() {
        let err = into_data(parse(
            r#"{"id":1,"ok":false,"error":"could not open /Users/dmitriy/Library/x.db","error_code":"internal"}"#,
        ))
        .unwrap_err();
        let shown = err.to_string();
        assert!(!shown.contains("dmitriy"), "{shown}");
        assert!(shown.contains("<path>"), "{shown}");
    }

    #[test]
    fn items_deserialise_into_the_shared_wire_type() {
        let page = expect_page(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"page":{"items":[{"id":"a","content":"hi",
                   "content_type":"text/plain","created_at":5,"pinned":true,
                   "is_sensitive":false}],"skipped_undecryptable":0}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(page.items[0].content, "hi");
        assert!(page.items[0].pinned);
    }

    #[test]
    fn a_wrong_shape_is_reported_rather_than_silently_defaulted() {
        assert!(
            expect_page(into_data(parse(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#)).unwrap())
                .is_err()
        );
    }

    #[test]
    fn an_untagged_failure_still_becomes_an_error() {
        assert!(into_data(parse(r#"{"id":1,"ok":false,"error":"unknown method"}"#)).is_err());
    }

    #[test]
    fn an_unknown_error_code_survives_the_daemon_boundary() {
        let error = into_data(parse(
            r#"{"id":1,"ok":false,"error":"new refusal","error_code":"future_daemon_state"}"#,
        ))
        .unwrap_err();
        assert_eq!(error.ui_error().code, "future_daemon_state");
        assert_eq!(error.to_string(), "new refusal");
    }

    #[test]
    fn not_ready_reads_as_still_starting_up() {
        let err = into_data(parse(
            r#"{"id":1,"ok":false,"error":"still starting","error_code":"not_ready"}"#,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("starting up"), "{err}");
    }

    #[test]
    fn peers_and_pairings_deserialise() {
        let peers = expect_peers(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"peers":[{"pairing_id":"p1","name":"laptop",
                   "last_addr":"10.0.0.2:47654","last_seen_ms":5,"online":true}]}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(peers[0].name, "laptop");
        assert!(peers[0].online);

        let pairing = expect_pairing(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"pairing":{"code":"ABC-DEF","pairing_id":"p1","listen_addr":"10.0.0.1:47654"}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(pairing.code, "ABC-DEF");
    }

    #[test]
    fn a_sync_report_deserialises_including_a_per_peer_failure() {
        let results = expect_sync(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"sync":[{"pairing_id":"p1","name":"phone","sent":2,
                   "received":3,"error":null},{"pairing_id":"p2","name":"laptop","sent":0,
                   "received":0,"error":"unreachable"}]}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].received, 3);
        assert!(results[1].error.is_some());
    }

    /// `clear` must render a successful empty clear rather than a shape error.
    #[test]
    fn clear_accepts_both_a_count_and_an_empty_reply() {
        assert_eq!(
            expect_count(into_data(parse(r#"{"id":1,"ok":true,"data":{"count":7}}"#)).unwrap())
                .unwrap(),
            7
        );
        assert_eq!(
            expect_count(into_data(parse(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#)).unwrap())
                .unwrap(),
            0
        );
    }

    /// Reveal must not be wired to `Method::Copy`. The regression this guards
    /// against is silent: `copy` returns the full item, so routing `get`
    /// through it would look correct in every test that only checks the
    /// returned content — while publishing a secret to the system pasteboard.
    #[tokio::test]
    async fn revealing_asks_for_the_item_and_never_puts_it_on_the_clipboard() {
        // No daemon is listening here, so the assertion is about which request
        // was built rather than about the reply: `Unreachable` proves it tried
        // to send something, and the only send path `get` has is `Method::Get`.
        let err = DaemonBackend::new().get("any-id").await.unwrap_err();
        assert!(matches!(err, BackendError::Unreachable), "{err:?}");

        let method = serde_json::to_string(&Method::Get { id: "x".into() }).unwrap();
        assert!(method.contains("\"get\""), "{method}");
        assert!(!method.contains("copy"), "{method}");
    }

    /// Refusing has to leave the user something true to do, and must not read
    /// as a transient failure they could retry away.
    #[tokio::test]
    async fn reordering_refuses_structurally_rather_than_faking_it() {
        let err = DaemonBackend::new()
            .reorder_pinned(&["a".into(), "b".into()])
            .await
            .unwrap_err();
        assert!(matches!(err, BackendError::Unsupported(_)), "{err:?}");
        assert!(!MSG_NO_REORDER.contains('/'));
    }

    /// Parity finding 17: a page that dropped rows has to say how many, or a
    /// short page and a small history are the same thing to the user.
    #[test]
    fn a_page_carries_the_count_of_rows_that_would_not_open() {
        let page = expect_page(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"page":{"items":[],"skipped_undecryptable":3}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.skipped_undecryptable, 3);
    }

    /// The page wrapper must remain distinct from an empty acknowledgement.
    #[test]
    fn a_page_does_not_decode_as_the_empty_variant() {
        let data = into_data(parse(
            r#"{"id":1,"ok":true,"data":{"page":{"items":[],"skipped_undecryptable":0}}}"#,
        ))
        .unwrap();
        assert!(matches!(data, Some(ResponseData::Page(_))), "{data:?}");
    }

    /// `restart_required` is what lets a Settings screen say "this one needs a
    /// restart" at the moment of the change. It has to survive the decode, and
    /// an empty list has to stay an empty list rather than becoming a shape
    /// error.
    #[test]
    fn settings_decode_with_the_fields_that_are_waiting_on_a_restart() {
        let applied = expect_config(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"config":{"config":{"poll_interval_ms":250},
                   "restart_required":["lan_visibility"]}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(applied.config.poll_interval_ms, 250);
        // Absent keys take their defaults, so a daemon on a newer build does
        // not blank the screen.
        assert_eq!(
            applied.config.history_limit,
            copypaste_ipc::ConfigData::default().history_limit
        );
        assert_eq!(applied.restart_required, ["lan_visibility"]);

        assert!(
            expect_config(
                into_data(parse(
                    r#"{"id":1,"ok":true,"data":{"config":{"config":{},"restart_required":[]}}}"#
                ))
                .unwrap()
            )
            .unwrap()
            .restart_required
            .is_empty()
        );
    }

    /// The export wrapper keeps its item array distinct from a history page.
    #[test]
    fn an_export_does_not_decode_as_a_page() {
        let export = expect_export(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"export":{"items":[{"content":"hi","content_type":"text/plain",
                   "created_at":5,"pinned":false,"is_sensitive":false}],"skipped_non_text":1,
                   "skipped_sensitive":2,"skipped_undecryptable":3}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(export.items.len(), 1);
        assert_eq!(export.skipped_sensitive, 2);
    }

    #[test]
    fn an_import_report_and_a_backup_report_decode_as_themselves() {
        let imported = expect_import(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"import":{"inserted":7,"skipped":2}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!((imported.inserted, imported.skipped), (7, 2));

        assert_eq!(
            expect_backup(
                into_data(parse(
                    r#"{"id":1,"ok":true,"data":{"backup":{"size_bytes":4096}}}"#
                ))
                .unwrap()
            )
            .unwrap()
            .size_bytes,
            4096
        );
    }

    /// A restore is the one call whose success carries no payload. It must read
    /// as done rather than as a reply of the wrong shape.
    #[test]
    fn a_restore_succeeds_on_an_empty_reply() {
        assert!(into_data(parse(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#)).is_ok());
    }

    #[test]
    fn a_not_found_code_becomes_a_not_found_error() {
        let err = into_data(parse(
            r#"{"id":1,"ok":false,"error":"no such item","error_code":"not_found"}"#,
        ))
        .unwrap_err();
        assert!(matches!(err, BackendError::NotFound(_)));
    }
}
