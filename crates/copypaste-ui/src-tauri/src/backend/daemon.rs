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

use std::sync::atomic::{AtomicU64, Ordering};

use copypaste_ipc::{
    socket_path, Item, Method, PairingData, PeerInfo, Request, Response, ResponseData, StatusData,
    SyncResult, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LinesCodec};

use super::{Backend, BackendError, Result};

/// Ids only have to be unique within this process; the daemon echoes them back
/// so a reply can be matched to its request.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Why `get` refuses. See the method for the full argument.
const MSG_NO_REVEAL: &str =
    "Revealing a hidden item isn't available yet. You can still copy it — the item goes \
     to the clipboard without being shown.";

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
        response.error.as_deref(),
    ))
}

fn expect_items(data: Option<ResponseData>) -> Result<Vec<Item>> {
    match data {
        Some(ResponseData::Items(items)) => Ok(items),
        _ => Err(BackendError::wrong_shape("a list of items")),
    }
}

fn expect_item(data: Option<ResponseData>) -> Result<Item> {
    match data {
        Some(ResponseData::Item(item)) => Ok(item),
        _ => Err(BackendError::wrong_shape("an item")),
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

/// `ResponseData` is `#[serde(untagged)]`, so a bare `Count` deserialises as
/// the first variant that fits. `Count(u64)` is unambiguous among the current
/// variants, but the fallback keeps a reply the daemon sent as `Empty {}` from
/// becoming a shape error — `clear` reporting "0 deleted" is right, and
/// refusing to render a successful clear is not.
fn expect_count(data: Option<ResponseData>) -> Result<u64> {
    match data {
        Some(ResponseData::Count(count)) => Ok(count),
        Some(ResponseData::Empty {}) | None => Ok(0),
        _ => Err(BackendError::wrong_shape("a count")),
    }
}

impl Backend for DaemonBackend {
    async fn list(&self, limit: u32, offset: u32) -> Result<Vec<Item>> {
        expect_items(self.call(Method::List { limit, offset }).await?)
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<Item>> {
        expect_items(
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

    /// Blocked on a daemon method that does not exist yet.
    ///
    /// `get` must read one item **without side effects**, because its only
    /// caller is the reveal gesture — a user asking to *look* at a secret.
    /// `copypaste_ipc::Method` has no such verb: the closest is `Copy`, which
    /// puts the content on the system pasteboard.
    ///
    /// Routing reveal through `Copy` was the first shape of this function and
    /// it is wrong. It would mean that looking at a password silently published
    /// it to every app on the machine that reads the pasteboard — including,
    /// on macOS 16, raising the system's new paste alert — as a side effect of
    /// a gesture that promised only to show it. "Reveal" and "copy" are two
    /// buttons on the row precisely because they are two intentions, and a
    /// clipboard manager quietly collapsing them is a security bug, not a
    /// convenience.
    ///
    /// `Method::List` does return sensitive plaintext, so the bridge could
    /// page history looking for the id. That is O(history), breaks past the
    /// server's 1 000-row clamp, and would put a secret in a response the
    /// caller did not ask for.
    ///
    /// The fix is a `Method::Get { id }` in `copypaste-ipc` plus an arm in the
    /// daemon's dispatcher — small, but in two crates this change does not own.
    /// Until then this refuses, which leaves the reveal button broken and
    /// visibly so. That is the better failure: the alternative leaves it
    /// working and quietly unsafe.
    async fn get(&self, _id: &str) -> Result<Item> {
        Err(BackendError::Unsupported(MSG_NO_REVEAL))
    }

    async fn copy(&self, id: &str) -> Result<Item> {
        expect_item(self.call(Method::Copy { id: id.to_string() }).await?)
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

    async fn sync(&self, pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        expect_sync(
            self.call(Method::SyncNow {
                pairing_id: pairing_id.map(str::to_string),
            })
            .await?,
        )
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
        let items = expect_items(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":[{"id":"a","content":"hi","content_type":"text/plain",
                   "created_at":5,"pinned":true,"is_sensitive":false}]}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(items[0].content, "hi");
        assert!(items[0].pinned);
    }

    #[test]
    fn a_wrong_shape_is_reported_rather_than_silently_defaulted() {
        assert!(
            expect_items(into_data(parse(r#"{"id":1,"ok":true,"data":{}}"#)).unwrap()).is_err()
        );
    }

    #[test]
    fn an_untagged_failure_still_becomes_an_error() {
        assert!(into_data(parse(r#"{"id":1,"ok":false,"error":"unknown method"}"#)).is_err());
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
                r#"{"id":1,"ok":true,"data":[{"pairing_id":"p1","name":"laptop",
                   "last_addr":"10.0.0.2:47654","last_seen_ms":5,"online":true}]}"#,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(peers[0].name, "laptop");
        assert!(peers[0].online);

        let pairing = expect_pairing(
            into_data(parse(
                r#"{"id":1,"ok":true,"data":{"code":"ABC-DEF","pairing_id":"p1","listen_addr":"10.0.0.1:47654"}}"#,
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
                r#"{"id":1,"ok":true,"data":[{"pairing_id":"p1","name":"phone","sent":2,
                   "received":3,"error":null},{"pairing_id":"p2","name":"laptop","sent":0,
                   "received":0,"error":"unreachable"}]}"#,
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
            expect_count(into_data(parse(r#"{"id":1,"ok":true,"data":7}"#)).unwrap()).unwrap(),
            7
        );
        assert_eq!(
            expect_count(into_data(parse(r#"{"id":1,"ok":true,"data":{}}"#)).unwrap()).unwrap(),
            0
        );
    }

    /// Reveal must not be wired to `Method::Copy`. The regression this guards
    /// against is silent: `copy` returns the full item, so routing `get`
    /// through it would look correct in every test that only checks the
    /// returned content — while publishing a secret to the system pasteboard.
    #[tokio::test]
    async fn revealing_refuses_rather_than_copying_a_secret_to_the_clipboard() {
        let err = DaemonBackend::new().get("any-id").await.unwrap_err();
        assert!(
            matches!(err, BackendError::Unsupported(_)),
            "reveal must refuse structurally, got {err:?}"
        );
        // Specifically it must not be `Unreachable`, which is what a real
        // socket call would produce here — that would mean it tried.
        let shown = err.to_string();
        assert!(!shown.contains("copypaste-daemon"), "{shown}");
        assert!(!shown.contains('/'), "{shown}");
    }

    #[test]
    fn the_reveal_refusal_offers_the_safe_alternative() {
        assert!(MSG_NO_REVEAL.contains("copy"));
        assert!(!MSG_NO_REVEAL.contains('/'));
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
