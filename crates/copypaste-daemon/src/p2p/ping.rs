//! Periodic RTT ping/pong sender for established peer connections.

use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc};

use copypaste_p2p::transport::DeviceFingerprint;
use copypaste_sync::protocol::{ControlMsg, PeerFrame};

use super::{PeerEvent, PeerRttMs, PeerSinks, PendingPings};

/// How often the RTT ping task wakes to send a [`ControlMsg::Ping`].
pub(super) const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum time to wait for a [`ControlMsg::Pong`] before discarding the
/// nonce from the pending-ping map. Prevents unbounded growth when a peer
/// never responds (e.g. old daemon that doesn't know the Ping variant).
pub(super) const PING_PONG_TIMEOUT: Duration = Duration::from_secs(10);

/// Periodic RTT ping sender for a single established peer connection.
///
/// Sends a [`ControlMsg::Ping`] frame every [`PING_INTERVAL`] through the
/// peer sink, recording the send time in `pending_pings`. Expires unmatched
/// nonces after [`PING_PONG_TIMEOUT`] so the map doesn't grow unbounded
/// against peers that don't speak the Ping/Pong protocol.
///
/// **Dead-connection detection (CopyPaste-8i3q):** when at least one nonce is
/// expired (i.e. a Pong never arrived within `PING_PONG_TIMEOUT`), the TCP
/// connection has silently died (NAT drop, OS suspend, etc.).  The task then:
/// 1. Removes `peer_fp` from `peer_sinks` so `list_peers` reports offline.
/// 2. Emits `PeerEvent::Disconnected` so the Tauri event bridge pushes an
///    immediate UI update without waiting for the next `list_peers` poll.
/// 3. Exits — dropping `peer_sink` closes one sender; the connection task's
///    `run_peer_connection_framed` will eventually exit when the dead TCP
///    stream errors, and its cleanup block (same `same_channel` guard as the
///    accept/connector paths) skips re-emitting `Disconnected` because `peer_fp`
///    is already gone from `peer_sinks`.
///
/// The task also exits when `peer_sink.send` fails (the connection task has
/// already exited and dropped its receiver).
#[allow(clippy::too_many_arguments)] // RTT + presence params pushed count over 5
pub(super) async fn ping_loop(
    peer_sink: mpsc::Sender<PeerFrame>,
    peer_fp: DeviceFingerprint,
    pending_pings: PendingPings,
    peer_rtt_ms: PeerRttMs,
    // CopyPaste-8i3q: needed to evict the stale sink when a ping times out.
    peer_sinks: PeerSinks,
    peer_key: DeviceFingerprint,
    peer_event_tx: broadcast::Sender<PeerEvent>,
) {
    let mut interval = tokio::time::interval(PING_INTERVAL);
    // Skip the first (immediate) tick so we don't ping before the catchup
    // replay is done — the first real ping fires after PING_INTERVAL.
    interval.tick().await;

    loop {
        interval.tick().await;

        // Expire stale pending pings before sending a new one.
        // CopyPaste-8i3q: if any nonce expired it means we sent a Ping and
        // never received a matching Pong within PING_PONG_TIMEOUT — the TCP
        // connection is silently dead.  Evict the peer proactively so
        // `list_peers` reports offline immediately (instead of waiting for the
        // OS to eventually detect the dead TCP socket, which can take minutes).
        let had_expired = {
            let mut map = pending_pings.lock().await;
            let before = map.len();
            let now = Instant::now();
            map.retain(|_, sent_at| now.duration_since(*sent_at) < PING_PONG_TIMEOUT);
            map.len() < before // true iff at least one nonce was just expired
        };

        if had_expired {
            tracing::warn!(
                peer = %peer_fp,
                "RTT: Pong not received within {:?} — treating connection as dead",
                PING_PONG_TIMEOUT,
            );

            // Remove the sink from the shared map so list_peers sees offline.
            // The cleanup guard in the connection task uses `same_channel` to
            // avoid re-removing an already-gone entry, so no double-eviction.
            // Lock ordering: acquire peer_sinks, drop it, then acquire peer_rtt_ms
            // to avoid holding two tokio Mutexes simultaneously (lock-order safety).
            peer_sinks.lock().await.remove(&peer_key);
            peer_rtt_ms.lock().await.remove(&peer_fp);

            // Notify subscribers (Tauri event bridge) immediately so the UI
            // goes offline without waiting for the next list_peers poll.
            // `send` returns Err when there are no active receivers — ignore.
            let _ = peer_event_tx.send(PeerEvent::Disconnected {
                fingerprint: peer_key.clone(),
            });

            // Exit the ping task — the connection task will exit on its own
            // when the dead TCP stream finally errors (may take minutes via OS
            // keepalive), but presence is already corrected above.
            tracing::debug!(peer = %peer_fp, "RTT: ping loop exiting (dead connection evicted)");
            return;
        }

        let nonce: u64 = {
            use std::time::{SystemTime, UNIX_EPOCH};
            // Use current epoch nanos as a simple unique nonce within a
            // connection. Collisions within a single connection are harmless
            // (wrong RTT at worst); true randomness is unnecessary here.
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        };

        let ping = PeerFrame::Control(ControlMsg::Ping { nonce });
        let sent_at = Instant::now();

        // Store the send time BEFORE sending (so the RTT includes the send
        // time, not just the network transit). The nonce uniquely identifies
        // this ping within the connection lifetime.
        pending_pings.lock().await.insert(nonce, sent_at);

        if peer_sink.send(ping).await.is_err() {
            // The connection task has exited and the receiver was dropped.
            // Clean up our RTT entry and exit.
            tracing::debug!(peer = %peer_fp, "RTT: ping loop exiting (sink closed)");
            peer_rtt_ms.lock().await.remove(&peer_fp);
            return;
        }

        tracing::trace!(peer = %peer_fp, nonce, "RTT: sent Ping");
    }
}
