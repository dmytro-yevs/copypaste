//! P2P subsystem orchestrator.
//!
//! Beta W2.1 — wires the `copypaste-p2p` crate's `DiscoveryService` and
//! `PeerTransport` into the daemon. The legacy `start_p2p` entry point used
//! by `daemon.rs` is preserved and upgraded to use the real mDNS-SD discovery
//! service (replacing the wave-1.3 stub). In parallel, this module exposes a
//! `P2pState` handle + `init()` / `list_peers()` / `pair_peer()` /
//! `unpair_peer()` / `get_own_fingerprint()` surface for `ipc.rs` consumers
//! (W2.2 will wire those into the IPC dispatcher).
//!
//! Pairing (`pair_peer` / `unpair_peer`) currently returns
//! [`P2pError::NotImplemented`] — the PAKE handshake lands in W2.4.

use std::sync::Arc;

use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot, Mutex};

use copypaste_core::{ClipboardItem, Database};
use copypaste_p2p::{
    discovery::{DiscoveryService, PeerInfo},
    transport::{PairedPeers, PeerTransport},
};

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

/// Start the long-running P2P subsystem.
///
/// Binds a `TcpListener`, registers with mDNS-SD via
/// `copypaste_p2p::DiscoveryService`, and spawns three background tasks:
///
/// - **accept_loop** — accepts incoming mTLS connections from paired peers.
/// - **subscriber_loop** — forwards new clipboard items to connected peers.
/// - **discovery_task** — keeps the discovery service alive for the lifetime
///   of the subsystem.
///
/// Returns a [`P2pHandle`] that keeps the subsystem alive.  Drop or send to
/// `shutdown_tx` to stop it.
///
/// # Errors
/// Returns an error if the TCP listener cannot be bound, or if the discovery
/// service fails to register / start.
pub async fn start_p2p(
    config: P2pConfig,
    _db: Arc<Mutex<Database>>,
    device_id: uuid::Uuid,
    _db_key: [u8; 32],
    new_item_rx: broadcast::Receiver<ClipboardItem>,
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

    // ── discovery service — real mDNS-SD via copypaste-p2p ────────────────────
    // Constructed up-front so the accept loop and (future) IPC handlers can
    // share the same `Arc<DiscoveryService>`.
    let discovery = Arc::new(DiscoveryService::new());
    let device_id_str = device_id.to_string();
    discovery
        .register(actual_port, &device_id_str, &config.device_name)
        .map_err(|e| anyhow::anyhow!("mDNS register failed: {e}"))?;

    let discovery_for_task = Arc::clone(&discovery);
    let device_name_for_task = config.device_name.clone();

    // ── accept loop ───────────────────────────────────────────────────────────
    // Accepts raw TCP connections; full mTLS handshake lands in W2.2/W2.4
    // (sync orchestrator + PAKE pairing).
    tokio::spawn(async move {
        accept_loop(listener, shutdown_rx).await;
    });

    // ── subscriber loop ───────────────────────────────────────────────────────
    // Receives freshly-inserted clipboard items from the broadcast channel and
    // (eventually) fans them out to connected peers via copypaste-sync.
    tokio::spawn(async move {
        subscriber_loop(new_item_rx).await;
    });

    // ── discovery task ────────────────────────────────────────────────────────
    // Starts the mDNS-SD daemon and keeps it alive for the lifetime of the
    // subsystem. The returned JoinHandle is awaited inside this task; on
    // shutdown dropping the parent task drops the inner JoinHandle and the
    // mdns-sd ServiceDaemon shuts down.
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

async fn accept_loop(listener: TcpListener, mut shutdown_rx: oneshot::Receiver<()>) {
    tracing::debug!(
        "P2P accept loop running on {}",
        listener.local_addr().unwrap()
    );
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        tracing::debug!(%peer_addr, "incoming P2P connection (mTLS pending W2.2)");
                        drop(stream);
                    }
                    Err(e) => {
                        tracing::warn!("P2P accept error: {e}");
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

async fn subscriber_loop(mut rx: broadcast::Receiver<ClipboardItem>) {
    tracing::debug!("P2P subscriber loop running");
    loop {
        match rx.recv().await {
            Ok(item) => {
                tracing::debug!(
                    item_id = %item.id,
                    "new clipboard item — will push to peers (W2.2 sync orchestrator)"
                );
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("P2P subscriber lagged by {n} items — some items not forwarded");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("P2P subscriber loop: broadcast channel closed, shutting down");
                break;
            }
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
}
