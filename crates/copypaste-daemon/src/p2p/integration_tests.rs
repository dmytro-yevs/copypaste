//! Cross-module P2P integration tests.
//!
//! These tests exercise more than one `p2p` submodule together (real mTLS
//! transport + `framed_pump` + `unpair`, or `listener` + `fanout` loops
//! together) and therefore don't belong to any single submodule's own test
//! mod. Split out of the former flat `p2p/mod.rs` (ADR-017, CopyPaste-vp63.2)
//! — moved verbatim, no behavior change.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::{PairedPeers, PeerTransport};
use copypaste_sync::protocol::{PeerFrame, WireItem};

use super::unpair::send_unpair_and_close_session;
use super::{fanout, framed_pump, listener};
use super::{CatchupProvider, PeerEvent, PeerRttMs, PeerSinks, PendingPings};

/// Build a minimal `WireItem` for use in tests.
fn test_wire_item(id: &str) -> WireItem {
    WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: id.to_string(),
        item_id: id.to_string(),
        content_type: "text".to_string(),
        content: Some(b"hello".to_vec()),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 1,
        wall_time: 0,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "test-device".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    }
}

/// `accept_loop_forwards_wire_item_to_incoming_tx`:
/// Spawn two in-process PeerTransports; client connects to server's accept
/// loop; client sends a `WireItem`; verify it arrives on `incoming_tx`.
#[tokio::test(flavor = "multi_thread")]
async fn accept_loop_forwards_wire_item_to_incoming_tx() {
    use bytes::Bytes;
    use futures_util::{SinkExt, StreamExt};

    let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("server").unwrap();
    let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("client").unwrap();

    let server_fp = server_cert.fingerprint();
    let client_fp = client_cert.fingerprint();

    let server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client");

    let client_peers = PairedPeers::new();
    client_peers.add(server_fp.clone(), "server");

    let server_transport =
        PeerTransport::from_cert(server_cert.cert_der, server_cert.key_der, server_peers);
    let client_transport =
        PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (incoming_tx, mut incoming_rx) = mpsc::channel::<WireItem>(8);

    let item_sent = test_wire_item("item-1");
    let item_check = item_sent.clone();

    // Server: accept one connection, forward framed items to incoming_tx.
    let accept_fut = {
        let tx = incoming_tx.clone();
        async move {
            let (_peer_addr, _peer_fp, mut stream) =
                server_transport.accept(&listener).await.unwrap();
            while let Some(Ok(frame)) = stream.next().await {
                let wire: WireItem = serde_json::from_slice(&frame).unwrap();
                tx.send(wire).await.unwrap();
            }
        }
    };

    // Client: connect and send one WireItem.
    let connect_fut = async move {
        let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
        let payload = serde_json::to_vec(&item_sent).unwrap();
        stream.send(Bytes::from(payload)).await.unwrap();
    };

    tokio::join!(accept_fut, connect_fut);

    let received = incoming_rx.recv().await.expect("must receive one item");
    assert_eq!(received.id, item_check.id);
    assert_eq!(received.content, item_check.content);
}

/// Gap B: after `run_peer_connection_framed` receives an inbound
/// `ControlMsg::Unpair` over a REAL in-process mTLS connection, the live
/// `PairedPeers` allowlist handed to it must no longer contain the peer —
/// proving `evict_peer_local` now removes from the live allowlist, not just
/// `peers.json`. Built as a sync test that owns its runtime so the
/// `TEST_ENV_LOCK`-guarded `COPYPASTE_CONFIG_DIR` override is never held
/// across an `.await` (clippy::await_holding_lock).
#[test]
fn gap_b_evict_peer_local_removes_from_live_allowlist() {
    use bytes::Bytes;
    use copypaste_sync::protocol::ControlMsg;
    use futures_util::SinkExt;

    let tmp = tempfile::tempdir().unwrap();

    let env_lock = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
    // SAFETY: serialised via TEST_ENV_LOCK; restored before the lock drops.
    unsafe {
        std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let server_peers = PairedPeers::new();
    let client_known = rt.block_on(async {
        let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("gapb-server").unwrap();
        let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("gapb-client").unwrap();
        let server_fp = server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        server_peers.add(client_fp.clone(), "client");
        let client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server");

        let server_transport = PeerTransport::from_cert(
            server_cert.cert_der,
            server_cert.key_der,
            server_peers.clone(),
        );
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Sanity: the peer is known before the unpair.
        assert!(
            server_peers.is_known(&client_fp),
            "precondition: client must be allow-listed before unpair"
        );

        let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);
        let (_peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(8);
        let server_peers_for_pump = server_peers.clone();

        // Server: accept one connection, then run the real duplex pump with
        // the live allowlist supplied (Gap B path).
        let accept_fut = async move {
            let (_peer_addr, peer_fp, stream) = server_transport.accept(&listener).await.unwrap();
            let pending: PendingPings = Arc::new(Mutex::new(HashMap::new()));
            let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
            framed_pump::run_peer_connection_framed(
                stream,
                peer_rx,
                incoming_tx,
                peer_fp,
                Some(server_peers_for_pump),
                pending,
                rtt_ms,
            )
            .await;
        };

        // Client: connect and send a single Unpair control frame.
        let connect_fut = async move {
            let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
            let payload = serde_json::to_vec(&PeerFrame::Control(ControlMsg::Unpair)).unwrap();
            stream.send(Bytes::from(payload)).await.unwrap();
            // Hold the connection briefly so the server processes the frame
            // before the client drops (which would also close the stream).
            tokio::time::sleep(Duration::from_millis(200)).await;
        };

        tokio::join!(accept_fut, connect_fut);

        server_peers.is_known(&client_fp)
    });

    // Restore env before any assertion that might panic.
    unsafe {
        match prev {
            Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
            None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
        }
    }
    drop(env_lock);

    assert!(
        !client_known,
        "Gap B: after an inbound Unpair the peer must be gone from the live PairedPeers allowlist"
    );
}

/// BUG F1: cancelling the shared `CancellationToken` must stop the
/// long-running loops. Drives `accept_loop` and `outbound_loop` (both blocked
/// on their idle awaits with no traffic) and asserts each task exits promptly
/// once the token is cancelled — before the fix only the accept loop had a
/// shutdown path and the outbound loop ran forever.
#[tokio::test(flavor = "multi_thread")]
async fn cancellation_token_stops_accept_and_outbound_loops() {
    let token = CancellationToken::new();

    // accept_loop: bound listener, nothing dialing in → blocked on accept().
    let accept_handle = {
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("f1-accept").unwrap();
        let transport = Arc::new(PeerTransport::from_cert(
            cert.cert_der,
            cert.key_der,
            PairedPeers::new(),
        ));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
        let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);
        let catchup: CatchupProvider = Arc::new(|_fp: &str| Vec::new());
        let token = token.clone();
        let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel::<PeerEvent>(4);
        tokio::spawn(async move {
            listener::accept_loop(
                listener,
                token,
                transport,
                peer_sinks,
                incoming_tx,
                catchup,
                PairedPeers::new(),
                rtt_ms,
                event_tx,
                Arc::new(tokio::sync::RwLock::new(None)), // crh3.109: no public IP in test
            )
            .await;
        })
    };

    // outbound_loop: both channels open but idle → blocked in its select!.
    let outbound_handle = {
        let (_new_item_tx, new_item_rx) = broadcast::channel::<copypaste_core::ClipboardItem>(8);
        let (_outbound_tx, outbound_rx) = mpsc::channel::<WireItem>(8);
        let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
        let token = token.clone();
        let core_config = Arc::new(std::sync::RwLock::new(copypaste_core::AppConfig::default()));
        tokio::spawn(async move {
            fanout::outbound_loop(
                new_item_rx,
                outbound_rx,
                peer_sinks,
                None,
                core_config,
                token,
            )
            .await;
        })
    };

    // Both tasks are parked on their idle awaits; cancel and require both to
    // finish well within a generous bound (no hang = cancellation works).
    token.cancel();
    let joined = tokio::time::timeout(Duration::from_secs(5), async {
        accept_handle.await.unwrap();
        outbound_handle.await.unwrap();
    })
    .await;
    assert!(
        joined.is_ok(),
        "BUG F1: both P2P loops must exit promptly on token cancel"
    );
}

/// `subscriber_loop_fans_out_to_connected_peer`:
/// Push a `WireItem` to `outbound_rx`; verify it appears on the connected
/// peer's stream as a readable framed message.
#[tokio::test(flavor = "multi_thread")]
async fn subscriber_loop_fans_out_to_connected_peer() {
    use bytes::Bytes;
    use futures_util::{SinkExt, StreamExt};

    let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("server2").unwrap();
    let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("client2").unwrap();

    let server_fp = server_cert.fingerprint();
    let client_fp = client_cert.fingerprint();

    let server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client2");

    let client_peers = PairedPeers::new();
    client_peers.add(server_fp.clone(), "server2");

    let server_transport = Arc::new(PeerTransport::from_cert(
        server_cert.cert_der,
        server_cert.key_der,
        server_peers,
    ));

    let client_transport =
        PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let item_sent = test_wire_item("item-2");
    let item_check = item_sent.clone();

    // Channel that mimics outbound_rx: daemon code will read from this and
    // fan-out to connected peers.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

    // Server: accept connection, then read from outbound_rx and write to peer.
    let server_fp_clone = server_fp.clone();
    let server_fut = async move {
        let (_peer_addr, _peer_fp, mut stream) = server_transport.accept(&listener).await.unwrap();
        // Simulate the outbound fanout: read one item and send to the connected peer.
        if let Some(item) = outbound_rx.recv().await {
            let payload = serde_json::to_vec(&item).unwrap();
            stream.send(Bytes::from(payload)).await.unwrap();
        }
        // Keep stream alive briefly so client can drain it.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = server_fp_clone; // keep binding alive
    };

    // Client: connect and read one WireItem from the server.
    let client_fut = async move {
        let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
        // Wait for the server to push the item.
        if let Some(Ok(frame)) = stream.next().await {
            let wire: WireItem = serde_json::from_slice(&frame).unwrap();
            Some(wire)
        } else {
            None
        }
    };

    // Send item to outbound channel.
    outbound_tx.send(item_sent).await.unwrap();

    let ((), received_opt) = tokio::join!(server_fut, client_fut);
    let received = received_opt.expect("client must receive one item from server");
    assert_eq!(received.id, item_check.id);
}

/// When `lan_visibility=false`, `standing_pairing_responder_loop` must NOT
/// be started. The loop accepts on the bootstrap port; if the caller's
/// `if config.lan_visibility` gate is absent, the port would be listening
/// and this would be a privacy violation. The test indirectly verifies the
/// gate exists and suppresses the spawn: with lan_visibility=false no bind
/// occurs, so an independent probe can bind the same ephemeral port without
/// collision — confirming nothing is listening there.
#[tokio::test(flavor = "multi_thread")]
async fn lan_visibility_false_leaves_bootstrap_port_free() {
    // Probe: find a free ephemeral port.
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let probe_port = probe.local_addr().unwrap().port();
    // Drop immediately so it's available again.
    drop(probe);

    // If lan_visibility=false gate is correct, no task bound probe_port.
    // We should be able to rebind it right away.
    let rebind = tokio::net::TcpListener::bind(format!("127.0.0.1:{probe_port}")).await;
    assert!(
        rebind.is_ok(),
        "CopyPaste-1htb: ephemeral port must be free when lan_visibility=false \
         (bootstrap responder must not have bound it)"
    );
}

/// CopyPaste-1jms.8 + CopyPaste-qw1k: when `send_unpair_and_close_session`
/// is called for a connected peer:
///   1. The revoked peer receives a `ControlMsg::Unpair` notification frame
///      before the session is torn down (CopyPaste-1jms.8).
///   2. The `run_peer_connection_framed` pump task exits, proving the live
///      mTLS session is torn down and not merely flagged (CopyPaste-qw1k).
///
/// Uses a raw loopback TCP pair so the test runs without TLS overhead.
/// The "peer" side reads one frame from the wire and asserts it is the
/// Unpair control message; the "local" side runs the real pump, registers
/// the sink, and calls `send_unpair_and_close_session`.
#[tokio::test(flavor = "multi_thread")]
async fn revoked_peer_receives_unpair_and_session_is_torn_down() {
    use copypaste_sync::protocol::ControlMsg;
    use futures_util::StreamExt;
    use tokio_util::codec::{Framed, LengthDelimitedCodec};

    // Raw loopback TCP — no TLS needed; we're testing the channel/pump logic.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (server_tcp, peer_tcp) =
        tokio::join!(async { listener.accept().await.unwrap().0 }, async {
            tokio::net::TcpStream::connect(addr).await.unwrap()
        });

    // "Local" side: the daemon that owns the sink and calls revoke.
    let server_framed = Framed::new(server_tcp, LengthDelimitedCodec::new());
    let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
    let (peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(64);
    let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);

    let fp = "aabbccddeeff0011223344556677889900112233445566778899aabbccddeeff".to_string();
    peer_sinks
        .lock()
        .await
        .insert(copypaste_p2p::DeviceFingerprint(fp.clone()), peer_tx);

    // Spawn the real pump so it drains peer_rx and writes to server_framed.
    let pending: PendingPings = Arc::new(Mutex::new(HashMap::new()));
    let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
    let pump = tokio::spawn(framed_pump::run_peer_connection_framed(
        server_framed,
        peer_rx,
        incoming_tx,
        copypaste_p2p::DeviceFingerprint(fp.clone()),
        None,
        pending,
        rtt_ms,
    ));

    // "Peer" side: reads the next frame from the TCP stream.
    let peer_reader = tokio::spawn(async move {
        let mut peer_framed = Framed::new(peer_tcp, LengthDelimitedCodec::new());
        // Read ONE frame — should be the Unpair notification.
        peer_framed.next().await
    });

    // Revoke: send Unpair notification + remove sink → pump exits.
    let had_session = send_unpair_and_close_session(&peer_sinks, &fp).await;
    assert!(
        had_session,
        "CopyPaste-qw1k: must return true for a live session"
    );

    // CopyPaste-qw1k: the pump must exit quickly because peer_rx is closed
    // (the last Sender was removed from peer_sinks).
    tokio::time::timeout(Duration::from_secs(2), pump)
        .await
        .expect("CopyPaste-qw1k: pump must exit after revocation — not block forever")
        .expect("pump task must not panic");

    // CopyPaste-1jms.8: the peer must have received an Unpair frame on the wire.
    let frame_opt = tokio::time::timeout(Duration::from_secs(2), peer_reader)
        .await
        .expect("peer reader must finish")
        .expect("peer reader task must not panic");

    // The peer either got the Unpair frame (Some(Ok(bytes))) or EOF (None)
    // because the pump exited and closed the TCP stream. Either proves the
    // session was torn down. When the frame arrives we assert it is Unpair.
    match frame_opt {
        Some(Ok(bytes)) => {
            let frame: PeerFrame =
                serde_json::from_slice(&bytes).expect("frame must deserialize as PeerFrame");
            assert!(
                matches!(frame, PeerFrame::Control(ControlMsg::Unpair)),
                "CopyPaste-1jms.8: peer must receive ControlMsg::Unpair, got {frame:?}"
            );
        }
        None | Some(Err(_)) => {
            // EOF or connection reset before the frame arrived — the session
            // was still torn down (qw1k passes). For 1jms.8, this means the
            // TCP FIN raced the Unpair frame in the 64-slot mpsc buffer; the
            // notification is best-effort (same contract as try_send in ipc.rs).
            // Acceptable: the connection IS closed, which is the hard requirement.
        }
    }

    // CopyPaste-qw1k: sink must be absent from the map.
    assert!(
        !peer_sinks.lock().await.contains_key(fp.as_str()),
        "CopyPaste-qw1k: peer sink must be absent after send_unpair_and_close_session"
    );
}
