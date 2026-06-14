//! Integration tests for the relay SSE push endpoint (`GET /devices/:id/subscribe`).
//!
//! Unlike the `oneshot`-based tests in `integration.rs`, SSE is a long-lived
//! streaming connection, so these tests bind a real ephemeral TCP port, serve
//! a router that wires `routes_items::subscribe` (mirroring the production
//! route table), and read the raw SSE byte stream off a `TcpStream` with a
//! read timeout. We parse just enough of the `text/event-stream` framing
//! (`event:`, `id:`, `data:` lines separated by a blank line) to assert that
//! the expected item arrives.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/quota.rs"]
mod quota;
#[path = "../src/routes/devices.rs"]
mod routes_devices;
#[path = "../src/routes/health.rs"]
mod routes_health;
#[path = "../src/routes/items.rs"]
mod routes_items;
#[path = "../src/state.rs"]
mod state;

use config::RelayConfig;
use state::{AppState, RelayStore};

const DEVICE_B: &str = "22222222-2222-2222-2222-222222222222";

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}
fn valid_pop() -> String {
    B64.encode([0xDE_u8; 32])
}

fn sample_content_b64(payload: &[u8]) -> String {
    B64.encode(payload)
}

/// Build a router mirroring the production route table, including the SSE
/// `subscribe` route under test.
fn relay_router(state: AppState, config: RelayConfig) -> axum::Router {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/health", get(routes_health::handle))
        .route("/devices", post(routes_devices::register))
        .route("/devices/{device_id}", get(routes_devices::get_device))
        .route(
            "/devices/{device_id}/items",
            get(routes_items::pull).post(routes_items::push),
        )
        .route(
            "/devices/{device_id}/items/{item_id}",
            delete(routes_items::delete_item),
        )
        .route(
            "/devices/{device_id}/subscribe",
            get(routes_items::subscribe),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
        .layer(axum::Extension(config))
}

/// Spawn the relay router on an ephemeral port. Returns the bound `SocketAddr`
/// and the shared state so the test can register devices / push items directly.
async fn spawn_relay() -> (std::net::SocketAddr, AppState) {
    let config = RelayConfig::default();
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .unwrap();
    });
    (addr, app_state)
}

/// Open a raw HTTP GET against the relay and return the connected socket after
/// writing the request line + headers. The caller reads the SSE stream off it.
async fn open_sse(addr: std::net::SocketAddr, path: &str, token: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    stream
}

/// Read from `stream` until `needle` appears in the accumulated buffer or the
/// overall timeout elapses. Returns the full buffer read so far (lossy String).
async fn read_until(stream: &mut TcpStream, needle: &str, overall: Duration) -> String {
    let mut buf = Vec::new();
    let deadline = tokio::time::Instant::now() + overall;
    loop {
        if String::from_utf8_lossy(&buf).contains(needle) {
            return String::from_utf8_lossy(&buf).into_owned();
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return String::from_utf8_lossy(&buf).into_owned();
        }
        let mut chunk = [0u8; 4096];
        match tokio::time::timeout(remaining, stream.read(&mut chunk)).await {
            Ok(Ok(0)) => return String::from_utf8_lossy(&buf).into_owned(), // EOF
            Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
            Ok(Err(_)) => return String::from_utf8_lossy(&buf).into_owned(),
            Err(_) => return String::from_utf8_lossy(&buf).into_owned(), // timeout
        }
    }
}

/// A new item pushed to device B's inbox AFTER an SSE subscription is open must
/// arrive on the stream (push, not poll).
#[tokio::test]
async fn sse_delivers_item_pushed_after_subscribe() {
    let (addr, state) = spawn_relay().await;

    let b_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_B.to_string(),
            "Device B".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Open the SSE subscription with an empty inbox (since=0).
    let mut sse = open_sse(
        addr,
        &format!("/devices/{DEVICE_B}/subscribe?since=0"),
        &b_token,
    )
    .await;

    // Drain the HTTP response headers first (up to the blank line).
    let _headers = read_until(&mut sse, "\r\n\r\n", Duration::from_secs(2)).await;

    // Now push a NEW item into B's inbox via the store (simulating a fan-out
    // write from another device's POST). The relay must notify the open SSE
    // subscription.
    let unique = b"sse-after-subscribe-payload";
    {
        let mut s = state.lock().unwrap();
        s.push_item(
            DEVICE_B,
            "text".to_string(),
            sample_content_b64(unique),
            5000,
            10 * 1024 * 1024,
        )
        .unwrap();
    }

    let body = read_until(&mut sse, "event: item", Duration::from_secs(5)).await;
    assert!(
        body.contains("event: item"),
        "SSE stream must emit an `item` event after a push; got:\n{body}"
    );
    let expected_b64 = sample_content_b64(unique);
    assert!(
        body.contains(&expected_b64),
        "SSE data must carry the pushed item's content_b64 verbatim; got:\n{body}"
    );
}

/// On connect with `?since=<cursor>`, the relay must immediately flush items
/// already in the inbox newer than the cursor (backfill), without waiting for a
/// new push.
#[tokio::test]
async fn sse_backfills_preexisting_item_on_connect() {
    let (addr, state) = spawn_relay().await;

    let b_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_B.to_string(),
            "Device B".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Pre-existing item BEFORE any subscription exists.
    let unique = b"sse-backfill-payload";
    {
        let mut s = state.lock().unwrap();
        s.push_item(
            DEVICE_B,
            "text".to_string(),
            sample_content_b64(unique),
            1000,
            10 * 1024 * 1024,
        )
        .unwrap();
    }

    // Subscribe from before the item's wall_time — it must be replayed on connect.
    let mut sse = open_sse(
        addr,
        &format!("/devices/{DEVICE_B}/subscribe?since=0"),
        &b_token,
    )
    .await;

    let body = read_until(&mut sse, "event: item", Duration::from_secs(5)).await;
    let expected_b64 = sample_content_b64(unique);
    assert!(
        body.contains("event: item") && body.contains(&expected_b64),
        "SSE connect must backfill the pre-existing item from the since cursor; got:\n{body}"
    );
    // The SSE event id must be the item id (1) for Last-Event-ID resume.
    assert!(
        body.contains("id: 1"),
        "SSE event must carry `id: <item id>`; got:\n{body}"
    );
}

/// Resource-leak regression (P1/High): when an SSE producer is parked on an
/// idle inbox (blocked on the broadcast `rx.recv()`), a client TCP disconnect
/// must tear the producer task down promptly. Previously the producer only
/// noticed disconnect when `tx.send` failed during a drain, so with an empty
/// inbox it stayed parked — leaking the task, the broadcast receiver, and the
/// cloned `Arc<AppState>` until the next push or the 30-day eviction. The fix
/// `select!`s the broadcast wake against `tx.closed()`.
///
/// Observable: each open subscription's producer owns exactly one receiver of
/// the device's wake channel, so `notifier_receiver_count` == live producer
/// count. After the client drops, it must fall back to 0.
#[tokio::test]
async fn sse_producer_tears_down_on_client_disconnect_idle_inbox() {
    let (addr, state) = spawn_relay().await;

    let b_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_B.to_string(),
            "Device B".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Open the subscription against an EMPTY inbox: the producer backfills
    // nothing and parks on the broadcast wake — the exact leak scenario.
    let mut sse = open_sse(
        addr,
        &format!("/devices/{DEVICE_B}/subscribe?since=0"),
        &b_token,
    )
    .await;
    let _headers = read_until(&mut sse, "\r\n\r\n", Duration::from_secs(2)).await;

    // Wait until the producer has registered its broadcast receiver (count==1),
    // i.e. it is parked on the wake channel with an idle inbox.
    let parked = wait_for_count(&state, DEVICE_B, 1, Duration::from_secs(5)).await;
    assert!(
        parked,
        "SSE producer should hold exactly one wake-channel receiver once parked; \
         got {}",
        state.lock().unwrap().notifier_receiver_count(DEVICE_B)
    );

    // Client disconnects: dropping the socket closes the TCP connection, which
    // drops the SSE response body (the ReceiverStream) and thus the last `tx`
    // receiver. `tx.closed()` must wake the producer and end the task, dropping
    // its `rx` — receiver count falls to 0.
    drop(sse);

    let torn_down = wait_for_count(&state, DEVICE_B, 0, Duration::from_secs(5)).await;
    assert!(
        torn_down,
        "SSE producer must tear down (drop its wake receiver) on client \
         disconnect even with an idle inbox; receiver count stuck at {}",
        state.lock().unwrap().notifier_receiver_count(DEVICE_B)
    );

    // Server must remain responsive afterward: a subsequent push to the same
    // (now subscriber-less) device must not panic and must succeed.
    {
        let mut s = state.lock().unwrap();
        s.push_item(
            DEVICE_B,
            "text".to_string(),
            sample_content_b64(b"post-disconnect"),
            6000,
            10 * 1024 * 1024,
        )
        .unwrap();
    }
    assert_eq!(
        state.lock().unwrap().notifier_receiver_count(DEVICE_B),
        0,
        "no producer should be revived by a push after the client disconnected"
    );
}

/// Poll the device's SSE wake-channel receiver count until it equals `want` or
/// the deadline elapses. Deterministic: bounded by `overall`, short backoff.
async fn wait_for_count(state: &AppState, device_id: &str, want: usize, overall: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + overall;
    loop {
        if state.lock().unwrap().notifier_receiver_count(device_id) == want {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// SSE must require auth via the same BearerToken contract as /poll.
#[tokio::test]
async fn sse_rejects_missing_auth() {
    let (addr, state) = spawn_relay().await;
    {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_B.to_string(),
            "Device B".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap();
    }

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /devices/{DEVICE_B}/subscribe?since=0 HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();

    let head = read_until(&mut stream, "\r\n", Duration::from_secs(2)).await;
    assert!(
        head.contains("401"),
        "SSE without a bearer token must be 401 Unauthorized; got:\n{head}"
    );
}
