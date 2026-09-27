//! Fixtures shared by the peer-store test modules. Test-only.

use std::path::PathBuf;

use super::{Peer, DEFAULT_FILE_NAME};
use crate::transport::PairingToken;

/// A plausible peer record, with a real token behind it so the pairing id and
/// the PSK are consistent with each other.
pub(super) fn peer(name: &str) -> Peer {
    let token = PairingToken::generate();
    Peer {
        pairing_id: token.pairing_id(),
        name: name.to_string(),
        psk: token.psk(),
        last_addr: Some("192.168.1.7:47654".parse().expect("addr")),
        last_seen_ms: 1_753_900_000_000,
    }
}

/// The same peer with a later successful-contact timestamp.
pub(super) fn with_last_seen(peer: &Peer, at_ms: i64) -> Peer {
    Peer {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        psk: peer.psk,
        last_addr: peer.last_addr,
        last_seen_ms: at_ms,
    }
}

pub(super) fn store_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join(DEFAULT_FILE_NAME)
}
