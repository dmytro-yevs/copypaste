//! P2P subsystem initialisation helpers.

use copypaste_p2p::{
    discovery::{DiscoveryService, PeerInfo},
    transport::PairedPeers,
};

use super::{P2pError, P2pState};
use crate::keychain;

/// Initialise a `P2pState` synchronously: generate a fresh self-signed cert,
/// build a discovery service, and call `register()` for mDNS-SD.
///
/// The returned `P2pState` is safe to share across IPC handlers. A real
/// `TcpListener` is *not* bound here — the long-running [`crate::p2p::start_p2p`] entry
/// point owns the accept loop. `init` is intended for the lightweight IPC
/// query path (list/pair/unpair/own_fingerprint).
///
/// # Errors
/// Returns [`P2pError::Transport`] if cert generation fails, or
/// [`P2pError::Discovery`] if mDNS registration cannot be configured.
pub fn init(listen_port: u16, device_id: &str, device_name: &str) -> Result<P2pState, P2pError> {
    use copypaste_p2p::transport::PeerTransport;
    use std::sync::Arc;
    use tokio::sync::Mutex;

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

/// Compute the canonical device fingerprint from a raw public key.
///
/// Delegates to [`keychain::own_fingerprint`] for consistency with the rest
/// of the daemon (single source of truth for fingerprint format).
pub fn get_own_fingerprint(public_key: &[u8]) -> String {
    keychain::own_fingerprint(public_key)
}

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
pub(super) fn load_peers_from_path_into(path: &std::path::Path, peers: &PairedPeers) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
