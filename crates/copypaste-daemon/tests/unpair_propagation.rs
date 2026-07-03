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

/// CopyPaste-8ebg.5 regression (bd CopyPaste-vgpy item A): pending-unpair
/// delivery MUST dial the revoked peer through a **scoped, throwaway**
/// allowlist (`PeerTransport::connect_with_retry_scoped`) and MUST NEVER
/// populate the shared/live `PairedPeers` allowlist that `accept()` also
/// consults for inbound connections. An earlier version temporarily
/// `live_peers.add()`-ed the revoked fingerprint before dialing and
/// `.remove()`-ed it afterward — since `accept()` and the delivery dial share
/// one `PairedPeers`, a revoked peer connecting IN during that window would
/// also have been accepted and resumed full sync (see
/// `crates/copypaste-daemon/src/p2p/connector/pending_unpair.rs`).
///
/// This test drives BOTH directions concurrently during the delivery window:
/// A performs the real `connect_with_retry_scoped` delivery to B while B
/// simultaneously attempts to dial IN to A using A's shared live allowlist.
/// The inbound attempt must be rejected the whole time, and the shared
/// allowlist must never report the revoked fingerprint as known.
#[tokio::test(flavor = "multi_thread")]
async fn gap_a_shared_live_allowlist_never_populated_during_pending_unpair_delivery() {
    let a_cert = copypaste_p2p::cert::SelfSignedCert::generate("unpair-A2").unwrap();
    let b_cert = copypaste_p2p::cert::SelfSignedCert::generate("unpair-B2").unwrap();
    let a_fp = a_cert.fingerprint();
    let b_fp = b_cert.fingerprint();

    // A's SHARED live allowlist. B has been revoked (mutual-unpair), so it
    // starts — and per CopyPaste-8ebg.5 must STAY — empty for the entire
    // pending-unpair delivery window.
    let a_live = PairedPeers::new();
    let a_transport = std::sync::Arc::new(PeerTransport::from_cert(
        a_cert.cert_der.clone(),
        a_cert.key_der.clone(),
        a_live.clone(),
    ));
    let a_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a_addr = a_listener.local_addr().unwrap();

    // B's inbound side — the delivery target ("B is back online" and
    // reachable at its last-known address).
    let b_accept_peers = PairedPeers::new();
    b_accept_peers.add(a_fp.clone(), "device-A2");
    let b_accept_transport = PeerTransport::from_cert(
        b_cert.cert_der.clone(),
        b_cert.key_der.clone(),
        b_accept_peers,
    );
    let b_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let b_addr = b_listener.local_addr().unwrap();

    let b_accept = tokio::spawn(async move {
        let (_addr, _peer_fp, mut stream) = b_accept_transport.accept(&b_listener).await.unwrap();
        matches!(
            stream.next().await,
            Some(Ok(frame)) if matches!(
                serde_json::from_slice::<PeerFrame>(&frame),
                Ok(PeerFrame::Control(ControlMsg::Unpair))
            )
        )
    });

    // A's inbound side, run concurrently with the delivery dial below. If the
    // OLD buggy pattern (temporarily `a_live.add(b_fp)` before dialing) were
    // still present, a connection from B during this window would be
    // accepted. With the fix (scoped throwaway allowlist), `a_live` is never
    // touched, so this must be rejected regardless of timing.
    let a_accept_transport = std::sync::Arc::clone(&a_transport);
    let a_accept = tokio::spawn(async move {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            a_accept_transport.accept(&a_listener),
        )
        .await
    });

    // B attempts to dial IN to A "during" the delivery window (mirrors a
    // revoked peer trying to resume sync while A is mid-scoped-dial to B). B
    // trusts A's fingerprint (that half of a real revoke can be asymmetric in
    // timing) — what must hold is that A's side rejects it.
    let b_dial_peers = PairedPeers::new();
    b_dial_peers.add(a_fp.clone(), "device-A2-dial");
    let b_dial_transport = PeerTransport::from_cert(
        b_cert.cert_der.clone(),
        b_cert.key_der.clone(),
        b_dial_peers,
    );
    let a_fp_for_dial = a_fp.clone();
    let b_dial =
        tokio::spawn(async move { b_dial_transport.connect(a_addr, &a_fp_for_dial).await });

    // A's connector-mirroring delivery step: dial B using the SCOPED
    // throwaway allowlist (CopyPaste-8ebg.5's `connect_with_retry_scoped`)
    // and send the Unpair frame — all WITHOUT ever adding B to `a_live`.
    let mut stream = a_transport
        .connect_with_retry_scoped(b_addr, &b_fp)
        .await
        .unwrap();
    let payload = serde_json::to_vec(&PeerFrame::Control(ControlMsg::Unpair)).unwrap();
    stream.send(Bytes::from(payload)).await.unwrap();
    drop(stream);

    // --- Assertions ---
    assert!(
        b_accept.await.unwrap(),
        "delivery must still work through the scoped API"
    );

    // The core regression guard: B dialing IN to A during the delivery window
    // must be rejected — the shared live allowlist was never populated with
    // B's revoked fingerprint.
    let b_dial_result = b_dial.await.unwrap();
    assert!(
        b_dial_result.is_err(),
        "CopyPaste-8ebg.5 regression: a revoked peer must NOT be accepted via \
         A's shared live allowlist during pending-unpair delivery"
    );

    // A's accept() call must never have succeeded for B either (a timeout or
    // a handshake/verification error are both acceptable "rejected"
    // outcomes; only Ok would be a regression).
    match a_accept.await.unwrap() {
        Ok(Ok((_, fp, _))) => panic!(
            "CopyPaste-8ebg.5 regression: A accepted an inbound connection from \
             revoked peer {fp:?} during pending-unpair delivery"
        ),
        _ => {} // timeout or handshake/verification error — expected.
    }

    assert!(
        !a_live.is_known(&b_fp),
        "the shared live allowlist must never contain the revoked fingerprint"
    );
}
