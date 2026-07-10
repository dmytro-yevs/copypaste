//! Pure builders translating FFI-shaped inputs into wire/crypto types. No I/O,
//! no locking.

use std::collections::HashMap;

use copypaste_p2p::transport::PairedPeers;

use super::registry::PeerSessionKey;

/// Seed a fresh [`PairedPeers`] allowlist from the caller's fingerprint list.
pub(super) fn build_paired_peers(allowed: &[String]) -> PairedPeers {
    let peers = PairedPeers::new();
    for fp in allowed {
        // Display name is cosmetic here; the verifier only checks membership.
        peers.add(fp.clone(), "p2p-peer");
    }
    peers
}

/// Build a fresh `HashMap` of fingerprint → session key from the FFI list.
pub(super) fn build_session_key_map(session_keys: Vec<PeerSessionKey>) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::with_capacity(session_keys.len());
    for sk in session_keys {
        map.insert(sk.fingerprint, sk.session_key);
    }
    map
}
