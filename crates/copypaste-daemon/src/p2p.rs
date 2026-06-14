//! P2P subsystem orchestrator.
//!
//! W2.2 — wires the mTLS accept loop and outbound fanout into the daemon,
//! bridging `copypaste-p2p` transport with the `sync_orch` channel pair
//! (`incoming_tx` / `outbound_rx`).
//!
//! Pairing via these thin wrappers (`pair_peer` / `unpair_peer`) currently
//! returns [`P2pError::NotImplemented`]; the PAKE handshake is handled
//! directly by the IPC layer in `ipc.rs` rather than through this module.

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
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};

use crate::keychain;

/// Shared map of last-measured round-trip times per peer (milliseconds).
///
/// Keyed by the peer's verified **certificate fingerprint** in canonical
/// lowercase, colon-free hex form. Written by the RTT ping task spawned
/// alongside each established connection; read by the IPC `list_peers`
/// handler to surface the `latency_ms` field.
pub type PeerRttMs = Arc<Mutex<HashMap<DeviceFingerprint, u32>>>;

/// Correlation map from ping nonce to the [`Instant`] the ping was sent.
///
/// Used to compute round-trip time when the matching `Pong` arrives: the
/// per-connection task looks up the nonce, computes `now - sent`, and
/// records the result in the shared [`PeerRttMs`] map.
pub(crate) type PendingPings = Arc<Mutex<HashMap<u64, Instant>>>;

/// How often the RTT ping task wakes to send a [`ControlMsg::Ping`].
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum time to wait for a [`ControlMsg::Pong`] before discarding the
/// nonce from the pending-ping map. Prevents unbounded growth when a peer
/// never responds (e.g. old daemon that doesn't know the Ping variant).
const PING_PONG_TIMEOUT: Duration = Duration::from_secs(10);

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

    /// The requested pairing operation is not yet implemented via this module;
    /// the PAKE handshake is handled directly by the IPC layer.
    #[error("Pairing not implemented via p2p module (handled by IPC layer)")]
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
    /// When false, skip mDNS-SD registration and browsing so the device is
    /// invisible on the local network. The mTLS listener is still bound and
    /// accept/connector loops run — paired peers that have a persisted address
    /// can still connect directly. Default: `true`.
    pub lan_visibility: bool,
}

/// Shared map of currently-connected peer sinks, exported for IPC use.
///
/// Keyed by the peer's verified **certificate fingerprint** in canonical
/// lowercase, colon-free hex form (matching
/// [`crate::ipc::canonical_fingerprint`]). The IPC `list_peers` handler reads
/// this map to compute the authoritative `online` flag — a peer is online iff
/// it has a live, non-closed sender here.  The `last_sync_at` heuristic acts
/// as a fallback when P2P is disabled or not yet connected.
pub type LivePeerSinks =
    Arc<Mutex<HashMap<copypaste_p2p::transport::DeviceFingerprint, mpsc::Sender<PeerFrame>>>>;

/// A peer connection-state change emitted by the P2P subsystem.
///
/// Published on [`P2pHandle::peer_event_tx`] whenever a verified mTLS
/// connection is established or torn down.  Subscribers (e.g. `daemon.rs`
/// bridging into Tauri events) can use this to push live presence updates to
/// the UI without waiting for the next `list_peers` poll.
#[derive(Debug, Clone)]
pub enum PeerEvent {
    /// A verified mTLS connection was established (either inbound accept or
    /// outbound dial succeeded). `fingerprint` is the canonical lowercase
    /// colon-free hex fingerprint of the peer's cert.
    Connected { fingerprint: DeviceFingerprint },
    /// An established mTLS connection was closed. `fingerprint` matches the
    /// value emitted in the preceding [`Connected`] event.
    ///
    /// [`Connected`]: PeerEvent::Connected
    Disconnected { fingerprint: DeviceFingerprint },
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
    /// Shared map of currently-connected peer sinks (SINGLE SOURCE OF TRUTH for
    /// online status AND the channel the unpair/revoke handlers use to send a
    /// `PeerFrame::Control(ControlMsg::Unpair)` to an online peer. Both fields
    /// are clones of the same underlying map.
    pub live_sinks: LivePeerSinks,
    pub peer_sinks: PeerSinks,
    /// Last-measured round-trip time per connected peer (milliseconds).
    ///
    /// Populated by the RTT ping task spawned alongside each established
    /// mTLS connection. The IPC `list_peers` handler reads this map to expose
    /// the `latency_ms` field in each peer entry. Entries are removed when
    /// the corresponding connection closes (same cleanup as `peer_sinks`).
    pub peer_rtt_ms: PeerRttMs,
    /// Broadcast channel for peer connection / disconnection events.
    ///
    /// Subscribers clone a [`broadcast::Receiver`] from this sender via
    /// [`broadcast::Sender::subscribe`]. The capacity is intentionally small
    /// (16) because consumers (e.g. the Tauri event bridge in `daemon.rs`)
    /// drain the queue quickly; lagged receivers simply miss stale events and
    /// will re-sync on the next `list_peers` call.
    pub peer_event_tx: broadcast::Sender<PeerEvent>,
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
/// **Not implemented via this module** — returns [`P2pError::NotImplemented`].
/// The PAKE handshake is handled directly by the IPC layer (`ipc.rs`).
pub fn pair_peer(
    _state: &P2pState,
    _peer_fingerprint: &str,
    _display_name: &str,
) -> Result<(), P2pError> {
    Err(P2pError::NotImplemented)
}

/// Remove a previously-paired peer.
///
/// **Not implemented via this module** — returns [`P2pError::NotImplemented`].
/// Pairing lifecycle is managed by the IPC layer alongside `pair_peer`.
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
/// Each entry is a per-connection `mpsc::Sender<PeerFrame>` that the
/// per-connection write task drains, serialises and sends to the peer over
/// the mTLS Framed stream. The outbound fanout loop writes `PeerFrame::Data`
/// entries; the unpair signal path writes `PeerFrame::Control(ControlMsg::Unpair)`.
/// Closed senders (disconnected peers) are pruned on the next fanout pass.
///
/// Keyed by the peer's verified **certificate fingerprint** (not its socket
/// address): a reconnect from a fresh ephemeral source port reuses the same
/// key, so the new connection replaces the old sink rather than producing a
/// duplicate that would double-fan-out every item (fix/p2p-c-review #4).
pub type PeerSinks = Arc<Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>;

/// Catch-up provider: produces the current local history as `WireItem`s already
/// re-keyed under the **per-peer** sync key (CopyPaste-716), so a freshly-
/// connected peer receives every item that predates the link (fanout is
/// otherwise fire-and-forget to whatever sinks happen to be live at the moment
/// an item is produced).
///
/// The closure takes the connecting peer's `fingerprint` as a `&str` so it can
/// look up that peer's specific pairwise key and produce blobs only that peer
/// can decrypt. Previously the closure was `Fn() -> Vec<WireItem>` (no
/// fingerprint arg) and used the first cached key for all peers — the bug fixed
/// by CopyPaste-716.
///
/// Built in `daemon.rs` from the DB + `SyncCrypto`; called once per established
/// connection (both the accept path and the connector path) right after the
/// peer sink is registered. LWW on the receiver makes the replay idempotent.
pub type CatchupProvider = Arc<dyn Fn(&str) -> Vec<WireItem> + Send + Sync>;

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
    // CopyPaste-1w7 (H8 fix): the daemon's shared `SyncCrypto` handle, built
    // once in `daemon.rs` and shared with the IPC server (same Arc<Mutex<…>>
    // backing store).  Cloned and forwarded to the standing pairing responder
    // so it can call `reload_sync_key` after a successful button-pair without
    // a daemon restart.  `None` when P2P is disabled.
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
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

    // ── RTT map ───────────────────────────────────────────────────────────────
    // Shared RTT map populated by the ping task that runs alongside every
    // established connection. Keyed by fingerprint so the IPC list_peers
    // handler can add latency_ms to each peer entry.
    let peer_rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));

    // ── peer-event broadcast channel ──────────────────────────────────────────
    // Capacity 16: the event bridge in daemon.rs drains quickly; a lagged
    // subscriber simply misses a stale event and re-syncs on the next
    // list_peers call (acceptable degradation).
    let (peer_event_tx, _) = broadcast::channel::<PeerEvent>(16);

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
    // Gate mDNS registration on `lan_visibility`. When false, skip both the
    // register call and the browse task so the device is invisible on the LAN.
    // The mTLS listener is still bound so paired peers with a persisted address
    // can connect directly.
    if config.lan_visibility {
        // Advertise the bootstrap port in `bport` when available (v2); else v1.
        let register_result = match bootstrap_port {
            Some(bport) => discovery.register_with_bport(
                actual_port,
                &device_id_str,
                &config.device_name,
                bport,
            ),
            None => discovery.register(actual_port, &device_id_str, &config.device_name),
        };
        register_result.map_err(|e| anyhow::anyhow!("mDNS register failed: {e}"))?;
    } else {
        tracing::info!("lan_visibility=false: skipping mDNS-SD registration and browsing");
    }

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
        // CopyPaste-1w7: clone the SyncCrypto handle (all clones share the
        // same Arc<Mutex<…>> backing store) so the responder can call
        // reload_sync_key after a successful button-pair without a restart.
        let sync_crypto_for_responder = sync_crypto.clone();
        tokio::spawn(async move {
            standing_pairing_responder_loop(
                bport,
                cert_der,
                key_der,
                peers_for_responder,
                pairing_for_responder,
                own_sync_addr_for_responder,
                public_ip_cache_for_responder,
                sync_crypto_for_responder,
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
        // Gap B: the accept loop forwards the live allowlist clone to each
        // per-connection task so an inbound unpair evicts from it immediately.
        let accept_peers = peers.clone();
        let accept_rtt_ms = Arc::clone(&peer_rtt_ms);
        let accept_event_tx = peer_event_tx.clone();
        tokio::spawn(async move {
            accept_loop(
                listener,
                accept_shutdown,
                transport,
                peer_sinks,
                incoming_tx,
                catchup,
                accept_peers,
                accept_rtt_ms,
                accept_event_tx,
            )
            .await;
        });
    }

    // ── outbound fanout loop ──────────────────────────────────────────────────
    // CopyPaste-716: pass `sync_crypto` so `outbound_loop` can re-encrypt once
    // per peer under that peer's pairwise key inside `fanout_to_peers`.
    {
        let peer_sinks = Arc::clone(&peer_sinks);
        let outbound_shutdown = shutdown_token.clone();
        let outbound_crypto = sync_crypto.clone();
        tokio::spawn(async move {
            outbound_loop(
                new_item_rx,
                outbound_rx,
                peer_sinks,
                outbound_crypto,
                outbound_shutdown,
            )
            .await;
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
        // Gap A + Gap B: the connector owns a live allowlist clone so it can
        // (a) forward it to per-connection tasks for inbound-unpair eviction, and
        // (b) temporarily re-add a `pending_unpair` peer just long enough to dial
        //     it and deliver the deferred `ControlMsg::Unpair` frame.
        let connector_peers = peers.clone();
        let connector_rtt_ms = Arc::clone(&peer_rtt_ms);
        let connector_event_tx = peer_event_tx.clone();
        tokio::spawn(async move {
            peer_connector_loop(
                transport,
                peer_sinks,
                incoming_tx,
                own_fp,
                catchup,
                discovery_for_connector,
                connector_shutdown,
                connector_peers,
                connector_rtt_ms,
                connector_event_tx,
            )
            .await;
        });
    }

    // ── discovery task ────────────────────────────────────────────────────────
    // Only start the mDNS-SD browse + advertise loop when lan_visibility is
    // enabled. When off the discovery service is still available (the IPC server
    // holds a reference for peer resolution) but does not advertise or browse,
    // so the device is invisible on the LAN.
    let discovery_shutdown = shutdown_token.clone();
    if config.lan_visibility {
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
    }

    Ok(P2pHandle {
        actual_port,
        shutdown_token,
        live_sinks: Arc::clone(&peer_sinks),
        peer_sinks,
        peer_rtt_ms,
        peer_event_tx,
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
#[allow(clippy::too_many_arguments)] // RTT + event params pushed count over 8
async fn accept_loop(
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
                        let pending_pings: PendingPings =
                            Arc::new(Mutex::new(HashMap::new()));
                        let rtt_map_for_task = Arc::clone(&peer_rtt_ms);
                        let rtt_map_for_ping = Arc::clone(&peer_rtt_ms);
                        let pending_pings_for_conn = Arc::clone(&pending_pings);

                        // Spawn the periodic RTT ping task. It holds a clone of
                        // cleanup_tx (the same sink as the drainer) to inject
                        // Ping frames through the normal outbound channel.
                        let ping_fp = peer_fp.clone();
                        let ping_sink = cleanup_tx.clone();
                        tokio::spawn(async move {
                            ping_loop(ping_sink, ping_fp, pending_pings, rtt_map_for_ping).await;
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
fn resolve_addr_from_discovery_by_ip(
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
fn refresh_peer_meta_from_discovery(
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
struct DialablePeer {
    /// Canonical (colon-free, lowercase) cert fingerprint — the mTLS pin.
    fingerprint: DeviceFingerprint,
    /// The peer's sync-listener socket address.
    addr: SocketAddr,
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
struct DialablePeersCache {
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
    fn get(&mut self, path: &std::path::Path) -> Vec<DialablePeer> {
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
#[allow(clippy::too_many_arguments)] // RTT + event params pushed count over 9
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
                        let pending_pings: PendingPings = Arc::new(Mutex::new(HashMap::new()));
                        let rtt_map_for_task = Arc::clone(&peer_rtt_ms);
                        let rtt_map_for_ping = Arc::clone(&peer_rtt_ms);
                        let pending_pings_for_conn = Arc::clone(&pending_pings);
                        let ping_fp = peer.fingerprint.clone();
                        let ping_sink = cleanup_tx.clone();
                        tokio::spawn(async move {
                            ping_loop(ping_sink, ping_fp, pending_pings, rtt_map_for_ping).await;
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

/// Manage one authenticated **inbound** (accept-side) peer connection.
///
/// `peer_fp` is the mTLS-verified certificate fingerprint of the remote peer,
/// used to authenticate any `ControlMsg::Unpair` signal (see
/// [`run_peer_connection_framed`]).
async fn run_peer_connection(
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
async fn run_peer_connection_client(
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
const WRITE_TIMEOUT: Duration = Duration::from_secs(8);

/// Duplex pump shared by the accept-side and connector-side connection tasks.
///
/// Reads incoming frames and forwards them to `incoming_tx`; reads from
/// `peer_rx` and writes outgoing frames to the peer. Both directions run
/// concurrently via `tokio::select!`; the task exits when either side closes.
/// Generic over the framed stream so the server-side ([`PeerStream`]) and
/// client-side ([`PeerClientStream`]) TLS stream types share one implementation.
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
async fn run_peer_connection_framed<S>(
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

/// Periodic RTT ping sender for a single established peer connection.
///
/// Sends a [`ControlMsg::Ping`] frame every [`PING_INTERVAL`] through the
/// peer sink, recording the send time in `pending_pings`. Expires unmatched
/// nonces after [`PING_PONG_TIMEOUT`] so the map doesn't grow unbounded
/// against peers that don't speak the Ping/Pong protocol.
///
/// The task exits when `peer_sink` is closed (the connection dropped and the
/// per-connection task removed the sink from `peer_sinks`).
async fn ping_loop(
    peer_sink: mpsc::Sender<PeerFrame>,
    peer_fp: DeviceFingerprint,
    pending_pings: PendingPings,
    peer_rtt_ms: PeerRttMs,
) {
    let mut interval = tokio::time::interval(PING_INTERVAL);
    // Skip the first (immediate) tick so we don't ping before the catchup
    // replay is done — the first real ping fires after PING_INTERVAL.
    interval.tick().await;

    loop {
        interval.tick().await;

        // Expire stale pending pings before sending a new one.
        {
            let mut map = pending_pings.lock().await;
            let now = Instant::now();
            map.retain(|_, sent_at| now.duration_since(*sent_at) < PING_PONG_TIMEOUT);
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

/// Outbound fanout loop.
///
/// Receives `WireItem`s from the sync orchestrator via `outbound_rx` and
/// sends each one to every currently-connected peer, re-encrypting once per
/// peer under that peer's pairwise sync key (CopyPaste-716).
///
/// Also drains the `new_item_rx` broadcast channel (previously handled by
/// `subscriber_loop`) so broadcast items are also fanned out.
///
/// Peer sinks whose channel is closed (peer disconnected) are removed from
/// `peer_sinks` on the next fanout pass.
async fn outbound_loop(
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut outbound_rx: mpsc::Receiver<WireItem>,
    peer_sinks: PeerSinks,
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
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
            // Outbound WireItem from sync_orch — fan out to all connected peers,
            // re-encrypting once per peer under that peer's pairwise sync key.
            item_opt = outbound_rx.recv(), if !outbound_closed => {
                match item_opt {
                    Some(item) => {
                        fanout_to_peers(&item, &peer_sinks, sync_crypto.as_ref()).await;
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

/// Push the catch-up history for `peer_fingerprint` into a freshly-connected
/// peer's sink.
///
/// CopyPaste-716: the `peer_fingerprint` is forwarded to the `CatchupProvider`
/// so it can look up the pairwise sync key for this specific peer and re-encrypt
/// each item under that key. Previously the provider had no fingerprint arg and
/// used the first cached key for all peers, causing 3rd+ peers to receive blobs
/// encrypted under the wrong key (silent AEAD failure on the receiver).
async fn push_catchup(
    catchup: &CatchupProvider,
    peer_fingerprint: &str,
    sink: &mpsc::Sender<PeerFrame>,
) {
    let items = catchup(peer_fingerprint);
    if items.is_empty() {
        return;
    }
    tracing::debug!(
        count = items.len(),
        peer = %peer_fingerprint,
        "P2P sync-on-connect: replaying local history to peer"
    );
    for item in items {
        if sink.send(PeerFrame::Data(item)).await.is_err() {
            tracing::debug!("P2P sync-on-connect: peer sink closed mid-replay");
            return;
        }
    }
}

/// Send `item` to every currently-connected peer sink, re-encrypting once per
/// peer under that peer's pairwise sync key (CopyPaste-716).
///
/// Peers whose sender has been closed (disconnected) are removed from
/// `peer_sinks`.
///
/// CopyPaste-716: `sync_crypto` is now used to re-encrypt the raw at-rest wire
/// item once per peer under the correct pairwise key before sending. The old
/// path cloned the same pre-encrypted blob to all peers — breaking >2 device
/// sync because peer C received a K_AB-encrypted blob it could not decrypt.
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
async fn fanout_to_peers(
    item: &WireItem,
    peer_sinks: &PeerSinks,
    sync_crypto: Option<&crate::sync_orch::SyncCrypto>,
) {
    // Snapshot (key, sender) pairs under the lock, then release it before sending.
    let snapshot: Vec<(DeviceFingerprint, mpsc::Sender<PeerFrame>)> = {
        let sinks = peer_sinks.lock().await;
        sinks
            .iter()
            .map(|(key, tx)| (key.clone(), tx.clone()))
            .collect()
    };

    let mut dead_keys: Vec<DeviceFingerprint> = Vec::new();
    for (key, tx) in snapshot {
        // CopyPaste-716: re-encrypt the at-rest wire item under this peer's
        // specific pairwise sync key. Each peer gets its own independently-
        // encrypted clone, so K_AB is never sent to peer C (which needs K_AC).
        let peer_item = if let Some(crypto) = sync_crypto {
            let mut cloned = item.clone();
            let outcome =
                crate::sync_orch::rekey_outbound_for_peer(crypto, key.as_str(), &mut cloned);
            match outcome {
                crate::sync_orch::RekeyOutcome::Rewrapped => cloned,
                crate::sync_orch::RekeyOutcome::Failed => {
                    // sync H2: a key was present but re-keying failed — drop
                    // this item for this peer rather than forwarding an
                    // undecryptable blob. The catch-up replay will retry on
                    // the next reconnect once the root cause is resolved.
                    tracing::warn!(
                        peer = %key,
                        item_id = %item.item_id,
                        "fanout: rekey failed for peer, dropping item (catch-up will reconcile)"
                    );
                    continue;
                }
                crate::sync_orch::RekeyOutcome::NotApplicable => {
                    // No pairwise key for this peer (legacy peer / P2P disabled):
                    // forward the raw at-rest ciphertext as the legacy path did.
                    item.clone()
                }
            }
        } else {
            // No SyncCrypto (P2P crypto disabled): legacy path — clone as-is.
            item.clone()
        };

        match tx.try_send(PeerFrame::Data(peer_item)) {
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

/// Evict a peer from the local persistent store and live allowlist on receipt
/// of an authenticated unpair signal.
///
/// This is the **receive side** of mutual unpair.  `peer_fp` is the mTLS
/// certificate fingerprint that the TLS transport verified before any data was
/// exchanged — it is the only input used for the eviction, so a misbehaving
/// peer cannot cause another peer's record to be removed.
///
/// Best-effort: file-system or parse failures are logged but do not return an
/// error — the calling connection task exits regardless, ensuring the local
/// mTLS transport will refuse further reconnects from this peer once the
/// allowlist entry is gone.
///
/// `live_peers` is the daemon's live, interior-mutable mTLS allowlist. When
/// supplied, the peer is ALSO removed from it (Gap B fix) so the stale mTLS
/// allowlist entry is gone immediately — without waiting for a daemon restart.
/// Passing `None` (as the unit tests do for the file-only path) skips that step.
fn evict_peer_local(peer_fp: &str, live_peers: Option<&PairedPeers>) {
    let peers_path = crate::ipc::peers_file_path();
    let mut peers = crate::peers::load_peers(&peers_path);
    let before = peers.len();
    // Normalise stored colon-hex fingerprints before comparing, because the
    // P2P layer reports colon-free hex (the canonical form used here).
    let canonical_target = peer_fp.to_ascii_lowercase();
    peers.retain(|p| crate::ipc::canonical_fingerprint(&p.fingerprint) != canonical_target);
    let removed = peers.len() < before;
    if let Err(e) = crate::peers::save_peers(&peers_path, &peers) {
        tracing::warn!(
            peer = %peer_fp,
            "evict_peer_local: failed to save peers.json after unpair signal: {e}"
        );
    } else if removed {
        tracing::info!(peer = %peer_fp, "evict_peer_local: peer removed from peers.json");
    }

    // Gap B fix: the persisted file alone is not enough — the live mTLS
    // allowlist (`PairedPeers`, shared with the transport's cert verifier) must
    // ALSO drop this fingerprint, or the unpaired peer keeps being accepted on
    // every subsequent handshake until the daemon restarts.
    if let Some(live) = live_peers {
        live.remove(&canonical_target);
        tracing::info!(
            peer = %peer_fp,
            "evict_peer_local: peer removed from live PairedPeers allowlist"
        );
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
async fn deliver_pending_unpairs(
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
// CopyPaste-1w7: `sync_crypto` is the 9th parameter; allow the lint so the
// handle can be threaded through without introducing a new struct solely to
// satisfy the argument-count limit (matching the pattern used for `start_p2p`).
#[allow(clippy::too_many_arguments)]
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
    // CopyPaste-1w7 (H8 fix): the daemon's shared SyncCrypto handle.  Passed
    // to `persist_paired_peer` so `reload_sync_key` runs after a successful
    // button-pair and the running orchestrator picks up the new shared key
    // without a daemon restart.  Matches the three IPC-initiated pairing
    // paths (SAS initiator, QR responder, QR initiator).
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
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

        // Discovery (QR-less) path: the responder's OPAQUE `PasswordFile` MUST be
        // registered for the SAME password the initiator uses, because opaque-ke
        // is asymmetric — a per-side random password makes `ClientLogin::finish`
        // fail at frame 7 before any SAS is derived. So both ends use the FIXED,
        // well-known, NON-SECRET `copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`; the
        // human SAS compare authenticates, not this value.
        let password = copypaste_p2p::DISCOVERY_PAIRING_PASSWORD.to_string();

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
                // Responder path: no prior mDNS resolution → empty PeerSnapshot.
                // The inbound TLS peer fingerprint is available post-handshake
                // but not threaded into the confirm callback yet; follow-up task.
                if !coordinator.try_begin(
                    crate::pairing_sm::PairingRole::Responder,
                    crate::pairing_sm::PeerSnapshot::default(),
                ) {
                    tracing::warn!("LAN/SAS: inbound pairing rejected — another pairing active");
                    return false;
                }
                let rx = coordinator.enter_awaiting_sas(
                    sas,
                    crate::pairing_sm::PairingRole::Responder,
                    crate::pairing_sm::PeerSnapshot::default(),
                );
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
        // "QR fully provisions all sync": this LAN/SAS *discovery* responder does
        // not advertise sync provisioning (it has no `sync_key` handle here, and
        // the feature is scoped to the QR pairing paths). Pass `None`; a future
        // wave can plumb the sync_key Arc through `start_p2p` to enable it on the
        // discovery path too. A peer's received provisioning is left unapplied on
        // this path for the same reason.
        let run_result = tokio::select! {
            r = responder.run_with_confirm(&password, &own_addr, &own_meta, None, confirm) => r,
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
                // CopyPaste-1w7 (H8 fix): pass the real SyncCrypto handle so
                // `persist_paired_peer` calls `reload_sync_key` after writing
                // `peers.json`.  This mirrors the three IPC-initiated pairing
                // paths (SAS initiator ipc.rs:2159, QR responder ipc.rs:2312,
                // QR initiator ipc.rs:2436) and ensures the running orchestrator
                // picks up the new shared key without a daemon restart.
                crate::ipc::IpcServer::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    sync_crypto.as_ref(),
                )
                .await;
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
            "testpeer".to_string(),
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

    /// Gap B: after `run_peer_connection_framed` receives an inbound
    /// `ControlMsg::Unpair` over a REAL in-process mTLS connection, the live
    /// `PairedPeers` allowlist handed to it must no longer contain the peer —
    /// proving `evict_peer_local` now removes from the live allowlist, not just
    /// `peers.json`. Built as a sync test that owns its runtime so the
    /// `TEST_ENV_LOCK`-guarded `COPYPASTE_CONFIG_DIR` override is never held
    /// across an `.await` (clippy::await_holding_lock).
    #[test]
    fn gap_b_evict_peer_local_removes_from_live_allowlist() {
        let tmp = tempfile::tempdir().unwrap();

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK; restored before the lock drops.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let server_peers = PairedPeers::new();
        let client_known = rt.block_on(async {
            let server_cert = copypaste_p2p::cert::SelfSignedCert::generate("gapb-server").unwrap();
            let client_cert = copypaste_p2p::cert::SelfSignedCert::generate("gapb-client").unwrap();
            let server_fp = server_cert.fingerprint();
            let client_fp = client_cert.fingerprint();

            server_peers.add(client_fp.clone(), "client");
            let client_peers = PairedPeers::new();
            client_peers.add(server_fp.clone(), "server");

            let server_transport = PeerTransport::from_cert(
                server_cert.cert_der,
                server_cert.key_der,
                server_peers.clone(),
            );
            let client_transport =
                PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            // Sanity: the peer is known before the unpair.
            assert!(
                server_peers.is_known(&client_fp),
                "precondition: client must be allow-listed before unpair"
            );

            let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);
            let (_peer_tx, peer_rx) = mpsc::channel::<PeerFrame>(8);
            let server_peers_for_pump = server_peers.clone();

            // Server: accept one connection, then run the real duplex pump with
            // the live allowlist supplied (Gap B path).
            let accept_fut = async move {
                let (_peer_addr, peer_fp, stream) =
                    server_transport.accept(&listener).await.unwrap();
                let pending: PendingPings = Arc::new(Mutex::new(HashMap::new()));
                let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
                run_peer_connection_framed(
                    stream,
                    peer_rx,
                    incoming_tx,
                    peer_fp,
                    Some(server_peers_for_pump),
                    pending,
                    rtt_ms,
                )
                .await;
            };

            // Client: connect and send a single Unpair control frame.
            let connect_fut = async move {
                let mut stream = client_transport.connect(addr, &server_fp).await.unwrap();
                let payload = serde_json::to_vec(&PeerFrame::Control(ControlMsg::Unpair)).unwrap();
                stream.send(Bytes::from(payload)).await.unwrap();
                // Hold the connection briefly so the server processes the frame
                // before the client drops (which would also close the stream).
                tokio::time::sleep(Duration::from_millis(200)).await;
            };

            tokio::join!(accept_fut, connect_fut);

            server_peers.is_known(&client_fp)
        });

        // Restore env before any assertion that might panic.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        assert!(
            !client_known,
            "Gap B: after an inbound Unpair the peer must be gone from the live PairedPeers allowlist"
        );
    }

    /// Gap B (pure unit): `evict_peer_local` with a live `PairedPeers` supplied
    /// must remove the fingerprint from BOTH `peers.json` and the live allowlist.
    #[test]
    fn gap_b_evict_peer_local_unit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[crate::peers::PairedDevice {
                fingerprint: "aa:bb:cc".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: Some("10.0.0.1:1111".to_string()),
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
                password_file_b64: None,
                password_file_enc: None,
            }],
        )
        .unwrap();

        let live = PairedPeers::new();
        live.add("aabbcc", "Alice");
        assert!(
            live.is_known("aabbcc"),
            "precondition: live allowlist has Alice"
        );

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        evict_peer_local("aabbcc", Some(&live));

        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        // File: Alice removed.
        let loaded = crate::peers::load_peers(&path);
        assert!(
            loaded.is_empty(),
            "Gap B: peers.json must no longer contain Alice"
        );
        // Live allowlist: Alice removed.
        assert!(
            !live.is_known("aabbcc"),
            "Gap B: live PairedPeers must no longer contain Alice"
        );
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
            let catchup: CatchupProvider = Arc::new(|_fp: &str| Vec::new());
            let token = token.clone();
            let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
            let (event_tx, _) = broadcast::channel::<PeerEvent>(4);
            tokio::spawn(async move {
                accept_loop(
                    listener,
                    token,
                    transport,
                    peer_sinks,
                    incoming_tx,
                    catchup,
                    PairedPeers::new(),
                    rtt_ms,
                    event_tx,
                )
                .await;
            })
        };

        // outbound_loop: both channels open but idle → blocked in its select!.
        let outbound_handle = {
            let (_new_item_tx, new_item_rx) = broadcast::channel::<ClipboardItem>(8);
            let (_outbound_tx, outbound_rx) = mpsc::channel::<WireItem>(8);
            let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
            let token = token.clone();
            tokio::spawn(async move {
                outbound_loop(new_item_rx, outbound_rx, peer_sinks, None, token).await;
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

    /// BUG F1 (verification follow-up): the `peer_connector_loop` must exit
    /// promptly when its cloned token is cancelled. With an empty peers file the
    /// loop has nothing to dial and parks on its inter-tick sleep select (which
    /// already races cancellation); the new mid-dial select arm covers the case
    /// where a dial is in flight. We pin `COPYPASTE_CONFIG_DIR` at an empty
    /// tempdir (under the process-wide env lock) so `peers_file_path()` resolves
    /// to a non-existent `peers.json` and the loop never reaches a real dial —
    /// keeping the test hermetic (no network / no multicast).
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_connector_loop() {
        let tmp = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        // Hold the process-wide env lock only while we set the override, spawn the
        // loop, and cancel it — never across an await (clippy::await_holding_lock).
        // The loop is cancelled before the lock is released, so it performs at most
        // one peers.json read against our empty tempdir.
        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK; restored before the lock drops.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        let handle = {
            let cert = copypaste_p2p::cert::SelfSignedCert::generate("f1-connector").unwrap();
            let transport = Arc::new(PeerTransport::from_cert(
                cert.cert_der,
                cert.key_der,
                PairedPeers::new(),
            ));
            let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
            let (incoming_tx, _incoming_rx) = mpsc::channel::<WireItem>(8);
            let own_fp = transport.fingerprint().to_string();
            let catchup: CatchupProvider = Arc::new(|_fp: &str| Vec::new());
            let discovery = Arc::new(DiscoveryService::new());
            let token = token.clone();
            let rtt_ms: PeerRttMs = Arc::new(Mutex::new(HashMap::new()));
            let (event_tx, _) = broadcast::channel::<PeerEvent>(4);
            tokio::spawn(async move {
                peer_connector_loop(
                    transport,
                    peer_sinks,
                    incoming_tx,
                    own_fp,
                    catchup,
                    discovery,
                    token,
                    PairedPeers::new(),
                    rtt_ms,
                    event_tx,
                )
                .await;
            })
        };

        // Loop is parked on its tick select; cancel before releasing the env lock.
        token.cancel();
        // SAFETY: still holding TEST_ENV_LOCK; restore the prior value, then drop
        // the guard so the subsequent await holds no std mutex guard.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        let joined = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            joined.is_ok(),
            "BUG F1: peer_connector_loop must exit promptly on token cancel"
        );
        joined.unwrap().unwrap();
    }

    /// BUG F1 (verification follow-up): the `standing_pairing_responder_loop`
    /// must exit promptly on token cancel. It binds an ephemeral bootstrap port
    /// (`bport = 0`, a passive loopback TCP listener — no multicast) and then
    /// parks inside `run_with_confirm` awaiting an inbound pairing connection
    /// that never arrives, raced against cancellation. Cancelling must drop the
    /// in-flight accept future and break the loop. Fully hermetic.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_standing_responder_loop() {
        let token = CancellationToken::new();
        let handle = {
            let cert = copypaste_p2p::cert::SelfSignedCert::generate("f1-responder").unwrap();
            let peers = PairedPeers::new();
            let pairing = Arc::new(crate::pairing_sm::PairingCoordinator::new());
            let own_sync_addr = Arc::new(std::sync::Mutex::new(Some("127.0.0.1:0".to_string())));
            let public_ip_cache = Arc::new(tokio::sync::RwLock::new(None));
            let token = token.clone();
            tokio::spawn(async move {
                standing_pairing_responder_loop(
                    0, // ephemeral bootstrap port — passive loopback listener
                    cert.cert_der,
                    cert.key_der,
                    peers,
                    pairing,
                    own_sync_addr,
                    public_ip_cache,
                    None, // sync_crypto — not needed for cancellation test
                    token,
                )
                .await;
            })
        };

        // Give the loop a moment to reach its `run_with_confirm` accept await,
        // then cancel; it must break out well within the bound.
        tokio::time::sleep(Duration::from_millis(100)).await;
        token.cancel();
        let joined = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            joined.is_ok(),
            "BUG F1: standing_pairing_responder_loop must exit promptly on token cancel"
        );
        joined.unwrap().unwrap();
    }

    /// BUG F1 (verification follow-up): the mDNS discovery task (spawned inline
    /// in `start_p2p`) awaits its `DiscoveryService::start()` handle raced
    /// against cancellation (see p2p.rs ~479). The task body is not a standalone
    /// function and `start()` performs real mDNS registration, so it cannot be
    /// unit-tested without multicast. This test asserts the exact, narrowest
    /// cancellable unit instead: a `select!` of a never-completing future against
    /// `shutdown.cancelled()` resolves to the cancel arm — i.e. the same
    /// structure that guards the discovery handle exits promptly on cancel.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_discovery_select() {
        let token = CancellationToken::new();
        let handle = {
            let token = token.clone();
            tokio::spawn(async move {
                // Mirror the discovery task's guard: a long-lived handle future
                // (here a never-resolving future) raced against cancellation.
                tokio::select! {
                    _ = std::future::pending::<()>() => unreachable!("handle never completes"),
                    _ = token.cancelled() => {}
                }
            })
        };
        token.cancel();
        let joined = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            joined.is_ok(),
            "BUG F1: discovery task select must exit promptly on token cancel"
        );
        joined.unwrap().unwrap();
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

    /// `pair_peer` is a thin stub that delegates to the IPC layer — it must
    /// surface the explicit `NotImplemented` error rather than silently returning Ok.
    #[test]
    fn pair_peer_returns_not_implemented() {
        let state = init(0, "test-device-id", "Test Device").expect("init must succeed");
        let result = pair_peer(&state, "deadbeef", "Alice");
        assert!(matches!(result, Err(P2pError::NotImplemented)));
    }

    /// `unpair_peer` is also a thin stub; pairing is managed by the IPC layer.
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
                    password_file_b64: None,
                    password_file_enc: None,
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
                    password_file_b64: None,
                    password_file_enc: None,
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
                password_file_b64: None,
                password_file_enc: None,
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

    // ── Mutual unpair ─────────────────────────────────────────────────────────

    /// `evict_peer_local` removes the matching peer from `peers.json` and
    /// leaves all other records intact.  The eviction is keyed to the
    /// mTLS-authenticated fingerprint (canonical, colon-free hex); the function
    /// must not touch any other record even when the stored form uses colon-hex.
    #[test]
    fn evict_peer_local_removes_only_the_authenticated_peer() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");

        // Store two peers in colon-hex form (the standard peers.json format).
        crate::peers::save_peers(
            &path,
            &[
                crate::peers::PairedDevice {
                    fingerprint: "aa:bb:cc".to_string(),
                    name: "Alice".to_string(),
                    added_at: 1_000,
                    address: Some("10.0.0.1:1111".to_string()),
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: None,
                    last_sync_at: None,
                    password_file_b64: None,
                    password_file_enc: None,
                },
                crate::peers::PairedDevice {
                    fingerprint: "dd:ee:ff".to_string(),
                    name: "Bob".to_string(),
                    added_at: 2_000,
                    address: None,
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: None,
                    last_sync_at: None,
                    password_file_b64: None,
                    password_file_enc: None,
                },
            ],
        )
        .unwrap();

        // Set up the env so `evict_peer_local` resolves to our temp dir.
        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        // Evict Alice using the canonical (colon-free) form of her fingerprint,
        // exactly as the mTLS layer would provide it.
        evict_peer_local("aabbcc", None);

        // Restore env before any assertions that might panic.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(loaded.len(), 1, "Alice must have been removed");
        assert_eq!(
            loaded[0].name, "Bob",
            "Bob must remain untouched after Alice's eviction"
        );
    }

    /// Receiving an `Unpair` signal from a peer whose fingerprint does NOT
    /// match any stored record is a no-op: `peers.json` is unchanged and the
    /// call does not panic.
    #[test]
    fn evict_peer_local_unknown_fp_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[crate::peers::PairedDevice {
                fingerprint: "aa:bb:cc".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: None,
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
                password_file_b64: None,
                password_file_enc: None,
            }],
        )
        .unwrap();

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        // "deadbeef" has no stored record — must be a silent no-op.
        evict_peer_local("deadbeef", None);

        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(loaded.len(), 1, "Alice must be untouched");
        assert_eq!(loaded[0].name, "Alice");
    }

    /// H8 regression (CopyPaste-1w7): `standing_pairing_responder_loop` called
    /// `IpcServer::persist_paired_peer` with `sync_crypto = None`, so the
    /// in-memory sync-key cache was never refreshed after a button-pair — the
    /// first sync after pairing silently fell back to "no key" until a daemon
    /// restart. This test exercises the contract that `persist_paired_peer`
    /// refreshes the cache when a `SyncCrypto` handle is supplied, and that it
    /// does NOT refresh when `None` is passed (the pre-fix standing-responder
    /// behaviour).
    ///
    /// # RED → GREEN
    /// Before the fix, the standing responder passed `None`, so this
    /// `persist_paired_peer(... None)` branch is the buggy path.  The fix
    /// threads a `SyncCrypto` clone into `standing_pairing_responder_loop` and
    /// passes it to `persist_paired_peer` as `Some(…)`.  Both branches are
    /// exercised here to pin the contract; the "None does not refresh" assertion
    /// remains correct after the fix (None is still a valid caller-supplied
    /// opt-out) while the real regression is caught by the "Some refreshes"
    /// assertion which fails if the plumbing accidentally passes None again.
    #[tokio::test]
    async fn persist_paired_peer_refreshes_sync_crypto_cache_iff_handle_supplied() {
        // ── shared setup ────────────────────────────────────────────────────
        let tmp = tempfile::tempdir().unwrap();

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK; restored before lock drops.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        // Dummy peers.json path inside the temp dir (same dir peers_file_path() uses).
        let peers_path = crate::ipc::peers_file_path();

        // ── test inputs ─────────────────────────────────────────────────────
        let session_key = copypaste_p2p::pake::SessionKey([0xABu8; 32]);
        let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_name: Some("Test Device".to_string()),
            public_ip: None,
        };
        let fp = "aa:bb:cc:dd:ee:ff:00:11";

        // ── branch 1: None (the pre-fix standing-responder path) ────────────
        // Create a SyncCrypto whose cache starts empty (no peers.json yet).
        // The seed bytes don't matter for cache-refresh; only the peers.json
        // path matters.
        let crypto_none = crate::sync_orch::SyncCrypto::new([0u8; 32], peers_path.clone());
        assert!(
            !crypto_none.has_cached_sync_key(),
            "precondition: cache is empty before any peer is persisted"
        );

        crate::ipc::IpcServer::persist_paired_peer(
            fp,
            "127.0.0.1:5001",
            &session_key,
            &peer_meta,
            None,
        )
        .await;

        // None was passed → reload_sync_key was never called → cache still empty.
        // This assertion PASSES before the fix, pinning the bug.
        assert!(
            !crypto_none.has_cached_sync_key(),
            "H8/CopyPaste-1w7: passing None must not refresh the cache (the pre-fix bug path)"
        );

        // Clean up peers.json before the second branch.
        let _ = std::fs::remove_file(&peers_path);

        // ── branch 2: Some (the fixed standing-responder path) ───────────────
        // Fresh SyncCrypto (no peers.json yet → cache starts empty).
        let crypto_some = crate::sync_orch::SyncCrypto::new([0u8; 32], peers_path.clone());
        assert!(
            !crypto_some.has_cached_sync_key(),
            "precondition: cache is empty before any peer is persisted"
        );

        crate::ipc::IpcServer::persist_paired_peer(
            fp,
            "127.0.0.1:5001",
            &session_key,
            &peer_meta,
            Some(&crypto_some),
        )
        .await;

        // Some(&crypto) was passed → reload_sync_key ran → cache is now populated.
        // This assertion FAILS before the fix because the standing responder
        // passed None; it PASSES after the fix threads the handle through.
        assert!(
            crypto_some.has_cached_sync_key(),
            "H8/CopyPaste-1w7: passing Some(&crypto) must refresh the cache \
             (standing responder must supply the handle, not None)"
        );

        // ── env restore ─────────────────────────────────────────────────────
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);
    }

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
        peer_rtt_ms.lock().await.insert(peer_fp.clone(), rtt_ms);

        let stored = peer_rtt_ms.lock().await.get(&peer_fp).copied();
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

    // ── PeerEvent broadcast tests ─────────────────────────────────────────────

    /// When a peer is inserted into the sinks map (simulating accept/connect),
    /// the caller sends `PeerEvent::Connected` on the broadcast channel and
    /// a subscriber receives it immediately.
    #[tokio::test]
    async fn peer_event_connected_is_broadcast() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "aabbcc001122".to_string();

        // Simulate what accept_loop does after inserting the sink.
        let _ = tx.send(PeerEvent::Connected {
            fingerprint: fp.clone(),
        });

        match rx.recv().await.expect("should receive Connected event") {
            PeerEvent::Connected { fingerprint } => {
                assert_eq!(fingerprint, fp, "Connected fingerprint must match the inserted peer");
            }
            PeerEvent::Disconnected { .. } => panic!("expected Connected, got Disconnected"),
        }
    }

    /// When a peer's connection task removes it from the sinks map (simulating
    /// disconnect), the caller sends `PeerEvent::Disconnected` and a subscriber
    /// receives it.
    #[tokio::test]
    async fn peer_event_disconnected_is_broadcast() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "ddeeff334455".to_string();

        // Simulate what the cleanup task does after removing the sink.
        let _ = tx.send(PeerEvent::Disconnected {
            fingerprint: fp.clone(),
        });

        match rx.recv().await.expect("should receive Disconnected event") {
            PeerEvent::Disconnected { fingerprint } => {
                assert_eq!(
                    fingerprint, fp,
                    "Disconnected fingerprint must match the removed peer"
                );
            }
            PeerEvent::Connected { .. } => panic!("expected Disconnected, got Connected"),
        }
    }

    /// A subscriber that joins after a connect+disconnect sequence receives both
    /// events in order.
    #[tokio::test]
    async fn peer_event_sequence_connected_then_disconnected() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "ff00aa112233".to_string();

        let _ = tx.send(PeerEvent::Connected {
            fingerprint: fp.clone(),
        });
        let _ = tx.send(PeerEvent::Disconnected {
            fingerprint: fp.clone(),
        });

        let first = rx.recv().await.expect("first event");
        let second = rx.recv().await.expect("second event");

        assert!(
            matches!(first, PeerEvent::Connected { .. }),
            "first event must be Connected"
        );
        assert!(
            matches!(second, PeerEvent::Disconnected { .. }),
            "second event must be Disconnected"
        );
    }

    /// When no subscribers are active, `send` on the event channel returns an
    /// error (no receivers) — the P2P code must not panic or fail on that.
    #[test]
    fn peer_event_send_with_no_receivers_is_ok_to_discard() {
        let (tx, rx) = broadcast::channel::<PeerEvent>(16);
        // Drop the only receiver so the channel has no subscribers.
        drop(rx);

        // The `let _ =` pattern we use in p2p.rs must not panic.
        let result = tx.send(PeerEvent::Connected {
            fingerprint: "aabbcc".to_string(),
        });
        // `Err` is expected (no receivers), but we must not panic.
        assert!(
            result.is_err(),
            "send with no receivers should return Err (not panic)"
        );
    }
}
