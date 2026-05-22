//! P2P subsystem orchestrator.
//!
//! Stubs out the mTLS listener, mDNS registration/browse, and broadcast
//! subscriber.  Full implementation lands when copypaste-p2p and
//! copypaste-sync are merged via the intg-p2p-crates branch.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot, Mutex};

use copypaste_core::{ClipboardItem, Database};

/// Configuration for the P2P subsystem.
pub struct P2pConfig {
    /// TCP port to listen on.  0 = OS-assigned ephemeral port.
    pub listen_port: u16,
    /// Human-readable name advertised via mDNS.
    pub device_name: String,
    /// When false `start_p2p` returns immediately without spawning any tasks.
    pub enabled: bool,
}

/// Live handle to a running P2P subsystem.
pub struct P2pHandle {
    /// The actual TCP port bound by the listener (useful when `listen_port` was 0).
    pub actual_port: u16,
    /// Send `()` to request a graceful shutdown of all P2P tasks.
    pub shutdown_tx: oneshot::Sender<()>,
}

/// Start the P2P subsystem.
///
/// Binds a `TcpListener`, registers with mDNS-SD, and spawns three background
/// tasks:
///
/// - **accept_loop** — accepts incoming mTLS connections from paired peers.
/// - **subscriber_loop** — forwards new clipboard items to connected peers.
/// - **discovery_task** — registers own service and browses for peers.
///
/// Returns a [`P2pHandle`] that keeps the subsystem alive.  Drop or send to
/// `shutdown_tx` to stop it.
///
/// # Errors
/// Returns an error if the TCP listener cannot be bound.
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

    // ── accept loop ───────────────────────────────────────────────────────────
    // Accepts raw TCP connections; full mTLS handshake will be performed by
    // copypaste-p2p once that crate is merged.
    tokio::spawn(async move {
        accept_loop(listener, shutdown_rx).await;
    });

    // ── subscriber loop ───────────────────────────────────────────────────────
    // Receives freshly-inserted clipboard items from the broadcast channel and
    // fans them out to connected peers.  Full serialisation + framing will be
    // handled by copypaste-sync.
    tokio::spawn(async move {
        subscriber_loop(new_item_rx).await;
    });

    // ── discovery task ────────────────────────────────────────────────────────
    // Advertises this device on `_copypaste._tcp.local.` and discovers peers.
    // Full implementation uses copypaste-p2p::discovery::DiscoveryService.
    tokio::spawn(async move {
        discovery_task(actual_port, device_id, &config.device_name).await;
    });

    Ok(P2pHandle {
        actual_port,
        shutdown_tx,
    })
}

// ── private helpers ───────────────────────────────────────────────────────────

async fn accept_loop(listener: TcpListener, mut shutdown_rx: oneshot::Receiver<()>) {
    tracing::debug!("P2P accept loop running on {}", listener.local_addr().unwrap());
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        tracing::debug!(%peer_addr, "incoming P2P connection (mTLS pending)");
                        // TODO(intg-p2p-crates): upgrade to mTLS and hand off to
                        // copypaste-sync::SyncEngine.
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
                    "new clipboard item — will push to peers (stub)"
                );
                // TODO(intg-p2p-crates): serialise via copypaste-sync protocol
                // and write to each connected PeerStream.
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

async fn discovery_task(port: u16, device_id: uuid::Uuid, device_name: &str) {
    tracing::info!(
        port,
        %device_id,
        device_name,
        "P2P discovery task running (mDNS registration stub)"
    );
    // TODO(intg-p2p-crates): replace with:
    //   let svc = copypaste_p2p::discovery::DiscoveryService::new();
    //   svc.register(port, &device_id.to_string(), device_name).unwrap();
    //   let _handle = svc.start().await.unwrap();
    //   /* browse loop */
}
