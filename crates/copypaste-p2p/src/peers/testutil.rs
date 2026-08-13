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

/// A pairing as `pair_create` writes it: stored so the listener holds the key,
/// never contacted, so its code is still redeemable.
pub(super) fn unredeemed(name: &str) -> Peer {
    let token = PairingToken::generate();
    Peer {
        pairing_id: token.pairing_id(),
        name: name.to_string(),
        psk: token.psk(),
        last_addr: None,
        last_seen_ms: 0,
    }
}

/// The same pairing after a session has proved it works.
pub(super) fn redeemed(peer: &Peer, at_ms: i64) -> Peer {
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

/// Move an unredeemed pairing's deadline into the past, through the file, so
/// the test does not have to wait five minutes and the on-disk shape is
/// exercised on the way.
pub(super) fn age_out(path: &std::path::Path, pairing_id: &str) {
    let bytes = std::fs::read(path).expect("read");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    value["pending"][pairing_id] = serde_json::json!(1);
    std::fs::write(path, serde_json::to_vec(&value).expect("encode")).expect("write");
}
