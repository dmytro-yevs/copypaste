use bytes::Bytes;
use copypaste_core::{encrypt_for_cloud, SyncKey};
use copypaste_p2p::pake::SessionKey;
use copypaste_p2p::transport::{PairedPeers, PeerTransport as ClientTransport};
use copypaste_sync::protocol::WireItem;
use futures_util::SinkExt;
use std::time::Duration;

use crate::{generate_device_cert, P2P_WIRE_KEY_VERSION};

use super::{poll, start, stop, update_peers, PeerSessionKey};

/// Spin up the shared test runtime once. Tests must not create nested
/// runtimes when they call `start` (which blocks on the runtime to bind).
fn test_runtime() -> &'static tokio::runtime::Runtime {
    crate::ffi_pairing::runtime().expect("test tokio runtime builds")
}

/// A fixed 32-byte PAKE session key both ends agree on (bootstrap output).
const TEST_SESSION_KEY: [u8; 32] = [0x5Au8; 32];

/// Derive the shared content key the listener and the dialer both use, the
/// SAME way `shared_sync_key_from_session` does.
fn shared_test_key() -> SyncKey {
    let sk = SessionKey(TEST_SESSION_KEY);
    SyncKey::from_bytes(*sk.derive_xchacha_key(crate::P2P_SYNC_KEY_SALT))
}

/// Drive a single dial against the running listener from a dedicated OS
/// thread + runtime: connect over mTLS pinning `listener_fp`, send one
/// framed `WireItem`, then hold the link briefly so the listener can read
/// it. Returns `Ok(())` if the handshake + send succeeded, `Err` if the
/// handshake was rejected (e.g. revoked/unpinned at accept).
fn dial_and_send(
    addr: std::net::SocketAddr,
    listener_fp: String,
    client_cert_der: Vec<u8>,
    client_key_der: Vec<u8>,
    wire: WireItem,
) -> Result<(), String> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("client runtime");
        rt.block_on(async move {
            let peers = PairedPeers::new();
            peers.add(listener_fp.clone(), "listener");
            let transport = ClientTransport::from_cert(client_cert_der, client_key_der, peers);
            let mut framed = transport
                .connect(addr, &listener_fp)
                .await
                .map_err(|e| format!("connect failed: {e}"))?;
            let payload = serde_json::to_vec(&wire).map_err(|e| e.to_string())?;
            framed
                .send(Bytes::from(payload))
                .await
                .map_err(|e| format!("send failed: {e}"))?;
            // Keep the link open briefly so the listener reads the frame.
            tokio::time::sleep(Duration::from_millis(300)).await;
            Ok::<(), String>(())
        })
    })
    .join()
    .expect("client thread")
}

/// Build a sync-key-wrapped text `WireItem` carrying `plaintext` under
/// `shared` (the on-wire shape the listener decrypts).
fn make_wire_item(shared: &SyncKey, plaintext: &[u8]) -> WireItem {
    let item_id = uuid::Uuid::new_v4().to_string();
    let blob = encrypt_for_cloud(shared, &item_id, plaintext).expect("wrap item");
    WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: item_id.clone(),
        item_id,
        content_type: "text".to_string(),
        content: Some(blob),
        content_nonce: None,
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 7,
        wall_time: 7,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "test-dialer".to_string(),
        key_version: P2P_WIRE_KEY_VERSION,
        file_name: None,
        mime: None,
    }
}

/// Variant of `dial_and_send` that transmits raw serialised bytes rather than
/// a `WireItem`.  Useful for injecting control frames such as
/// `PeerFrame::Control(ControlMsg::Unpair)` that are not `WireItem`s.
///
/// After sending the bytes the dialer holds the link open for `hold_ms`
/// milliseconds so the listener's read loop has time to process the frame
/// before the connection drops.
fn dial_and_send_bytes(
    addr: std::net::SocketAddr,
    listener_fp: String,
    client_cert_der: Vec<u8>,
    client_key_der: Vec<u8>,
    payload: Vec<u8>,
    hold_ms: u64,
) -> Result<(), String> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("client runtime");
        rt.block_on(async move {
            let peers = PairedPeers::new();
            peers.add(listener_fp.clone(), "listener");
            let transport = ClientTransport::from_cert(client_cert_der, client_key_der, peers);
            let mut framed = transport
                .connect(addr, &listener_fp)
                .await
                .map_err(|e| format!("connect failed: {e}"))?;
            framed
                .send(Bytes::from(payload))
                .await
                .map_err(|e| format!("send failed: {e}"))?;
            tokio::time::sleep(Duration::from_millis(hold_ms)).await;
            Ok::<(), String>(())
        })
    })
    .join()
    .expect("client thread")
}

/// Allowlist pinning + loopback handshake: a dialer whose fingerprint IS in
/// the allowlist completes the handshake; the listener decrypts its framed
/// item and surfaces it via `poll`.
#[test]
fn allowlist_pinned_peer_handshakes_and_item_is_received() {
    let listener_cert = generate_device_cert().expect("listener cert");
    let client_cert = generate_device_cert().expect("client cert");
    let client_fp = client_cert.fingerprint.clone();
    let listener_fp = listener_cert.fingerprint.clone();

    let handle = start(
        test_runtime(),
        0,
        listener_cert.cert_der.clone(),
        listener_cert.key_der.clone(),
        vec![client_fp.clone()],
        Vec::new(), // no revocations
        vec![PeerSessionKey {
            fingerprint: client_fp.clone(),
            session_key: TEST_SESSION_KEY.to_vec(),
        }],
        Vec::new(), // no catch-up history
        "listener-device".to_string(),
    )
    .expect("listener starts");

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
        .parse()
        .expect("addr");
    let shared = shared_test_key();
    let plaintext = b"hello from the macOS dialer".to_vec();
    let wire = make_wire_item(&shared, &plaintext);

    dial_and_send(
        addr,
        listener_fp,
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        wire,
    )
    .expect("pinned peer handshake must succeed");

    // Poll for the decrypted item (give the accept task a moment to run).
    let mut got = Vec::new();
    for _ in 0..50 {
        got = poll(handle.listener_id).expect("poll");
        if !got.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    stop(handle.listener_id).expect("stop");

    assert_eq!(got.len(), 1, "listener must surface the dialer's item");
    assert_eq!(got[0].plaintext, plaintext, "decrypted plaintext mismatch");
}

/// Denylist enforced AT ACCEPT: a dialer whose (pinned) fingerprint is also
/// in the revoked list is dropped before any catch-up/frame, so `poll`
/// never yields its item.
#[test]
fn revoked_peer_is_rejected_at_accept() {
    let listener_cert = generate_device_cert().expect("listener cert");
    let client_cert = generate_device_cert().expect("client cert");
    let client_fp = client_cert.fingerprint.clone();
    let listener_fp = listener_cert.fingerprint.clone();

    // The fingerprint is BOTH allowed (so TLS would complete) AND revoked
    // (so the accept-time denylist check must drop it).
    let handle = start(
        test_runtime(),
        0,
        listener_cert.cert_der.clone(),
        listener_cert.key_der.clone(),
        vec![client_fp.clone()],
        vec![client_fp.clone()], // revoked
        vec![PeerSessionKey {
            fingerprint: client_fp.clone(),
            session_key: TEST_SESSION_KEY.to_vec(),
        }],
        Vec::new(),
        "listener-device".to_string(),
    )
    .expect("listener starts");

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
        .parse()
        .expect("addr");
    let shared = shared_test_key();
    let wire = make_wire_item(&shared, b"should never be received");

    // The TLS handshake itself may complete (fp is pinned) — the rejection
    // is at the application layer (denylist) right after accept. The dialer
    // may therefore see send succeed; what matters is the listener drops the
    // connection and never decrypts the item.
    let _ = dial_and_send(
        addr,
        listener_fp,
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        wire,
    );

    // Give the accept task time to run and (correctly) drop the connection.
    std::thread::sleep(Duration::from_millis(400));
    let got = poll(handle.listener_id).expect("poll");
    stop(handle.listener_id).expect("stop");

    assert!(
        got.is_empty(),
        "a revoked peer's item must NOT be received (denylist enforced at accept)"
    );
}

/// Registry lifecycle: start registers a handle, poll on a live id returns
/// (empty) Ok, stop deregisters, and a poll/stop after stop is a safe no-op.
#[test]
fn start_poll_stop_registry_lifecycle() {
    let cert = generate_device_cert().expect("cert");
    let handle = start(
        test_runtime(),
        0,
        cert.cert_der.clone(),
        cert.key_der.clone(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        "lifecycle-device".to_string(),
    )
    .expect("listener starts");

    assert!(handle.listener_id >= 1, "ids start at 1");
    assert!(handle.actual_port > 0, "OS assigned a real port");

    // Live id: poll yields an empty drain without error.
    let drained = poll(handle.listener_id).expect("poll live id");
    assert!(drained.is_empty());

    // Stop deregisters.
    stop(handle.listener_id).expect("stop");

    // After stop: poll returns empty (unknown id), stop is idempotent.
    let after = poll(handle.listener_id).expect("poll after stop is Ok");
    assert!(after.is_empty(), "unknown id drains empty");
    stop(handle.listener_id).expect("second stop is a no-op");

    // An entirely unknown id is also a safe no-op for update_peers.
    update_peers(999_999, Vec::new(), Vec::new(), Vec::new())
        .expect("update on unknown id is a no-op");
}

/// `update_peers` on a running listener swaps the roster: a newly-revoked
/// fingerprint is removed from the pinned allowlist so its next handshake is
/// rejected at TLS.
#[test]
fn update_peers_evicts_revoked_from_allowlist() {
    let listener_cert = generate_device_cert().expect("listener cert");
    let client_cert = generate_device_cert().expect("client cert");
    let client_fp = client_cert.fingerprint.clone();
    let listener_fp = listener_cert.fingerprint.clone();

    let handle = start(
        test_runtime(),
        0,
        listener_cert.cert_der.clone(),
        listener_cert.key_der.clone(),
        vec![client_fp.clone()],
        Vec::new(),
        vec![PeerSessionKey {
            fingerprint: client_fp.clone(),
            session_key: TEST_SESSION_KEY.to_vec(),
        }],
        Vec::new(),
        "listener-device".to_string(),
    )
    .expect("listener starts");

    // Now revoke the client: remove from allowlist + add to denylist.
    update_peers(
        handle.listener_id,
        Vec::new(),              // no longer allowed
        vec![client_fp.clone()], // revoked
        Vec::new(),
    )
    .expect("update_peers");

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
        .parse()
        .expect("addr");
    let shared = shared_test_key();
    let wire = make_wire_item(&shared, b"post-revocation item");

    // After the live update, the dialer's fingerprint is no longer pinned
    // AND is on the denylist. The handshake is rejected at the verifier
    // and/or the connection is dropped at accept by the denylist re-check —
    // either way the load-bearing guarantee is that NO item is received.
    // (The client-side connect may or may not observe the rejection
    // depending on TLS alert timing, so we assert on the security outcome
    // — no item delivered — rather than on the dialer's error.)
    let _ = dial_and_send(
        addr,
        listener_fp,
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        wire,
    );

    std::thread::sleep(Duration::from_millis(300));
    let got = poll(handle.listener_id).expect("poll");
    stop(handle.listener_id).expect("stop");

    assert!(
        got.is_empty(),
        "a revoked/unpinned dialer's item must NOT be received after a live roster update"
    );
}

/// PG-1 (CopyPaste-7d8x): inbound `PeerFrame::Control(ControlMsg::Unpair)` evicts
/// the peer from the live allowlist.
///
/// # What this tests
///
/// The `run_connection` inbound read loop parses every frame as a `PeerFrame`
/// (not a bare `WireItem`). When a `ControlMsg::Unpair` frame arrives the handler:
///   1. Removes the peer from `peer_state.peers` (the interior-mutable `PairedPeers`
///      shared with the `PeerTransport` verifier) so subsequent handshakes are rejected.
///   2. Removes the peer from `peer_state.allowed`.
///   3. Adds the peer to `peer_state.revoked` (defence-in-depth: the accept-time
///      denylist check will also drop any reconnect).
///   4. Closes the current connection.
///
/// This test injects a serialised `PeerFrame::Control(ControlMsg::Unpair)` over a
/// live mTLS connection and then asserts the security outcome: a reconnect attempt
/// with the same cert is rejected (no item is received after the Unpair).
///
/// Previously this test was requested-but-never-added when the code path was
/// first written. This is the instrumented regression test that closes the gap.
#[test]
fn inbound_unpair_control_frame_evicts_peer_from_allowlist() {
    use copypaste_sync::protocol::{ControlMsg, PeerFrame};

    let listener_cert = generate_device_cert().expect("listener cert");
    let client_cert = generate_device_cert().expect("client cert");
    let client_fp = client_cert.fingerprint.clone();
    let listener_fp = listener_cert.fingerprint.clone();

    let handle = start(
        test_runtime(),
        0,
        listener_cert.cert_der.clone(),
        listener_cert.key_der.clone(),
        vec![client_fp.clone()],
        Vec::new(), // no initial revocations
        vec![PeerSessionKey {
            fingerprint: client_fp.clone(),
            session_key: TEST_SESSION_KEY.to_vec(),
        }],
        Vec::new(), // no catch-up history
        "listener-device".to_string(),
    )
    .expect("listener starts");

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
        .parse()
        .expect("addr");

    // Step 1: send a legitimate item to confirm the peer is initially allowed.
    let shared = shared_test_key();
    let first_item = make_wire_item(&shared, b"item before unpair");
    dial_and_send(
        addr,
        listener_fp.clone(),
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        first_item,
    )
    .expect("first connection (before Unpair) must succeed");

    // Poll briefly to drain the first item.
    std::thread::sleep(Duration::from_millis(400));
    let before = poll(handle.listener_id).expect("poll before unpair");
    assert_eq!(
        before.len(),
        1,
        "listener must receive the pre-Unpair item (confirms peer WAS allowed)"
    );

    // Step 2: connect again and send a `PeerFrame::Control(ControlMsg::Unpair)` frame.
    // The frame serialises to {"control":"unpair"} (ControlMsg is tagged "control",
    // rename_all = "snake_case" → variant Unpair → "unpair").
    let unpair_payload =
        serde_json::to_vec(&PeerFrame::Control(ControlMsg::Unpair)).expect("serialise Unpair");
    // The listener read loop must process the Unpair and close the link.
    // hold_ms=400 gives the listener task time to process the frame.
    let _ = dial_and_send_bytes(
        addr,
        listener_fp.clone(),
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        unpair_payload,
        400,
    );

    // Give the accept task and run_connection task time to complete the eviction.
    std::thread::sleep(Duration::from_millis(400));

    // Step 3: attempt a third connection with the same cert.
    // Because run_connection evicted the peer from `peer_state.peers` (the interior-
    // mutable PairedPeers shared with the PeerTransport verifier) AND added it to
    // `peer_state.revoked`, this reconnect should be refused: either the TLS verifier
    // rejects the handshake (unpinned) or the accept-time denylist check drops it.
    let post_item = make_wire_item(&shared, b"item after unpair - must not be received");
    // The reconnect may fail at TLS (unpinned) or succeed+then-be-dropped (denylist);
    // either way, the security assertion is that NO item is received.
    let _ = dial_and_send(
        addr,
        listener_fp,
        client_cert.cert_der.clone(),
        client_cert.key_der.clone(),
        post_item,
    );

    std::thread::sleep(Duration::from_millis(400));
    let after = poll(handle.listener_id).expect("poll after unpair");
    stop(handle.listener_id).expect("stop");

    assert!(
        after.is_empty(),
        "after an inbound Unpair control frame, the peer must be evicted — \
         a subsequent connection from the same fingerprint must NOT deliver items \
         (PG-1 / CopyPaste-7d8x)"
    );
}
