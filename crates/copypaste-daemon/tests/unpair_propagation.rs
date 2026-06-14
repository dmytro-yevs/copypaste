//! Integration tests for mutual-unpair propagation (bd CopyPaste-aqo).
//!
//! These exercise the PUBLIC `copypaste_daemon::peers` durable pending-unpair
//! API (Gap A) and assert the end-to-end "queue while offline → deliver on
//! reconnect" contract over a real in-process mTLS connection. The connector's
//! private `deliver_pending_unpairs` helper performs the same five steps these
//! tests drive through the public surface:
//!   1. read `pending_unpair.json`,
//!   2. temporarily allow-list the peer,
//!   3. dial + send a single `ControlMsg::Unpair` frame,
//!   4. dequeue the record,
//!   5. drop the transient allow-list entry.
//!
//! Hermetic: loopback TCP only, fresh self-signed certs, tempdir-backed files,
//! no Keychain, no mDNS.

use copypaste_daemon::peers;
use copypaste_p2p::transport::{PairedPeers, PeerTransport};
use copypaste_sync::protocol::{ControlMsg, PeerFrame};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;

/// Gap A — durable queue: a pending-unpair record persists across the
/// "peer offline" moment and is then deliverable on reconnect.
///
/// We simulate peer B being offline at unpair time by queuing the record to
/// `pending_unpair.json` (the exact durable store the IPC handlers write to when
/// `try_send` finds no live sink). We then bring B "online" as a real mTLS
/// listener, replay the connector's delivery steps through the public API, and
/// assert: (a) B receives a `ControlMsg::Unpair` frame, and (b) the record is
/// dequeued so it is delivered exactly once.
#[tokio::test(flavor = "multi_thread")]
async fn gap_a_pending_unpair_delivered_on_reconnect() {
    let tmp = tempfile::tempdir().unwrap();
    let peers_path = tmp.path().join("peers.json");
    let pending_path = peers::pending_unpair_path_for(&peers_path);

    // Fresh identities for A (us, the unpairer) and B (the peer being unpaired).
    let a_cert = copypaste_p2p::cert::SelfSignedCert::generate("unpair-A").unwrap();
    let b_cert = copypaste_p2p::cert::SelfSignedCert::generate("unpair-B").unwrap();
    let a_fp = a_cert.fingerprint();
    let b_fp = b_cert.fingerprint();

    // B's listener (B pins A; this is the standing accept side).
    let b_peers = PairedPeers::new();
    b_peers.add(a_fp.clone(), "device-A");
    let b_transport =
        PeerTransport::from_cert(b_cert.cert_der.clone(), b_cert.key_der.clone(), b_peers);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let b_addr = listener.local_addr().unwrap();

    // --- "Offline at unpair time": queue the durable pending-unpair record. ---
    // A's live `try_send` would have been dropped here (no live sink to B), so
    // the IPC handler instead persists B's fingerprint + last-known address.
    peers::queue_pending_unpair(&pending_path, &b_fp, Some(&b_addr.to_string()), "device-B")
        .unwrap();
    assert_eq!(
        peers::load_pending_unpairs(&pending_path).len(),
        1,
        "record must be durably queued before delivery"
    );

    // B's accept side: wait for ONE connection and read ONE frame, reporting
    // whether it was the expected Unpair control frame.
    let b_accept = tokio::spawn(async move {
        let (_addr, _peer_fp, mut stream) = b_transport.accept(&listener).await.unwrap();
        match stream.next().await {
            Some(Ok(frame)) => {
                matches!(
                    serde_json::from_slice::<PeerFrame>(&frame),
                    Ok(PeerFrame::Control(ControlMsg::Unpair))
                )
            }
            _ => false,
        }
    });

    // --- A's connector delivery steps (mirrors `deliver_pending_unpairs`). ---
    // A pins B only transiently to dial; A's own live allowlist starts empty.
    let a_live = PairedPeers::new();
    let a_transport = PeerTransport::from_cert(
        a_cert.cert_der.clone(),
        a_cert.key_der.clone(),
        a_live.clone(),
    );

    let pending = peers::load_pending_unpairs(&pending_path);
    assert_eq!(pending.len(), 1);
    let entry = &pending[0];

    // Step 2: temporarily allow-list B.
    a_live.add(entry.fingerprint.clone(), entry.name.clone());
    assert!(
        a_live.is_known(&b_fp),
        "B must be transiently allow-listed for the dial"
    );

    // Step 3: dial + send a single Unpair frame.
    let addr: std::net::SocketAddr = entry.address.as_deref().unwrap().parse().unwrap();
    let mut stream = a_transport.connect(addr, &b_fp).await.unwrap();
    let payload = serde_json::to_vec(&PeerFrame::Control(ControlMsg::Unpair)).unwrap();
    stream.send(Bytes::from(payload)).await.unwrap();
    drop(stream);

    // Step 4: dequeue the record (delivered exactly once).
    peers::remove_pending_unpair(&pending_path, &entry.fingerprint).unwrap();

    // Step 5: drop the transient allow-list entry — B must not resume sync.
    a_live.remove(&b_fp);

    // Assertions.
    let received_unpair = b_accept.await.unwrap();
    assert!(
        received_unpair,
        "Gap A: reconnected peer B must receive a ControlMsg::Unpair frame"
    );
    assert!(
        peers::load_pending_unpairs(&pending_path).is_empty(),
        "Gap A: the pending-unpair record must be dequeued after delivery (exactly-once)"
    );
    assert!(
        !a_live.is_known(&b_fp),
        "Gap A: B must not remain allow-listed after the transient delivery window"
    );
}

/// Gap A — a record with NO dial address is retained (not silently lost): the
/// connector cannot dial it, but the intent to unpair survives for a future
/// improvement that learns the address out-of-band.
#[test]
fn gap_a_addressless_pending_unpair_is_retained() {
    let tmp = tempfile::tempdir().unwrap();
    let pending_path = tmp.path().join("pending_unpair.json");

    peers::queue_pending_unpair(&pending_path, "deadbeef", None, "Ghost").unwrap();
    let loaded = peers::load_pending_unpairs(&pending_path);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].address, None);
    assert_eq!(loaded[0].fingerprint, "deadbeef");
}
