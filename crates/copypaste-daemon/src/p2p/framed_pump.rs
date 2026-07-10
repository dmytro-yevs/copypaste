//! Duplex pump shared by inbound and outbound mTLS connection tasks.

use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use copypaste_p2p::transport::{DeviceFingerprint, PairedPeers};
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};
use copypaste_sync::{ReplayGuard, REPLAY_GUARD_CAPACITY};

use super::unpair::{evict_peer_local, stamp_peer_sync};
use super::{PeerRttMs, PendingPings};

/// Minimum interval between `stamp_peer_sync` calls triggered by successful
/// inbound `Data` frames on a single connection (CopyPaste-dkwl).
///
/// A sync-on-connect catch-up replay or a burst of clipboard items can
/// deliver many `Data` frames in quick succession; without this throttle each
/// one would trigger a `peers.json` read-modify-write.
const INBOUND_STAMP_THROTTLE: Duration = Duration::from_secs(60);

/// Maximum time a single outbound `framed.send().await` may block before the
/// pump tears the connection down.
///
/// Without this bound a half-closed peer (e.g. Android dials one-shot every few
/// seconds, sends FIN, then leaves the socket in CLOSE_WAIT) makes
/// `framed.send().await` to the dead socket block forever. While the
/// `tokio::select!` is parked in the write arm it never re-polls the read arm,
/// so the EOF is never observed, the task never returns, `peer_rx` is never
/// dropped, and the per-peer sink `Sender` never closes — which silently kills
/// steady-state sync in both directions (connector never re-dials; the accept
/// loop keeps treating the dead peer as connected). Bounding the write forces
/// teardown so the sink closes and recovery can proceed.
pub(super) const WRITE_TIMEOUT: Duration = Duration::from_secs(8);

/// Compile-time assertion: `WRITE_TIMEOUT` must stay strictly below
/// [`copypaste_p2p::transport::TCP_KEEPALIVE_TIME`] (CopyPaste-vgpy).
///
/// `TCP_KEEPALIVE_TIME`'s doc comment already describes the intended
/// ordering — it calls itself "defense-in-depth **alongside** the
/// daemon-side write timeout" for the case where a peer vanishes with no FIN.
/// That relationship only holds if `WRITE_TIMEOUT` is the faster detector:
/// on a connection with an outstanding write, `WRITE_TIMEOUT` (8 s) must
/// trip and tear the connection down before the OS keepalive prober would
/// even start (`TCP_KEEPALIVE_TIME` = 20 s of idle time). If `WRITE_TIMEOUT`
/// were ever raised to meet or exceed `TCP_KEEPALIVE_TIME`, the write-timeout
/// guard would stop being the primary (faster) recovery path and silently
/// degrade to redundant with — or slower than — the OS-level keepalive,
/// defeating the reason it exists (see the comment on `WRITE_TIMEOUT` above).
/// This was previously assumed across the two crates with no assertion; this
/// makes a regression a build failure instead of a silent slowdown of dead-
/// peer detection, mirroring the `CONNECTOR_TICK`/`MIN_HEALTHY_DWELL`
/// assertion in `p2p/connector/mod.rs`.
const _: () = assert!(
    WRITE_TIMEOUT.as_nanos() < copypaste_p2p::transport::TCP_KEEPALIVE_TIME.as_nanos(),
    "WRITE_TIMEOUT must stay below copypaste_p2p::transport::TCP_KEEPALIVE_TIME or the \
     write-timeout guard stops being the faster dead-peer detector it was designed to be"
);

/// Manage one authenticated **inbound** (accept-side) peer connection.
///
/// `peer_fp` is the mTLS-verified certificate fingerprint of the remote peer,
/// used to authenticate any `ControlMsg::Unpair` signal (see
/// [`run_peer_connection_framed`]).
pub(super) async fn run_peer_connection(
    framed: copypaste_p2p::transport::PeerStream,
    peer_rx: mpsc::Receiver<PeerFrame>,
    incoming_tx: mpsc::Sender<WireItem>,
    peer_fp: DeviceFingerprint,
    live_peers: Option<PairedPeers>,
    pending_pings: PendingPings,
    peer_rtt_ms: PeerRttMs,
) {
    run_peer_connection_framed(
        framed,
        peer_rx,
        incoming_tx,
        peer_fp,
        live_peers,
        pending_pings,
        peer_rtt_ms,
    )
    .await
}

/// Manage one authenticated **outbound** (connector-side) peer connection.
///
/// Identical duplex pump as [`run_peer_connection`] but for the client-side TLS
/// stream type returned by [`copypaste_p2p::PeerTransport::connect_with_retry`].
pub(super) async fn run_peer_connection_client(
    framed: copypaste_p2p::transport::PeerClientStream,
    peer_rx: mpsc::Receiver<PeerFrame>,
    incoming_tx: mpsc::Sender<WireItem>,
    peer_fp: DeviceFingerprint,
    live_peers: Option<PairedPeers>,
    pending_pings: PendingPings,
    peer_rtt_ms: PeerRttMs,
) {
    run_peer_connection_framed(
        framed,
        peer_rx,
        incoming_tx,
        peer_fp,
        live_peers,
        pending_pings,
        peer_rtt_ms,
    )
    .await
}

/// Duplex pump shared by the accept-side and connector-side connection tasks.
///
/// Reads incoming frames and forwards them to `incoming_tx`; reads from
/// `peer_rx` and writes outgoing frames to the peer. Both directions run
/// concurrently via `tokio::select!`; the task exits when either side closes.
/// Generic over the framed stream so the server-side (`PeerStream`) and
/// client-side (`PeerClientStream`) TLS stream types share one implementation.
///
/// ## Security — unpair signal eviction
///
/// On receiving `PeerFrame::Control(ControlMsg::Unpair)` the local peer
/// record for `peer_fp` is evicted from `peers.json` and the live mTLS
/// allowlist.  The eviction is keyed to `peer_fp`, which is the **mTLS
/// certificate fingerprint verified by the transport layer** before this
/// function is ever called — it is NOT a field inside the message itself.
/// This means a misbehaving or compromised peer can only cause its OWN
/// pairing to be removed, never that of any other peer.
pub(super) async fn run_peer_connection_framed<S>(
    mut framed: tokio_util::codec::Framed<S, tokio_util::codec::LengthDelimitedCodec>,
    mut peer_rx: mpsc::Receiver<PeerFrame>,
    incoming_tx: mpsc::Sender<WireItem>,
    peer_fp: DeviceFingerprint,
    live_peers: Option<PairedPeers>,
    // Per-connection nonce → send-time map shared with the ping sender task.
    // On Pong receipt, we look up the nonce here to compute elapsed time.
    pending_pings: PendingPings,
    // Shared map of last-measured RTTs per peer; written on each Pong receipt.
    peer_rtt_ms: PeerRttMs,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // CopyPaste-dkwl: per-connection throttle for inbound-exchange stamping —
    // see `INBOUND_STAMP_THROTTLE`.
    let mut last_inbound_stamp: Option<Instant> = None;

    // CopyPaste-sreb: per-connection replay guard. Constructed inline (not
    // shared) so a compromised or buggy peer can only replay within its own
    // connection's window, matching the "per-sender" guarantee documented on
    // `ReplayGuard` itself.
    let mut replay_guard = ReplayGuard::new(REPLAY_GUARD_CAPACITY);

    loop {
        tokio::select! {
            // Inbound: peer sent a frame — deserialise and dispatch.
            frame_opt = framed.next() => {
                match frame_opt {
                    Some(Ok(frame)) => {
                        match serde_json::from_slice::<PeerFrame>(&frame) {
                            Ok(PeerFrame::Data(wire)) => {
                                if replay_guard.is_replay(&wire.item_id, wire.lamport_ts) {
                                    tracing::debug!(
                                        peer = %peer_fp,
                                        item_id = %wire.item_id,
                                        lamport_ts = wire.lamport_ts,
                                        "dropping replayed item (CopyPaste-sreb)"
                                    );
                                    continue;
                                }
                                if incoming_tx.send(wire).await.is_err() {
                                    // incoming_tx closed means sync_orch shut down.
                                    tracing::debug!("incoming_tx closed, dropping peer connection");
                                    return;
                                }
                                // CopyPaste-dkwl: a `Data` frame we could
                                // decode and hand off is proof of a real
                                // application-level exchange with this peer —
                                // stamp `last_sync_at` here (throttled) rather
                                // than relying solely on the connection-time
                                // stamp in `listener.rs`/`connector/mod.rs`,
                                // which does not observe whether syncing
                                // actually keeps working after connect.
                                let stamp_due = last_inbound_stamp
                                    .is_none_or(|t| t.elapsed() >= INBOUND_STAMP_THROTTLE);
                                if stamp_due {
                                    stamp_peer_sync(&crate::ipc::peers_file_path(), &peer_fp);
                                    last_inbound_stamp = Some(Instant::now());
                                }
                            }
                            Ok(PeerFrame::Control(ControlMsg::Unpair)) => {
                                // Security: evict using ONLY the mTLS-authenticated
                                // peer_fp, never a field from the message body.  This
                                // ensures a peer can only remove its OWN pairing.
                                tracing::info!(
                                    peer = %peer_fp,
                                    "received unpair signal from authenticated peer — evicting"
                                );
                                evict_peer_local(&peer_fp, live_peers.as_ref());
                                return;
                            }
                            Ok(PeerFrame::Control(ControlMsg::Ping { nonce })) => {
                                // Reply immediately with a matching Pong so the
                                // remote peer can measure the round-trip time.
                                let pong = PeerFrame::Control(ControlMsg::Pong { nonce });
                                match serde_json::to_vec(&pong) {
                                    Ok(payload) => {
                                        match tokio::time::timeout(
                                            WRITE_TIMEOUT,
                                            framed.send(Bytes::from(payload)),
                                        )
                                        .await
                                        {
                                            Ok(Ok(())) => {
                                                tracing::trace!(
                                                    peer = %peer_fp,
                                                    nonce,
                                                    "RTT: sent Pong"
                                                );
                                            }
                                            Ok(Err(e)) => {
                                                tracing::warn!("RTT: failed to send Pong to peer: {e}");
                                                return;
                                            }
                                            Err(_elapsed) => {
                                                tracing::warn!(
                                                    peer = %peer_fp,
                                                    "RTT: Pong write timed out — tearing down connection"
                                                );
                                                return;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("RTT: failed to serialise Pong: {e}");
                                    }
                                }
                            }
                            Ok(PeerFrame::Control(ControlMsg::Pong { nonce })) => {
                                // Record the RTT for this peer. Look up the nonce
                                // in the pending-pings map and compute elapsed time.
                                let sent_at = {
                                    let mut map = pending_pings.lock().await;
                                    map.remove(&nonce)
                                };
                                if let Some(sent_at) = sent_at {
                                    let rtt_ms = sent_at.elapsed().as_millis() as u32;
                                    tracing::debug!(
                                        peer = %peer_fp,
                                        rtt_ms,
                                        "RTT: measured"
                                    );
                                    peer_rtt_ms.lock().await.insert(peer_fp.clone(), rtt_ms);
                                }
                            }
                            Ok(PeerFrame::Control(ControlMsg::DeviceInfo {
                                ref model,
                                ref os_version,
                                ref app_version,
                                ref public_ip,
                            })) => {
                                // crh3.109: refresh the peer's stale metadata
                                // captured at pairing time.  Best-effort: a
                                // write failure is logged but never disrupts sync.
                                let peers_path = crate::ipc::peers_file_path();
                                match crate::peers::update_peer_device_info(
                                    &peers_path,
                                    &peer_fp,
                                    model.as_deref(),
                                    os_version.as_deref(),
                                    app_version.as_deref(),
                                    public_ip.as_deref(),
                                ) {
                                    Ok(true) => {
                                        tracing::debug!(
                                            peer = %peer_fp,
                                            model = ?model,
                                            os_version = ?os_version,
                                            app_version = ?app_version,
                                            "peer device-info refreshed (crh3.109)"
                                        );
                                    }
                                    Ok(false) => {} // Nothing changed — no log noise.
                                    Err(e) => {
                                        tracing::warn!(
                                            peer = %peer_fp,
                                            error = %e,
                                            "failed to persist peer device-info refresh (crh3.109)"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to deserialise frame from peer: {e}");
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::warn!("peer frame error: {e}");
                        return;
                    }
                    None => {
                        // Peer closed connection cleanly.
                        return;
                    }
                }
            }
            // Outbound: sync_orch or the IPC unpair handler wants to push a frame.
            frame_opt = peer_rx.recv() => {
                match frame_opt {
                    Some(frame) => {
                        match serde_json::to_vec(&frame) {
                            Ok(payload) => {
                                match tokio::time::timeout(
                                    WRITE_TIMEOUT,
                                    framed.send(Bytes::from(payload)),
                                )
                                .await
                                {
                                    Ok(Ok(())) => {}
                                    Ok(Err(e)) => {
                                        tracing::warn!("failed to send frame to peer: {e}");
                                        return;
                                    }
                                    Err(_elapsed) => {
                                        tracing::warn!(
                                            timeout = ?WRITE_TIMEOUT,
                                            "peer write timed out — tearing down half-closed connection"
                                        );
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to serialise frame for peer: {e}");
                            }
                        }
                    }
                    None => {
                        // peer_rx channel closed — no more outbound frames for this peer.
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// CopyPaste-vgpy: explicit runtime pin of the compile-time assertion
    /// declared next to `WRITE_TIMEOUT` — kept alongside it (rather than
    /// relying solely on the `const _: ()` assert) so the invariant shows up
    /// in test output/coverage like any other regression guard, mirroring
    /// `connector_tick_is_below_min_healthy_dwell` in `p2p/connector/mod.rs`.
    #[test]
    fn write_timeout_is_below_tcp_keepalive_time() {
        assert!(
            WRITE_TIMEOUT < copypaste_p2p::transport::TCP_KEEPALIVE_TIME,
            "WRITE_TIMEOUT ({WRITE_TIMEOUT:?}) must stay below TCP_KEEPALIVE_TIME ({:?}) so a \
             dead peer with an outstanding write is torn down by the (faster) write-timeout \
             guard before the OS keepalive prober would even start",
            copypaste_p2p::transport::TCP_KEEPALIVE_TIME,
        );
    }

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

    /// A stream that accepts reads/writes but never makes progress: reads stay
    /// `Pending` (no EOF, no data) and writes stay `Pending` (the kernel send
    /// buffer is "full"). Models a half-closed / wedged peer socket so a
    /// `framed.send().await` blocks indefinitely.
    struct StuckStream;

    impl tokio::io::AsyncRead for StuckStream {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    impl tokio::io::AsyncWrite for StuckStream {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            std::task::Poll::Pending
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    /// A stuck writer (half-closed peer) must not park the pump forever: the
    /// write timeout fires, the task returns, and `peer_rx` is dropped so the
    /// per-peer sink `Sender` reports closed — which is what unblocks both the
    /// connector re-dial and the accept loop's duplicate guard.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn stuck_writer_drops_sink_within_write_timeout() {
        let framed = tokio_util::codec::Framed::new(
            StuckStream,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let (peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(8);
        let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);

        // Queue an outbound item so the pump enters the write arm and blocks.
        peer_tx
            .send(PeerFrame::Data(test_wire_item("a")))
            .await
            .unwrap();

        let pending: PendingPings = Arc::new(Mutex::new(HashMap::new()));
        let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
        let handle = tokio::spawn(run_peer_connection_framed(
            framed,
            peer_rx,
            incoming_tx,
            copypaste_p2p::DeviceFingerprint("testpeer".to_string()),
            None,
            pending,
            rtt_ms,
        ));

        // The sink Sender must close once the pump tears down on write timeout.
        // With paused time the timer advances automatically when the runtime is
        // otherwise idle, so a generous bound keeps the test instant yet robust.
        tokio::time::timeout(WRITE_TIMEOUT * 2, handle)
            .await
            .expect("pump task must return after write timeout, not block forever")
            .expect("pump task must not panic");

        assert!(
            peer_tx.is_closed(),
            "peer sink Sender must be closed after the pump tears down a stuck writer"
        );
    }

    /// Encode a `PeerFrame::Data(item)` as a length-delimited frame and write
    /// it into `sink`, mirroring what a real peer's `framed.send()` produces.
    async fn send_data_frame<S>(
        sink: &mut tokio_util::codec::Framed<S, tokio_util::codec::LengthDelimitedCodec>,
        item: WireItem,
    ) where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = serde_json::to_vec(&PeerFrame::Data(item)).unwrap();
        sink.send(Bytes::from(payload)).await.unwrap();
    }

    fn new_pending_and_rtt() -> (PendingPings, PeerRttMs) {
        (
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(HashMap::new())),
        )
    }

    /// CopyPaste-sreb: a per-connection `ReplayGuard` is now constructed
    /// inline in `run_peer_connection_framed`, so a duplicate
    /// `(item_id, lamport_ts)` delivered twice on the same connection must
    /// only reach `incoming_tx` once.
    #[tokio::test]
    async fn duplicate_item_within_one_connection_is_dropped() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let mut client = tokio_util::codec::Framed::new(
            client_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let server = tokio_util::codec::Framed::new(
            server_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );

        let (_peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(8);
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<WireItem>(8);
        let (pending, rtt_ms) = new_pending_and_rtt();

        let handle = tokio::spawn(run_peer_connection_framed(
            server,
            peer_rx,
            incoming_tx,
            copypaste_p2p::DeviceFingerprint("testpeer".to_string()),
            None,
            pending,
            rtt_ms,
        ));

        let mut item = test_wire_item("dup-item");
        item.lamport_ts = 1;
        send_data_frame(&mut client, item.clone()).await;
        send_data_frame(&mut client, item.clone()).await;

        let first = tokio::time::timeout(Duration::from_secs(5), incoming_rx.recv())
            .await
            .expect("timed out waiting for first delivery")
            .expect("first delivery must arrive");
        assert_eq!(first.item_id, "dup-item");

        // The duplicate must never arrive: dropping the client sink closes
        // the connection, which lets the pump task return; then incoming_rx
        // must be empty (and closed) rather than yielding a second item.
        drop(client);
        let second = tokio::time::timeout(Duration::from_secs(5), incoming_rx.recv())
            .await
            .expect("pump must exit promptly after client closes");
        assert!(second.is_none(), "duplicate item must have been dropped");

        handle.await.expect("pump task must not panic");
    }

    /// Same item_id but a strictly higher lamport_ts is a legitimate CRDT
    /// update, not a replay — it must pass through.
    #[tokio::test]
    async fn higher_lamport_ts_same_item_passes_through() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let mut client = tokio_util::codec::Framed::new(
            client_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let server = tokio_util::codec::Framed::new(
            server_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );

        let (_peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(8);
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<WireItem>(8);
        let (pending, rtt_ms) = new_pending_and_rtt();

        let handle = tokio::spawn(run_peer_connection_framed(
            server,
            peer_rx,
            incoming_tx,
            copypaste_p2p::DeviceFingerprint("testpeer".to_string()),
            None,
            pending,
            rtt_ms,
        ));

        let mut item_v1 = test_wire_item("evolving-item");
        item_v1.lamport_ts = 1;
        let mut item_v2 = test_wire_item("evolving-item");
        item_v2.lamport_ts = 2;

        send_data_frame(&mut client, item_v1).await;
        send_data_frame(&mut client, item_v2).await;

        let first = tokio::time::timeout(Duration::from_secs(5), incoming_rx.recv())
            .await
            .expect("timed out waiting for first delivery")
            .expect("first delivery must arrive");
        assert_eq!(first.lamport_ts, 1);

        let second = tokio::time::timeout(Duration::from_secs(5), incoming_rx.recv())
            .await
            .expect("timed out waiting for second delivery")
            .expect("higher lamport_ts update must arrive");
        assert_eq!(second.lamport_ts, 2);

        drop(client);
        handle.await.expect("pump task must not panic");
    }

    /// The `ReplayGuard` is constructed per-connection, so two independent
    /// connections (e.g. one inbound + one outbound to different peers)
    /// sharing one `incoming_tx` must not share replay state: the identical
    /// `(item_id, lamport_ts)` pair sent on both connections must be admitted
    /// on each.
    #[tokio::test]
    async fn guards_are_independent_across_two_connections() {
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<WireItem>(16);

        let (client_a_io, server_a_io) = tokio::io::duplex(4096);
        let mut client_a = tokio_util::codec::Framed::new(
            client_a_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let server_a = tokio_util::codec::Framed::new(
            server_a_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );

        let (client_b_io, server_b_io) = tokio::io::duplex(4096);
        let mut client_b = tokio_util::codec::Framed::new(
            client_b_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let server_b = tokio_util::codec::Framed::new(
            server_b_io,
            tokio_util::codec::LengthDelimitedCodec::new(),
        );

        let (_peer_tx_a, peer_rx_a) = mpsc::channel::<PeerFrame>(8);
        let (_peer_tx_b, peer_rx_b) = mpsc::channel::<PeerFrame>(8);
        let (pending_a, rtt_ms_a) = new_pending_and_rtt();
        let (pending_b, rtt_ms_b) = new_pending_and_rtt();

        let handle_a = tokio::spawn(run_peer_connection_framed(
            server_a,
            peer_rx_a,
            incoming_tx.clone(),
            copypaste_p2p::DeviceFingerprint("peer-a".to_string()),
            None,
            pending_a,
            rtt_ms_a,
        ));
        let handle_b = tokio::spawn(run_peer_connection_framed(
            server_b,
            peer_rx_b,
            incoming_tx,
            copypaste_p2p::DeviceFingerprint("peer-b".to_string()),
            None,
            pending_b,
            rtt_ms_b,
        ));

        let mut item = test_wire_item("shared-item");
        item.lamport_ts = 7;

        send_data_frame(&mut client_a, item.clone()).await;
        send_data_frame(&mut client_b, item.clone()).await;

        let mut seen = Vec::new();
        for _ in 0..2 {
            let received = tokio::time::timeout(Duration::from_secs(5), incoming_rx.recv())
                .await
                .expect("timed out waiting for delivery")
                .expect("both connections must independently admit the item");
            seen.push(received.item_id);
        }
        assert_eq!(seen, vec!["shared-item", "shared-item"]);

        drop(client_a);
        drop(client_b);
        handle_a.await.expect("pump task a must not panic");
        handle_b.await.expect("pump task b must not panic");
    }
}
