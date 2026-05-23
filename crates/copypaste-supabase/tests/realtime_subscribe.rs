//! Beta-bonus: Phoenix Channel subscribe (phx_join) format + reconnect-on-disconnect.
//!
//! Strategy: instead of pulling in mockito (HTTP only) we spin up a real
//! WebSocket server on an ephemeral local port using `tokio-tungstenite`.
//!
//! Two scenarios are covered:
//!
//!   1. `subscribe_message_format` — start a one-shot WS server, point a
//!      hand-rolled tungstenite client at it, verify the first text frame
//!      the client sends matches the Phoenix wire format expected by Supabase
//!      Realtime: `[join_ref, msg_ref, topic, "phx_join", payload]`.
//!
//!   2. `reconnect_after_disconnect` — start a WS server that accepts a
//!      connection, then immediately closes it. Drive `RealtimeClient` against
//!      it and verify the server sees at least TWO incoming connections within
//!      a bounded window — proving the reconnect loop fired.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use copypaste_supabase::{PhoenixEvent, PhoenixMessage, RealtimeClient, RealtimeConfig};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, connect_async};

// ---------------------------------------------------------------------------
// 1. Subscribe message format (pure protocol assertion via real WS hop)
// ---------------------------------------------------------------------------

/// The first text frame the daemon sends after the WebSocket handshake must be
/// a Phoenix `phx_join` for the `realtime:clipboard_items` topic, serialised as
/// `[join_ref, msg_ref, topic, event, payload]`.
#[tokio::test]
async fn subscribe_message_format_matches_phoenix_wire() {
    // Reproduce what realtime.rs constructs in `run_session`.
    let join = PhoenixMessage::join("1", "1", RealtimeConfig::DEFAULT_TOPIC);
    let wire = join.to_wire().expect("serialise join");

    // Send it through a real WS hop so we exercise the same code path the
    // production client uses on the wire.
    let (server_addr, mut frames_rx) = spawn_ws_capture_server().await;
    let url = format!("ws://{server_addr}/realtime/v1/websocket");
    let (mut ws, _resp) = connect_async(url).await.expect("connect");
    ws.send(Message::Text(wire.clone())).await.expect("send");
    // Allow the server task to read.
    let received = tokio::time::timeout(Duration::from_secs(2), frames_rx.recv())
        .await
        .expect("server received a frame in time")
        .expect("frame is Some");
    let _ = ws.close(None).await;

    // The frame is a 5-element JSON array.
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(&received).expect("frame parses as JSON array");
    assert_eq!(arr.len(), 5, "Phoenix wire must be a 5-element array");

    // Element 2 (topic) is the clipboard_items topic.
    assert_eq!(
        arr[2].as_str(),
        Some(RealtimeConfig::DEFAULT_TOPIC),
        "topic must be {}",
        RealtimeConfig::DEFAULT_TOPIC
    );
    // Element 3 (event) is phx_join.
    assert_eq!(arr[3].as_str(), Some(PhoenixEvent::JOIN));
    // join_ref / msg_ref are present (non-null).
    assert!(
        arr[0].is_string(),
        "join_ref must be a string, got {}",
        arr[0]
    );
    assert!(
        arr[1].is_string(),
        "msg_ref must be a string, got {}",
        arr[1]
    );
    // Payload is an object (Phoenix expects an object, even if empty).
    assert!(
        arr[4].is_object(),
        "payload must be an object, got {}",
        arr[4]
    );
}

/// Symmetric round-trip — `PhoenixMessage::from_wire` of the join frame yields
/// a structurally-equal message back. Pins the serde contract.
#[test]
fn subscribe_message_round_trip_via_from_wire() {
    let join = PhoenixMessage::join("1", "1", RealtimeConfig::DEFAULT_TOPIC);
    let wire = join.to_wire().expect("serialise");
    let parsed = PhoenixMessage::from_wire(&wire).expect("parse");

    assert_eq!(parsed.topic, RealtimeConfig::DEFAULT_TOPIC);
    assert_eq!(parsed.event, PhoenixEvent::JOIN);
    assert_eq!(parsed.join_ref.as_deref(), Some("1"));
    assert_eq!(parsed.msg_ref.as_deref(), Some("1"));
}

// ---------------------------------------------------------------------------
// 2. Reconnect-on-disconnect (live WS server flake harness)
// ---------------------------------------------------------------------------

/// `RealtimeClient` must reconnect after the server drops the WebSocket.
/// We start a server that accepts every TCP connection but closes the WS
/// immediately after the handshake, then assert the connection counter
/// reaches 2+ within a generous window.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_after_disconnect_fires_at_least_twice() {
    let (server_addr, connection_count) = spawn_ws_drop_server().await;

    // Build a config that points at the local flake server and uses tiny
    // backoff windows so we don't sit around in the test.
    let mut config = RealtimeConfig::new(
        format!("http://{server_addr}"),
        "test-anon-key",
        RealtimeConfig::DEFAULT_TOPIC,
        /* enabled */ true,
    );
    config.initial_backoff = Duration::from_millis(50);
    config.max_backoff = Duration::from_millis(200);
    config.heartbeat_interval = Duration::from_secs(60); // irrelevant here

    let (client, _rx) = RealtimeClient::new(config);
    let handle = client.connect().await.expect("connect must return handle");

    // Give the reconnect loop a few cycles to fire.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let observed = connection_count.load(Ordering::SeqCst);
    handle.shutdown().await;

    assert!(
        observed >= 2,
        "expected the client to reconnect at least once (>=2 connections), got {observed}"
    );
}

// ---------------------------------------------------------------------------
// Test fixtures: local WS servers
// ---------------------------------------------------------------------------

/// Spawn a WebSocket server on `127.0.0.1:0` that:
///   * Accepts ONE connection,
///   * Reads the first text frame,
///   * Forwards it on the returned mpsc channel,
///   * Then drops the connection.
async fn spawn_ws_capture_server() -> (SocketAddr, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");

    let (tx, rx) = mpsc::channel::<String>(4);

    tokio::spawn(async move {
        if let Ok((stream, _peer)) = listener.accept().await {
            handle_capture_conn(stream, tx).await;
        }
    });

    (addr, rx)
}

async fn handle_capture_conn(stream: TcpStream, tx: mpsc::Sender<String>) {
    let ws = match accept_async(stream).await {
        Ok(w) => w,
        Err(_) => return,
    };
    let (mut _sink, mut src) = ws.split();
    if let Some(Ok(Message::Text(text))) = src.next().await {
        let _ = tx.send(text).await;
    }
}

/// Spawn a WebSocket server that accepts connections and immediately closes
/// them. Returns an atomic counter that increments on every TCP accept.
async fn spawn_ws_drop_server() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    tokio::spawn(async move {
        while let Ok((stream, _peer)) = listener.accept().await {
            count_clone.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                // Complete the WS handshake then close immediately so the
                // client sees `Disconnected`, not `ConnectError`.
                if let Ok(mut ws) = accept_async(stream).await {
                    let _ = ws.close(None).await;
                }
            });
        }
    });

    (addr, count)
}
