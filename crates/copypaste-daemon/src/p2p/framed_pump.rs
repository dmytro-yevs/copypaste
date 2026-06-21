//! Duplex pump shared by inbound and outbound mTLS connection tasks.

use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use copypaste_p2p::transport::{DeviceFingerprint, PairedPeers};
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};

use super::{PeerRttMs, PendingPings};
use super::unpair::evict_peer_local;

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
/// stream type returned by [`PeerTransport::connect_with_retry`].
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
    loop {
        tokio::select! {
            // Inbound: peer sent a frame — deserialise and dispatch.
            frame_opt = framed.next() => {
                match frame_opt {
                    Some(Ok(frame)) => {
                        match serde_json::from_slice::<PeerFrame>(&frame) {
                            Ok(PeerFrame::Data(wire)) => {
                                if incoming_tx.send(wire).await.is_err() {
                                    // incoming_tx closed means sync_orch shut down.
                                    tracing::debug!("incoming_tx closed, dropping peer connection");
                                    return;
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
