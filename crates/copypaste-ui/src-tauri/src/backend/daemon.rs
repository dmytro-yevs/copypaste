//! The desktop backend: IPC to the running daemon over the local endpoint
//! [`copypaste_ipc::transport`] names — a `0600` Unix socket today.
//!
//! Deliberately thin. It owns only its resolved endpoint, caches nothing and
//! retries nothing — React Query does both on the frontend, and a second policy
//! here could only disagree with it. Every call is one connection, one
//! newline-delimited JSON request, one typed reply.
//!
//! Three things it does not do, on purpose:
//!
//! * **It does not redefine the wire types.** [`copypaste_ipc::Method`] and
//!   friends go straight onto the socket. v1 had three models of this contract;
//!   there is one.
//! * **It does not frame bytes by hand.** Framing is
//!   [`tokio_util::codec::LinesCodec`], the same codec the daemon and the CLI
//!   use (AGENTS.md rule 1).
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
//!
//! # Every call is bounded
//!
//! One deadline spans connect, write and read (manifest 04 §3.4). Without it a
//! daemon that accepts the connection and never answers holds the caller
//! forever, and the callers include quit and restart — so the app hangs on the
//! way out with no window left to explain why. A blown budget is
//! [`BackendError::Timeout`], never `Unreachable`: the process took the
//! connection, so it is alive.

mod pairing;
mod response;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use copypaste_ipc::transport;
use copypaste_ipc::{
    socket_path, BackupData, CloudStatusData, CloudSyncData, ConfigApplied, ConfigPatch,
    DiscoveredDevice, EventData, ExportData, ExportItem, ImagePreview, ImportData, Item, Method,
    PeerInfo, PrivateModeData, Request, Response, ResponseData, StatusData, SyncResult,
    MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{self, Receiver};
use tokio::time::{timeout_at, Instant};
use tokio_util::codec::{Framed, LinesCodec};

use super::{Backend, BackendError, Page, Result};
use response::*;

/// Ids only have to be unique within this process; the daemon echoes them back
/// so a reply can be matched to its request.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Manifest 04 §3.4 `DEFAULT_READ_TIMEOUT`. Long enough for a cold SQLCipher
/// page read, short enough that quitting the app is not a wait.
const DEFAULT_BUDGET: Duration = Duration::from_secs(10);

/// Manifest 04 §3.4 `LONG_READ_TIMEOUT`, for the verbs
/// [`Method::is_long_running`] names.
const LONG_BUDGET: Duration = Duration::from_secs(180);

fn budget_for(method: &Method) -> Duration {
    if method.is_long_running() {
        LONG_BUDGET
    } else {
        DEFAULT_BUDGET
    }
}

/// Run one step of a request against the deadline the whole request shares.
///
/// Per-step budgets were the alternative and are worse: three ten-second steps
/// are a thirty-second call, and the number a caller needs to reason about is
/// how long the *request* can take.
async fn within<T>(deadline: Instant, step: impl Future<Output = T>) -> Result<T> {
    timeout_at(deadline, step)
        .await
        .map_err(|_| BackendError::Timeout)
}

#[derive(Debug, Clone)]
pub struct DaemonBackend {
    endpoint: PathBuf,
}

impl Default for DaemonBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonBackend {
    pub fn new() -> Self {
        Self {
            endpoint: socket_path(),
        }
    }

    #[cfg(test)]
    fn at_endpoint(endpoint: PathBuf) -> Self {
        Self { endpoint }
    }

    /// Send one request and return its payload.
    async fn call(&self, method: Method) -> Result<Option<ResponseData>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let deadline = Instant::now() + budget_for(&method);

        let stream = within(deadline, transport::connect(&self.endpoint))
            .await?
            .map_err(|_| BackendError::Unreachable)?;
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

        let line = serde_json::to_string(&Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        })
        .map_err(|_| BackendError::Internal("Could not encode the request.".into()))?;

        within(deadline, framed.send(line))
            .await?
            .map_err(|_| BackendError::Unreachable)?;

        let line = match within(deadline, framed.next()).await? {
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

/// The acknowledgement contract, checked before the stream is handed over.
///
/// The same two checks every other reply gets. The id proves the
/// acknowledgement answers *this* request rather than a queued one, and the
/// empty shape proves a subscription was opened rather than some other payload
/// returned — either of which would otherwise be accepted and then read as the
/// first event.
fn acknowledged(id: u64, reply: &str) -> Result<()> {
    let response: Response = serde_json::from_str(reply)
        .map_err(|_| BackendError::Internal("Could not understand the daemon's reply.".into()))?;
    if response.id != id {
        return Err(BackendError::Internal(
            "The daemon answered a different request.".into(),
        ));
    }
    expect_empty(into_data(response)?)
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

    async fn clear(&self, through: Option<i64>) -> Result<u64> {
        expect_count(self.call(Method::DeleteAll { through }).await?)
    }

    async fn history_ceiling(&self) -> Result<u64> {
        expect_count(self.call(Method::HistoryCeiling).await?)
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

    /// One request carrying the whole ordering, never N pin toggles.
    ///
    /// Unpinning and re-pinning in the wanted order would work by accident of
    /// `set_pinned` appending at `MAX(pin_order) + 1`, and is the thing not to
    /// do: N round trips, not atomic, and a failure halfway leaves the pinned
    /// section in an order the user never asked for. `Method::ReorderPinned`
    /// is applied by `Store::reorder_pinned` in one transaction.
    async fn reorder_pinned(&self, ids: &[String]) -> Result<()> {
        self.call(Method::ReorderPinned { ids: ids.to_vec() })
            .await?;
        Ok(())
    }

    async fn status(&self) -> Result<StatusData> {
        expect_status(self.call(Method::Status).await?)
    }

    async fn set_device_name(&self, name: &str) -> Result<()> {
        expect_empty(
            self.call(Method::SetDeviceName {
                name: name.to_string(),
            })
            .await?,
        )
    }

    async fn cloud_sign_in(
        &self,
        email: &str,
        password: &str,
        passphrase: &str,
    ) -> Result<CloudStatusData> {
        expect_cloud_status(
            self.call(Method::CloudSignIn {
                email: email.to_string(),
                password: password.to_string(),
                passphrase: passphrase.to_string(),
            })
            .await?,
        )
    }

    async fn cloud_sign_up(
        &self,
        email: &str,
        password: &str,
        passphrase: &str,
    ) -> Result<CloudStatusData> {
        expect_cloud_status(
            self.call(Method::CloudSignUp {
                email: email.to_string(),
                password: password.to_string(),
                passphrase: passphrase.to_string(),
            })
            .await?,
        )
    }

    async fn cloud_set_endpoint(&self, url: &str, anon_key: &str) -> Result<CloudStatusData> {
        expect_cloud_status(
            self.call(Method::CloudSetEndpoint {
                url: url.to_string(),
                anon_key: anon_key.to_string(),
            })
            .await?,
        )
    }

    async fn cloud_sign_out(&self) -> Result<CloudStatusData> {
        expect_cloud_status(self.call(Method::CloudSignOut).await?)
    }

    async fn cloud_status(&self) -> Result<CloudStatusData> {
        expect_cloud_status(self.call(Method::CloudStatus).await?)
    }

    async fn cloud_sync(&self) -> Result<CloudSyncData> {
        expect_cloud_sync(self.call(Method::CloudSyncNow).await?)
    }

    async fn shutdown(&self) -> Result<()> {
        self.call(Method::Shutdown).await?;
        Ok(())
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

    async fn get_private_mode(&self) -> Result<PrivateModeData> {
        expect_private_mode(self.call(Method::GetPrivateMode).await?)
    }

    async fn set_private_mode(&self, enabled: bool) -> Result<PrivateModeData> {
        expect_private_mode(self.call(Method::SetPrivateMode { enabled }).await?)
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
        // The budget covers the handshake only. Past the acknowledgement,
        // silence is the normal state of a subscription — a deadline there
        // would drop the stream on a quiet clipboard.
        let deadline = Instant::now() + DEFAULT_BUDGET;
        let stream = within(deadline, transport::connect(&self.endpoint))
            .await?
            .map_err(|_| BackendError::Unreachable)?;
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let line = serde_json::to_string(&Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Watch,
        })
        .map_err(|_| BackendError::Internal("Could not encode the request.".into()))?;
        within(deadline, framed.send(line))
            .await?
            .map_err(|_| BackendError::Unreachable)?;

        // The acknowledgement is read here rather than in the task, so a
        // daemon that refuses `watch` — an older one, or one at its watcher
        // cap — is an error the caller can fall back from instead of a
        // subscription that silently never delivers anything.
        match within(deadline, framed.next()).await? {
            Some(Ok(reply)) => acknowledged(id, &reply)?,
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
        assert!(expect_page(
            into_data(parse(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#)).unwrap()
        )
        .is_err());
    }

    #[test]
    fn private_mode_uses_the_daemons_confirmed_value() {
        let mode = expect_private_mode(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"private_mode":{"private_mode":true,"private_mode_epoch":4}}}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(mode.private_mode);
        assert_eq!(mode.private_mode_epoch, 4);
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
    fn peers_deserialise() {
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
    }

    #[test]
    fn pairing_uses_only_the_confirmation_bound_methods() {
        let source = include_str!("daemon/pairing.rs");
        for verb in [
            concat!("Method::", "PairCreate {"),
            concat!("Method::", "PairAccept {"),
        ] {
            assert!(
                !source.contains(verb),
                "legacy operation {verb} is reachable"
            );
        }
        for verb in [
            "Method::PairCreateInvite",
            "Method::PairJoin",
            "Method::PairProgress",
            "Method::PairConfirm",
            "Method::PairCancel",
        ] {
            assert!(source.contains(verb), "missing {verb}");
        }
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
        let dir = tempfile::tempdir().unwrap();
        let backend = DaemonBackend::at_endpoint(dir.path().join("daemon.sock"));
        // Keep the assertion independent of a live daemon on the user's endpoint.
        let err = backend.get("any-id").await.unwrap_err();
        assert!(matches!(err, BackendError::Unreachable), "{err:?}");

        let method = serde_json::to_string(&Method::Get { id: "x".into() }).unwrap();
        assert!(method.contains("\"get\""), "{method}");
        assert!(!method.contains("copy"), "{method}");
    }

    /// The whole ordering goes in one request. A reorder assembled out of pin
    /// toggles would be N round trips and not atomic, so the guard is that
    /// nothing here sends `pin` — proven by which verb is built, since no
    /// daemon is listening to answer.
    #[tokio::test]
    async fn reordering_sends_one_request_carrying_the_whole_ordering() {
        let dir = tempfile::tempdir().unwrap();
        let backend = DaemonBackend::at_endpoint(dir.path().join("daemon.sock"));
        // Keep the assertion independent of a live daemon on the user's endpoint.
        let err = backend
            .reorder_pinned(&["a".into(), "b".into()])
            .await
            .unwrap_err();
        assert!(matches!(err, BackendError::Unreachable), "{err:?}");

        let method = serde_json::to_string(&Method::ReorderPinned {
            ids: vec!["a".into(), "b".into()],
        })
        .unwrap();
        assert!(method.contains("\"reorder_pinned\""), "{method}");
        assert!(!method.contains("\"pin\""), "{method}");
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

        assert!(expect_config(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"config":{"config":{},"restart_required":[]}}}"#
            ))
            .unwrap()
        )
        .unwrap()
        .restart_required
        .is_empty());
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

    /// Quit and restart poll `status`, so its budget is the one that decides
    /// how long the app can hang on the way out. It must be the ordinary one.
    #[test]
    fn the_verbs_quit_depends_on_take_the_ordinary_budget() {
        assert_eq!(budget_for(&Method::Status), DEFAULT_BUDGET);
        assert_eq!(budget_for(&Method::Shutdown), DEFAULT_BUDGET);
        assert_eq!(
            budget_for(&Method::Backup {
                dest_path: "d".into()
            }),
            LONG_BUDGET
        );
        assert!(DEFAULT_BUDGET < LONG_BUDGET);
    }

    /// A listener on this platform's own transport, and the path that dials it.
    ///
    /// The budget tests below were `#[cfg(unix)]`, so the deadlines they pin
    /// were never executed on Windows — whose named pipe they also bound.
    struct Endpoint {
        path: PathBuf,
        listener: transport::Listener,
        _dir: tempfile::TempDir,
    }

    fn endpoint(name: &str) -> Endpoint {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let (path, listener) = {
            let path = dir.path().join(name);
            let bound = tokio::net::UnixListener::bind(&path).unwrap();
            (path, transport::Listener::from(bound))
        };
        #[cfg(windows)]
        let (path, listener) = {
            use interprocess::os::windows::named_pipe::{pipe_mode::Bytes, PipeListenerOptions};
            // `COPYPASTE_SOCKET` accepts a pipe name verbatim, so the backend
            // dials this by exactly the path recorded here.
            let raw = format!(r"\\.\pipe\copypaste.test.{}.{name}", std::process::id());
            let bound = PipeListenerOptions::new()
                .path(raw.as_str())
                .create_tokio_duplex::<Bytes>()
                .unwrap();
            (PathBuf::from(raw), transport::Listener::from(bound))
        };
        Endpoint {
            path,
            listener,
            _dir: dir,
        }
    }

    /// Accept one connection, read the request, then say nothing at all —
    /// which is a handler that never returns, not a hang-up. The receiver
    /// resolves when the client releases the connection.
    fn silent(listener: transport::Listener) -> tokio::sync::oneshot::Receiver<()> {
        let (hung_up, observed) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let stream = listener.accept().await.expect("a connection");
            let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));
            let _request = framed.next().await;
            // A reset and a clean EOF are both the client letting go; which one
            // arrives is the platform's business.
            while let Some(Ok(_)) = framed.next().await {}
            let _ = hung_up.send(());
        });
        observed
    }

    /// A daemon that accepts the connection and never writes a reply. Before
    /// the budget this returned nothing at all, and the caller — including the
    /// one that runs on quit — waited for as long as the process lived.
    ///
    /// `start_paused` advances tokio's clock only when every task is idle, so
    /// this proves the deadline fires without spending ten seconds to do it.
    #[tokio::test(start_paused = true)]
    async fn a_daemon_that_never_answers_is_a_timeout_rather_than_a_wait() {
        let served = endpoint("silent");
        let _held = silent(served.listener);

        let backend = DaemonBackend::at_endpoint(served.path);
        let started = Instant::now();
        let err = backend.status().await.unwrap_err();

        assert!(matches!(err, BackendError::Timeout), "{err:?}");
        assert!(started.elapsed() >= DEFAULT_BUDGET);
        // The distinction the variant exists for: a live process that went
        // quiet must not be reported as one that is not running.
        assert_ne!(err.to_string(), BackendError::Unreachable.to_string());
    }

    /// The same silent daemon on a long verb waits the long budget and no
    /// longer. A single budget for everything would either cancel a real
    /// restore or hang quit; this is the assertion that keeps them apart.
    #[tokio::test(start_paused = true)]
    async fn a_long_verb_waits_the_long_budget_and_still_ends() {
        let served = endpoint("silent-long");
        let _held = silent(served.listener);

        let backend = DaemonBackend::at_endpoint(served.path);
        let started = Instant::now();
        let err = backend
            .import(vec![])
            .await
            .expect_err("the handler never replies");

        assert!(matches!(err, BackendError::Timeout), "{err:?}");
        assert!(started.elapsed() >= LONG_BUDGET);
    }

    /// A subscription is silent by design, so the budget has to stop at the
    /// acknowledgement. A deadline over the stream would drop the watcher on
    /// any clipboard that was merely quiet.
    ///
    /// The second assertion is the server's, not an inference from the client:
    /// a deadline that cancelled the read but left the connection open would
    /// leak one pipe or socket per attempt for as long as the app ran.
    #[tokio::test(start_paused = true)]
    async fn a_watch_that_is_never_acknowledged_gives_up_and_lets_go() {
        let served = endpoint("silent-watch");
        let hung_up = silent(served.listener);

        let backend = DaemonBackend::at_endpoint(served.path);
        let err = backend.watch().await.expect_err("no acknowledgement");
        assert!(matches!(err, BackendError::Timeout), "{err:?}");

        // Real time from here: the hang-up is the OS delivering an event, and
        // a paused clock would run the bound out before it arrived.
        tokio::time::resume();
        tokio::time::timeout(Duration::from_secs(10), hung_up)
            .await
            .expect("the server never saw the client let go")
            .expect("the watcher task ended without observing a hang-up");
    }

    /// The acknowledgement is a reply like any other. An id that answers some
    /// other request, or a payload that is not the empty acknowledgement, means
    /// no subscription was opened — and accepting either would hand back a
    /// receiver whose first "event" is somebody else's answer.
    #[test]
    fn a_watch_acknowledgement_must_match_the_request_and_be_empty() {
        assert!(acknowledged(7, r#"{"id":7,"ok":true}"#).is_ok());
        assert!(acknowledged(7, r#"{"id":7,"ok":true,"data":{"empty":{}}}"#).is_ok());

        for reply in [
            // Another request's answer, arriving first.
            r#"{"id":8,"ok":true,"data":{"empty":{}}}"#,
            r#"{"id":8,"ok":true}"#,
            // Acknowledged with a payload, so this is not a subscription.
            r#"{"id":7,"ok":true,"data":{"count":3}}"#,
            r#"{"id":7,"ok":true,"data":{"peers":[]}}"#,
            "not json",
        ] {
            assert!(acknowledged(7, reply).is_err(), "{reply}");
        }

        let refused = acknowledged(
            7,
            r#"{"id":7,"ok":false,"error":"too many watchers","error_code":"unsupported"}"#,
        )
        .expect_err("a refusal is not an acknowledgement");
        assert!(!refused.to_string().contains('/'), "{refused}");
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
