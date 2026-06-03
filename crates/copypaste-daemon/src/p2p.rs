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
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

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
    /// Cancel this token to request a graceful shutdown of ALL P2P tasks.
    ///
    /// BUG F1: previously a single `oneshot::Sender<()>` whose receiver reached
    /// only `accept_loop`, leaking the responder/outbound/connector/discovery
    /// tasks on an in-process P2P restart. A [`CancellationToken`] is cloned into
    /// every long-running task instead, so one `cancel()` stops them all.
    pub shutdown_token: CancellationToken,
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
    // Shared `DiscoveryService` constructed once by the caller (daemon.rs) and
    // also handed to the IPC server via `with_discovery`.  Injecting it here
    // (instead of constructing a second instance) ensures discovered peers
    // surface through the `list_discovered` IPC handler.
    discovery: Arc<DiscoveryService>,
    // Shared discovery-pairing coordinator (LAN/SAS Phase 2). The standing
    // responder task routes its SAS confirmation through this SAME coordinator
    // the IPC `pair_get_sas` / `pair_confirm_sas` handlers observe, so the
    // responder user confirms exactly like the initiator. The caller (daemon.rs)
    // obtains it from the IPC server via `pairing_coordinator()`.
    pairing: Arc<crate::pairing_sm::PairingCoordinator>,
    // This daemon's own P2P sync-listener address (`host:port`) shared slot,
    // sent in-band over the bootstrap channel by the standing responder so the
    // initiator can persist where to dial us for sync. Same Arc the IPC server
    // populates via `set_p2p_sync_addr`.
    own_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    // B1: the SAME public-IP cache the IPC server reads and the STUN refresh task
    // writes (daemon.rs constructs it once). The standing LAN/SAS responder reads
    // it so it advertises our own global IP in-band, exactly like the IPC paths.
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
) -> anyhow::Result<P2pHandle> {
    let bind_addr = format!("0.0.0.0:{}", config.listen_port);
    let listener = TcpListener::bind(&bind_addr).await?;
    let actual_port = listener.local_addr()?.port();

    // LAN/SAS Phase 2: clone the cert DER+key BEFORE the transport consumes
    // `cert`, so the standing discovery-pairing responder can TLS-wrap its
    // bootstrap listener with the SAME identity the mTLS transport presents
    // (and whose fingerprint pairing advertises).
    let bootstrap_cert_der = cert.cert_der.clone();
    let bootstrap_key_der = cert.key_der.clone();

    tracing::info!(
        port = actual_port,
        device_id = %device_id,
        device_name = %config.device_name,
        "P2P subsystem started"
    );

    // BUG F1: one CancellationToken governs ALL long-running P2P tasks (accept,
    // standing responder, outbound, connector, discovery). Each task gets a
    // clone and a `cancelled()` arm; `daemon.rs` calls `cancel()` on shutdown.
    let shutdown_token = CancellationToken::new();

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

    // ── standing discovery-pairing bootstrap listener (LAN/SAS Phase 2) ────────
    // Bind ONE bootstrap listener up front so we learn the OS-assigned port and
    // can advertise it in the mDNS `bport` TXT key. The standing responder loop
    // (spawned below) re-binds this SAME port for each inbound pairing. A
    // listening socket is dropped (not connected) between iterations, so it
    // never enters TIME_WAIT and the immediate re-bind succeeds. Best-effort:
    // if the bind fails we advertise v1 (no bport) and discovery pairing is
    // simply unavailable on this instance — QR pairing is unaffected.
    let bootstrap_port: Option<u16> = match copypaste_p2p::bootstrap::BootstrapResponder::bind_on(
        0,
        bootstrap_cert_der.clone(),
        bootstrap_key_der.clone(),
    )
    .await
    {
        Ok(probe) => match probe.local_addr() {
            Ok(addr) => {
                // Drop the probe listener so the responder loop can re-bind
                // the same port for its first accept.
                let p = addr.port();
                drop(probe);
                Some(p)
            }
            Err(e) => {
                tracing::warn!("LAN/SAS: bootstrap listener local_addr failed: {e}");
                None
            }
        },
        Err(e) => {
            tracing::warn!("LAN/SAS: failed to bind bootstrap listener: {e}");
            None
        }
    };

    // ── discovery service ─────────────────────────────────────────────────────
    // Use the injected instance (shared with the IPC server) so that discovered
    // peers surface through the `list_discovered` handler.  The caller
    // (daemon.rs) constructs the Arc once and passes clones to both start_p2p
    // and `IpcServer::with_discovery`.
    let device_id_str = device_id.to_string();
    // Advertise the bootstrap port in `bport` when available (v2); else v1.
    let register_result = match bootstrap_port {
        Some(bport) => {
            discovery.register_with_bport(actual_port, &device_id_str, &config.device_name, bport)
        }
        None => discovery.register(actual_port, &device_id_str, &config.device_name),
    };
    register_result.map_err(|e| anyhow::anyhow!("mDNS register failed: {e}"))?;

    let discovery_for_task = Arc::clone(&discovery);
    let device_name_for_task = config.device_name.clone();

    // ── standing discovery-pairing responder loop (LAN/SAS Phase 2) ────────────
    // Accepts inbound SAS-pairing connections on the advertised `bport` and runs
    // `run_with_confirm`, routing the SAS through the SHARED pairing coordinator
    // so the LOCAL user confirms via `pair_get_sas` / `pair_confirm_sas` exactly
    // like the initiator. Authentication is the human SAS comparison: the
    // initiator sends an EPHEMERAL random password in-clear inside the bootstrap
    // TLS, and the SAS (derived from the post-PAKE bound_key) is the real
    // authenticator. On reject/mismatch/timeout the session key drops/zeroizes
    // and NOTHING is persisted (no rotate_peer).
    if let Some(bport) = bootstrap_port {
        let peers_for_responder = peers.clone();
        let pairing_for_responder = Arc::clone(&pairing);
        let own_sync_addr_for_responder = Arc::clone(&own_sync_addr);
        let public_ip_cache_for_responder = Arc::clone(&public_ip_cache);
        let cert_der = bootstrap_cert_der;
        let key_der = bootstrap_key_der;
        let responder_shutdown = shutdown_token.clone();
        tokio::spawn(async move {
            standing_pairing_responder_loop(
                bport,
                cert_der,
                key_der,
                peers_for_responder,
                pairing_for_responder,
                own_sync_addr_for_responder,
                public_ip_cache_for_responder,
                responder_shutdown,
            )
            .await;
        });
    }

    // ── accept loop ───────────────────────────────────────────────────────────
    {
        let transport = Arc::clone(&transport);
        let peer_sinks = Arc::clone(&peer_sinks);
        let incoming_tx = incoming_tx.clone();
        let catchup = Arc::clone(&catchup);
        let accept_shutdown = shutdown_token.clone();
        tokio::spawn(async move {
            accept_loop(
                listener,
                accept_shutdown,
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
        let outbound_shutdown = shutdown_token.clone();
        tokio::spawn(async move {
            outbound_loop(new_item_rx, outbound_rx, peer_sinks, outbound_shutdown).await;
        });
    }

    // ── peer connector loop (Phase 3) ─────────────────────────────────────────
    // Proactively DIALS paired peers that are not yet connected, so two paired
    // daemons establish a live mTLS link without waiting for the other side to
    // dial first. Reads each peer's persisted sync address from peers.json on
    // every tick (so a freshly-paired peer is picked up with no restart), skips
    // peers already in `peer_sinks`, never dials its own fingerprint, and
    // applies per-peer exponential backoff on failure.
    //
    // The injected `discovery` clone enables mDNS address refresh on dial
    // failure (P2P audit P2 #3): when a persisted address is stale (DHCP renew
    // / network switch) the connector consults the live discovery snapshot and
    // persists a fresher address before the next backoff tick.
    {
        let transport = Arc::clone(&transport);
        let peer_sinks = Arc::clone(&peer_sinks);
        let incoming_tx = incoming_tx.clone();
        let own_fp = transport.fingerprint().to_string();
        let catchup = Arc::clone(&catchup);
        // Clone the shared discovery Arc — the connector, the accept loop, and
        // the IPC handlers all observe the same `known_peers` map through it.
        let discovery_for_connector = Arc::clone(&discovery);
        let connector_shutdown = shutdown_token.clone();
        tokio::spawn(async move {
            peer_connector_loop(
                transport,
                peer_sinks,
                incoming_tx,
                own_fp,
                catchup,
                discovery_for_connector,
                connector_shutdown,
            )
            .await;
        });
    }

    // ── discovery task ────────────────────────────────────────────────────────
    let discovery_shutdown = shutdown_token.clone();
    tokio::spawn(async move {
        match discovery_for_task.start().await {
            Ok(handle) => {
                tracing::info!(
                    port = actual_port,
                    device_name = %device_name_for_task,
                    "mDNS-SD discovery service running"
                );
                // BUG F1: race the mDNS handle against cancellation so the task
                // exits promptly on shutdown instead of awaiting `handle` forever.
                tokio::select! {
                    _ = handle => {}
                    _ = discovery_shutdown.cancelled() => {
                        tracing::info!("mDNS-SD discovery task shutting down");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("mDNS-SD discovery failed to start: {e}");
            }
        }
    });

    Ok(P2pHandle {
        actual_port,
        shutdown_token,
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
    shutdown: CancellationToken,
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

                        // Stamp first/last sync times for this peer (once per
                        // established connection — see `stamp_peer_sync`).
                        stamp_peer_sync(&crate::ipc::peers_file_path(), &peer_fp);

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
            _ = shutdown.cancelled() => {
                tracing::info!("P2P accept loop shutting down");
                break;
            }
        }
    }
}

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
fn resolve_addr_from_discovery(
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
    // Injected mDNS discovery service — consulted as a fallback when a
    // persisted dial address fails (peer DHCP renew / network switch).
    discovery: Arc<DiscoveryService>,
    shutdown: CancellationToken,
) {
    tracing::debug!(%own_fp, "P2P peer connector loop running");
    let peers_path = crate::ipc::peers_file_path();
    let mut dial_state: HashMap<DeviceFingerprint, DialBackoff> = HashMap::new();

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

                        // Stamp first/last sync times for this peer (once per
                        // established connection — see `stamp_peer_sync`).
                        stamp_peer_sync(&peers_path, &peer.fingerprint);

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

                    // mDNS address refresh (P2P audit P2 #3): on dial failure,
                    // consult the live discovery snapshot to see if the peer
                    // has a fresh LAN address (DHCP renew / network switch).
                    // Only act when discovery returns an address that DIFFERS
                    // from the one that just failed — avoids a spurious write
                    // that would be a no-op at best.  The existing per-peer
                    // backoff already rate-limits how often we reach this
                    // branch, so no additional throttle is needed here.
                    if let Some(fresh_addr) =
                        resolve_addr_from_discovery(&discovery, &peer.fingerprint)
                    {
                        if fresh_addr != peer.addr {
                            tracing::info!(
                                fingerprint = %peer.fingerprint,
                                stale_addr = %peer.addr,
                                fresh_addr = %fresh_addr,
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
    shutdown: CancellationToken,
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
            // BUG F1: graceful shutdown — break out even while channels are open.
            _ = shutdown.cancelled() => {
                tracing::info!("P2P outbound loop shutting down");
                break;
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
/// Stamp first/last sync timestamps for a freshly-established peer connection.
///
/// Called ONCE per established connection (both the accept and connector paths),
/// right after the sync-on-connect catch-up. This per-connection cadence is the
/// throttle: `peers.json` is rewritten when a link comes up, never per synced
/// item, so there is no write amplification under a busy stream.
///
/// `peer_fp` is the verified mTLS certificate fingerprint (colon-free hex);
/// [`crate::peers::touch_sync_times`] canonicalises it against the colon-hex
/// form stored in `peers.json`. A missing peer record or a write failure only
/// logs at `debug` — sync-time stamping is best-effort and must never disrupt
/// the live connection.
fn stamp_peer_sync(peers_path: &std::path::Path, peer_fp: &DeviceFingerprint) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if let Err(e) = crate::peers::touch_sync_times(peers_path, peer_fp, now) {
        tracing::debug!(%peer_fp, "failed to stamp peer sync times: {e}");
    }
}

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

/// Standing discovery-pairing responder loop (LAN/SAS Phase 2).
///
/// Re-binds the bootstrap listener on the advertised `bport` and accepts ONE
/// inbound SAS-pairing connection per iteration. Each accepted connection runs
/// [`run_with_confirm`](copypaste_p2p::bootstrap::BootstrapResponder::run_with_confirm),
/// routing the derived SAS through the SHARED [`PairingCoordinator`](crate::pairing_sm::PairingCoordinator)
/// so the LOCAL user confirms via the IPC `pair_get_sas` / `pair_confirm_sas`
/// surface exactly like the initiator.
///
/// ## Security
/// The initiator transmits an EPHEMERAL random password in-clear inside the
/// (unauthenticated) bootstrap TLS channel; that password is NOT a secret. The
/// human SAS comparison — derived from the post-PAKE, post-channel-binding
/// `bound_key` — is the SOLE authenticator. Both sides exchange frame-10a
/// ACCEPT/REJECT inside `run_with_confirm`; on reject/mismatch/timeout the
/// session key drops/zeroizes and NOTHING is persisted (no `rotate_peer`). Only
/// on a both-accept success do we `rotate_peer` + `persist_paired_peer`,
/// identical to the QR path, so steady-state remains mutual fingerprint-pinned
/// mTLS.
///
/// ## Single active pairing
/// We only begin (`try_begin`) when the coordinator is `Idle`; a connection that
/// arrives while another pairing (inbound or the IPC-initiated outbound) is in
/// flight is dropped immediately so there is never more than one pending SAS.
async fn standing_pairing_responder_loop(
    bport: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    peers: PairedPeers,
    pairing: Arc<crate::pairing_sm::PairingCoordinator>,
    own_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    // B1: shared public-IP cache (the daemon's single STUN source). Read each
    // iteration so our own current global IP is advertised in-band to the peer.
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
    shutdown: CancellationToken,
) {
    tracing::info!(bport, "LAN/SAS standing pairing responder running");
    loop {
        // BUG F1: exit promptly if shutdown was requested between iterations.
        if shutdown.is_cancelled() {
            tracing::info!("LAN/SAS standing pairing responder shutting down");
            break;
        }
        // Re-bind the fixed bootstrap port for the next inbound pairing. A
        // listening socket is dropped (not connected) between iterations, so it
        // never enters TIME_WAIT and the re-bind succeeds immediately.
        let responder = match copypaste_p2p::bootstrap::BootstrapResponder::bind_on(
            bport,
            cert_der.clone(),
            key_der.clone(),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(bport, "LAN/SAS: re-bind bootstrap listener failed: {e}");
                // Brief backoff to avoid a hot loop if the port is wedged; race it
                // against cancellation so shutdown is not delayed by the sleep.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    _ = shutdown.cancelled() => break,
                }
                continue;
            }
        };

        // Resolve our own sync address + metadata for the in-band exchange.
        let own_addr = own_sync_addr
            .lock()
            .map(|s| s.clone().unwrap_or_default())
            .unwrap_or_else(|p| p.into_inner().clone().unwrap_or_default());
        let own_public_ip = public_ip_cache.read().await.clone();
        let own_meta = tokio::task::spawn_blocking(move || {
            crate::ipc::IpcServer::collect_own_peer_meta(own_public_ip)
        })
        .await
        .unwrap_or_default();

        // EPHEMERAL random password. The initiator sends its own random password
        // in-band; the responder side of `run_with_confirm` reconstructs the
        // PAKE from the initiator's transmitted password, so we register a
        // matching throwaway here. SAS authenticates, not this value.
        let password = copypaste_core::PairingToken::generate().to_pake_password();

        let coordinator = Arc::clone(&pairing);
        // Claim the single-active-pairing slot LAZILY inside the confirm
        // callback is too late (the handshake already ran); instead we gate at
        // the SAS step: the confirm callback only runs after frame 9, and we
        // refuse to surface a SAS if a pairing is already active.
        let confirm = move |sas: &str| {
            let coordinator = Arc::clone(&coordinator);
            let sas = sas.to_string();
            async move {
                // Single active pairing: if the coordinator is busy, reject.
                if !coordinator.try_begin(crate::pairing_sm::PairingRole::Responder) {
                    tracing::warn!("LAN/SAS: inbound pairing rejected — another pairing active");
                    return false;
                }
                let rx =
                    coordinator.enter_awaiting_sas(sas, crate::pairing_sm::PairingRole::Responder);
                match tokio::time::timeout(crate::pairing_sm::SAS_CONFIRM_TIMEOUT, rx).await {
                    Ok(Ok(accept)) => accept,
                    // Sender dropped (pair_abort) or timed out → reject.
                    _ => false,
                }
            }
        };

        // BUG F1: race the (potentially long, up to SAS_CONFIRM_TIMEOUT) inbound
        // handshake against cancellation. On shutdown we drop the responder
        // future — cancelling the in-flight handshake (the confirm await resolves
        // to a rejection, keys drop/zeroize) — and exit the loop.
        let run_result = tokio::select! {
            r = responder.run_with_confirm(&password, &own_addr, &own_meta, confirm) => r,
            _ = shutdown.cancelled() => {
                tracing::info!("LAN/SAS standing pairing responder shutting down (mid-accept)");
                if pairing.snapshot().is_active() {
                    pairing.finish(crate::pairing_sm::PairingState::Aborted);
                }
                if pairing.snapshot().is_terminal() {
                    pairing.reset();
                }
                break;
            }
        };
        match run_result {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    "LAN/SAS inbound pairing completed (both sides accepted)"
                );
                peers.rotate_peer(
                    &outcome.peer_fingerprint,
                    outcome.peer_fingerprint.clone(),
                    String::new(),
                );
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                };
                crate::ipc::IpcServer::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                );
                pairing.finish(crate::pairing_sm::PairingState::Confirmed);
            }
            Err(e) => {
                // Reject / mismatch / timeout / no inbound connection within the
                // accept window. NO persist, NO rotate_peer — the session key
                // already dropped/zeroized inside `run_with_confirm`. Only move
                // to a terminal state if we had actually begun a pairing (a bare
                // accept-timeout never claimed the coordinator).
                let snap = pairing.snapshot();
                if snap.is_active() {
                    pairing.finish(crate::pairing_sm::PairingState::Rejected);
                }
                tracing::debug!("LAN/SAS inbound pairing ended without success: {e}");
            }
        }

        // Reset to Idle so the next inbound (or IPC-initiated) pairing may begin.
        // The UI has a brief window to observe the terminal state via
        // `pair_get_sas` before this reset; v0.6 keeps it simple.
        if pairing.snapshot().is_terminal() {
            pairing.reset();
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
            file_name: None,
            mime: None,
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

    /// BUG F1: cancelling the shared `CancellationToken` must stop the
    /// long-running loops. Drives `accept_loop` and `outbound_loop` (both blocked
    /// on their idle awaits with no traffic) and asserts each task exits promptly
    /// once the token is cancelled — before the fix only the accept loop had a
    /// shutdown path and the outbound loop ran forever.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_accept_and_outbound_loops() {
        let token = CancellationToken::new();

        // accept_loop: bound listener, nothing dialing in → blocked on accept().
        let accept_handle = {
            let cert = copypaste_p2p::cert::SelfSignedCert::generate("f1-accept").unwrap();
            let transport = Arc::new(PeerTransport::from_cert(
                cert.cert_der,
                cert.key_der,
                PairedPeers::new(),
            ));
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
            let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);
            let catchup: CatchupProvider = Arc::new(Vec::new);
            let token = token.clone();
            tokio::spawn(async move {
                accept_loop(listener, token, transport, peer_sinks, incoming_tx, catchup).await;
            })
        };

        // outbound_loop: both channels open but idle → blocked in its select!.
        let outbound_handle = {
            let (_new_item_tx, new_item_rx) = broadcast::channel::<ClipboardItem>(8);
            let (_outbound_tx, outbound_rx) = mpsc::channel::<WireItem>(8);
            let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
            let token = token.clone();
            tokio::spawn(async move {
                outbound_loop(new_item_rx, outbound_rx, peer_sinks, token).await;
            })
        };

        // Both tasks are parked on their idle awaits; cancel and require both to
        // finish well within a generous bound (no hang = cancellation works).
        token.cancel();
        let joined = tokio::time::timeout(Duration::from_secs(5), async {
            accept_handle.await.unwrap();
            outbound_handle.await.unwrap();
        })
        .await;
        assert!(
            joined.is_ok(),
            "BUG F1: both P2P loops must exit promptly on token cancel"
        );
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

    // ── mDNS address refresh (P2P audit P2 #3) ───────────────────────────────

    /// `resolve_addr_from_discovery` returns `None` when the discovery service
    /// has no matching peer (empty).
    #[test]
    fn resolve_addr_from_discovery_returns_none_when_empty() {
        let discovery = DiscoveryService::new();
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(
            result.is_none(),
            "empty discovery must yield None for any fingerprint"
        );
    }

    /// `resolve_addr_from_discovery` returns `None` when no discovered peer
    /// has a matching `device_id` (fingerprint).
    #[test]
    fn resolve_addr_from_discovery_returns_none_for_unknown_peer() {
        let discovery = DiscoveryService::new();
        // Manually inject a peer with a different fingerprint via on_peer_found
        // callback simulation: insert directly into known_peers (test-internal).
        discovery.inject_peer_for_test(
            "bob.local.",
            PeerInfo {
                device_id: "1122334455".to_string(),
                device_name: "Bob".to_string(),
                ip_addrs: vec!["192.168.1.10".parse().unwrap()],
                port: 51000,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(result.is_none(), "non-matching peer must yield None");
    }

    /// `resolve_addr_from_discovery` returns a valid `SocketAddr` when a
    /// discovered peer's `device_id` matches the queried fingerprint and it has
    /// at least one routable IP address.
    #[test]
    fn resolve_addr_from_discovery_returns_addr_for_matching_peer() {
        let discovery = DiscoveryService::new();
        discovery.inject_peer_for_test(
            "alice.local.",
            PeerInfo {
                device_id: "aabbccdd".to_string(),
                device_name: "Alice".to_string(),
                ip_addrs: vec!["192.168.1.99".parse().unwrap()],
                port: 51515,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(result.is_some(), "matching peer must yield Some addr");
        let addr = result.unwrap();
        assert_eq!(addr.port(), 51515);
        assert_eq!(addr.ip().to_string(), "192.168.1.99");
    }

    /// `resolve_addr_from_discovery` prefers IPv4 over IPv6 when both are
    /// present (IPv4 is listed first after the sort in `peer_from_resolved`).
    #[test]
    fn resolve_addr_from_discovery_prefers_ipv4() {
        let discovery = DiscoveryService::new();
        discovery.inject_peer_for_test(
            "carol.local.",
            PeerInfo {
                device_id: "ccddee".to_string(),
                device_name: "Carol".to_string(),
                ip_addrs: vec!["192.168.2.5".parse().unwrap(), "::1".parse().unwrap()],
                port: 9000,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "ccddee");
        assert!(result.is_some());
        let addr = result.unwrap();
        assert!(!addr.ip().is_ipv6(), "must prefer IPv4 when available");
    }

    /// `update_peer_address` updates the `address` field of a matching peer and
    /// leaves all other fields (fingerprint, name, added_at, etc.) intact.
    #[test]
    fn update_peer_address_updates_matching_peer_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");

        crate::peers::save_peers(
            &path,
            &[
                crate::peers::PairedDevice {
                    fingerprint: "aabb".to_string(),
                    name: "Alice".to_string(),
                    added_at: 1_000,
                    address: Some("10.0.0.1:1000".to_string()),
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: Some(500),
                    last_sync_at: Some(999),
                },
                crate::peers::PairedDevice {
                    fingerprint: "ccdd".to_string(),
                    name: "Bob".to_string(),
                    added_at: 2_000,
                    address: Some("10.0.0.2:2000".to_string()),
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: None,
                    last_sync_at: None,
                },
            ],
        )
        .unwrap();

        let new_addr: SocketAddr = "192.168.9.9:7777".parse().unwrap();
        crate::peers::update_peer_address(&path, "aabb", new_addr).unwrap();

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(loaded.len(), 2);

        let alice = loaded.iter().find(|p| p.fingerprint == "aabb").unwrap();
        assert_eq!(
            alice.address.as_deref(),
            Some("192.168.9.9:7777"),
            "Alice's address must be updated"
        );
        // Other fields must be preserved.
        assert_eq!(alice.name, "Alice");
        assert_eq!(alice.added_at, 1_000);
        assert_eq!(alice.first_sync_at, Some(500), "first_sync_at must be kept");
        assert_eq!(alice.last_sync_at, Some(999), "last_sync_at must be kept");

        // Bob must be untouched.
        let bob = loaded.iter().find(|p| p.fingerprint == "ccdd").unwrap();
        assert_eq!(
            bob.address.as_deref(),
            Some("10.0.0.2:2000"),
            "Bob's address must be unchanged"
        );
    }

    /// `update_peer_address` is a no-op (and not an error) when no matching peer
    /// record exists.
    #[test]
    fn update_peer_address_no_match_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[crate::peers::PairedDevice {
                fingerprint: "aabb".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: Some("10.0.0.1:1000".to_string()),
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
            }],
        )
        .unwrap();

        let new_addr: SocketAddr = "192.168.9.9:7777".parse().unwrap();
        // "deadbeef" does not match "aabb".
        crate::peers::update_peer_address(&path, "deadbeef", new_addr).unwrap();

        let loaded = crate::peers::load_peers(&path);
        // Alice's address must be untouched.
        assert_eq!(
            loaded[0].address.as_deref(),
            Some("10.0.0.1:1000"),
            "unmatched update must not modify any record"
        );
    }
}
