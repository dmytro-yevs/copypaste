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

pub(super) fn store_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join(DEFAULT_FILE_NAME)
}
