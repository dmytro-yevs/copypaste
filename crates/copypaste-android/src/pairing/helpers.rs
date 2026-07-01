//! Free helpers shared by the discovery-pairing FFI surface (`ffi_pairing.rs`).

use copypaste_p2p::discovery::PeerInfo;

use crate::CopypasteError;

use super::state::PairingState;

/// Pick the IPv4-first resolvable `host:port` to dial for a discovered peer.
///
/// mDNS often resolves both IPv4 and IPv6 (incl. link-local) addresses; the
/// bootstrap dialer wants a single routable address. Prefer IPv4 (most reliable
/// on consumer LANs / Android), fall back to the first address. Returns `None`
/// when the peer advertised no addresses.
pub fn ipv4_first_addr(peer: &PeerInfo) -> Option<std::net::SocketAddr> {
    let ip = peer
        .ip_addrs
        .iter()
        .find(|ip| ip.is_ipv4())
        .or_else(|| peer.ip_addrs.first())?;
    Some(std::net::SocketAddr::new(
        *ip,
        peer.bport.unwrap_or(peer.port),
    ))
}

/// Map a `pair_with_discovered` failure outcome (handshake error, timeout, or
/// rejection) onto a terminal [`PairingState`] for the coordinator. A handshake
/// `Err` from a confirm-rejected SAS is reported as `Rejected`; everything else
/// (network/PAKE/MitM failure) is `Aborted`. Used by the spawned initiator task.
pub fn outcome_for_initiator_error(rejected: bool) -> PairingState {
    if rejected {
        PairingState::Rejected
    } else {
        PairingState::Aborted
    }
}

/// Build a `CopypasteError::P2pError` with a fixed reason (helper so the FFI
/// surface never constructs the variant inline at multiple call sites).
pub fn p2p_err(reason: impl Into<String>) -> CopypasteError {
    CopypasteError::P2pError {
        reason: reason.into(),
    }
}
