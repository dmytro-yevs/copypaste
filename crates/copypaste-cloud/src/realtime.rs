//! Supabase Realtime: a second device learns about a new row without polling.
//!
//! # Realtime is an accelerator, never the source of truth
//!
//! Read this before using anything in this module. Supabase Realtime, like the
//! v1 relay's SSE channel before it, is **at-most-once**: events that occur
//! while the socket is down are not replayed when it comes back. Manifest 05
//! §5.1 row 9a calls this "the single most important item in the table", and
//! §4.8 states the rule directly — the cursor poll in [`crate::sync`] is the
//! correctness mechanism, and this module only shortens the latency between a
//! write on one device and its appearance on another.
//!
//! Deleting the poll loop "because we have Realtime now" reintroduces silent
//! data loss on every reconnect. What Realtime is allowed to change is the poll
//! *interval*, and only once the channel has confirmed its join — not merely
//! once the socket has opened. A socket that is open but whose channel never
//! joined delivers nothing at all, and backing off on it would halve the sync
//! rate for no reason.
//!
//! # Why this speaks Phoenix by hand
//!
//! `CLAUDE.md` rule 1 says reach for a crate first. There is no maintained Rust
//! client for Supabase Realtime, and the Phoenix subset actually used here is
//! four message shapes: `phx_join`, `phx_reply`, `heartbeat`, `phx_leave`, plus
//! the `postgres_changes` payload. The v1 audit reached the same conclusion and
//! it still holds under exemption 1 — but it holds only for *this* subset. This
//! module is not a Phoenix client library: there is no channel registry, no
//! push/ref correlation table, no presence, no generic event router. Adding one
//! would be the wheel this rule exists to prevent.
//!
//! What is *not* hand-rolled: the websocket itself (`tokio-tungstenite`), the
//! JSON (`serde_json`), and the reconnect schedule (`backoff`).
//!
//! # The wire format
//!
//! Every frame is a five-element JSON array:
//!
//! ```text
//! [join_ref, ref, topic, event, payload]
//! ```
//!
//! Frames that are not exactly five elements are rejected. A `ref` that is not
//! a JSON string maps to *absent* rather than to an empty string: v1's
//! `CopyPaste-crh3.97` came from mapping a numeric ref to `Some("")`, so a
//! reply's ref never matched the heartbeat's and every heartbeat reply was
//! silently dropped.
//!
//! # Log hygiene
//!
//! A raw frame embeds clipboard ciphertext and metadata. Nothing in this module
//! logs a frame body, a payload, an access token or the socket URL. A parse
//! failure logs the frame length and a sixteen-byte hex prefix, which is enough
//! to tell "HTML error page" from "truncated JSON" and not enough to be worth
//! exfiltrating.

use std::sync::Arc;
use std::time::{Duration, Instant};

use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::rest::CloudItem;
use crate::CloudConfig;

// ---------------------------------------------------------------------------
// Tunables
// ---------------------------------------------------------------------------

/// The table we subscribe to. One table, one channel — see manifest 05 §4.1.
const TABLE: &str = "clipboard_items";

/// Phoenix topic. Supabase namespaces every channel under `realtime:`.
const TOPIC: &str = "realtime:clipboard_items";

/// Heartbeat cadence. The server drops a channel that has been silent for about
/// sixty seconds, so thirty gives one missed beat of slack.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// How long to wait for the socket and for the `phx_reply` that confirms the
/// join. Without a bound, a black-holed connection stalls `connect` forever.
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);

/// Reconnect schedule floor.
const RECONNECT_INITIAL: Duration = Duration::from_secs(1);

/// Reconnect schedule ceiling.
const RECONNECT_MAX: Duration = Duration::from_secs(60);

/// How much of an unparseable frame is safe to log: a length and this many
/// bytes as hex. Never the frame.
const LOG_PREFIX_BYTES: usize = 16;

/// Bounded queue between the socket task and [`RealtimeSubscription::next_event`].
///
/// Bounded on purpose. If the consumer stalls, backpressure reaches the socket
/// and the server stops being told we are healthy, which is the correct signal;
/// an unbounded queue would instead grow without limit holding decrypted-row
/// metadata in memory. Missing an event is safe — the poll loop is the backstop.
const EVENT_QUEUE: usize = 64;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The write half, after splitting. Reading and writing are split so that a
/// heartbeat and an inbound frame are not two mutable borrows of one socket
/// inside the same `select!`.
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One Postgres change on `clipboard_items`.
///
/// `Delete` carries only an `item_id` because a Postgres delete event carries
/// only the replica-identity columns. In practice it should be rare: manifest
/// 05 §3.5 makes a delete a *tombstone*, an ordinary row version with
/// `deleted = true`, which arrives here as [`RealtimeEvent::Update`]. A real
/// `Delete` means a row was removed out of band — a retention job, or a manual
/// operation — and the sync driver treats it as a hint to re-poll, never as an
/// instruction to remove a local row.
///
/// Only `Debug` is derived. `Clone` and `PartialEq` would impose those bounds
/// on `CloudItem`, which belongs to `rest.rs`; this enum should not constrain a
/// sibling's DTO for the convenience of a test.
#[derive(Debug)]
pub enum RealtimeEvent {
    Insert(CloudItem),
    Update(CloudItem),
    Delete { item_id: String },
}

/// Failures this module can produce.
///
/// Payloads are `&'static str`. No variant can carry the socket URL, the
/// access token, the anon key or a frame body: `CLAUDE.md` rule 4 plus the
/// sync-path rule that a token is never rendered. A closed set of literals
/// leaves nothing to interpolate into.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RealtimeError {
    /// The websocket could not be opened, or the handshake was rejected.
    #[error("realtime connection failed: {0}")]
    Connect(&'static str),

    /// The channel opened but the server did not confirm the join.
    #[error("realtime channel join was refused")]
    JoinRefused,

    /// A frame did not match the Phoenix envelope or a change payload was not
    /// the shape we can use. The offending bytes are not carried here.
    #[error("realtime protocol error: {0}")]
    Protocol(&'static str),

    /// The access token did not carry a subject claim, so the per-user filter
    /// cannot be built.
    ///
    /// This is a **hard error, never a silently-omitted filter**
    /// (`CopyPaste-nr2y`, manifest 05 §4.7). Without `user_id=eq.<uuid>` the
    /// Realtime server can place another account's rows into the stream before
    /// server-side RLS applies. The filter does not replace RLS; it backs it up,
    /// and defence in depth that silently disables itself is not defence.
    #[error("the session token carries no user id, so the per-user filter cannot be applied")]
    MissingUserId,
}

/// A live subscription to `clipboard_items`.
///
/// Owns a background task that holds the socket, sends heartbeats and
/// reconnects. Drop it or call [`RealtimeSubscription::close`] to stop; `close`
/// is preferable because it sends `phx_leave` first, so the server tears the
/// channel down immediately instead of waiting for a heartbeat timeout.
pub struct RealtimeSubscription {
    events: mpsc::Receiver<Result<RealtimeEvent, RealtimeError>>,
    shutdown: Arc<Notify>,
    task: JoinHandle<()>,
}

impl std::fmt::Debug for RealtimeSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealtimeSubscription")
            .finish_non_exhaustive()
    }
}

impl RealtimeSubscription {
    /// Open the socket, join the channel, and start delivering events.
    ///
    /// The first connect and join happen before this returns, so a bad URL, a
    /// rejected token or a missing subject claim surface here rather than as a
    /// silent no-op. Everything after that — disconnects, reconnects,
    /// heartbeats — is handled by the background task.
    ///
    /// `access_token` is the current user JWT. It is read at *every* reconnect
    /// from the value passed here, so a long-lived subscription should be
    /// dropped and re-created when the session is refreshed; a JWT captured once
    /// and used forever silently re-joins with a dead token (manifest 05 §4.7,
    /// non-negotiable 1).
    ///
    /// # Errors
    ///
    /// [`RealtimeError::MissingUserId`] if the token has no `sub` claim.
    ///
    /// [`RealtimeError::Connect`] if the socket cannot be opened.
    ///
    /// [`RealtimeError::JoinRefused`] if the channel join is not confirmed
    /// within [`JOIN_TIMEOUT`].
    pub async fn connect(config: &CloudConfig, access_token: &str) -> Result<Self, RealtimeError> {
        let user_id = jwt_subject(access_token).ok_or(RealtimeError::MissingUserId)?;
        let url = websocket_url(&config.url);
        let anon_key = config.anon_key.clone();
        let token = access_token.to_owned();

        let stream = open_channel(&url, &anon_key, &token, &user_id).await?;

        let (tx, events) = mpsc::channel(EVENT_QUEUE);
        let shutdown = Arc::new(Notify::new());
        let task = tokio::spawn(run(
            stream,
            url,
            anon_key,
            token,
            user_id,
            tx,
            Arc::clone(&shutdown),
        ));

        Ok(Self {
            events,
            shutdown,
            task,
        })
    }

    /// The next event, or `None` once the subscription has stopped for good.
    ///
    /// An `Err` here is informational: the task keeps running and reconnecting.
    /// `None` is terminal.
    pub async fn next_event(&mut self) -> Option<Result<RealtimeEvent, RealtimeError>> {
        self.events.recv().await
    }

    /// Leave the channel and stop the background task.
    ///
    /// Waits for the task to finish so that the `phx_leave` and the close frame
    /// are actually sent before the caller moves on.
    pub async fn close(self) {
        self.shutdown.notify_waiters();
        // The task also observes the dropped receiver, so it cannot deadlock on
        // a full queue while shutting down.
        drop(self.events);
        let _ = self.task.await;
    }
}

// ---------------------------------------------------------------------------
// The socket task
// ---------------------------------------------------------------------------

/// Outcome of one session on one socket.
enum Exit {
    /// The caller asked us to stop; the channel was left cleanly.
    Shutdown,
    /// The socket or the channel went away. Reconnect.
    Disconnected,
}

/// Run sessions back to back, reconnecting on the `backoff` schedule.
///
/// The reset rule is manifest 05 §4.7's: a session that ran at least as long as
/// the ceiling counts as *stable*, so the schedule goes back to its floor.
/// Without that, a healthy server that blips once an hour accumulates an
/// ever-growing delay and eventually stops reconnecting promptly at all. A
/// short-lived session does **not** reset, which is what stops a connect-crash
/// loop from hammering the endpoint.
#[allow(clippy::too_many_arguments)]
async fn run(
    first: WsStream,
    url: String,
    anon_key: String,
    token: String,
    user_id: String,
    tx: mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: Arc<Notify>,
) {
    let mut policy = reconnect_policy();
    let mut stream = Some(first);

    loop {
        let started = Instant::now();
        let exit = match stream.take() {
            Some(ws) => pump(ws, &tx, &shutdown).await,
            None => Exit::Disconnected,
        };
        if matches!(exit, Exit::Shutdown) || tx.is_closed() {
            return;
        }

        if started.elapsed() >= RECONNECT_MAX {
            policy.reset();
        }

        let Some(delay) = policy.next_backoff() else {
            // `max_elapsed_time` is `None`, so this is unreachable; treat it as
            // "give up" rather than as a reason to spin.
            let _ = tx
                .send(Err(RealtimeError::Connect("reconnects exhausted")))
                .await;
            return;
        };

        tokio::select! {
            biased;
            () = shutdown.notified() => return,
            () = tokio::time::sleep(delay) => {}
        }

        match open_channel(&url, &anon_key, &token, &user_id).await {
            Ok(ws) => stream = Some(ws),
            Err(RealtimeError::MissingUserId) => {
                // Cannot be recovered by retrying; the token is the problem.
                let _ = tx.send(Err(RealtimeError::MissingUserId)).await;
                return;
            }
            Err(e) => {
                tracing::debug!(error = %e, "realtime reconnect attempt failed");
            }
        }
    }
}

/// One session on one socket: dispatch inbound frames, send heartbeats, and
/// return when the socket dies or the caller asks us to stop.
async fn pump(
    ws: WsStream,
    tx: &mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: &Notify,
) -> Exit {
    // Split so the heartbeat write and the inbound read are disjoint borrows.
    let (mut write, mut read) = ws.split();

    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await; // the first tick is immediate; the join just happened.

    // Ref 1 is the join (see `join_frame`), so the heartbeat counter starts at 2.
    let mut next_ref: u64 = 2;

    loop {
        tokio::select! {
            biased;

            () = shutdown.notified() => {
                let _ = leave(&mut write, next_ref).await;
                return Exit::Shutdown;
            }

            _ = heartbeat.tick() => {
                let frame = json!([Value::Null, next_ref.to_string(), "phoenix", "heartbeat", {}]);
                next_ref += 1;
                // Bound the write. On a half-open socket `send` can stall
                // indefinitely, starving heartbeats until the server-side
                // timeout kills the connection ~60 s later. A write that does
                // not complete within one heartbeat interval *is* a disconnect
                // (manifest 05 §4.7).
                match tokio::time::timeout(
                    HEARTBEAT_INTERVAL,
                    write.send(Message::Text(frame.to_string())),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) | Err(_) => {
                        tracing::debug!("realtime heartbeat write failed or stalled; reconnecting");
                        return Exit::Disconnected;
                    }
                }
            }

            frame = read.next() => {
                let Some(frame) = frame else { return Exit::Disconnected };
                let text = match frame {
                    Ok(Message::Text(t)) => t,
                    // Binary frames are not part of the Phoenix protocol here.
                    Ok(Message::Binary(_)) => continue,
                    Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                    Ok(Message::Close(_)) => return Exit::Disconnected,
                    Err(_) => return Exit::Disconnected,
                };

                match dispatch(&text) {
                    Dispatch::Nothing => {}
                    Dispatch::Closed => return Exit::Disconnected,
                    Dispatch::Event(event) => {
                        if tx.send(Ok(event)).await.is_err() {
                            return Exit::Shutdown;
                        }
                    }
                    Dispatch::Failed(e) => {
                        if tx.send(Err(e)).await.is_err() {
                            return Exit::Shutdown;
                        }
                    }
                }
            }
        }
    }
}

/// Send `phx_leave` and close. Best effort — we are shutting down either way.
async fn leave(write: &mut WsSink, msg_ref: u64) -> Result<(), ()> {
    let frame = json!(["1", msg_ref.to_string(), TOPIC, "phx_leave", {}]);
    let farewell = async {
        write
            .send(Message::Text(frame.to_string()))
            .await
            .map_err(|_| ())?;
        SinkExt::close(write).await.map_err(|_| ())
    };
    tokio::time::timeout(JOIN_TIMEOUT, farewell)
        .await
        .map_err(|_| ())?
}

// ---------------------------------------------------------------------------
// Connect and join
// ---------------------------------------------------------------------------

/// Open the socket and join the channel, returning only once the server has
/// confirmed the join with a `phx_reply` whose status is `ok`.
///
/// Gating on the *reply* rather than on socket-open is deliberate: the caller
/// uses "realtime is live" to slow its poll loop, and a socket whose channel
/// never joined delivers nothing (manifest 05 §4.8).
async fn open_channel(
    url: &str,
    anon_key: &str,
    access_token: &str,
    user_id: &str,
) -> Result<WsStream, RealtimeError> {
    let mut request = url
        .into_client_request()
        .map_err(|_| RealtimeError::Connect("the configured url is not a websocket url"))?;
    // The publishable key goes in a header, not the query string: a URL ends up
    // in proxy logs and in error messages (`CopyPaste-lnjm`).
    request.headers_mut().insert(
        "apikey",
        anon_key
            .parse()
            .map_err(|_| RealtimeError::Connect("the anon key is not a valid header value"))?,
    );

    let connect = async {
        let (mut ws, _) = connect_async(request)
            .await
            .map_err(|_| RealtimeError::Connect("the websocket could not be opened"))?;

        ws.send(Message::Text(join_frame(access_token, user_id).to_string()))
            .await
            .map_err(|_| RealtimeError::Connect("the join could not be sent"))?;

        // Wait for the join confirmation, ignoring anything else that arrives
        // first.
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => match join_reply(&text) {
                    Some(true) => return Ok(ws),
                    Some(false) => return Err(RealtimeError::JoinRefused),
                    None => continue,
                },
                Some(Ok(Message::Close(_))) | None => {
                    return Err(RealtimeError::JoinRefused);
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) => {
                    return Err(RealtimeError::Connect("the websocket failed during join"))
                }
            }
        }
    };

    tokio::time::timeout(JOIN_TIMEOUT, connect)
        .await
        .map_err(|_| RealtimeError::Connect("timed out opening the channel"))?
}

/// The `phx_join` frame.
///
/// Three things here are load-bearing (manifest 05 §4.7):
///
/// 1. `access_token` is the *current* user JWT, not the anon key.
/// 2. `event` is `"*"`, never `"INSERT"`. INSERT-only silently drops every
///    cross-device update and delete — which, since a delete is a tombstone
///    update, means every delete.
/// 3. `filter` is always `user_id=eq.<uuid>`. See [`RealtimeError::MissingUserId`].
fn join_frame(access_token: &str, user_id: &str) -> Value {
    json!([
        "1",
        "1",
        TOPIC,
        "phx_join",
        {
            "config": {
                "access_token": access_token,
                "postgres_changes": [{
                    "event": "*",
                    "schema": "public",
                    "table": TABLE,
                    "filter": format!("user_id=eq.{user_id}"),
                }],
            }
        }
    ])
}

/// `Some(true)` for a confirmed join, `Some(false)` for a refused one, `None`
/// if this frame is not a reply to our join at all.
fn join_reply(text: &str) -> Option<bool> {
    let frame = parse_frame(text).ok()?;
    if frame.event != "phx_reply" || frame.topic != TOPIC {
        return None;
    }
    Some(frame.payload.get("status").and_then(Value::as_str) == Some("ok"))
}

// ---------------------------------------------------------------------------
// Frame parsing and dispatch
// ---------------------------------------------------------------------------

/// The Phoenix envelope.
#[derive(Debug, PartialEq)]
struct Frame {
    /// `None` when the field is null *or* not a string. See the module docs.
    join_ref: Option<String>,
    /// Same rule as `join_ref`.
    msg_ref: Option<String>,
    topic: String,
    event: String,
    payload: Value,
}

/// Reject anything that is not exactly a five-element array of the right
/// shapes. A frame is attacker-influenced input; it gets a parser, not an
/// index.
fn parse_frame(text: &str) -> Result<Frame, RealtimeError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| RealtimeError::Protocol("frame is not json"))?;
    let array = value
        .as_array()
        .ok_or(RealtimeError::Protocol("frame is not an array"))?;
    if array.len() != 5 {
        return Err(RealtimeError::Protocol("frame is not five elements"));
    }

    // A ref that is not a string is *absent*, not `Some("")`. See the module
    // docs and `CopyPaste-crh3.97`.
    let as_ref = |v: &Value| v.as_str().map(str::to_owned);

    Ok(Frame {
        join_ref: as_ref(&array[0]),
        msg_ref: as_ref(&array[1]),
        topic: array[2]
            .as_str()
            .ok_or(RealtimeError::Protocol("frame topic is not a string"))?
            .to_owned(),
        event: array[3]
            .as_str()
            .ok_or(RealtimeError::Protocol("frame event is not a string"))?
            .to_owned(),
        payload: array[4].clone(),
    })
}

/// What one inbound frame means to us.
enum Dispatch {
    Event(RealtimeEvent),
    Failed(RealtimeError),
    Closed,
    Nothing,
}

fn dispatch(text: &str) -> Dispatch {
    let frame = match parse_frame(text) {
        Ok(f) => f,
        Err(e) => {
            log_unparseable(text);
            return Dispatch::Failed(e);
        }
    };

    match frame.event.as_str() {
        "postgres_changes" => match change_event(&frame.payload) {
            Ok(Some(event)) => Dispatch::Event(event),
            Ok(None) => Dispatch::Nothing,
            Err(e) => {
                log_unparseable(text);
                Dispatch::Failed(e)
            }
        },
        // A re-join confirmation after a reconnect, or a heartbeat reply.
        "phx_reply" => Dispatch::Nothing,
        "phx_close" => Dispatch::Closed,
        "phx_error" => {
            // The payload can embed row data; log that it happened, not what
            // it said.
            tracing::warn!("realtime channel reported an error");
            Dispatch::Closed
        }
        _ => Dispatch::Nothing,
    }
}

/// Turn a `postgres_changes` payload into an event.
///
/// `Ok(None)` means "a change we do not model" — a change to another table, or
/// an event type we do not handle — which is not an error.
///
/// The payload shape is defensively read (manifest 05 §4.7): the change lives
/// under `payload.data`, `record`/`new` and `old_record`/`old` are both
/// accepted spellings, and the type is compared case-insensitively.
fn change_event(payload: &Value) -> Result<Option<RealtimeEvent>, RealtimeError> {
    let data = payload
        .get("data")
        .ok_or(RealtimeError::Protocol("change payload has no data"))?;

    if let Some(table) = data.get("table").and_then(Value::as_str) {
        if table != TABLE {
            return Ok(None);
        }
    }

    let kind = data
        .get("type")
        .and_then(Value::as_str)
        .ok_or(RealtimeError::Protocol("change payload has no type"))?
        .to_ascii_uppercase();

    let record = || data.get("record").or_else(|| data.get("new"));
    let old = || data.get("old_record").or_else(|| data.get("old"));

    match kind.as_str() {
        "INSERT" | "UPDATE" => {
            let row = record().ok_or(RealtimeError::Protocol("change payload has no record"))?;
            let item: CloudItem = serde_json::from_value(row.clone())
                .map_err(|_| RealtimeError::Protocol("record is not a clipboard row"))?;
            Ok(Some(if kind == "INSERT" {
                RealtimeEvent::Insert(item)
            } else {
                RealtimeEvent::Update(item)
            }))
        }
        "DELETE" => {
            let item_id = old()
                .and_then(|o| o.get("item_id"))
                .and_then(Value::as_str)
                .ok_or(RealtimeError::Protocol("delete payload carries no item id"))?
                .to_owned();
            Ok(Some(RealtimeEvent::Delete { item_id }))
        }
        _ => Ok(None),
    }
}

/// Log that a frame could not be parsed, without logging the frame.
///
/// Length plus a short hex prefix is enough to distinguish an HTML error page
/// from truncated JSON from a protocol change, and is not enough to disclose a
/// row (manifest 05 §4.7, log hygiene).
fn log_unparseable(text: &str) {
    let bytes = text.as_bytes();
    let prefix = &bytes[..bytes.len().min(LOG_PREFIX_BYTES)];
    tracing::warn!(
        len = bytes.len(),
        prefix = %hex::encode(prefix),
        "realtime frame could not be parsed"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the reconnect schedule.
///
/// `max_elapsed_time` is `None` so the subscription retries for as long as it
/// is alive; a clipboard that stops syncing after fifteen minutes offline would
/// be worse than useless. The randomisation factor is the crate's default,
/// which is what stops every device of a large account reconnecting in lockstep
/// after a server restart.
fn reconnect_policy() -> ExponentialBackoff {
    ExponentialBackoff {
        initial_interval: RECONNECT_INITIAL,
        max_interval: RECONNECT_MAX,
        max_elapsed_time: None,
        ..ExponentialBackoff::default()
    }
}

/// `https://…` / `http://…` -> the realtime websocket endpoint.
///
/// A non-`http` scheme is passed through untouched so a `ws://` loopback URL
/// works in a test harness.
fn websocket_url(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    let ws_base = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        trimmed.to_owned()
    };
    // `vsn` selects the Phoenix serializer; 1.0.0 is the five-element array
    // this module parses.
    format!("{ws_base}/realtime/v1/websocket?vsn=1.0.0")
}

/// The `sub` claim of a JWT, without verifying it.
///
/// We are not authenticating anything here — the server does that, and it holds
/// the signing key. All we need is the user id the token already asserts, so
/// that the channel filter can be built. Reading one claim is a base64url
/// decode and a JSON lookup; pulling a full JWT crate in would add a second
/// signature-verification stack to the tree for a value we deliberately do not
/// verify (`CLAUDE.md` rule 1, exemption 3).
///
/// Returns `None` for anything that is not a three-part token with a decodable
/// JSON payload carrying a non-empty string `sub` — which the caller turns into
/// a hard error rather than an omitted filter.
fn jwt_subject(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    // A JWT has three parts. Two is not a token we should be reading, and the
    // signature is not checked here — see the doc comment.
    parts.next()?;
    let decoded = B64URL.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    let sub = claims.get("sub")?.as_str()?;
    if sub.is_empty() {
        return None;
    }
    Some(sub.to_owned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Everything here is offline: frames are strings and the schedule is arithmetic.
// Nothing in this module's test set opens a socket.

#[cfg(test)]
mod tests {
    use super::*;

    /// A JWT-shaped token whose payload decodes to `{"sub": id}`. Not signed —
    /// nothing here verifies signatures, and pretending otherwise in a test
    /// would suggest that it does.
    fn token_for(id: &str) -> String {
        let payload = B64URL.encode(format!(r#"{{"sub":"{id}","role":"authenticated"}}"#));
        format!("header.{payload}.signature")
    }

    const USER: &str = "6b1e2f80-0000-4000-8000-000000000001";

    // --- the envelope -----------------------------------------------------

    #[test]
    fn a_five_element_frame_parses() {
        let frame =
            parse_frame(r#"["1","2","realtime:clipboard_items","phx_reply",{"status":"ok"}]"#)
                .unwrap();
        assert_eq!(frame.join_ref.as_deref(), Some("1"));
        assert_eq!(frame.msg_ref.as_deref(), Some("2"));
        assert_eq!(frame.topic, TOPIC);
        assert_eq!(frame.event, "phx_reply");
        assert_eq!(frame.payload["status"], "ok");
    }

    #[test]
    fn a_numeric_ref_is_absent_not_empty() {
        // CopyPaste-crh3.97. `Some("")` here made every heartbeat reply look
        // like it belonged to a different push.
        let frame = parse_frame(r#"[1,2,"phoenix","phx_reply",{}]"#).unwrap();
        assert_eq!(frame.join_ref, None);
        assert_eq!(frame.msg_ref, None);
    }

    #[test]
    fn a_null_ref_is_absent() {
        let frame = parse_frame(r#"[null,"3","phoenix","heartbeat",{}]"#).unwrap();
        assert_eq!(frame.join_ref, None);
        assert_eq!(frame.msg_ref.as_deref(), Some("3"));
    }

    #[test]
    fn frames_of_the_wrong_arity_are_rejected() {
        for text in [
            r#"["1","1","t","e"]"#,
            r#"["1","1","t","e",{},"extra"]"#,
            r#"[]"#,
            r#"{"join_ref":"1"}"#,
            r#"not json"#,
        ] {
            assert!(parse_frame(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn a_frame_with_a_non_string_topic_or_event_is_rejected() {
        assert!(parse_frame(r#"["1","1",7,"phx_reply",{}]"#).is_err());
        assert!(parse_frame(r#"["1","1","t",7,{}]"#).is_err());
    }

    // --- join -------------------------------------------------------------

    #[test]
    fn the_join_frame_carries_the_jwt_the_wildcard_and_the_filter() {
        let frame = join_frame("the.jwt.here", USER);
        let config = &frame[4]["config"];

        assert_eq!(config["access_token"], "the.jwt.here");

        let change = &config["postgres_changes"][0];
        assert_eq!(
            change["event"], "*",
            "INSERT-only drops updates and deletes"
        );
        assert_ne!(change["event"], "INSERT");
        assert_eq!(change["schema"], "public");
        assert_eq!(change["table"], TABLE);
        assert_eq!(change["filter"], format!("user_id=eq.{USER}"));

        // Envelope: five elements, our topic, and ref 1 (the heartbeat counter
        // starts at 2 because of this).
        assert_eq!(frame.as_array().unwrap().len(), 5);
        assert_eq!(frame[1], "1");
        assert_eq!(frame[2], TOPIC);
        assert_eq!(frame[3], "phx_join");
    }

    #[test]
    fn only_an_ok_reply_on_our_topic_confirms_the_join() {
        assert_eq!(
            join_reply(r#"["1","1","realtime:clipboard_items","phx_reply",{"status":"ok"}]"#),
            Some(true)
        );
        assert_eq!(
            join_reply(r#"["1","1","realtime:clipboard_items","phx_reply",{"status":"error"}]"#),
            Some(false)
        );
        // A reply for another topic, or another event, is not our confirmation.
        assert_eq!(
            join_reply(r#"[null,"2","phoenix","phx_reply",{"status":"ok"}]"#),
            None
        );
        assert_eq!(
            join_reply(r#"["1","1","realtime:clipboard_items","phx_error",{}]"#),
            None
        );
        assert_eq!(join_reply("garbage"), None);
    }

    #[test]
    fn a_token_without_a_subject_is_a_hard_error() {
        // CopyPaste-nr2y: the filter is never silently omitted.
        assert_eq!(jwt_subject(&token_for(USER)).as_deref(), Some(USER));

        // Present but empty is still no user id.
        let empty_sub = B64URL.encode(r#"{"sub":""}"#);
        assert_eq!(jwt_subject(&format!("h.{empty_sub}.s")), None);

        assert_eq!(jwt_subject("not.a"), None);
        assert_eq!(jwt_subject(""), None);
        assert_eq!(jwt_subject("a.b.c"), None);

        let no_sub = B64URL.encode(r#"{"role":"authenticated"}"#);
        assert_eq!(jwt_subject(&format!("h.{no_sub}.s")), None);
    }

    // --- change payloads --------------------------------------------------

    fn row_json(item_id: &str, deleted: bool) -> String {
        format!(
            r#"{{"item_id":"{item_id}","ciphertext":"Y3Q=","nonce":"bm9uY2U=",
                 "content_type":"text","created_at":1700000000000,
                 "deleted":{deleted},"origin_device_id":"dev-a"}}"#
        )
    }

    #[test]
    fn an_insert_becomes_an_insert_event() {
        let payload: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"INSERT","table":"clipboard_items","record":{}}}}}"#,
            row_json("item-1", false)
        ))
        .unwrap();

        match change_event(&payload).unwrap().unwrap() {
            RealtimeEvent::Insert(item) => assert_eq!(item.item_id, "item-1"),
            other => panic!("expected an insert, got {other:?}"),
        }
    }

    #[test]
    fn an_update_carrying_a_tombstone_becomes_an_update_event() {
        // A delete is a tombstone — an UPDATE with `deleted = true`. This is the
        // path a real delete travels; the DELETE event type is the exception.
        let payload: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"UPDATE","table":"clipboard_items","new":{}}}}}"#,
            row_json("item-2", true)
        ))
        .unwrap();

        match change_event(&payload).unwrap().unwrap() {
            RealtimeEvent::Update(item) => {
                assert_eq!(item.item_id, "item-2");
                assert!(item.deleted);
            }
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_new_and_old_spellings_are_both_accepted() {
        let with_new: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"insert","new":{}}}}}"#,
            row_json("item-3", false)
        ))
        .unwrap();
        assert!(matches!(
            change_event(&with_new).unwrap().unwrap(),
            RealtimeEvent::Insert(_)
        ));

        let with_old: Value =
            serde_json::from_str(r#"{"data":{"type":"delete","old":{"item_id":"item-4"}}}"#)
                .unwrap();
        match change_event(&with_old).unwrap().unwrap() {
            RealtimeEvent::Delete { item_id } => assert_eq!(item_id, "item-4"),
            other => panic!("expected a delete, got {other:?}"),
        }
    }

    #[test]
    fn a_delete_without_an_item_id_is_a_protocol_error_not_a_guess() {
        // Guessing an id here would delete the wrong row. The correct response
        // is to report and let the poll loop reconcile.
        let payload: Value =
            serde_json::from_str(r#"{"data":{"type":"DELETE","old_record":{"id":"row-pk"}}}"#)
                .unwrap();
        assert!(change_event(&payload).is_err());
    }

    #[test]
    fn a_change_to_another_table_is_ignored_not_an_error() {
        let payload: Value = serde_json::from_str(
            r#"{"data":{"type":"INSERT","table":"other","record":{"item_id":"x"}}}"#,
        )
        .unwrap();
        assert!(change_event(&payload).unwrap().is_none());
    }

    #[test]
    fn a_malformed_record_is_an_error_not_a_partial_item() {
        // INV-N3 in miniature: never manufacture a half-filled row.
        let payload: Value =
            serde_json::from_str(r#"{"data":{"type":"INSERT","record":{"item_id":"x"}}}"#).unwrap();
        assert!(change_event(&payload).is_err());
    }

    #[test]
    fn dispatch_routes_the_lifecycle_events() {
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_reply",{"status":"ok"}]"#),
            Dispatch::Nothing
        ));
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_close",{}]"#),
            Dispatch::Closed
        ));
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_error",{}]"#),
            Dispatch::Closed
        ));
        assert!(matches!(dispatch("{not a frame"), Dispatch::Failed(_)));
        assert!(matches!(
            dispatch(&format!(
                r#"["1","1","realtime:clipboard_items","postgres_changes",{{"data":{{"type":"INSERT","record":{}}}}}]"#,
                row_json("item-5", false)
            )),
            Dispatch::Event(RealtimeEvent::Insert(_))
        ));
    }

    // --- hygiene ----------------------------------------------------------

    #[test]
    fn errors_carry_no_url_no_token_and_no_frame() {
        let errors = [
            RealtimeError::Connect("the websocket could not be opened"),
            RealtimeError::JoinRefused,
            RealtimeError::Protocol("frame is not json"),
            RealtimeError::MissingUserId,
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains("://"), "url in {msg:?}");
            assert!(!msg.contains("supabase"), "host in {msg:?}");
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains("eyJ"), "jwt-like text in {msg:?}");
        }
    }

    // --- url --------------------------------------------------------------

    #[test]
    fn the_websocket_url_is_derived_from_the_rest_url() {
        assert_eq!(
            websocket_url("https://proj.supabase.co"),
            "wss://proj.supabase.co/realtime/v1/websocket?vsn=1.0.0"
        );
        // A trailing slash must not double up.
        assert_eq!(
            websocket_url("https://proj.supabase.co/"),
            "wss://proj.supabase.co/realtime/v1/websocket?vsn=1.0.0"
        );
        assert_eq!(
            websocket_url("http://127.0.0.1:54321"),
            "ws://127.0.0.1:54321/realtime/v1/websocket?vsn=1.0.0"
        );
        // The anon key is never in the URL.
        assert!(!websocket_url("https://proj.supabase.co").contains("apikey"));
    }

    // --- reconnect schedule ------------------------------------------------

    #[test]
    fn the_reconnect_schedule_grows_and_is_bounded() {
        let mut policy = reconnect_policy();
        let mut last = Duration::ZERO;
        let mut grew = false;

        for _ in 0..40 {
            let delay = policy.next_backoff().expect("the schedule never gives up");
            // The crate applies a randomisation factor, so assert the envelope
            // rather than an exact doubling.
            assert!(
                delay <= RECONNECT_MAX.mul_f64(1.5),
                "delay {delay:?} exceeded the ceiling"
            );
            if delay > last {
                grew = true;
            }
            last = delay;
        }
        assert!(grew, "the schedule never grew");

        // A stable session resets it to the floor.
        policy.reset();
        let first = policy.next_backoff().unwrap();
        assert!(
            first <= RECONNECT_INITIAL.mul_f64(1.5),
            "reset did not return to the floor: {first:?}"
        );
    }

    #[test]
    fn the_reconnect_schedule_never_expires() {
        // `max_elapsed_time = None`. A device that has been offline overnight
        // must still reconnect in the morning.
        assert!(reconnect_policy().max_elapsed_time.is_none());
    }
}
