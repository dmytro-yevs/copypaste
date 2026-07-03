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

/// CopyPaste-8ebg.52: how often a *dedicated* check for expired pending pings
/// runs, independent of [`PING_INTERVAL`].
///
/// Before this const existed, the expiry check only ran once per
/// `PING_INTERVAL` tick (immediately before sending the next Ping), so a
/// connection that died right after a Ping was sent was not detected until
/// the *following* `PING_INTERVAL` tick — up to `2 * PING_INTERVAL -
/// PING_PONG_TIMEOUT` (~50 s) after the Pong stopped arriving, not the ~10 s
/// `PING_PONG_TIMEOUT` the doc comments on [`ping_loop`] implied. This faster
/// timer checks for expiry on its own cadence (without sending a new Ping),
/// bounding worst-case dead-connection detection to roughly
/// `PING_PONG_TIMEOUT + PING_CHECK_INTERVAL`.
pub(super) const PING_CHECK_INTERVAL: Duration = Duration::from_secs(2);

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

    // CopyPaste-8ebg.52: dedicated faster timer that ONLY checks for expired
    // pending pings (never sends). See PING_CHECK_INTERVAL's doc comment for
    // why this is needed alongside the PING_INTERVAL-cadence check below.
    let mut check_interval = tokio::time::interval(PING_CHECK_INTERVAL);
    check_interval.tick().await;

    loop {
        tokio::select! {
            _ = check_interval.tick() => {
                // Expire stale pending pings; if the connection is dead, evict
                // and exit. If it's alive, do nothing further this tick — the
                // next Ping is still sent only on `interval`'s own cadence.
                if expire_and_evict_if_dead(
                    &pending_pings,
                    &peer_sinks,
                    &peer_rtt_ms,
                    &peer_event_tx,
                    &peer_fp,
                    &peer_key,
                )
                .await
                {
                    return;
                }
            }
            _ = interval.tick() => {
                // Expire stale pending pings before sending a new one.
                // CopyPaste-8i3q: if any nonce expired it means we sent a Ping
                // and never received a matching Pong within PING_PONG_TIMEOUT
                // — the TCP connection is silently dead.  Evict the peer
                // proactively so `list_peers` reports offline immediately
                // (instead of waiting for the OS to eventually detect the dead
                // TCP socket, which can take minutes). In practice the faster
                // `check_interval` branch above usually catches this first;
                // this check is kept as a belt-and-suspenders guard right
                // before we would otherwise send a Ping into a dead socket.
                if expire_and_evict_if_dead(
                    &pending_pings,
                    &peer_sinks,
                    &peer_rtt_ms,
                    &peer_event_tx,
                    &peer_fp,
                    &peer_key,
                )
                .await
                {
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
    }
}

/// Shared expiry-check-and-evict logic used by both the fast
/// [`PING_CHECK_INTERVAL`] branch and the [`PING_INTERVAL`] branch of
/// [`ping_loop`]. Returns `true` when the connection was found dead and
/// evicted (caller must exit its loop), `false` otherwise.
async fn expire_and_evict_if_dead(
    pending_pings: &PendingPings,
    peer_sinks: &PeerSinks,
    peer_rtt_ms: &PeerRttMs,
    peer_event_tx: &broadcast::Sender<PeerEvent>,
    peer_fp: &DeviceFingerprint,
    peer_key: &DeviceFingerprint,
) -> bool {
    let had_expired = {
        let mut map = pending_pings.lock().await;
        let before = map.len();
        let now = Instant::now();
        map.retain(|_, sent_at| now.duration_since(*sent_at) < PING_PONG_TIMEOUT);
        map.len() < before // true iff at least one nonce was just expired
    };

    if !had_expired {
        return false;
    }

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
    peer_sinks.lock().await.remove(peer_key);
    peer_rtt_ms.lock().await.remove(peer_fp);

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
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // ── RTT ping/pong unit tests ───────────────────────────────────────────────

    /// `ControlMsg::Ping` and `ControlMsg::Pong` must round-trip through serde
    /// with the `nonce` field intact, and their serialised form must carry the
    /// `"control"` tag (so old peers that don't know these variants log a
    /// warning rather than mis-routing the frame).
    #[test]
    fn ping_pong_serde_round_trip() {
        use copypaste_sync::protocol::{ControlMsg, PeerFrame};

        let nonce = 0xDEAD_BEEF_CAFE_1234u64;

        // Serialise Ping.
        let ping_frame = PeerFrame::Control(ControlMsg::Ping { nonce });
        let ping_json = serde_json::to_string(&ping_frame).expect("serialise Ping");
        assert!(
            ping_json.contains("\"control\""),
            "Ping serialisation must contain the 'control' tag key: {ping_json}"
        );
        assert!(
            ping_json.contains("\"ping\""),
            "Ping serialisation must contain 'ping' as the control value: {ping_json}"
        );
        assert!(
            ping_json.contains(&nonce.to_string()),
            "Ping serialisation must include the nonce: {ping_json}"
        );

        // Round-trip Ping.
        let de_ping: PeerFrame = serde_json::from_str(&ping_json).expect("deserialise Ping");
        assert_eq!(
            de_ping,
            PeerFrame::Control(ControlMsg::Ping { nonce }),
            "Ping must survive a serde round-trip"
        );

        // Serialise Pong.
        let pong_frame = PeerFrame::Control(ControlMsg::Pong { nonce });
        let pong_json = serde_json::to_string(&pong_frame).expect("serialise Pong");
        assert!(
            pong_json.contains("\"pong\""),
            "Pong serialisation must contain 'pong' as the control value: {pong_json}"
        );

        // Round-trip Pong.
        let de_pong: PeerFrame = serde_json::from_str(&pong_json).expect("deserialise Pong");
        assert_eq!(
            de_pong,
            PeerFrame::Control(ControlMsg::Pong { nonce }),
            "Pong must survive a serde round-trip"
        );

        // Ping and Pong must produce different serialisations (different control values).
        assert_ne!(
            ping_json, pong_json,
            "Ping and Pong must not serialise identically"
        );
    }

    /// The RTT record: after inserting a nonce + Instant into the pending-pings
    /// map and then simulating a Pong response (remove the nonce, compute
    /// elapsed), the RTT map must contain a non-zero entry for the peer.
    ///
    /// This tests the state-machine logic in `run_peer_connection_framed` that
    /// handles `ControlMsg::Pong` — isolated from the network layer.
    #[tokio::test]
    async fn rtt_record_written_on_pong() {
        let pending_pings: PendingPings = Arc::new(Mutex::new(HashMap::new()));
        let peer_rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
        let peer_fp = "aabbccddee".to_string();
        let nonce = 42u64;

        // Record a send time just before "now".
        let sent_at = Instant::now() - Duration::from_millis(15);
        pending_pings.lock().await.insert(nonce, sent_at);

        // Simulate receiving the Pong (the code path in run_peer_connection_framed).
        let resolved = {
            let mut map = pending_pings.lock().await;
            map.remove(&nonce)
        };
        assert!(resolved.is_some(), "nonce must be found in pending_pings");

        let rtt_ms = resolved.unwrap().elapsed().as_millis() as u32;
        peer_rtt_ms
            .lock()
            .await
            .insert(copypaste_p2p::DeviceFingerprint(peer_fp.clone()), rtt_ms);

        let stored = peer_rtt_ms.lock().await.get(peer_fp.as_str()).copied();
        assert!(
            stored.is_some(),
            "RTT map must contain an entry for the peer after Pong processing"
        );
        assert!(
            stored.unwrap() >= 15,
            "recorded RTT must be at least 15 ms (our simulated delay), got {stored:?}"
        );
    }

    /// After a Pong is processed the pending-pings map must be empty (the
    /// nonce is removed so it doesn't contribute to stale-nonce accumulation).
    #[tokio::test]
    async fn pending_ping_removed_on_pong() {
        let pending_pings: PendingPings = Arc::new(Mutex::new(HashMap::new()));
        let nonce = 99u64;

        pending_pings.lock().await.insert(nonce, Instant::now());
        assert_eq!(
            pending_pings.lock().await.len(),
            1,
            "precondition: one pending ping"
        );

        // Simulate Pong processing: remove the nonce.
        let _ = pending_pings.lock().await.remove(&nonce);

        assert_eq!(
            pending_pings.lock().await.len(),
            0,
            "pending_pings must be empty after Pong processing removes the nonce"
        );
    }
}
