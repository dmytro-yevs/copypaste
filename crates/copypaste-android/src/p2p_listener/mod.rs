//! Inbound mTLS P2P listener — the Android analog of the macOS daemon's
//! `accept_loop` (`copypaste-daemon/src/p2p.rs`).
//!
//! Today Android only DIALS out (`sync_with_peer`) and polls the relay; it
//! never accepts an inbound connection, so macOS can never INITIATE a P2P
//! session to an Android device. This module adds a persistent, cancellable
//! accept loop that binds `0.0.0.0:<port>`, completes mutual-TLS handshakes
//! against a pinned `PairedPeers` allowlist, pushes a one-shot catch-up of the
//! local history on each accepted connection, then keeps the link open and
//! decrypts every inbound `WireItem` frame into a [`SyncedItem`] for Kotlin to
//! drain via [`poll`].
//!
//! # FFI shape
//!
//! UDL has no interface objects, so the long-lived listener is represented by a
//! `u64` handle into a process-global registry (`LISTENER_REGISTRY`) — the
//! same idiom `lib.rs` uses for `DB_HANDLES`. The FFI surface (in `lib.rs`):
//!   * `start_p2p_listener(..)` — bind + register + spawn on the shared runtime,
//!     returning the handle and the OS-assigned port immediately.
//!   * `poll_p2p_listener(id)` — atomically drain the received-item buffer.
//!   * `update_p2p_listener_peers(id, ..)` — live roster/denylist refresh.
//!   * `stop_p2p_listener(id)` — cancel + deregister (idempotent).
//!
//! # Security (load-bearing — mirrors macOS)
//!
//! * Allowlist pinning IS the authenticator: only fingerprints seeded into
//!   `PairedPeers` complete the TLS handshake (transport.rs rejects unpinned
//!   fingerprints at `is_known`).
//! * The denylist is re-checked **at accept, before any catch-up or frame** —
//!   the inbound analog of the dialer's revoked-peer refusal. A revoked peer
//!   must never receive the history push.
//! * Per-peer session key: each peer's frames are decrypted with the key
//!   derived from THAT peer's verified fingerprint, never a global key.
//! * Key bytes are never logged.
//!
//! # Module layout (ADR-017 split)
//!
//! This is a thin shell: the registry/state types live in `registry`, the
//! pure FFI→wire builders in `builders`, the item↔wire codec in `codec`,
//! the per-connection duplex pump in `connection`, and the accept loop in
//! `accept`. Every cross-submodule item is `pub(super)` (visible within this
//! module's subtree only); [`PeerSessionKey`] and [`P2pListenerHandle`] stay
//! fully `pub` (re-exported at `lib.rs`, consumed by `ffi_p2p_session.rs`) and
//! are UDL-exported dictionaries with a FROZEN field shape.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::PeerTransport;

use crate::{CopypasteError, LocalItem, SyncedItem};

mod accept;
mod builders;
mod codec;
mod connection;
mod registry;

#[cfg(test)]
mod tests;

pub use registry::{P2pListenerHandle, PeerSessionKey};

use accept::accept_loop;
use builders::{build_paired_peers, build_session_key_map};
use registry::{registry, ListenerHandle, NEXT_LISTENER_ID};

/// Bind `0.0.0.0:listen_port`, register a listener, and spawn its accept loop on
/// the shared runtime. Returns immediately with the handle + actual bound port.
///
/// `listen_port == 0` lets the kernel assign a free port (read back from
/// `local_addr()`).
#[allow(clippy::too_many_arguments)] // FFI contract mirrors `sync_with_peer`'s shape.
pub fn start(
    runtime: &'static tokio::runtime::Runtime,
    listen_port: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    allowed_fingerprints: Vec<String>,
    revoked_fingerprints: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
    local_items: Vec<LocalItem>,
    device_id: String,
) -> Result<P2pListenerHandle, CopypasteError> {
    // Bind on the runtime so the TcpListener is created inside a tokio context.
    let listener = runtime
        .block_on(async move { TcpListener::bind(("0.0.0.0", listen_port)).await })
        .map_err(|e| CopypasteError::P2pError {
            reason: format!("failed to bind P2P listener on 0.0.0.0:{listen_port}: {e}"),
        })?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| CopypasteError::P2pError {
            reason: format!("failed to read P2P listener local_addr: {e}"),
        })?
        .port();

    let peers = build_paired_peers(&allowed_fingerprints);
    // The transport clones the same `PairedPeers` Arc, so later add/remove on
    // `peer_state.peers` are reflected on subsequent handshakes.
    let transport = Arc::new(PeerTransport::from_cert(cert_der, key_der, peers.clone()));

    let peer_state = Arc::new(Mutex::new(registry::PeerState {
        peers,
        allowed: allowed_fingerprints,
        revoked: revoked_fingerprints,
        session_keys: build_session_key_map(session_keys),
    }));
    let received: Arc<Mutex<Vec<SyncedItem>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancellationToken::new();

    let listener_id = NEXT_LISTENER_ID.fetch_add(1, Ordering::SeqCst);

    {
        let mut reg = registry().lock().map_err(|_| CopypasteError::P2pError {
            reason: "listener registry mutex poisoned".to_string(),
        })?;
        reg.insert(
            listener_id,
            ListenerHandle {
                cancel: cancel.clone(),
                actual_port,
                received: Arc::clone(&received),
                peer_state: Arc::clone(&peer_state),
            },
        );
    }

    let local_items = Arc::new(local_items);
    let device_id = Arc::new(device_id);
    runtime.spawn(accept_loop(
        listener,
        transport,
        peer_state,
        local_items,
        device_id,
        received,
        cancel,
    ));

    Ok(P2pListenerHandle {
        listener_id,
        actual_port,
    })
}

/// Atomically drain the received-item buffer for `listener_id`. Returns an empty
/// vec for an unknown id (the listener may have been stopped between polls).
pub fn poll(listener_id: u64) -> Result<Vec<SyncedItem>, CopypasteError> {
    let reg = registry().lock().map_err(|_| CopypasteError::P2pError {
        reason: "listener registry mutex poisoned".to_string(),
    })?;
    let Some(handle) = reg.get(&listener_id) else {
        return Ok(Vec::new());
    };
    let mut buf = handle
        .received
        .lock()
        .map_err(|_| CopypasteError::P2pError {
            reason: "received-item buffer mutex poisoned".to_string(),
        })?;
    Ok(std::mem::take(&mut *buf))
}

/// Live roster/denylist/session-key refresh without restarting the listener.
/// A revoked fingerprint is removed from the pinned allowlist immediately (so
/// the TLS verifier rejects it on the next handshake) and added to the
/// denylist (so an in-flight accept is refused). No-op for an unknown id.
pub fn update_peers(
    listener_id: u64,
    allowed: Vec<String>,
    revoked: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
) -> Result<(), CopypasteError> {
    let reg = registry().lock().map_err(|_| CopypasteError::P2pError {
        reason: "listener registry mutex poisoned".to_string(),
    })?;
    let Some(handle) = reg.get(&listener_id) else {
        return Ok(());
    };
    let mut state = handle
        .peer_state
        .lock()
        .map_err(|_| CopypasteError::P2pError {
            reason: "peer-state mutex poisoned".to_string(),
        })?;

    // Reconcile the pinned allowlist in place: the `PeerTransport` holds a clone
    // of this same `PairedPeers` (shared Arc), so add/remove take effect on the
    // next handshake without rebuilding the transport.
    let desired: std::collections::HashSet<&String> = allowed.iter().collect();
    // Remove any previously-pinned fingerprint that is no longer allowed.
    for fp in &state.allowed {
        if !desired.contains(fp) {
            state.peers.remove(fp);
        }
    }
    // Add every desired fingerprint (add is idempotent).
    for fp in &allowed {
        state.peers.add(fp.clone(), "p2p-peer");
    }
    // Evict revoked fingerprints from the pinned allowlist immediately so an
    // in-flight or next handshake from a revoked peer is rejected at TLS.
    for fp in &revoked {
        state.peers.remove(fp);
    }

    state.allowed = allowed;
    state.revoked = revoked;
    state.session_keys = build_session_key_map(session_keys);
    Ok(())
}

/// Cancel and deregister `listener_id`. Idempotent: a second call (or an unknown
/// id) is a no-op. Firing the cancel token breaks the accept loop's `select!`
/// and drops the listener socket; in-flight per-connection tasks also observe
/// the cancel and exit.
pub fn stop(listener_id: u64) -> Result<(), CopypasteError> {
    let handle = {
        let mut reg = registry().lock().map_err(|_| CopypasteError::P2pError {
            reason: "listener registry mutex poisoned".to_string(),
        })?;
        reg.remove(&listener_id)
    };
    if let Some(handle) = handle {
        handle.cancel.cancel();
    }
    Ok(())
}
