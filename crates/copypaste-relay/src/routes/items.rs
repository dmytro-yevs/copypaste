use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::BearerToken;
use crate::config::RelayConfig;
use crate::error::RelayError;
use crate::models::{PullItem, PullParams, PushRequest, PushResponse};
use crate::quota::{self, QuotaViolation, Tier};
use crate::state::{AppState, DEFAULT_PULL_LIMIT, MAX_PULL_LIMIT};

// The SSE items below (issue #26) are wired into production via
// `routes/mod.rs` and exercised by `tests/sse_subscribe.rs`, but several other
// test binaries `#[path]`-include this file standalone (to reuse `pull`/`push`)
// without the `subscribe` route, so the compiler sees these as dead there.
// `#[allow(dead_code)]` mirrors the pattern already used for `push_item` etc.

/// SSE keepalive interval. A comment frame (`:\n\n`) every 25s keeps the
/// connection alive through proxies / load balancers that idle-timeout silent
/// connections (issue #26). Chosen below the common 30–60s proxy idle window.
#[allow(dead_code)]
const SSE_KEEPALIVE_SECS: u64 = 25;

/// Bound on the mpsc channel feeding the SSE response body. Each slot holds one
/// pre-rendered `Event`. A slow client that stops reading fills this buffer and
/// applies backpressure to the producer task (which parks on `send`), rather
/// than letting the relay buffer an unbounded number of events in memory.
#[allow(dead_code)]
const SSE_CHANNEL_CAP: usize = 64;

/// Page size used by the SSE producer when draining the inbox from its cursor.
/// Bounded so a single drain can't clone an unbounded slice under the store
/// mutex; the producer loops until the inbox is exhausted, so a backlog larger
/// than one page is still fully delivered across successive reads.
#[allow(dead_code)]
const SSE_DRAIN_PAGE: usize = MAX_PULL_LIMIT;

/// DELETE /devices/:device_id/items/:item_id
///
/// Remove a single item from a device's legacy fanout inbox.
pub async fn delete_item(
    State(state): State<AppState>,
    Path((device_id, item_id)): Path<(String, String)>,
    BearerToken(token): BearerToken,
) -> Result<StatusCode, RelayError> {
    // Survive mutex poisoning (security HIGH #1, INFO #21): recover the
    // inner data rather than crashing the request. Matches the pattern
    // already used in devices.rs.
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
    store.verify_token(&device_id, &token)?;
    // Advance last_seen so an actively-deleting device is not evicted by
    // cleanup_inactive_devices (which reaps on last_seen.elapsed(), not
    // registered_at). Without this, a device that continuously polls and
    // deletes items still gets evicted once registered_at passes the threshold.
    store.update_last_seen(&device_id);
    store.delete_item(&device_id, &item_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Push / Pull — wall-clock sync routes
// POST /devices/:id/items  — push an encrypted item into the device's inbox.
// GET  /devices/:id/items?since=<wall_time> — pull items newer than wall_time.
// ---------------------------------------------------------------------------

/// POST /devices/:device_id/items
///
/// Body: `{ "content_type": "text"|"image"|"file", "content_b64": "<base64>", "wall_time": <u64> }`
/// Response (201): `{ "id": <i64> }`
///
/// Auth: `Authorization: Bearer <token>` — must match the token for `device_id`.
/// Quota: the decoded `content_b64` must satisfy two independent size limits:
///   1. The per-tier item-size quota (`quota::check_item_size`), checked here
///      *before* acquiring the store mutex so an oversized payload is rejected
///      cheaply with `413 ITEM_SIZE_EXCEEDED`. Free-tier limits are applied
///      conservatively (text ≤ 1 MiB, image ≤ 10 MiB) regardless of the
///      sender's tier — see the relay v2 quotas plan.
///   2. The operator-configured `RELAY_MAX_ITEM_BYTES` body cap, still
///      enforced inside `push_item` (returns `413 PAYLOAD_TOO_LARGE`).
pub async fn push(
    State(state): State<AppState>,
    Extension(config): Extension<RelayConfig>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Json(body): Json<PushRequest>,
) -> Result<(StatusCode, Json<PushResponse>), RelayError> {
    // Auth FIRST — verify the bearer token before doing any CPU/alloc work on
    // the request body. Previously the base64 decode (up to ~13 MiB) happened
    // before auth, letting unauthenticated callers trigger full decode work
    // (pre-auth CPU/alloc amplification). Authenticating first makes
    // unauthenticated pushes fail fast at the lock with a cheap map lookup.
    {
        // Short critical section: auth only, no decode under the lock.
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.verify_token(&device_id, &token)?;
    }

    // Per-tier item-size quota — checked after auth but before re-taking the
    // store mutex for the actual insert. We decode `content_b64` once here to
    // measure the true ciphertext size; `push_item_decoded` re-validates the
    // base64 (and the operator body cap) under the lock.
    let decoded_len = B64
        .decode(&body.content_b64)
        .map_err(|_| RelayError::BadRequest("content_b64 must be valid base64".to_string()))?
        .len();
    // Free-tier limits are applied conservatively for all senders (the relay
    // does not yet look up the sender's tier from the bearer token).
    quota::check_item_size(Tier::Free, decoded_len, &body.content_type).map_err(|v| match v {
        QuotaViolation::ItemTooLarge { limit_bytes } => {
            RelayError::ItemSizeExceeded { limit_bytes }
        }
        // `check_item_size` only ever returns `ItemTooLarge`.
        _ => RelayError::Internal("unexpected quota violation in item-size check".into()),
    })?;

    // Survive mutex poisoning (security HIGH #1).
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());

    // TOCTOU guard: the background evictor (`cleanup_inactive_devices`) may
    // have removed the device record in the window between the first lock
    // (verify_token) and this second one. Re-check existence here so an
    // evicted-but-authenticated device is not surfaced as a 404
    // (DeviceNotFound from push_item_decoded). Collapse to Unauthorized for
    // consistency with verify_token_at's policy: token-guarded routes never
    // return a distinct 404 for a missing device (enumeration oracle — see
    // state.rs verify_token_at comment).
    if !store.devices.contains_key(&device_id) {
        return Err(RelayError::Unauthorized);
    }

    // Advance last_seen so an actively-pushing device is never evicted by
    // cleanup_inactive_devices — which reaps on last_seen.elapsed(), not
    // registered_at. Without this call, last_seen stays at registered_at
    // forever and the device is evicted after the inactivity threshold even
    // though it is actively syncing.
    store.update_last_seen(&device_id);

    // Honor the operator-configured RELAY_MAX_ITEM_BYTES rather than the
    // hardcoded default (security HIGH #2) — previously
    // `RelayConfig::default().max_item_bytes` silently ignored env vars.
    let max_item_bytes = config.max_item_bytes;

    let id = store.push_item_decoded(
        &device_id,
        body.content_type,
        body.content_b64,
        decoded_len,
        body.wall_time,
        max_item_bytes,
    )?;

    Ok((StatusCode::CREATED, Json(PushResponse { id })))
}

/// GET /devices/:device_id/items?since=<wall_time>
///
/// Returns all items in `device_id`'s inbox with `wall_time > since`, ordered ascending.
/// Auth: `Authorization: Bearer <token>` — must match the token for `device_id`.
pub async fn pull(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Query(params): Query<PullParams>,
) -> Result<Json<Vec<PullItem>>, RelayError> {
    // Resolve the page size before taking the lock: default when absent,
    // clamped to the hard ceiling so a caller-supplied `limit` cannot force an
    // oversized clone under the global mutex (M4).
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PULL_LIMIT)
        .min(MAX_PULL_LIMIT);

    // Survive mutex poisoning (security HIGH #1).
    // pull needs a mutable borrow to call update_last_seen after auth.
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());

    // Auth: verify token belongs to this device.
    store.verify_token(&device_id, &token)?;

    // Advance last_seen so an actively-polling device (even one with an empty
    // inbox) is never reaped by cleanup_inactive_devices. Without this,
    // last_seen stays at registered_at forever and the device is evicted after
    // the inactivity threshold even though it is continuously polling.
    store.update_last_seen(&device_id);

    let items = store.pull_items(&device_id, params.since, params.since_id, limit)?;
    Ok(Json(items))
}

// ---------------------------------------------------------------------------
// SSE push (issue #26)
// GET /devices/:id/subscribe?since=<wall_time>&since_id=<id>
// ---------------------------------------------------------------------------

/// GET /devices/:device_id/subscribe?since=<wall_time>&since_id=<id>
///
/// Real-time Server-Sent Events stream of new inbox items for `device_id`,
/// additive to (and independent of) `GET .../items` polling — poll remains the
/// backstop. Auth is the same `Authorization: Bearer <token>` contract as
/// pull, verified once on connect.
///
/// On connect the stream first **backfills**: it flushes every item already in
/// the inbox past the `(since, since_id)` cursor (each as an `item` event),
/// exactly as `pull` would return them. It then **streams**: a per-device
/// broadcast wake channel (fired from `push_item_decoded` after the recipient
/// inbox write commits) wakes the producer, which re-reads the inbox from its
/// advancing cursor and emits each new item.
///
/// Event framing (SSE `text/event-stream`):
///   - `event: item`
///   - `id: <item id>`  (the per-device ascending item id; lets clients track a
///     Last-Event-ID, though the authoritative resume mechanism is the
///     `?since=&since_id=` query cursor on reconnect)
///   - `data: <JSON>` — the same object an element of the `pull` array carries:
///     `{ "id", "content_type", "content_b64", "wall_time" }`. The relay never
///     decrypts: `content_b64` is the opaque ciphertext passed through verbatim.
///
/// A keepalive comment (`:\n\n`) is sent every `SSE_KEEPALIVE_SECS` to survive
/// idle-timeout proxies.
///
/// Inbox semantics mirror `pull`: cursor-based, at-least-once, **no
/// delete-on-read**. The same item can therefore be delivered over both SSE and
/// a concurrent poll; the client dedups by `item_id` (the existing LWW path).
// Wired in `routes/mod.rs`; dead in the standalone `#[path]`-include test
// binaries that don't mount the subscribe route — see the note above the SSE
// constants.
#[allow(dead_code)]
pub async fn subscribe(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Query(params): Query<PullParams>,
) -> Result<impl IntoResponse, RelayError> {
    // Auth + subscribe under one short critical section. We verify the token,
    // advance last_seen (an actively-subscribed device must not be reaped), and
    // obtain a wake receiver — all before spawning the producer so an
    // unauthenticated caller never opens a stream.
    let mut rx = {
        let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.verify_token(&device_id, &token)?;
        store.update_last_seen(&device_id);
        store.subscribe_notifier(&device_id)
    };

    let (tx, stream_rx) = mpsc::channel::<Result<Event, Infallible>>(SSE_CHANNEL_CAP);

    // Producer task: owns the advancing cursor and the broadcast receiver. It
    // drains the inbox from the cursor (backfill on first pass, then on each
    // wake), emitting one `item` event per row, and parks on `rx.recv()` between
    // drains. It exits when the SSE body is dropped (client disconnect →
    // `tx.send` errors) or the device's wake channel is closed (device evicted).
    let producer_state = state.clone();
    let producer_device = device_id.clone();
    tokio::spawn(async move {
        let mut cursor_wall = params.since;
        let mut cursor_id = params.since_id;

        loop {
            // Drain everything currently past the cursor, paging so a single
            // lock-hold clones at most SSE_DRAIN_PAGE items.
            loop {
                let page = {
                    let mut store = producer_state.lock().unwrap_or_else(|e| e.into_inner());
                    // Keep the subscriber alive against the inactivity reaper.
                    store.update_last_seen(&producer_device);
                    match store.pull_items(&producer_device, cursor_wall, cursor_id, SSE_DRAIN_PAGE)
                    {
                        Ok(items) => items,
                        // Device gone (evicted) — stop the stream.
                        Err(_) => return,
                    }
                };
                if page.is_empty() {
                    break;
                }
                for item in &page {
                    cursor_wall = item.wall_time;
                    cursor_id = Some(item.id);
                    let event = match sse_item_event(item) {
                        Ok(ev) => ev,
                        Err(()) => continue, // unserializable item — skip, not fatal
                    };
                    if tx.send(Ok(event)).await.is_err() {
                        return; // client disconnected
                    }
                }
            }

            // Park until the next push wakes us (or the channel closes), while
            // ALSO watching for client disconnect. When the inbox is empty the
            // producer would otherwise block forever on `rx.recv()`: a client
            // TCP disconnect drops the `ReceiverStream` (and thus all `tx`
            // receivers) but does NOT wake the broadcast `rx`, so the task +
            // broadcast receiver + Arc<AppState> would leak until the next push
            // to this device or the 30-day eviction. `tx.closed()` resolves as
            // soon as every receiver of `tx` is dropped, letting us tear the
            // producer down promptly and release `rx` and the cloned state
            // (resource leak P1/High).
            tokio::select! {
                r = rx.recv() => match r {
                    Ok(()) => {} // re-drain
                    // Lagged: we missed N wake ticks under a push burst, but ticks
                    // are contentless — a single re-drain from the cursor recovers
                    // every missed item, so just loop and re-read.
                    Err(RecvError::Lagged(_)) => {}
                    // Sender dropped (device evicted / store shut down) — end stream.
                    Err(RecvError::Closed) => return,
                },
                // Client disconnected: the SSE body (ReceiverStream) was dropped,
                // so `tx` has no receivers left. End the producer task, dropping
                // `rx` and the cloned `Arc<AppState>`.
                _ = tx.closed() => return,
            }
        }
    });

    let stream = ReceiverStream::new(stream_rx);
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(SSE_KEEPALIVE_SECS))))
}

/// Render a single inbox item as an SSE `item` event: `event: item`, `id:
/// <item id>`, `data: <PullItem JSON>`. Returns `Err(())` only if JSON
/// serialization fails (the same shape `pull` already serializes, so this is
/// effectively infallible — handled defensively rather than via `unwrap`).
// Only reachable via `subscribe`; dead in the standalone `#[path]`-include test
// binaries — see the note above the SSE constants.
#[allow(dead_code)]
fn sse_item_event(item: &PullItem) -> Result<Event, ()> {
    Event::default()
        .event("item")
        .id(item.id.to_string())
        .json_data(item)
        .map_err(|e| {
            tracing::warn!(item_id = item.id, error = %e, "failed to serialize SSE item event");
        })
}
