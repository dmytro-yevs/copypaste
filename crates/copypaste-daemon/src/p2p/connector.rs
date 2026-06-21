//! Proactive peer connector loop, dialable-peer helpers, and pending-unpair delivery.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::SinkExt;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use copypaste_p2p::{
    connector::{should_dial_peer, DialBackoff},
    discovery::DiscoveryService,
    transport::{DeviceFingerprint, PairedPeers, PeerTransport},
};
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};

use super::{CatchupProvider, PeerEvent, PeerRttMs, PeerSinks};
use super::fanout::push_catchup;
use super::framed_pump::{run_peer_connection_client, WRITE_TIMEOUT};
use super::ping::ping_loop;
use super::unpair::stamp_peer_sync;

/// How often the [`peer_connector_loop`] wakes to check for paired-but-not-
/// connected peers to dial.
const CONNECTOR_TICK: Duration = Duration::from_secs(3);

/// Resolve a fresh dial address for `fingerprint` from the mDNS discovery
/// service.
///
/// Iterates the current snapshot of discovered peers and returns a
/// `SocketAddr` for the first peer whose `device_id` matches `fingerprint`
/// (exact string match after the caller has already normalised both sides to
/// canonical form).  The first IPv4 address is preferred over IPv6 to maximise
/// compatibility; if only IPv6 addresses are present the first one is used.
///
/// Returns `None` when:
/// - discovery has no peers at all,
/// - no peer's `device_id` matches `fingerprint`, or
/// - the matching peer has an empty `ip_addrs` list.
///
/// This is **best-effort**: the discovery snapshot may be stale (mDNS
/// re-announcement period is typically 1–5 minutes) and is never guaranteed to
/// reflect a peer's current address.  The connector must not rely on it as the
/// sole source of truth — it is a fallback consulted only after a persisted
/// address fails.
pub(super) fn resolve_addr_from_discovery(
    discovery: &DiscoveryService,
    fingerprint: &str,
) -> Option<SocketAddr> {
    // `resolve_peer` matches by device_id (already the right semantic).
    let peer = discovery.resolve_peer(fingerprint)?;
    // Prefer IPv4 for broadest compatibility; fall back to the first address
    // regardless of family if no IPv4 is found.  `ip_addrs` is sorted IPv4-
    // first by `peer_from_resolved`, so `find` over a non-empty vec is O(n)
    // with n typically ≤ 2.
    let ip = peer
        .ip_addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| peer.ip_addrs.first())?;
    Some(SocketAddr::new(*ip, peer.port))
}

/// IP-correlated fallback for [`resolve_addr_from_discovery`].
///
/// The device_id-keyed lookup above never matches a real peer: mDNS advertises
/// a per-device UUID as `device_id`, but a paired peer is keyed by its SHA-256
/// cert fingerprint — the two are different strings, so `resolve_peer` returns
/// `None` and the connector keeps dialing a stale persisted port forever.
///
/// On a LAN the host IP uniquely identifies a peer, so when the persisted dial
/// address fails we correlate by IP instead: find the discovered peer that
/// advertises the same IP as the address that just failed and adopt its freshly
/// announced sync port. This is what self-heals the common failure mode — both
/// peers bind an **ephemeral** sync-listener port that drifts on every
/// daemon/app restart, leaving the port persisted at pairing time stale.
pub(super) fn resolve_addr_from_discovery_by_ip(
    discovery: &DiscoveryService,
    failed_addr: SocketAddr,
) -> Option<SocketAddr> {
    let want_ip = failed_addr.ip();
    discovery
        .peers()
        .into_iter()
        .find(|p| p.ip_addrs.contains(&want_ip))
        .map(|p| SocketAddr::new(want_ip, p.port))
}

/// Proactively refresh a paired peer's `name`, `address`, and `local_ip` from
/// the live mDNS discovery snapshot.
///
/// Called every [`CONNECTOR_TICK`] for each dialable peer, regardless of
/// connection state.  Correlates by the IP component of the peer's persisted
/// `address` — the mDNS `device_id` is a UUID, never a cert fingerprint, so
/// fingerprint-keyed lookup (`resolve_addr_from_discovery`) would never match.
///
/// When a matching mDNS peer is found and any of its fields (name, sync port,
/// IP) differ from what is persisted, [`crate::peers::update_peer_meta`] rewrites
/// `peers.json` in place (atomic 0600 rename).  The next [`crate::ipc`]
/// `list_peers` poll then surfaces the fresh values to the UI.
///
/// # Out-of-scope fields
/// `model`, `os_version`, `app_version`, and `public_ip` are learned in-band
/// over the bootstrap channel at pairing time and are NOT carried by mDNS TXT
/// records — they are untouched here.  Refreshing them reactively would require
/// a new wire-protocol extension and is deferred to a future release.
pub(super) fn refresh_peer_meta_from_discovery(
    peers_path: &std::path::Path,
    fingerprint: &str,
    persisted_addr: SocketAddr,
    discovery: &DiscoveryService,
) {
    let want_ip = persisted_addr.ip();
    let Some(discovered) = discovery
        .peers()
        .into_iter()
        .find(|p| p.ip_addrs.contains(&want_ip))
    else {
        // Peer not in the current mDNS snapshot — nothing to refresh.
        return;
    };

    let fresh_addr = SocketAddr::new(want_ip, discovered.port);
    let fresh_name = discovered.device_name.as_str();
    let local_ip_str = want_ip.to_string();

    match crate::peers::update_peer_meta(
        peers_path,
        fingerprint,
        fresh_name,
        fresh_addr,
        &local_ip_str,
    ) {
        Ok(true) => {
            tracing::debug!(
                %fingerprint,
                %fresh_addr,
                name = %fresh_name,
                "connector: refreshed peer name+addr from mDNS"
            );
        }
        Ok(false) => {} // Nothing changed — no log noise.
        Err(e) => {
            tracing::warn!(
                %fingerprint,
                error = %e,
                "connector: failed to persist mDNS meta refresh"
            );
        }
    }
}

/// A dialable paired peer resolved from `peers.json`.
#[derive(Clone)]
pub(super) struct DialablePeer {
    /// Canonical (colon-free, lowercase) cert fingerprint — the mTLS pin.
    pub(super) fingerprint: DeviceFingerprint,
    /// The peer's sync-listener socket address.
    pub(super) addr: SocketAddr,
}

/// CopyPaste-c1dd: mtime-gated cache for the dialable-peer list so the connector
/// loop does not re-read + re-parse `peers.json` from disk on every 3 s tick.
///
/// `peers.json` only changes when the user pairs/unpairs or when
/// `refresh_peer_meta_from_discovery` writes an updated peer record; both bump
/// the file mtime, which invalidates the cache. The steady state (no pairing
/// activity) reads only the cheap `fs::metadata` mtime and reuses the parsed
/// Vec, avoiding a full read+JSON-parse every tick.
#[derive(Default)]
pub(super) struct DialablePeersCache {
    /// Last observed file modification time; `None` until the first read.
    last_mtime: Option<std::time::SystemTime>,
    /// Cached parse result reused while the mtime is unchanged.
    cached: Vec<DialablePeer>,
}

impl DialablePeersCache {
    /// Return the dialable peers for `path`, re-reading + re-parsing from disk
    /// only when the file mtime has changed since the last call (or on the first
    /// call, or if the mtime cannot be read — fail safe by always re-reading).
    ///
    /// Returns an owned `Vec` (a cheap clone of the cached list — a handful of
    /// `String` + `SocketAddr` per peer) so the connector loop keeps its
    /// existing by-value iteration; the avoided cost is the per-tick file read +
    /// JSON parse, not the small Vec clone.
    pub(super) fn get(&mut self, path: &std::path::Path) -> Vec<DialablePeer> {
        let current_mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        // Re-read when: first call (last_mtime None), mtime changed, or mtime is
        // unavailable (treat as "may have changed" to never serve stale data).
        let stale = match (current_mtime, self.last_mtime) {
            (Some(now), Some(prev)) => now != prev,
            _ => true,
        };
        if stale {
            self.cached = dialable_peers_from_path(path);
            self.last_mtime = current_mtime;
        }
        self.cached.clone()
    }
}

/// Read `peers.json` and return the paired peers that carry a parseable sync
/// `address` — the set the connector may dial. Peers with no address (legacy
/// records, or a peer that never advertised one) are skipped: the connector
/// has nothing to dial and relies on the peer dialing us instead.
pub(super) fn dialable_peers_from_path(path: &std::path::Path) -> Vec<DialablePeer> {
    let stored = crate::peers::load_peers(path);
    let mut out = Vec::new();
    for dev in &stored {
        if dev.fingerprint.is_empty() {
            continue;
        }
        let Some(addr_str) = dev.address.as_deref() else {
            continue;
        };
        let addr = match addr_str.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(addr = %addr_str, error = %e, "skipping peer with unparseable sync address");
                continue;
            }
        };
        out.push(DialablePeer {
            fingerprint: crate::ipc::canonical_fingerprint(&dev.fingerprint),
            addr,
        });
    }
    out
}

/// Proactively dial paired peers that are not currently connected (Phase 3).
///
/// Each tick re-reads `peers.json` (so a peer paired at runtime is picked up
/// without a restart), then for every paired peer that has a sync address and
/// is **not** already in `peer_sinks`, dials it with
/// [`PeerTransport::connect_with_retry`]. On success the per-connection sink is
/// registered in `peer_sinks` (keyed by fingerprint) and the SAME
/// [`run_peer_connection`] handler the accept loop uses is spawned, so inbound
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
#[allow(clippy::too_many_arguments)] // RTT + event params pushed count over 9
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

/// Deliver any durable `pending_unpair.json` records (Gap A).
///
/// For each queued [`PendingUnpair`](crate::peers::PendingUnpair) that carries a
/// parseable dial address (and is not our own fingerprint), this:
///   1. TEMPORARILY allow-lists the peer's fingerprint on the live
///      [`PairedPeers`] so the outbound mTLS handshake is accepted by the peer
///      (the peer pins US, but we must pin THEM to connect);
///   2. dials the peer and sends ONE `PeerFrame::Control(ControlMsg::Unpair)`;
///   3. removes the peer from the live allowlist again (it must NOT resume sync);
///   4. removes the record from `pending_unpair.json` so it is delivered once.
///
/// Best-effort: a dial/connect/send failure leaves the record in place for a
/// retry on the next tick (the entry is removed from the live allowlist either
/// way, so a transient allow-list never lingers). Records with no address are
/// left untouched — there is nothing to dial.
pub(super) async fn deliver_pending_unpairs(
    transport: &PeerTransport,
    pending_path: &std::path::Path,
    own_fp: &str,
    live_peers: &PairedPeers,
) {
    let pending = crate::peers::load_pending_unpairs(pending_path);
    if pending.is_empty() {
        return;
    }

    for entry in pending {
        let canonical = crate::ipc::canonical_fingerprint(&entry.fingerprint);
        if canonical.is_empty() || canonical == own_fp {
            // Never dial ourselves; drop a degenerate record so it cannot wedge
            // the queue forever.
            let _ = crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint);
            continue;
        }
        let Some(addr_str) = entry.address.as_deref() else {
            // No address — cannot dial. Leave it queued for a future improvement
            // that learns the address out-of-band.
            continue;
        };
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(
                    addr = %addr_str,
                    error = %e,
                    "pending-unpair: unparseable address — dropping record"
                );
                let _ = crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint);
                continue;
            }
        };

        // Temporarily allow-list so our own client config will accept the peer's
        // pinned cert on the handshake. Removed again below regardless of outcome.
        live_peers.add(canonical.clone(), entry.name.clone());

        let dialed = transport.connect_with_retry(addr, &canonical).await;
        match dialed {
            Ok(mut stream) => {
                let frame = PeerFrame::Control(ControlMsg::Unpair);
                let sent = match serde_json::to_vec(&frame) {
                    Ok(payload) => {
                        tokio::time::timeout(WRITE_TIMEOUT, stream.send(Bytes::from(payload)))
                            .await
                            .map(|r| r.is_ok())
                            .unwrap_or(false)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "pending-unpair: failed to serialise Unpair frame");
                        false
                    }
                };
                // Close our end promptly — we have nothing more to say.
                drop(stream);
                if sent {
                    tracing::info!(
                        peer = %canonical,
                        %addr,
                        "pending-unpair: delivered deferred Unpair to reconnected peer"
                    );
                    if let Err(e) =
                        crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint)
                    {
                        tracing::warn!(
                            peer = %canonical,
                            error = %e,
                            "pending-unpair: delivered but failed to dequeue record"
                        );
                    }
                } else {
                    tracing::debug!(
                        peer = %canonical,
                        "pending-unpair: connect ok but send failed — will retry next tick"
                    );
                }
            }
            Err(e) => {
                tracing::debug!(
                    peer = %canonical,
                    %addr,
                    error = %e,
                    "pending-unpair: dial failed — will retry next tick"
                );
            }
        }

        // Always drop the transient allow-list entry so the peer can never
        // resume normal sync through this window.
        live_peers.remove(&canonical);
    }
}
