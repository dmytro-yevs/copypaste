use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::BearerToken;
use crate::config::RelayConfig;
use crate::error::RelayError;
use crate::models::{PullItem, PullParams, PushRequest, PushResponse};
use crate::quota::{self, QuotaViolation};
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

/// CopyPaste-h7i8: maximum concurrent SSE connections per device.
///
/// Each open `GET /devices/:id/subscribe` stream holds one `broadcast::Receiver`
/// on the device's wake channel. The number of live receivers is the number of
/// open streams for that device (one producer task per stream, each holding
/// exactly one receiver). Without this cap, an attacker (or misbehaving client)
/// can open an unbounded number of simultaneous streams per device, each with
/// its own producer task and associated resources (tokio task stack, broadcast
/// receiver, `Arc<AppState>` clone, mpsc channel). This cap refuses any subscribe
/// request that would push the per-device concurrent connection count above the
/// limit, returning HTTP 429 with a descriptive message.
///
/// A well-behaved client maintains at most one SSE stream per device; a limit
/// of 8 is generous enough for legitimate multi-window UIs while bounding the
/// blast radius of a misbehaving or malicious client.
#[allow(dead_code)]
const SSE_MAX_CONNECTIONS_PER_DEVICE: usize = 8;

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

/// CopyPaste-crh3.72: exact decoded byte length of a STANDARD (padded) base64
/// string, computed in O(1) from the encoded length WITHOUT allocating the
/// decoded bytes. Returns `None` when the length is not a valid padded-base64
/// length (not a multiple of 4). Character validity is NOT checked here — the
/// quota only needs the size, and `push_item_decoded` re-decodes (and rejects
/// malformed base64) under the store lock.
fn decoded_len_padded_b64(s: &str) -> Option<usize> {
    let n = s.len();
    if n == 0 {
        return Some(0);
    }
    if !n.is_multiple_of(4) {
        return None;
    }
    let bytes = s.as_bytes();
    let pad = if bytes[n - 1] == b'=' {
        if bytes[n - 2] == b'=' {
            2
        } else {
            1
        }
    } else {
        0
    };
    Some(n / 4 * 3 - pad)
}

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
    // CopyPaste-crh3.115: capture the sender's registered tier in the same short
    // critical section as auth, so the item-size quota below uses the ACTUAL tier
    // rather than a hardcoded Tier::Free.
    let tier = {
        // Short critical section: auth + tier lookup, no decode under the lock.
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.verify_token(&device_id, &token)?;
        store.device_tier(&device_id)
    };

    // Per-tier item-size quota — checked after auth but before re-taking the
    // store mutex for the actual insert. We decode `content_b64` once here to
    // measure the true ciphertext size; `push_item_decoded` re-validates the
    // base64 (and the operator body cap) under the lock.
    // CopyPaste-crh3.72: compute the decoded length in O(1) from the encoded
    // string length WITHOUT allocating the (up to ~10 MiB) decoded Vec just to
    // measure it. Full base64 character validation is deferred to
    // `push_item_decoded`, which re-decodes under the store lock anyway.
    let decoded_len = decoded_len_padded_b64(&body.content_b64)
        .ok_or_else(|| RelayError::BadRequest("content_b64 must be valid base64".to_string()))?;
    // CopyPaste-crh3.115: enforce the sender's actual registered tier (looked up
    // from the bearer-authenticated device above) rather than a blanket Free cap.
    quota::check_item_size(tier, decoded_len, &body.content_type).map_err(|v| match v {
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

/// HTTP response header name for the pull watermark hint (CopyPaste-tspz).
///
/// The relay echoes the effective next cursor position in this header so
/// a client that loses its in-memory watermark (e.g. after a 401-forced
/// re-registration during burst-drain) can retrieve the last confirmed
/// cursor from the response headers rather than discarding progress and
/// re-fetching from scratch.
///
/// Format: `<wall_time>,<id>` where `wall_time` is the last item's `wall_time`
/// (or the incoming `since` when the page is empty) and `id` is the last
/// item's `id` (or `since_id` when the page is empty / 0 when absent).
/// The cursor is strictly non-decreasing — clients should save the largest
/// value seen and use it as `?since=<wall_time>&since_id=<id>` on resume.
///
/// Consumers: any client that persists this header value across reconnects
/// can resume an interrupted burst-drain without re-fetching already-ingested
/// items.
pub const RELAY_WATERMARK_HEADER: &str = "relay-watermark";

/// `Relay-Has-More: true|false` (CopyPaste-8ebg.58) — explicit signal that
/// more qualifying items exist past this page, independent of whether the
/// page came back shorter than the requested `limit`. A short page is
/// ambiguous on its own (see `PullPage::has_more` in `state::inbox::pull`): it can
/// mean either "inbox exhausted" or "the byte-budget cap truncated this page
/// mid-drain". Callers MUST use this header instead of inferring "caught up"
/// from `items.len() < limit`.
pub const RELAY_HAS_MORE_HEADER: &str = "relay-has-more";

/// GET /devices/:device_id/items?since=<wall_time>
///
/// Returns all items in `device_id`'s inbox with `wall_time > since`, ordered ascending.
/// Auth: `Authorization: Bearer <token>` — must match the token for `device_id`.
///
/// # Response headers
///
/// `Relay-Watermark: <wall_time>,<id>` (CopyPaste-tspz) — the effective next
/// cursor position after this page.  Clients interrupted mid-drain (e.g. by a
/// 401 token expiry) can persist this header and use it as `?since=&since_id=`
/// on reconnect to resume without discarding progress.
pub async fn pull(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Query(params): Query<PullParams>,
) -> Result<impl IntoResponse, RelayError> {
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

    let page = store.pull_items(&device_id, params.since, params.since_id, limit)?;
    let items = page.items;

    // CopyPaste-tspz: build the watermark header from the effective next cursor.
    //
    // If the page is non-empty, the next cursor is the last item's (wall_time, id).
    // If the page is empty, echo the incoming (since, since_id) back so the client
    // always has a valid cursor to persist, even on an empty-inbox reconnect.
    //
    // The header value is `<wall_time>,<id>` — both i64/u64, comma-separated,
    // easily parsed by `split_once(',')`.
    let watermark_val = if let Some(last) = items.last() {
        format!("{},{}", last.wall_time, last.id)
    } else {
        format!("{},{}", params.since, params.since_id.unwrap_or(0))
    };
    // HeaderValue::from_str is infallible for digit+comma strings.
    let watermark_header =
        HeaderValue::from_str(&watermark_val).unwrap_or_else(|_| HeaderValue::from_static("0,0"));

    let mut resp = Json(items).into_response();
    resp.headers_mut()
        .insert(RELAY_WATERMARK_HEADER, watermark_header);
    // CopyPaste-8ebg.58: explicit has_more signal — see RELAY_HAS_MORE_HEADER.
    resp.headers_mut().insert(
        RELAY_HAS_MORE_HEADER,
        HeaderValue::from_static(if page.has_more { "true" } else { "false" }),
    );
    Ok(resp)
}

// ---------------------------------------------------------------------------
// SSE push (issue #26)
// GET /devices/:id/subscribe?since=<wall_time>&since_id=<id>
// ---------------------------------------------------------------------------

/// `GET /devices/:device_id/subscribe?since=<wall_time>&since_id=<id>`
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
    //
    // CopyPaste-h7i8: enforce the per-device concurrent SSE connection cap
    // INSIDE the critical section (after auth, before opening the stream) so
    // the count check and receiver creation are atomic with respect to other
    // concurrent subscribe calls for the same device.
    let mut rx = {
        let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.verify_token(&device_id, &token)?;
        store.update_last_seen(&device_id);

        // Count live receivers = open SSE streams for this device. The check
        // is inside the lock so a burst of concurrent subscriptions cannot
        // race past the limit.
        let live = store.notifier_receiver_count(&device_id);
        if live >= SSE_MAX_CONNECTIONS_PER_DEVICE {
            tracing::warn!(
                device_id = %device_id,
                live_connections = live,
                limit = SSE_MAX_CONNECTIONS_PER_DEVICE,
                "CopyPaste-h7i8: SSE connection limit reached for device"
            );
            return Err(RelayError::TooManyConnections {
                limit: SSE_MAX_CONNECTIONS_PER_DEVICE,
            });
        }

        store.subscribe_notifier(&device_id)
    };

    let (tx, stream_rx) = mpsc::channel::<Result<Event, Infallible>>(SSE_CHANNEL_CAP);

    // Producer task: owns the advancing cursor and the broadcast receiver. It
    // drains the inbox from the cursor (backfill on first pass, then on each
    // wake), emitting one `item` event per row, and parks on `rx.recv()` between
    // drains. It exits when the SSE body is dropped (client disconnect →
    // `tx.send` errors) or the device's wake channel is closed (device evicted).
    //
    // CopyPaste-bp3o: the producer JoinHandle is retained in a monitoring task
    // instead of being dropped. If the producer panics, the monitor logs the
    // event at ERROR so it surfaces in production telemetry. The producer is
    // NOT restarted on panic (it is a per-connection task; the connection is
    // already dead). The monitor task is lightweight: it blocks only on
    // producer completion, then exits.
    let producer_state = state.clone();
    let producer_device = device_id.clone();
    let producer_handle = tokio::spawn(async move {
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
                        Ok(page) => page.items,
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

    // CopyPaste-bp3o: monitor the producer handle. If the producer panics,
    // the monitor logs it at ERROR. Without this the panic is silently
    // swallowed — the SSE stream goes dead with no log entry, making the
    // failure invisible to operators. The monitor task is self-terminating:
    // it exits as soon as the producer finishes (normal disconnect or panic).
    // We use a plain `tokio::spawn` here rather than `spawn_oneshot_supervised`
    // to avoid an extra task layer; the monitor itself contains no logic that
    // can panic (only an `.await` + match), so supervision of the monitor would
    // be unnecessary complexity.
    tokio::spawn(async move {
        match producer_handle.await {
            Ok(()) => {} // Producer exited normally (client disconnected).
            Err(join_err) if join_err.is_panic() => {
                tracing::error!("CopyPaste-bp3o: SSE producer task panicked");
            }
            Err(_) => {} // Cancelled — normal on server shutdown.
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

#[cfg(test)]
mod decoded_len_tests {
    use super::decoded_len_padded_b64;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;

    /// CopyPaste-crh3.72: the O(1) length must equal a real decode for every
    /// padding case (len % 3 == 0/1/2 → pad 0/2/1), without allocating.
    #[test]
    fn matches_real_decode_for_all_lengths() {
        for len in 0..200usize {
            let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let encoded = B64.encode(&data);
            assert_eq!(
                decoded_len_padded_b64(&encoded),
                Some(data.len()),
                "len={len} encoded={encoded}",
            );
        }
        for len in [1024usize, 4096, 10_000] {
            let encoded = B64.encode(vec![0xABu8; len]);
            assert_eq!(decoded_len_padded_b64(&encoded), Some(len));
        }
    }

    #[test]
    fn rejects_invalid_padded_length() {
        assert_eq!(decoded_len_padded_b64("abc"), None); // length 3
        assert_eq!(decoded_len_padded_b64("abcde"), None); // length 5
        assert_eq!(decoded_len_padded_b64(""), Some(0));
    }
}
