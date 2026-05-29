//! P2P subsystem orchestrator.
//!
//! W2.2 — wires the mTLS accept loop and outbound fanout into the daemon,
//! bridging `copypaste-p2p` transport with the `sync_orch` channel pair
//! (`incoming_tx` / `outbound_rx`).
//!
//! Pairing (`pair_peer` / `unpair_peer`) currently returns
//! [`P2pError::NotImplemented`] — the PAKE handshake lands in W2.4.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

use copypaste_core::{ClipboardItem, Database};
use copypaste_p2p::{
    connector::{should_dial_peer, DialBackoff},
    discovery::{DiscoveryService, PeerInfo},
    transport::{DeviceFingerprint, PairedPeers, PeerTransport},
};
use copypaste_sync::protocol::WireItem;

use crate::keychain;

/// Errors emitted by the daemon-side P2P surface.
#[derive(Debug, Error)]
pub enum P2pError {
    /// Discovery service failed to start or register.
    #[error("Discovery error: {0}")]
    Discovery(String),

    /// Transport (mTLS) setup failed.
    #[error("Transport error: {0}")]
    Transport(String),

    /// I/O error while binding the TCP listener.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The requested pairing operation is not yet implemented (PAKE — W2.4).
    #[error("Pairing not yet implemented (PAKE lands in W2.4)")]
    NotImplemented,
}

/// Configuration for the P2P subsystem.
pub struct P2pConfig {
    /// TCP port to listen on.  0 = OS-assigned ephemeral port.
    pub listen_port: u16,
    /// Human-readable name advertised via mDNS.
    pub device_name: String,
    /// When false `start_p2p` returns immediately without spawning any tasks.
    pub enabled: bool,
}

/// Live handle to a running P2P subsystem (returned from [`start_p2p`]).
pub struct P2pHandle {
    /// The actual TCP port bound by the listener (useful when `listen_port` was 0).
    pub actual_port: u16,
    /// Send `()` to request a graceful shutdown of all P2P tasks.
    pub shutdown_tx: oneshot::Sender<()>,
}

/// Lightweight, synchronously-constructed P2P state used by the IPC layer.
///
/// Holds the discovery service (already configured) plus an
/// `Arc<PeerTransport>` ready for outbound `connect()` / inbound `accept()`
/// calls. Distinct from [`P2pHandle`] (which owns the long-running background
/// tasks) — `P2pState` is the pure-data view that IPC handlers query.
pub struct P2pState {
    /// mDNS-SD discovery service. Already configured via `register()`.
    pub discovery: Arc<DiscoveryService>,
    /// mTLS transport with own self-signed cert.
    pub transport: Arc<PeerTransport>,
    /// Snapshot of paired peers.
    pub peers: Arc<Mutex<PairedPeers>>,
}

/// Initialise a `P2pState` synchronously: generate a fresh self-signed cert,
/// build a discovery service, and call `register()` for mDNS-SD.
///
/// The returned `P2pState` is safe to share across IPC handlers. A real
/// `TcpListener` is *not* bound here — the long-running [`start_p2p`] entry
/// point owns the accept loop. `init` is intended for the lightweight IPC
/// query path (list/pair/unpair/own_fingerprint).
///
/// # Errors
/// Returns [`P2pError::Transport`] if cert generation fails, or
/// [`P2pError::Discovery`] if mDNS registration cannot be configured.
pub fn init(listen_port: u16, device_id: &str, device_name: &str) -> Result<P2pState, P2pError> {
    let peers = PairedPeers::new();
    let transport = PeerTransport::new_with_generated_cert(device_id, peers.clone())
        .map_err(|e| P2pError::Transport(e.to_string()))?;

    let discovery = DiscoveryService::new();
    discovery
        .register(listen_port, device_id, device_name)
        .map_err(|e| P2pError::Discovery(e.to_string()))?;

    Ok(P2pState {
        discovery: Arc::new(discovery),
        transport: Arc::new(transport),
        peers: Arc::new(Mutex::new(peers)),
    })
}

/// Return the list of peers currently visible via mDNS-SD.
///
/// Replaces the wave-1.3 IPC stub (`ipc.rs::"list_peers"`).
pub fn list_peers(state: &P2pState) -> Vec<PeerInfo> {
    state.discovery.peers()
}

/// Pair with a peer using PAKE (Password-Authenticated Key Exchange).
///
/// **Not yet implemented** — returns [`P2pError::NotImplemented`].
/// PAKE-based pairing lands in W2.4.
pub fn pair_peer(
    _state: &P2pState,
    _peer_fingerprint: &str,
    _display_name: &str,
) -> Result<(), P2pError> {
    Err(P2pError::NotImplemented)
}

/// Remove a previously-paired peer.
///
/// **Not yet implemented** — returns [`P2pError::NotImplemented`].
/// Lands in W2.4 alongside `pair_peer`.
pub fn unpair_peer(_state: &P2pState, _peer_fingerprint: &str) -> Result<(), P2pError> {
    Err(P2pError::NotImplemented)
}

/// Compute the canonical device fingerprint from a raw public key.
///
/// Delegates to [`keychain::own_fingerprint`] for consistency with the rest
/// of the daemon (single source of truth for fingerprint format).
pub fn get_own_fingerprint(public_key: &[u8]) -> String {
    keychain::own_fingerprint(public_key)
}

/// Shared map of currently-connected peer sinks.
///
/// Each entry is a per-connection `mpsc::Sender<WireItem>` that the
/// per-connection write task drains, serialises and sends to the peer over
/// the mTLS Framed stream. The outbound fanout loop writes to every live
/// sender; closed senders (disconnected peers) are pruned on the next
/// fanout pass.
///
/// Keyed by the peer's verified **certificate fingerprint** (not its socket
/// address): a reconnect from a fresh ephemeral source port reuses the same
/// key, so the new connection replaces the old sink rather than producing a
/// duplicate that would double-fan-out every item (fix/p2p-c-review #4).
type PeerSinks = Arc<Mutex<HashMap<DeviceFingerprint, mpsc::Sender<WireItem>>>>;

/// Catch-up provider: produces the current local history as `WireItem`s already
/// re-keyed under the shared sync key, so a freshly-connected peer receives every
/// item that predates the link (fanout is otherwise fire-and-forget to whatever
/// sinks happen to be live at the moment an item is produced).
///
/// Built in `daemon.rs` from the DB + `SyncCrypto`; called once per established
/// connection (both the accept path and the connector path) right after the
/// peer sink is registered. LWW on the receiver makes the replay idempotent.
pub type CatchupProvider = Arc<dyn Fn() -> Vec<WireItem> + Send + Sync>;

/// Load peers persisted in `peers.json` into the live `PairedPeers` allowlist
/// (fix/p2p-c-review #2).
///
/// Each stored record carries the user-facing colon-hex `fingerprint`; it is
/// normalised to the canonical lowercase, colon-free hex the mTLS verifier
/// compares against ([`copypaste_p2p::cert::fingerprint_of`]). Returns the
/// number of peers loaded. Read/parse failures are logged and treated as an
/// empty store so a missing/corrupt file never blocks P2P startup.
pub fn load_persisted_peers_into(peers: &PairedPeers) -> usize {
    let path = crate::ipc::peers_file_path();
    let loaded = load_peers_from_path_into(&path, peers);
    if loaded > 0 {
        tracing::info!(loaded, path = %path.display(), "loaded persisted P2P peers into allowlist");
    }
    loaded
}

/// Path-taking core of [`load_persisted_peers_into`] (test seam).
fn load_peers_from_path_into(path: &std::path::Path, peers: &PairedPeers) -> usize {
    let stored = crate::peers::load_peers(path);
    let mut loaded = 0usize;
    for dev in &stored {
        if dev.fingerprint.is_empty() {
            continue;
        }
        let canonical = crate::ipc::canonical_fingerprint(&dev.fingerprint);
        let name = if dev.name.is_empty() {
            dev.fingerprint.clone()
        } else {
            dev.name.clone()
        };
        peers.add(canonical, name);
        loaded += 1;
    }
    loaded
}

/// Start the long-running P2P subsystem.
///
/// Binds a `TcpListener`, registers with mDNS-SD via
/// `copypaste_p2p::DiscoveryService`, and spawns three background tasks:
///
/// - **accept_loop** — accepts incoming mTLS connections from paired peers,
///   performs the TLS handshake, spawns a per-connection read/write task,
///   and forwards received frames to `incoming_tx`.
/// - **outbound_loop** — reads from `outbound_rx` (items from sync_orch to
///   push to peers) and fans them out to all connected peer sinks.
/// - **discovery_task** — keeps the mDNS-SD service alive for the lifetime
///   of the subsystem.
///
/// Returns a [`P2pHandle`] that keeps the subsystem alive.  Drop or send to
/// `shutdown_tx` to stop it.
///
/// # Errors
/// Returns an error if the TCP listener cannot be bound, or if the discovery
/// service fails to register / start.
#[allow(clippy::too_many_arguments)]
// CRITICAL-1: `cert` is threaded in so the transport presents the SAME cert
// whose fingerprint the IPC pairing handlers advertise. One extra argument is
// the minimal way to keep advertised and pinned fingerprints provably equal.
pub async fn start_p2p(
    config: P2pConfig,
    _db: Arc<Mutex<Database>>,
    device_id: uuid::Uuid,
    _db_key: zeroize::Zeroizing<[u8; 32]>,
    cert: copypaste_p2p::cert::SelfSignedCert,
    peers: PairedPeers,
    new_item_rx: broadcast::Receiver<ClipboardItem>,
    incoming_tx: mpsc::Sender<WireItem>,
    outbound_rx: mpsc::Receiver<WireItem>,
    catchup: CatchupProvider,
) -> anyhow::Result<P2pHandle> {
    let bind_addr = format!("0.0.0.0:{}", config.listen_port);
    let listener = TcpListener::bind(&bind_addr).await?;
    let actual_port = listener.local_addr()?.port();

    tracing::info!(
        port = actual_port,
        device_id = %device_id,
        device_name = %config.device_name,
        "P2P subsystem started"
    );

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // ── mTLS transport ────────────────────────────────────────────────────────
    // Use the cert generated once by the daemon (CRITICAL-1). Its fingerprint
    // is the stable device identity peers verify at handshake time — and the
    // SAME value the IPC pairing handlers advertise, because the daemon derived
    // their `cert_fingerprint` from this exact cert before calling us.
    //
    // fix/p2p-c-review #2: `peers` is the SAME live allowlist the IPC PAKE
    // handlers mutate (interior-mutable `PairedPeers`). We seed it from the
    // persisted `peers.json` so previously-paired peers are accepted on
    // startup, then hand a clone to the transport. Both observe later updates.
    let loaded = load_persisted_peers_into(&peers);
    tracing::info!(
        loaded_peers = loaded,
        active_peers = peers.active_count(),
        "P2P allowlist seeded from peers.json"
    );
    let transport = PeerTransport::from_cert(cert.cert_der, cert.key_der, peers.clone());
    tracing::info!(fingerprint = %transport.fingerprint(), "P2P mTLS transport identity");
    let transport = Arc::new(transport);

    // ── peer sinks map ────────────────────────────────────────────────────────
    // Shared across the accept loop (inserts new sinks) and the outbound loop
    // (reads and writes to each sink). Protected by an async Mutex so neither
    // task has to block the executor.
    let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));

    // ── discovery service ─────────────────────────────────────────────────────
    let discovery = Arc::new(DiscoveryService::new());
    let device_id_str = device_id.to_string();
    discovery
        .register(actual_port, &device_id_str, &config.device_name)
        .map_err(|e| anyhow::anyhow!("mDNS register failed: {e}"))?;

    let discovery_for_task = Arc::clone(&discovery);
    let device_name_for_task = config.device_name.clone();

    // ── accept loop ───────────────────────────────────────────────────────────
    {
        let transport = Arc::clone(&transport);
        let peer_sinks = Arc::clone(&peer_sinks);
        let incoming_tx = incoming_tx.clone();
        let catchup = Arc::clone(&catchup);
        tokio::spawn(async move {
            accept_loop(
                listener,
                shutdown_rx,
                transport,
                peer_sinks,
                incoming_tx,
                catchup,
            )
            .await;
        });
    }

    // ── outbound fanout loop ──────────────────────────────────────────────────
    {
        let peer_sinks = Arc::clone(&peer_sinks);
        tokio::spawn(async move {
            outbound_loop(new_item_rx, outbound_rx, peer_sinks).await;
        });
    }

    // ── peer connector loop (Phase 3) ─────────────────────────────────────────
    // Proactively DIALS paired peers that are not yet connected, so two paired
    // daemons establish a live mTLS link without waiting for the other side to
    // dial first. Reads each peer's persisted sync address from peers.json on
    // every tick (so a freshly-paired peer is picked up with no restart), skips
    // peers already in `peer_sinks`, never dials its own fingerprint, and
    // applies per-peer exponential backoff on failure.
    {
        let transport = Arc::clone(&transport);
        let peer_sinks = Arc::clone(&peer_sinks);
        let incoming_tx = incoming_tx.clone();
        let own_fp = transport.fingerprint().to_string();
        let catchup = Arc::clone(&catchup);
        tokio::spawn(async move {
            peer_connector_loop(transport, peer_sinks, incoming_tx, own_fp, catchup).await;
        });
    }

    // ── discovery task ────────────────────────────────────────────────────────
    tokio::spawn(async move {
        match discovery_for_task.start().await {
            Ok(handle) => {
                tracing::info!(
                    port = actual_port,
                    device_name = %device_name_for_task,
                    "mDNS-SD discovery service running"
                );
                let _ = handle.await;
            }
            Err(e) => {
                tracing::warn!("mDNS-SD discovery failed to start: {e}");
            }
        }
    });

    Ok(P2pHandle {
        actual_port,
        shutdown_tx,
    })
}

// ── private helpers ───────────────────────────────────────────────────────────

/// Accept incoming mTLS connections.
///
/// For each connection that completes the TLS handshake successfully, spawns a
/// per-connection task that:
/// - Reads `WireItem` frames from the peer and forwards them to `incoming_tx`.
/// - Drains a per-peer `mpsc::Receiver<WireItem>` and writes frames to the peer.
///
/// The per-peer sender is stored in `peer_sinks` (keyed by the peer's cert
/// fingerprint) so the outbound fanout loop can deliver outgoing items.
async fn accept_loop(
    listener: TcpListener,
    mut shutdown_rx: oneshot::Receiver<()>,
    transport: Arc<PeerTransport>,
    peer_sinks: PeerSinks,
    incoming_tx: mpsc::Sender<WireItem>,
    catchup: CatchupProvider,
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

                        // Per-peer write channel: the outbound loop sends items here;
                        // the write half of the per-connection task drains and serialises them.
                        let (peer_tx, peer_rx) = mpsc::channel::<WireItem>(64);

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

                        // Sync-on-connect catch-up: push the current local history
                        // ONCE into this peer's sink so items produced before the
                        // link came up are delivered. Items are already re-keyed
                        // under the shared sync key by the provider; LWW on the
                        // receiver makes the replay idempotent.
                        push_catchup(&catchup, &cleanup_tx).await;

                        let incoming_tx = incoming_tx.clone();
                        let peer_sinks = Arc::clone(&peer_sinks);
                        tokio::spawn(async move {
                            run_peer_connection(framed, peer_rx, incoming_tx).await;
                            // Clean up the sink when the connection drops — but only
                            // if it is still *this* connection's sink (a later
                            // reconnect may have replaced it under the same key).
                            let mut sinks = peer_sinks.lock().await;
                            if sinks
                                .get(&peer_key)
                                .is_some_and(|tx| tx.same_channel(&cleanup_tx))
                            {
                                sinks.remove(&peer_key);
                            }
                            drop(sinks);
                            tracing::debug!(%peer_addr, %peer_fp, "peer connection closed");
                        });
                    }
                    Err(e) => {
                        tracing::warn!("P2P accept/handshake error: {e}");
                    }
                }
            }
            _ = &mut shutdown_rx => {
                tracing::info!("P2P accept loop shutting down");
                break;
            }
        }
    }
}

/// How often the [`peer_connector_loop`] wakes to check for paired-but-not-
/// connected peers to dial.
const CONNECTOR_TICK: Duration = Duration::from_secs(3);

/// A dialable paired peer resolved from `peers.json`.
struct DialablePeer {
    /// Canonical (colon-free, lowercase) cert fingerprint — the mTLS pin.
    fingerprint: DeviceFingerprint,
    /// The peer's sync-listener socket address.
    addr: SocketAddr,
}

/// Read `peers.json` and return the paired peers that carry a parseable sync
/// `address` — the set the connector may dial. Peers with no address (legacy
/// records, or a peer that never advertised one) are skipped: the connector
/// has nothing to dial and relies on the peer dialing us instead.
fn dialable_peers_from_path(path: &std::path::Path) -> Vec<DialablePeer> {
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
/// * Per-peer exponential backoff ([`CONNECTOR_BACKOFF_STEPS`]) spaces out
///   retries to an offline peer instead of dialing every tick.
///
/// # Double-connect race (both sides dialing)
/// `peer_sinks` is keyed by cert fingerprint. If both daemons dial each other
/// at once, two connections may briefly exist; whichever sink is inserted last
/// wins the map slot and the superseded connection's per-connection task drops
/// its (now-unreferenced) sink and exits when the stream closes — no duplicate
/// fan-out. We additionally re-check `contains_key` immediately before dialing
/// to skip a peer the accept loop just connected.
async fn peer_connector_loop(
    transport: Arc<PeerTransport>,
    peer_sinks: PeerSinks,
    incoming_tx: mpsc::Sender<WireItem>,
    own_fp: DeviceFingerprint,
    catchup: CatchupProvider,
) {
    tracing::debug!(%own_fp, "P2P peer connector loop running");
    let peers_path = crate::ipc::peers_file_path();
    let mut dial_state: HashMap<DeviceFingerprint, DialBackoff> = HashMap::new();

    loop {
        tokio::time::sleep(CONNECTOR_TICK).await;

        let peers = dialable_peers_from_path(&peers_path);
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

            // Respect per-peer backoff.
            if let Some(state) = dial_state.get(&peer.fingerprint) {
                if !state.may_dial(now) {
                    continue;
                }
            }

            tracing::debug!(fingerprint = %peer.fingerprint, addr = %peer.addr, "connector dialing paired peer");
            match transport
                .connect_with_retry(peer.addr, &peer.fingerprint)
                .await
            {
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
                        let (peer_tx, peer_rx) = mpsc::channel::<WireItem>(64);
                        let cleanup_tx = peer_tx.clone();
                        sinks.insert(peer.fingerprint.clone(), peer_tx);
                        drop(sinks);

                        tracing::info!(fingerprint = %peer.fingerprint, addr = %peer.addr, "connector established outbound mTLS link");

                        // Sync-on-connect catch-up: replay local history once so
                        // items produced before this link came up reach the peer.
                        push_catchup(&catchup, &cleanup_tx).await;

                        let incoming_tx = incoming_tx.clone();
                        let peer_sinks = Arc::clone(&peer_sinks);
                        let peer_key = peer.fingerprint.clone();
                        tokio::spawn(async move {
                            run_peer_connection_client(stream, peer_rx, incoming_tx).await;
                            let mut sinks = peer_sinks.lock().await;
                            if sinks
                                .get(&peer_key)
                                .is_some_and(|tx| tx.same_channel(&cleanup_tx))
                            {
                                sinks.remove(&peer_key);
                            }
                            drop(sinks);
                            tracing::debug!(fingerprint = %peer_key, "connector outbound connection closed");
                        });
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
                }
            }
        }
    }
}

/// Manage one authenticated **inbound** (accept-side) peer connection.
async fn run_peer_connection(
    framed: copypaste_p2p::transport::PeerStream,
    peer_rx: mpsc::Receiver<WireItem>,
    incoming_tx: mpsc::Sender<WireItem>,
) {
    run_peer_connection_framed(framed, peer_rx, incoming_tx).await
}

/// Manage one authenticated **outbound** (connector-side) peer connection.
///
/// Identical duplex pump as [`run_peer_connection`] but for the client-side TLS
/// stream type returned by [`PeerTransport::connect_with_retry`].
async fn run_peer_connection_client(
    framed: copypaste_p2p::transport::PeerClientStream,
    peer_rx: mpsc::Receiver<WireItem>,
    incoming_tx: mpsc::Sender<WireItem>,
) {
    run_peer_connection_framed(framed, peer_rx, incoming_tx).await
}

/// Duplex pump shared by the accept-side and connector-side connection tasks.
///
/// Reads incoming frames and forwards them to `incoming_tx`; reads from
/// `peer_rx` and writes outgoing frames to the peer. Both directions run
/// concurrently via `tokio::select!`; the task exits when either side closes.
/// Generic over the framed stream so the server-side ([`PeerStream`]) and
/// client-side ([`PeerClientStream`]) TLS stream types share one implementation.
async fn run_peer_connection_framed<S>(
    mut framed: tokio_util::codec::Framed<S, tokio_util::codec::LengthDelimitedCodec>,
    mut peer_rx: mpsc::Receiver<WireItem>,
    incoming_tx: mpsc::Sender<WireItem>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        tokio::select! {
            // Inbound: peer sent a frame — deserialise and forward to sync_orch.
            frame_opt = framed.next() => {
                match frame_opt {
                    Some(Ok(frame)) => {
                        match serde_json::from_slice::<WireItem>(&frame) {
                            Ok(wire) => {
                                if incoming_tx.send(wire).await.is_err() {
                                    // incoming_tx closed means sync_orch shut down.
                                    tracing::debug!("incoming_tx closed, dropping peer connection");
                                    return;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to deserialise WireItem from peer: {e}");
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
            // Outbound: sync_orch wants to push an item to this peer.
            item_opt = peer_rx.recv() => {
                match item_opt {
                    Some(item) => {
                        match serde_json::to_vec(&item) {
                            Ok(payload) => {
                                if let Err(e) = framed.send(Bytes::from(payload)).await {
                                    tracing::warn!("failed to send WireItem to peer: {e}");
                                    return;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to serialise WireItem for peer: {e}");
                            }
                        }
                    }
                    None => {
                        // peer_rx channel closed — no more outbound items for this peer.
                        return;
                    }
                }
            }
        }
    }
}

/// Outbound fanout loop.
///
/// Receives `WireItem`s from the sync orchestrator via `outbound_rx` and
/// sends each one to every currently-connected peer. Also drains the
/// `new_item_rx` broadcast channel (previously handled by `subscriber_loop`)
/// so broadcast items are also fanned out.
///
/// Peer sinks whose channel is closed (peer disconnected) are removed from
/// `peer_sinks` on the next fanout pass.
async fn outbound_loop(
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut outbound_rx: mpsc::Receiver<WireItem>,
    peer_sinks: PeerSinks,
) {
    tracing::debug!("P2P outbound fanout loop running");

    let mut new_item_closed = false;
    let mut outbound_closed = false;

    loop {
        if new_item_closed && outbound_closed {
            tracing::info!("P2P outbound loop: both upstream channels closed, shutting down");
            break;
        }

        tokio::select! {
            // New clipboard item from the local monitor (broadcast channel).
            result = new_item_rx.recv(), if !new_item_closed => {
                match result {
                    Ok(_item) => {
                        // The clipboard item is stored in the DB; the sync orchestrator
                        // converts it to a WireItem and sends it via outbound_rx.
                        // We log only at debug to avoid double-counting.
                        tracing::debug!("P2P: new local clipboard item (sync_orch will forward)");
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("P2P outbound loop lagged by {n} items");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("P2P outbound loop: broadcast channel closed");
                        new_item_closed = true;
                    }
                }
            }
            // Outbound WireItem from sync_orch — fan out to all connected peers.
            item_opt = outbound_rx.recv(), if !outbound_closed => {
                match item_opt {
                    Some(item) => {
                        fanout_to_peers(&item, &peer_sinks).await;
                    }
                    None => {
                        tracing::info!("P2P outbound loop: outbound_rx channel closed");
                        outbound_closed = true;
                    }
                }
            }
        }
    }
}

/// Push the catch-up history into a single freshly-connected peer's sink.
///
/// Calls the [`CatchupProvider`] (which reads local history and re-keys it under
/// the shared sync key) and forwards each item to `sink`. Best-effort: a closed
/// sink (peer already gone) just stops the replay. Called exactly once per
/// established connection, before/at the start of the duplex pump.
async fn push_catchup(catchup: &CatchupProvider, sink: &mpsc::Sender<WireItem>) {
    let items = catchup();
    if items.is_empty() {
        return;
    }
    tracing::debug!(
        count = items.len(),
        "P2P sync-on-connect: replaying local history to peer"
    );
    for item in items {
        if sink.send(item).await.is_err() {
            tracing::debug!("P2P sync-on-connect: peer sink closed mid-replay");
            return;
        }
    }
}

/// Send `item` to every currently-connected peer sink.
///
/// Peers whose sender has been closed (disconnected) are removed from
/// `peer_sinks`.
///
/// M2: the `peer_sinks` lock is held only long enough to *snapshot* the
/// senders (each `mpsc::Sender` is cheap to clone) — never across the actual
/// send. The previous implementation held the lock across `tx.send().await`,
/// so a single slow/backpressured peer stalled all connection management
/// (accept/dial loops insert and remove sinks under the same lock). We now use
/// the non-blocking `try_send` on the dropped-guard snapshot: a `Closed`
/// channel means the peer is gone (pruned), while a transiently `Full` channel
/// (bounded at 64) just drops this best-effort fanout item for that peer — the
/// sync-on-connect catch-up replay reconciles it on the next reconnect, and we
/// must not evict a live peer merely for being momentarily behind.
async fn fanout_to_peers(item: &WireItem, peer_sinks: &PeerSinks) {
    // Snapshot (key, sender) pairs under the lock, then release it before sending.
    let snapshot: Vec<(DeviceFingerprint, mpsc::Sender<WireItem>)> = {
        let sinks = peer_sinks.lock().await;
        sinks
            .iter()
            .map(|(key, tx)| (key.clone(), tx.clone()))
            .collect()
    };

    let mut dead_keys: Vec<DeviceFingerprint> = Vec::new();
    for (key, tx) in snapshot {
        match tx.try_send(item.clone()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(
                    peer = %key,
                    "peer sink full — dropping fanout item (catch-up will reconcile)"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(peer = %key, "peer sink closed — will prune");
                dead_keys.push(key);
            }
        }
    }

    if !dead_keys.is_empty() {
        let mut sinks = peer_sinks.lock().await;
        for key in dead_keys {
            sinks.remove(&key);
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use copypaste_p2p::transport::{PairedPeers, PeerTransport};
    use copypaste_sync::protocol::WireItem;
    use tokio::net::TcpListener;

    // ── W2.2 integration tests ────────────────────────────────────────────────

    /// Build a minimal `WireItem` for use in tests.
    fn test_wire_item(id: &str) -> WireItem {
        WireItem {
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
        }
    }

    /// `accept_loop_forwards_wire_item_to_incoming_tx`:
    /// Spawn two in-process PeerTransports; client connects to server's accept
    /// loop; client sends a `WireItem`; verify it arrives on `incoming_tx`.
    #[tokio::test(flavor = "multi_thread")]
    async fn accept_loop_forwards_wire_item_to_incoming_tx() {
        let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("server").unwrap();
        let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("client").unwrap();

        let server_fp = server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        let server_peers = PairedPeers::new();
        server_peers.add(client_fp.clone(), "client");

        let client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server");

        let server_transport =
            PeerTransport::from_cert(server_cert.cert_der, server_cert.key_der, server_peers);
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (incoming_tx, mut incoming_rx) = mpsc::channel::<WireItem>(8);

        let item_sent = test_wire_item("item-1");
        let item_check = item_sent.clone();

        // Server: accept one connection, forward framed items to incoming_tx.
        let accept_fut = {
            let tx = incoming_tx.clone();
            async move {
                let (_peer_addr, _peer_fp, mut stream) =
                    server_transport.accept(&listener).await.unwrap();
                while let Some(Ok(frame)) = stream.next().await {
                    let wire: WireItem = serde_json::from_slice(&frame).unwrap();
                    tx.send(wire).await.unwrap();
                }
            }
        };

        // Client: connect and send one WireItem.
        let connect_fut = async move {
            let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
            let payload = serde_json::to_vec(&item_sent).unwrap();
            stream.send(Bytes::from(payload)).await.unwrap();
        };

        tokio::join!(accept_fut, connect_fut);

        let received = incoming_rx.recv().await.expect("must receive one item");
        assert_eq!(received.id, item_check.id);
        assert_eq!(received.content, item_check.content);
    }

    /// `subscriber_loop_fans_out_to_connected_peer`:
    /// Push a `WireItem` to `outbound_rx`; verify it appears on the connected
    /// peer's stream as a readable framed message.
    #[tokio::test(flavor = "multi_thread")]
    async fn subscriber_loop_fans_out_to_connected_peer() {
        let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("server2").unwrap();
        let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("client2").unwrap();

        let server_fp = server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        let server_peers = PairedPeers::new();
        server_peers.add(client_fp.clone(), "client2");

        let client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server2");

        let server_transport = Arc::new(PeerTransport::from_cert(
            server_cert.cert_der,
            server_cert.key_der,
            server_peers,
        ));
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let item_sent = test_wire_item("item-2");
        let item_check = item_sent.clone();

        // Channel that mimics outbound_rx: daemon code will read from this and
        // fan-out to connected peers.
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

        // Server: accept connection, then read from outbound_rx and write to peer.
        let server_fp_clone = server_fp.clone();
        let server_fut = async move {
            let (_peer_addr, _peer_fp, mut stream) =
                server_transport.accept(&listener).await.unwrap();
            // Simulate the outbound fanout: read one item and send to the connected peer.
            if let Some(item) = outbound_rx.recv().await {
                let payload = serde_json::to_vec(&item).unwrap();
                stream.send(Bytes::from(payload)).await.unwrap();
            }
            // Keep stream alive briefly so client can drain it.
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = server_fp_clone; // keep binding alive
        };

        // Client: connect and read one WireItem from the server.
        let client_fut = async move {
            let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
            // Wait for the server to push the item.
            if let Some(Ok(frame)) = stream.next().await {
                let wire: WireItem = serde_json::from_slice(&frame).unwrap();
                Some(wire)
            } else {
                None
            }
        };

        // Send item to outbound channel.
        outbound_tx.send(item_sent).await.unwrap();

        let ((), received_opt) = tokio::join!(server_fut, client_fut);
        let received = received_opt.expect("client must receive one item from server");
        assert_eq!(received.id, item_check.id);
    }

    /// `init` must build a `P2pState` end-to-end without panicking and without
    /// requiring any I/O beyond cert generation + mDNS registration (which
    /// does not bind sockets yet — `start()` does).
    #[test]
    fn p2p_state_initializes_without_panic() {
        let state = init(0, "test-device-id", "Test Device").expect("init must succeed");
        // own fingerprint should be populated (hex SHA-256 of cert DER).
        assert!(
            !state.transport.fingerprint().is_empty(),
            "transport must expose a non-empty fingerprint after init"
        );
    }

    /// Before any peer is discovered via mDNS, `list_peers` must return an
    /// empty slice — never panic, never block.
    #[test]
    fn list_peers_returns_empty_initially() {
        let state = init(0, "test-device-id", "Test Device").expect("init must succeed");
        let peers = list_peers(&state);
        assert!(
            peers.is_empty(),
            "fresh P2pState must have zero known peers"
        );
    }

    /// `pair_peer` is a placeholder until W2.4 — it must surface the explicit
    /// `NotImplemented` error rather than silently returning Ok.
    #[test]
    fn pair_peer_returns_not_implemented() {
        let state = init(0, "test-device-id", "Test Device").expect("init must succeed");
        let result = pair_peer(&state, "deadbeef", "Alice");
        assert!(matches!(result, Err(P2pError::NotImplemented)));
    }

    /// `unpair_peer` is also a placeholder until W2.4.
    #[test]
    fn unpair_peer_returns_not_implemented() {
        let state = init(0, "test-device-id", "Test Device").expect("init must succeed");
        let result = unpair_peer(&state, "deadbeef");
        assert!(matches!(result, Err(P2pError::NotImplemented)));
    }

    /// `get_own_fingerprint` must match `keychain::own_fingerprint` exactly —
    /// this protects against the surface drifting away from the single source
    /// of truth used by the rest of the daemon.
    #[test]
    fn get_own_fingerprint_matches_keychain() {
        let pk = [0u8; 32];
        assert_eq!(get_own_fingerprint(&pk), keychain::own_fingerprint(&pk));
    }

    /// fix/p2p-c-review #2 — a peer persisted in `peers.json` is loaded into the
    /// live `PairedPeers` allowlist at `start_p2p` time and accepted by
    /// `is_known` (normalised to the canonical lowercase, colon-free hex the
    /// mTLS verifier uses).
    #[test]
    fn persisted_peer_is_known_after_loading() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");

        // Two records in the colon-hex form the IPC pairing handlers write,
        // one with a display name and one (PAKE responder side) without.
        let fp_colon = std::iter::repeat_n("aa", 32).collect::<Vec<_>>().join(":");
        let fp_canonical = crate::ipc::canonical_fingerprint(&fp_colon);
        let json = format!(
            r#"[{{"fingerprint":"{fp_colon}","name":"Alice's Mac","added_at":1700000000}},
                {{"fingerprint":"bb:bb","added_at":1700000001}}]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let peers = PairedPeers::new();
        assert!(
            !peers.is_known(&fp_canonical),
            "precondition: empty allowlist"
        );

        let loaded = load_peers_from_path_into(&path, &peers);
        assert_eq!(loaded, 2, "both persisted peers loaded");

        assert!(
            peers.is_known(&fp_canonical),
            "persisted peer must be accepted by is_known after loading"
        );
        // The lean (name-less) record is also honoured, normalised.
        assert!(peers.is_known("bbbb"), "name-less peer also loaded");
    }

    /// Phase 3: the connector resolves only paired peers that carry a parseable
    /// sync `address` from `peers.json`; records with no address (or a malformed
    /// one) are skipped, and the fingerprint is normalised to canonical hex.
    #[test]
    fn dialable_peers_resolves_address_records_only() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_colon = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let fp_canonical = crate::ipc::canonical_fingerprint(&fp_colon);
        let json = format!(
            r#"[
                {{"fingerprint":"{fp_colon}","name":"A","added_at":1,"address":"127.0.0.1:4242"}},
                {{"fingerprint":"cd:cd","added_at":2}},
                {{"fingerprint":"ef:ef","added_at":3,"address":"not-an-addr"}}
            ]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(
            dialable.len(),
            1,
            "only the record with a valid address is dialable"
        );
        assert_eq!(dialable[0].fingerprint, fp_canonical);
        assert_eq!(dialable[0].addr, "127.0.0.1:4242".parse().unwrap());
    }

    /// A peer persisted with a real LAN sync address is considered dialable by
    /// the connector (the Android→macOS background-sync direction depends on the
    /// macOS daemon advertising — and the peer persisting — a routable LAN
    /// address, not loopback). The resolved `addr` round-trips exactly.
    #[test]
    fn peer_with_lan_address_is_dialable() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_colon = std::iter::repeat_n("a1", 32).collect::<Vec<_>>().join(":");
        let fp_canonical = crate::ipc::canonical_fingerprint(&fp_colon);
        let json = format!(
            r#"[{{"fingerprint":"{fp_colon}","name":"Mac","added_at":1,"address":"192.168.1.50:43117"}}]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(dialable.len(), 1, "LAN-addressed peer must be dialable");
        assert_eq!(dialable[0].fingerprint, fp_canonical);
        assert_eq!(
            dialable[0].addr,
            "192.168.1.50:43117".parse::<SocketAddr>().unwrap()
        );
        assert!(
            !dialable[0].addr.ip().is_loopback(),
            "a real LAN peer address must not be loopback"
        );
    }

    /// Connector dial policy for the two non-LAN cases:
    /// * an EMPTY address record is skipped entirely (nothing to dial — the
    ///   connector relies on the peer dialing us instead);
    /// * a LOOPBACK address still parses and is therefore dialable, which keeps
    ///   single-host / loopback tests working (it simply fails and backs off on
    ///   a real cross-host LAN, which is harmless).
    #[test]
    fn dial_policy_skips_empty_addr_but_keeps_loopback() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_empty = std::iter::repeat_n("b2", 32).collect::<Vec<_>>().join(":");
        let fp_loop = std::iter::repeat_n("c3", 32).collect::<Vec<_>>().join(":");
        let fp_loop_canonical = crate::ipc::canonical_fingerprint(&fp_loop);
        // One record with no `address` key at all, one with a loopback address.
        let json = format!(
            r#"[
                {{"fingerprint":"{fp_empty}","name":"NoAddr","added_at":1}},
                {{"fingerprint":"{fp_loop}","name":"Loop","added_at":2,"address":"127.0.0.1:7000"}}
            ]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(
            dialable.len(),
            1,
            "only the loopback record is dialable; the address-less record is skipped"
        );
        assert_eq!(dialable[0].fingerprint, fp_loop_canonical);
        assert!(dialable[0].addr.ip().is_loopback());
    }

    /// A missing `peers.json` loads zero peers and never errors.
    #[test]
    fn missing_peers_file_loads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let peers = PairedPeers::new();
        assert_eq!(load_peers_from_path_into(&path, &peers), 0);
        assert_eq!(peers.active_count(), 0);
    }
}
