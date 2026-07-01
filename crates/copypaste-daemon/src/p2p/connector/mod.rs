//! Proactive peer connector loop, dialable-peer helpers, and pending-unpair delivery.
//!
//! Split (ADR-017, CopyPaste-vp63.48) into:
//! - [`discovery_resolve`] — mDNS address resolution/refresh helpers.
//! - [`dialable`] — dialable-peer list + mtime-gated cache.
//! - [`pending_unpair`] — durable `pending_unpair.json` delivery (Gap A).
//! - this file — the main [`peer_connector_loop`] orchestration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use copypaste_p2p::{
    connector::{should_dial_peer, DialBackoff},
    discovery::DiscoveryService,
    transport::{DeviceFingerprint, PairedPeers, PeerTransport},
};
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};

use super::fanout::push_catchup;
use super::framed_pump::run_peer_connection_client;
use super::ping::ping_loop;
use super::unpair::stamp_peer_sync;
use super::{CatchupProvider, PeerEvent, PeerRttMs, PeerSinks};

mod dialable;
mod discovery_resolve;
mod pending_unpair;

use dialable::DialablePeersCache;
use pending_unpair::deliver_pending_unpairs;

// Re-established at the exact reach the flat `connector.rs` file had
// (`pub(super)`, i.e. visible to `p2p`), now that these helpers live one
// directory level deeper — mirrors the same pattern used for `sync_orch/rekey`.
// `dialable_peers_from_path` (bare re-export) is only reached from the p2p
// unit tests (`p2p/mod.rs`'s `#[cfg(test)] mod tests`); production code goes
// through `DialablePeersCache` instead. Gate to avoid an unused-import
// warning in non-test builds.
#[cfg(test)]
pub(in crate::p2p) use dialable::dialable_peers_from_path;
pub(in crate::p2p) use discovery_resolve::resolve_addr_from_discovery;
use discovery_resolve::{refresh_peer_meta_from_discovery, resolve_addr_from_discovery_by_ip};

/// How often the [`peer_connector_loop`] wakes to check for paired-but-not-
/// connected peers to dial.
const CONNECTOR_TICK: Duration = Duration::from_secs(3);

/// Proactively dial paired peers that are not currently connected (Phase 3).
///
/// Each tick re-reads `peers.json` (so a peer paired at runtime is picked up
/// without a restart), then for every paired peer that has a sync address and
/// is **not** already in `peer_sinks`, dials it with
/// [`PeerTransport::connect_with_retry`]. On success the per-connection sink is
/// registered in `peer_sinks` (keyed by fingerprint) and the SAME
/// `run_peer_connection` handler the accept loop uses is spawned, so inbound
/// items flow to `incoming_tx` and outbound items fan out.
///
/// # Avoiding deadlock / thrash
/// * Locks on `peer_sinks` are held only for the brief insert/contains checks
///   (never across the `connect_with_retry` await), so the accept loop and the
///   fanout loop are never blocked by an in-flight dial.
/// * Already-connected peers are skipped (cheap `contains_key`).
/// * We never dial our own fingerprint (`own_fp`).
/// * Per-peer exponential backoff (`CONNECTOR_BACKOFF_STEPS`) spaces out
///   retries to an offline peer instead of dialing every tick.
///
/// # Double-connect race (both sides dialing)
/// `peer_sinks` is keyed by cert fingerprint. If both daemons dial each other
/// at once, two connections may briefly exist; whichever sink is inserted last
/// wins the map slot and the superseded connection's per-connection task drops
/// its (now-unreferenced) sink and exits when the stream closes — no duplicate
/// fan-out. We additionally re-check `contains_key` immediately before dialing
/// to skip a peer the accept loop just connected.
#[allow(clippy::too_many_arguments)] // RTT + event + public_ip params pushed count over 9
pub(super) async fn peer_connector_loop(
    transport: Arc<PeerTransport>,
    peer_sinks: PeerSinks,
    incoming_tx: mpsc::Sender<WireItem>,
    own_fp: DeviceFingerprint,
    catchup: CatchupProvider,
    // Injected mDNS discovery service — consulted as a fallback when a
    // persisted dial address fails (peer DHCP renew / network switch).
    discovery: Arc<DiscoveryService>,
    shutdown: CancellationToken,
    // Live mTLS allowlist (Gap A + Gap B). Forwarded to per-connection tasks for
    // inbound-unpair eviction, and used to TEMPORARILY allow-list a
    // `pending_unpair` peer just long enough to dial it and deliver the deferred
    // `ControlMsg::Unpair` frame.
    live_peers: PairedPeers,
    // Shared RTT map — updated by the ping task spawned per connection.
    peer_rtt_ms: PeerRttMs,
    // Broadcast channel for peer connect/disconnect events.
    peer_event_tx: broadcast::Sender<PeerEvent>,
    // crh3.109: STUN-resolved public IP cache. Read once per new outbound
    // connection so DeviceInfo carries the current WAN address of THIS device.
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
) {
    tracing::debug!(%own_fp, "P2P peer connector loop running");
    let peers_path = crate::ipc::peers_file_path();
    let pending_path = crate::peers::pending_unpair_path_for(&peers_path);
    let mut dial_state: HashMap<DeviceFingerprint, DialBackoff> = HashMap::new();
    // CopyPaste-c1dd: mtime-gated cache so peers.json is not re-read+parsed every
    // 3 s tick when nothing changed.
    let mut dialable_cache = DialablePeersCache::default();

    loop {
        // BUG F1: race the inter-tick sleep against cancellation so shutdown wins
        // instead of waiting up to CONNECTOR_TICK for the next wake.
        tokio::select! {
            _ = tokio::time::sleep(CONNECTOR_TICK) => {}
            _ = shutdown.cancelled() => {
                tracing::info!("P2P peer connector loop shutting down");
                break;
            }
        }

        // Gap A: drain durable pending-unpair deliveries first. Each entry that
        // has a dial address is temporarily allow-listed, dialed, sent a single
        // `Unpair` frame, then removed from both the live allowlist and the file.
        deliver_pending_unpairs(&transport, &pending_path, &own_fp, &live_peers).await;

        let peers = dialable_cache.get(&peers_path);
        // Drop dial-state for peers no longer present (unpaired) so the map
        // does not grow unbounded across re-pairings.
        let live: std::collections::HashSet<&str> =
            peers.iter().map(|p| p.fingerprint.as_str()).collect();
        dial_state.retain(|fp, _| live.contains(fp.as_str()));

        for peer in peers {
            // Never dial ourselves (our own record could appear if a future
            // bug wrote it, or in a single-host test that pairs a daemon's
            // own fingerprint).
            if peer.fingerprint == own_fp {
                continue;
            }

            // Live mDNS meta refresh: every tick, correlate the peer by IP and
            // adopt its freshly-announced device_name, sync port, and local_ip
            // into peers.json so `list_peers` surfaces up-to-date values even
            // without a dial.  Cheap — just a snapshot read + optional file write
            // when something actually changed.  The on-failure address refresh
            // (below) is a superset of this for the error path; this call covers
            // the steady-state case (connected peer renames itself).
            refresh_peer_meta_from_discovery(&peers_path, &peer.fingerprint, peer.addr, &discovery);

            // M1: skip peers we already have a *healthy* live sink for, but
            // force-replace a stale (closed-but-unreaped) one. Checking only
            // `contains_key` let a dead connection block reconnection until the
            // cleanup pass evicted it. Mirror the accept path's health check.
            let sink_health = {
                let sinks = peer_sinks.lock().await;
                sinks.get(&peer.fingerprint).map(|tx| !tx.is_closed())
            };
            let now = Instant::now();
            if !should_dial_peer(sink_health) {
                // A healthy connection is live. M3: this is also the moment to
                // reset the peer's backoff — but only once the link has proven
                // stable for MIN_HEALTHY_DWELL, so a flapping peer never resets.
                if let Some(state) = dial_state.get_mut(&peer.fingerprint) {
                    if state.maybe_reset_after_dwell(now) {
                        tracing::debug!(
                            fingerprint = %peer.fingerprint,
                            "connector: connection healthy past dwell — backoff reset"
                        );
                    }
                }
                continue;
            }

            // The sink is absent or stale, so the link is down. M3: tell the
            // backoff state the connection dropped, clearing `connected_since`.
            // Otherwise a sub-dwell flap leaves the OLD connect instant in place
            // and a later `maybe_reset_after_dwell` would measure wall-time from
            // it and wrongly reset the backoff even though the new connection
            // never dwelled — defeating the anti-flap guarantee. The backoff
            // index is preserved so escalation continues.
            if let Some(state) = dial_state.get_mut(&peer.fingerprint) {
                state.record_disconnected();
            }

            // Respect per-peer backoff.
            if let Some(state) = dial_state.get(&peer.fingerprint) {
                if !state.may_dial(now) {
                    continue;
                }
            }

            tracing::debug!(fingerprint = %peer.fingerprint, addr = %peer.addr, "connector dialing paired peer");
            // BUG F1 (verification follow-up): race the dial against cancellation.
            // `connect_with_retry` can take up to ~60s (4 attempts × ~15s) before it
            // returns, so without this select a shutdown that lands mid-dial would
            // stall the connector loop for the full retry budget. `connect_with_retry`
            // only opens a TCP/TLS connection and is cancel-safe, so dropping the
            // in-flight future on cancel is sound. On cancel we exit the loop.
            let dial = tokio::select! {
                biased;
                _ = shutdown.cancelled() => {
                    tracing::info!("P2P peer connector loop shutting down (cancelled mid-dial)");
                    break;
                }
                result = transport.connect_with_retry(peer.addr, &peer.fingerprint) => result,
            };
            match dial {
                Ok(stream) => {
                    // Re-check under the lock: the accept loop may have
                    // registered an inbound connection from this peer while we
                    // were dialing. If so, drop our outbound duplicate.
                    let mut sinks = peer_sinks.lock().await;
                    if sinks.contains_key(&peer.fingerprint) {
                        tracing::debug!(
                            fingerprint = %peer.fingerprint,
                            "connector: peer already connected (accept loop won the race) — dropping outbound duplicate"
                        );
                        drop(sinks);
                        drop(stream);
                    } else {
                        let (peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(64);
                        let cleanup_tx = peer_tx.clone();
                        sinks.insert(peer.fingerprint.clone(), peer_tx);
                        drop(sinks);

                        // Notify subscribers that this peer is now online.
                        let _ = peer_event_tx.send(PeerEvent::Connected {
                            fingerprint: peer.fingerprint.clone(),
                        });

                        tracing::info!(fingerprint = %peer.fingerprint, addr = %peer.addr, "connector established outbound mTLS link");

                        // Stamp first/last sync times for this peer (once per
                        // established connection — see `stamp_peer_sync`).
                        stamp_peer_sync(&peers_path, &peer.fingerprint);

                        // crh3.109: advertise our own current device metadata
                        // to the peer so it can refresh its stale pairing-time
                        // snapshot. `try_send` is non-blocking and fire-and-forget.
                        {
                            let meta = crate::device_meta::get_cached(crate::ipc::BUILD_VERSION);
                            let own_public_ip =
                                public_ip_cache.try_read().ok().and_then(|g| g.clone());
                            let frame = PeerFrame::Control(ControlMsg::DeviceInfo {
                                model: meta.device_model.clone(),
                                os_version: meta.os_version.clone(),
                                app_version: Some(meta.app_version.clone()),
                                public_ip: own_public_ip,
                            });
                            let _ = cleanup_tx.try_send(frame);
                        }

                        // Clone the sink sender for the catch-up replay BEFORE the
                        // drainer task takes ownership of `cleanup_tx`. The drainer
                        // MUST start first: `push_catchup` does a bounded
                        // `send().await` over the ENTIRE local history (commonly far
                        // more than the 64-slot channel capacity), so with no active
                        // receiver draining `peer_rx` it deadlocks the moment the
                        // buffer fills — the sink then stays full forever and the
                        // peer receives nothing.
                        let catchup_tx = cleanup_tx.clone();

                        let incoming_tx = incoming_tx.clone();
                        let peer_sinks = Arc::clone(&peer_sinks);
                        let peer_key = peer.fingerprint.clone();
                        let peer_fp_for_task = peer.fingerprint.clone();
                        let live_peers_for_task = live_peers.clone();
                        // Clone for the cleanup task's Disconnected event.
                        let disconnect_event_tx = peer_event_tx.clone();

                        // RTT: per-connection pending-pings map shared between
                        // the ping sender task and the connection task.
                        let pending_pings = Arc::new(Mutex::new(HashMap::new()));
                        let rtt_map_for_task = Arc::clone(&peer_rtt_ms);
                        let rtt_map_for_ping = Arc::clone(&peer_rtt_ms);
                        let pending_pings_for_conn = Arc::clone(&pending_pings);
                        let ping_fp = peer.fingerprint.clone();
                        let ping_sink = cleanup_tx.clone();
                        // CopyPaste-8i3q: pass peer_sinks + peer_key + event_tx
                        // so ping_loop can evict the stale sink and emit
                        // Disconnected when a Pong times out.
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
                            run_peer_connection_client(
                                stream,
                                peer_rx,
                                incoming_tx,
                                peer_fp_for_task,
                                Some(live_peers_for_task),
                                pending_pings_for_conn,
                                rtt_map_for_task,
                            )
                            .await;
                            let mut sinks = peer_sinks.lock().await;
                            if sinks
                                .get(&peer_key)
                                .is_some_and(|tx| tx.same_channel(&cleanup_tx))
                            {
                                sinks.remove(&peer_key);
                                // Emit Disconnected only when we owned the sink.
                                let _ = disconnect_event_tx.send(PeerEvent::Disconnected {
                                    fingerprint: peer_key.clone(),
                                });
                            }
                            drop(sinks);
                            tracing::debug!(fingerprint = %peer_key, "connector outbound connection closed");
                        });

                        // Drainer is now consuming `peer_rx`, so replaying the local
                        // history (sync-on-connect) cannot deadlock on a full sink.
                        // Items are re-keyed under this peer's pairwise sync key
                        // (CopyPaste-716).
                        push_catchup(&catchup, peer.fingerprint.as_str(), &catchup_tx).await;
                    }
                    // M3: a successful connect records the connection start but
                    // does NOT reset the backoff yet — a flapping peer that
                    // connects then immediately drops must not wipe accumulated
                    // backoff. The reset is gated on MIN_HEALTHY_DWELL and
                    // applied lazily on a later tick (see the skip branch above).
                    dial_state
                        .entry(peer.fingerprint.clone())
                        .or_default()
                        .record_connected(Instant::now());
                }
                Err(e) => {
                    let step = dial_state
                        .entry(peer.fingerprint.clone())
                        .or_default()
                        .record_failure(Instant::now());
                    tracing::debug!(
                        fingerprint = %peer.fingerprint,
                        addr = %peer.addr,
                        backoff_secs = step,
                        error = %e,
                        "connector dial failed — backing off"
                    );

                    // mDNS address refresh (P2P audit P2 #3): on dial failure,
                    // consult the live discovery snapshot to see if the peer
                    // has a fresh LAN address (DHCP renew / network switch).
                    // Only act when discovery returns an address that DIFFERS
                    // from the one that just failed — avoids a spurious write
                    // that would be a no-op at best.  The existing per-peer
                    // backoff already rate-limits how often we reach this
                    // branch, so no additional throttle is needed here.
                    if let Some(fresh_addr) =
                        resolve_addr_from_discovery(&discovery, &peer.fingerprint).or_else(|| {
                            // device_id match failed (mDNS device_id is a UUID,
                            // not the cert fingerprint) — correlate by IP and
                            // adopt the peer's freshly advertised port.
                            resolve_addr_from_discovery_by_ip(&discovery, peer.addr)
                        })
                    {
                        if fresh_addr != peer.addr {
                            tracing::info!(
                                fingerprint = %peer.fingerprint,
                                stale_addr = %peer.addr,
                                %fresh_addr,
                                "connector: mDNS returned a fresher address — updating peers.json"
                            );
                            if let Err(persist_err) = crate::peers::update_peer_address(
                                &peers_path,
                                &peer.fingerprint,
                                fresh_addr,
                            ) {
                                tracing::warn!(
                                    fingerprint = %peer.fingerprint,
                                    error = %persist_err,
                                    "connector: failed to persist refreshed peer address"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
