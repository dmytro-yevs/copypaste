//! mTLS accept loop — inbound connection handler.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::{DeviceFingerprint, PairedPeers, PeerTransport};
use copypaste_sync::protocol::{PeerFrame, WireItem};

use super::{CatchupProvider, PeerEvent, PeerRttMs, PeerSinks};
use super::fanout::push_catchup;
use super::framed_pump::run_peer_connection;
use super::ping::ping_loop;
use super::unpair::stamp_peer_sync;

/// Accept incoming mTLS connections.
///
/// For each connection that completes the TLS handshake successfully, spawns a
/// per-connection task that:
/// - Reads `WireItem` frames from the peer and forwards them to `incoming_tx`.
/// - Drains a per-peer `mpsc::Receiver<WireItem>` and writes frames to the peer.
///
/// The per-peer sender is stored in `peer_sinks` (keyed by the peer's cert
/// fingerprint) so the outbound fanout loop can deliver outgoing items.
#[allow(clippy::too_many_arguments)] // RTT + event params pushed count over 8
pub(super) async fn accept_loop(
    listener: TcpListener,
    shutdown: CancellationToken,
    transport: Arc<PeerTransport>,
    peer_sinks: PeerSinks,
    incoming_tx: mpsc::Sender<WireItem>,
    catchup: CatchupProvider,
    // The live mTLS allowlist (shared with the transport's cert verifier).
    // Forwarded to `run_peer_connection` so an inbound `ControlMsg::Unpair`
    // evicts the peer from BOTH peers.json and this live allowlist (Gap B).
    live_peers: PairedPeers,
    // Shared RTT map — updated by the ping task spawned per connection.
    peer_rtt_ms: PeerRttMs,
    // Broadcast channel for peer connect/disconnect events.
    peer_event_tx: broadcast::Sender<PeerEvent>,
) {
    // fix/p2p-c-review #3: the previous `"unknown".parse().unwrap()` fallback
    // panicked because `"unknown"` is not a valid `SocketAddr`. `local_addr`
    // is practically infallible here (the socket is open), but log a string
    // instead of unwrapping so a closed-socket edge can never crash the task.
    match listener.local_addr() {
        Ok(addr) => tracing::debug!(%addr, "P2P accept loop running"),
        Err(e) => tracing::debug!(error = %e, "P2P accept loop running (local_addr unavailable)"),
    }

    loop {
        tokio::select! {
            result = transport.accept(&listener) => {
                match result {
                    Ok((peer_addr, peer_fp, framed)) => {
                        tracing::info!(%peer_addr, %peer_fp, "mTLS handshake completed");

                        // Per-peer write channel: the outbound loop sends frames here;
                        // the write half of the per-connection task drains and serialises them.
                        let (peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(64);

                        // fix/p2p-c-review #4: key by the verified cert fingerprint,
                        // not the ephemeral socket address. A reconnect from a new
                        // source port then replaces the stale sink instead of adding
                        // a duplicate (which would double every outbound item).
                        let peer_key: DeviceFingerprint = peer_fp.clone();

                        // `same_channel` lets the cleanup task below avoid evicting a
                        // *newer* connection's sink if this (older) connection drops
                        // after being superseded by a reconnect under the same key.
                        let cleanup_tx = peer_tx.clone();

                        // Churn fix: do NOT replace a still-healthy sink for the
                        // same fingerprint. When both daemons dial each other a
                        // duplicate connection arrives here; overwriting the live
                        // sink resets the healthy link ("connection reset by
                        // peer"). Keep the existing connection and drop this
                        // duplicate instead. A sink whose receiver was dropped
                        // (peer task exited) is closed → we may replace it.
                        {
                            let mut sinks = peer_sinks.lock().await;
                            let healthy = sinks
                                .get(&peer_key)
                                .is_some_and(|tx| !tx.is_closed());
                            if healthy {
                                drop(sinks);
                                tracing::debug!(%peer_fp, "duplicate inbound connection — existing sink healthy, dropping duplicate");
                                drop(framed);
                                continue;
                            }
                            sinks.insert(peer_key.clone(), peer_tx);
                        }

                        // Notify subscribers (e.g. Tauri event bridge) that
                        // this peer is now online. `send` returns an error when
                        // there are no active receivers — that is fine; just
                        // ignore it (no subscriber yet or all have dropped).
                        let _ = peer_event_tx.send(PeerEvent::Connected {
                            fingerprint: peer_fp.clone(),
                        });

                        // Stamp first/last sync times for this peer (once per
                        // established connection — see `stamp_peer_sync`).
                        stamp_peer_sync(&crate::ipc::peers_file_path(), &peer_fp);

                        // Clone the sink sender for the catch-up replay BEFORE the
                        // drainer task takes ownership of `cleanup_tx`. The drainer
                        // MUST start first: `push_catchup` does a bounded
                        // `send().await` over the ENTIRE local history (commonly far
                        // more than the 64-slot channel capacity), so with no active
                        // receiver draining `peer_rx` it deadlocks the moment the
                        // buffer fills — the sink then stays full forever and the
                        // peer receives nothing. (Mirror of the connector-path fix.)
                        let catchup_tx = cleanup_tx.clone();
                        // CopyPaste-716: clone the fingerprint separately for
                        // push_catchup — peer_fp_for_task is moved into the spawn.
                        let catchup_fp = peer_fp.clone();

                        let incoming_tx = incoming_tx.clone();
                        let peer_sinks = Arc::clone(&peer_sinks);
                        let peer_fp_for_task = peer_fp.clone();
                        let live_peers_for_task = live_peers.clone();
                        // Clone the event sender for the cleanup task that fires
                        // the Disconnected event when the connection drops.
                        let disconnect_event_tx = peer_event_tx.clone();

                        // RTT: create a per-connection pending-pings map shared
                        // between the ping sender task and the connection task.
                        let pending_pings = Arc::new(Mutex::new(HashMap::new()));
                        let rtt_map_for_task = Arc::clone(&peer_rtt_ms);
                        let rtt_map_for_ping = Arc::clone(&peer_rtt_ms);
                        let pending_pings_for_conn = Arc::clone(&pending_pings);

                        // Spawn the periodic RTT ping task. It holds a clone of
                        // cleanup_tx (the same sink as the drainer) to inject
                        // Ping frames through the normal outbound channel.
                        // CopyPaste-8i3q: also pass peer_sinks + peer_key +
                        // peer_event_tx so ping_loop can evict the stale sink
                        // and emit Disconnected when a Pong times out.
                        let ping_fp = peer_fp.clone();
                        let ping_sink = cleanup_tx.clone();
                        let ping_sinks = Arc::clone(&peer_sinks);
                        let ping_key = peer_key.clone();
                        let ping_event_tx = peer_event_tx.clone();
                        tokio::spawn(async move {
                            ping_loop(
                                ping_sink,
                                ping_fp,
                                pending_pings,
                                rtt_map_for_ping,
                                ping_sinks,
                                ping_key,
                                ping_event_tx,
                            )
                            .await;
                        });

                        tokio::spawn(async move {
                            run_peer_connection(
                                framed,
                                peer_rx,
                                incoming_tx,
                                peer_fp_for_task,
                                Some(live_peers_for_task),
                                pending_pings_for_conn,
                                rtt_map_for_task,
                            )
                            .await;
                            // Clean up the sink when the connection drops — but only
                            // if it is still *this* connection's sink (a later
                            // reconnect may have replaced it under the same key).
                            let mut sinks = peer_sinks.lock().await;
                            if sinks
                                .get(&peer_key)
                                .is_some_and(|tx| tx.same_channel(&cleanup_tx))
                            {
                                sinks.remove(&peer_key);
                                // Emit Disconnected only when we actually removed
                                // the sink (not when superseded by a reconnect).
                                let _ = disconnect_event_tx.send(PeerEvent::Disconnected {
                                    fingerprint: peer_key.clone(),
                                });
                            }
                            drop(sinks);
                            tracing::debug!(%peer_addr, %peer_fp, "peer connection closed");
                        });

                        // Drainer is now consuming `peer_rx`, so replaying the local
                        // history (sync-on-connect) cannot deadlock on a full sink.
                        // Items are re-keyed under this peer's pairwise sync key
                        // (CopyPaste-716); LWW on the receiver makes the replay
                        // idempotent.
                        push_catchup(&catchup, catchup_fp.as_str(), &catchup_tx).await;
                    }
                    Err(e) => {
                        tracing::warn!("P2P accept/handshake error: {e}");
                    }
                }
            }
            _ = shutdown.cancelled() => {
                tracing::info!("P2P accept loop shutting down");
                break;
            }
        }
    }
}
