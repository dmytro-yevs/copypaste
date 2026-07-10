//! P2P subsystem orchestrator.
//!
//! W2.2 — wires the mTLS accept loop and outbound fanout into the daemon,
//! bridging `copypaste-p2p` transport with the `sync_orch` channel pair
//! (`incoming_tx` / `outbound_rx`).
//!
//! Pairing is handled entirely by the IPC layer in `ipc.rs`
//! (`pair_peer_with_password`, `unpair_peer` IPC methods) — this module
//! does not expose pairing entry points.
//!
//! ## Module layout
//!
//! | Sub-module | Contents |
//! |---|---|
//! | `types` | Shared types: `P2pConfig`, `P2pHandle`, `P2pState`, `PeerEvent`, sink/RTT aliases |
//! | `init` | `init`, `list_peers`, `get_own_fingerprint`, `load_persisted_peers_into` |
//! | `listener` | `accept_loop` — mTLS accept loop — + `spawn_accept_loop` |
//! | `connector` | `peer_connector_loop`, `DialablePeer(s)Cache`, `deliver_pending_unpairs`, discovery helpers, `spawn_connector_loop` |
//! | `framed_pump` | `run_peer_connection_framed`, inbound/outbound wrappers, `WRITE_TIMEOUT` |
//! | `ping` | `ping_loop`, `PING_INTERVAL`, `PING_PONG_TIMEOUT` |
//! | `fanout` | `outbound_loop`, `fanout_to_peers`, `push_catchup`, `spawn_outbound_loop` |
//! | `unpair` | `send_unpair_and_close_session`, `evict_peer_local`, `stamp_peer_sync` |
//! | `pairing_responder` | `standing_pairing_responder_loop`, `probe_bootstrap_port`, `spawn_standing_responder_if_visible` |
//! | `discovery_task` | `register_mdns`, `spawn_discovery_task` |
//!
//! `integration_tests` and `misc_tests` (test-only) hold cross-submodule and
//! out-of-scope-relocation tests respectively — see their module docs.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use copypaste_core::{ClipboardItem, Database};
use copypaste_p2p::{
    discovery::DiscoveryService,
    transport::{PairedPeers, PeerTransport},
};
use copypaste_sync::protocol::WireItem;

// ── sub-modules ───────────────────────────────────────────────────────────────
mod connector;
mod discovery_task;
mod fanout;
mod framed_pump;
mod init;
mod listener;
mod pairing_responder;
mod ping;
mod types;
mod unpair;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod misc_tests;

// ── public re-exports (init surface) ─────────────────────────────────────────
pub use init::{get_own_fingerprint, init, list_peers, load_persisted_peers_into};
pub use unpair::send_unpair_and_close_session;

// CopyPaste-ptgcc: per-peer rekey-failure counter, surfaced by `list_peers`.
pub(crate) use fanout::rekey_failure_snapshot;
#[cfg(test)]
pub(crate) use fanout::{clear_rekey_failure, record_rekey_failure};

// ── types (split out to `types.rs`, ADR-017 CopyPaste-vp63.2) ────────────────
pub(crate) use types::PendingPings;
pub use types::{
    CatchupProvider, LivePeerSinks, P2pConfig, P2pError, P2pHandle, P2pState, PeerEvent, PeerRttMs,
    PeerSinks,
};

// ── start_p2p ─────────────────────────────────────────────────────────────────

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
    // CopyPaste-7ub: the shared live core config (config.toml). The outbound
    // fanout loop reads `sync_on_wifi_only` from it so P2P honours the
    // "Wi-Fi only" privacy setting exactly like the relay and cloud paths
    // (previously P2P transmitted on cellular regardless of the flag).
    core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    // CopyPaste-yw2k: non-secret Supabase account identity slot; forwarded
    // to the standing pairing responder so it can include it in `PeerMeta`.
    cloud_account_id: Option<Arc<std::sync::Mutex<Option<String>>>>,
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
    // CopyPaste-qvtg.1: log only the short fingerprint prefix at info — the full
    // 64-char mTLS-pinning identity stays at debug so it does not leak into
    // persistent log stores on every start.
    let transport_fp = transport.fingerprint();
    tracing::info!(
        fingerprint_prefix = %transport_fp.get(..23).unwrap_or(transport_fp),
        "P2P mTLS transport identity"
    );
    tracing::debug!(fingerprint = %transport_fp, "P2P mTLS transport identity (full)");
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
    // (spawned below) re-binds this SAME port for each inbound pairing.
    let bootstrap_port: Option<u16> = pairing_responder::probe_bootstrap_port(
        bootstrap_cert_der.clone(),
        bootstrap_key_der.clone(),
    )
    .await;

    // ── mDNS registration ───────────────────────────────────────────────────────
    // Gate mDNS registration on `lan_visibility`. When false, skip both the
    // register call and the browse task so the device is invisible on the LAN.
    // The mTLS listener is still bound so paired peers with a persisted address
    // can connect directly.
    let device_id_str = device_id.to_string();
    discovery_task::register_mdns(
        &discovery,
        config.lan_visibility,
        actual_port,
        &device_id_str,
        &config.device_name,
        bootstrap_port,
    )?;

    // ── standing discovery-pairing responder loop (LAN/SAS Phase 2) ────────────
    // Accepts inbound SAS-pairing connections on the advertised `bport` and runs
    // `run_with_confirm`, routing the SAS through the SHARED pairing coordinator
    // so the LOCAL user confirms via `pair_get_sas` / `pair_confirm_sas` exactly
    // like the initiator. Authentication is the human SAS comparison: the
    // initiator sends an EPHEMERAL random password in-clear inside the bootstrap
    // TLS, and the SAS (derived from the post-PAKE bound_key) is the real
    // authenticator. On reject/mismatch/timeout the session key drops/zeroizes
    // and NOTHING is persisted (no rotate_peer).
    pairing_responder::spawn_standing_responder_if_visible(
        config.lan_visibility,
        bootstrap_port,
        bootstrap_cert_der,
        bootstrap_key_der,
        peers.clone(),
        pairing,
        own_sync_addr,
        Arc::clone(&public_ip_cache),
        sync_crypto.clone(),
        device_id,
        cloud_account_id,
        shutdown_token.clone(),
    );

    // ── accept loop ───────────────────────────────────────────────────────────
    listener::spawn_accept_loop(
        listener,
        shutdown_token.clone(),
        Arc::clone(&transport),
        Arc::clone(&peer_sinks),
        incoming_tx.clone(),
        Arc::clone(&catchup),
        // Gap B: the accept loop forwards the live allowlist clone to each
        // per-connection task so an inbound unpair evicts from it immediately.
        peers.clone(),
        Arc::clone(&peer_rtt_ms),
        peer_event_tx.clone(),
        // crh3.109: share the STUN cache so each inbound connection can
        // advertise our current public IP in the DeviceInfo control frame.
        Arc::clone(&public_ip_cache),
    );

    // ── outbound fanout loop ──────────────────────────────────────────────────
    // CopyPaste-716: pass `sync_crypto` so `outbound_loop` can re-encrypt once
    // per peer under that peer's pairwise key inside `fanout_to_peers`.
    fanout::spawn_outbound_loop(
        new_item_rx,
        outbound_rx,
        Arc::clone(&peer_sinks),
        sync_crypto,
        core_config,
        shutdown_token.clone(),
    );

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
    let own_fp = transport.fingerprint().to_string();
    connector::spawn_connector_loop(
        transport,
        Arc::clone(&peer_sinks),
        incoming_tx,
        copypaste_p2p::DeviceFingerprint(own_fp),
        catchup,
        Arc::clone(&discovery),
        shutdown_token.clone(),
        peers,
        Arc::clone(&peer_rtt_ms),
        peer_event_tx.clone(),
        public_ip_cache,
    );

    // ── discovery task ────────────────────────────────────────────────────────
    // Only start the mDNS-SD browse + advertise loop when lan_visibility is
    // enabled. When off the discovery service is still available (the IPC server
    // holds a reference for peer resolution) but does not advertise or browse,
    // so the device is invisible on the LAN.
    discovery_task::spawn_discovery_task(
        discovery,
        config.device_name.clone(),
        actual_port,
        config.lan_visibility,
        shutdown_token.clone(),
    );

    Ok(P2pHandle {
        actual_port,
        shutdown_token,
        live_sinks: Arc::clone(&peer_sinks),
        peer_sinks,
        peer_rtt_ms,
        peer_event_tx,
    })
}
