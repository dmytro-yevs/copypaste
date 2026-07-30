//! The peer node: everything that has to be running for pairing and sync to
//! work, with no daemon around it.
//!
//! The rest of this crate provides the parts — a Noise channel, a last-write-
//! wins session, an mDNS browser — and deliberately knows nothing about a
//! database. This module owns the parts: the paired-device list, the
//! advertisement, the inbound listener and its concurrency limit, and the four
//! operations a user drives all of it with ([`Node::pair_create`],
//! [`Node::pair_accept`], [`Node::unpair`], [`Node::sync_one`]).
//!
//! It is generic over [`SyncSource`](crate::sync::SyncSource), which is the
//! whole of what it needs from a history. The desktop daemon supplies one over
//! its store; the Android app supplies one over the same core store in its own
//! process. Neither is a `Node` of its own — that is what put pairing out of
//! reach of everything except the daemon binary.
//!
//! # Discovery is a convenience, never a dependency
//!
//! `Discovery::start` returns `Ok` on a host with no multicast — a container,
//! or a corporate network with mDNS filtered. The peer list stays empty,
//! `online` reads false for everyone, and an explicit address still pairs and
//! still syncs. Nothing here treats a discovery failure as an error.

mod channel;
mod dial;
mod error;
mod listen;

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::discovery::{DiscoveredPeer, Discovery};
use crate::peers::{Peer, PeerStore};

pub use channel::{NoiseChannel, READ_TIMEOUT, SESSION_TIMEOUT};
pub use error::NodeError;
pub use listen::{bind, listen};

/// How many peers may be mid-session at once, inbound.
///
/// A session holds a database connection's worth of work and a decrypted batch
/// in memory, and a device has a handful of peers, not hundreds. Connections
/// past the limit are dropped without a handshake: refusing early tells an
/// unauthenticated dialler nothing it did not already know from the open port.
const MAX_CONCURRENT_PEER_SESSIONS: usize = 4;

/// A freshly minted pairing, and the one rendering of its secret.
///
/// `code` is shown to the user once and never stored, logged or retrievable
/// again — hence the redacting `Debug`.
pub struct NewPairing {
    pub code: String,
    pub pairing_id: String,
    /// Where the other device should dial, when this host has a routable
    /// address. `None` means the user has to supply one.
    pub listen_addr: Option<String>,
}

impl std::fmt::Debug for NewPairing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPairing")
            .field("pairing_id", &self.pairing_id)
            .field("code", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Everything the peer half of a device shares.
///
/// Constructed once and never rebuilt; no field is optional except
/// `discovery`, where `None` is a decision (the user turned LAN visibility
/// off) rather than a degraded state.
pub struct Node {
    peers: PeerStore,
    discovery: Option<Discovery>,
    /// The port [`listen`] binds, which is what a pairing tells a peer to dial.
    port: u16,
    sessions: Arc<Semaphore>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("port", &self.port)
            .field("peers", &self.peers.len())
            .finish_non_exhaustive()
    }
}

impl Node {
    #[must_use]
    pub fn new(peers: PeerStore, discovery: Option<Discovery>, port: u16) -> Self {
        Self {
            peers,
            discovery,
            port,
            sessions: Arc::new(Semaphore::new(MAX_CONCURRENT_PEER_SESSIONS)),
        }
    }

    #[must_use]
    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    #[must_use]
    pub fn discovery(&self) -> Option<&Discovery> {
        self.discovery.as_ref()
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Everything currently visible on the LAN. Empty when discovery is off or
    /// degraded, which is never an error: an explicit address always works.
    #[must_use]
    pub fn seen(&self) -> Vec<DiscoveredPeer> {
        self.discovery
            .as_ref()
            .map(Discovery::peers)
            .unwrap_or_default()
    }

    /// Whether a given pairing is visible right now.
    #[must_use]
    pub fn find(&self, pairing_id: &str) -> Option<DiscoveredPeer> {
        self.discovery.as_ref()?.find(pairing_id)
    }

    /// Re-advertise after the set of pairings changed.
    ///
    /// Best-effort by construction: a new pairing must not fail because mDNS is
    /// unavailable, and the address the user was given still works.
    pub fn republish(&self) {
        let ids: Vec<String> = self
            .peers
            .list()
            .into_iter()
            // Cloned rather than moved out: `Peer` has a `Drop` that wipes its
            // key, so its fields cannot be destructured.
            .map(|peer| peer.pairing_id.clone())
            .collect();
        let Some(discovery) = self.discovery.as_ref() else {
            return;
        };
        if let Err(e) = discovery.republish(&ids) {
            debug!(error = %e, "could not republish the discovery record");
        }
    }

    /// Where a peer should dial this device, when that can be determined.
    #[must_use]
    pub fn listen_addr(&self) -> Option<String> {
        Some(SocketAddr::new(crate::netif::routable_ip()?, self.port).to_string())
    }

    /// Mint a pairing and hand back the code to read out to the other device.
    ///
    /// The peer is stored *now*, before the other device has ever been heard
    /// from, because the pre-shared key has to be in the listener's candidate
    /// list before the other half dials in. `name` is a placeholder until the
    /// first session, which is where the peer's own device name arrives.
    pub fn pair_create(&self, name: &str) -> Result<NewPairing, NodeError> {
        let token = crate::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();

        self.peers
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: placeholder_name(name),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
            })
            .map_err(|e| {
                warn!(error = %e, "could not store a new pairing");
                NodeError::PeerStore
            })?;
        self.republish();

        Ok(NewPairing {
            // The one and only rendering of the secret. Not logged, not
            // stored, not retrievable again.
            code: token.to_code(),
            pairing_id,
            listen_addr: self.listen_addr(),
        })
    }

    /// Forget a peer. `Ok(false)` when there was no such pairing.
    ///
    /// Local and unilateral: the other device keeps its half until it also
    /// unpairs, which is why this cannot fail on an unreachable peer. What it
    /// does do is remove the pre-shared key from the listener's candidate list,
    /// so that pairing can no longer authenticate here.
    pub fn unpair(&self, pairing_id: &str) -> Result<bool, NodeError> {
        let removed = self.peers.remove(pairing_id).map_err(|e| {
            warn!(error = %e, "could not remove a pairing");
            NodeError::PeerStore
        })?;
        if removed {
            self.republish();
        }
        Ok(removed)
    }

    /// Record that a session with this peer succeeded.
    ///
    /// The name comes off the wire and is cosmetic — never an identity — so it
    /// is only taken when the peer offered one.
    fn touch_peer(&self, peer: &Peer, addr: Option<SocketAddr>, name: Option<&str>) {
        let updated = Peer {
            pairing_id: peer.pairing_id.clone(),
            name: match name {
                Some(name) if !name.trim().is_empty() => name.to_string(),
                _ => peer.name.clone(),
            },
            // `[u8; 32]` is `Copy`, so this reads the field rather than moving
            // out of a type that has a `Drop` (see `peers::Peer`).
            psk: peer.psk,
            last_addr: addr.or(peer.last_addr),
            last_seen_ms: crate::now_ms(),
        };
        if let Err(e) = self.peers.upsert(updated) {
            warn!(error = %e, "could not record a successful peer session");
        }
    }
}

/// A peer always has *some* name: the list is unreadable otherwise, and the
/// real one arrives with the first hello.
#[must_use]
pub fn placeholder_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "unnamed device".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> (Node, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let peers = PeerStore::open(&dir.path().join("peers.json")).unwrap();
        (Node::new(peers, None, 0), dir)
    }

    #[test]
    fn a_new_pairing_is_stored_before_the_other_device_dials_in() {
        let (node, _dir) = node();
        let pairing = node.pair_create("laptop").unwrap();

        assert!(!pairing.code.is_empty());
        assert_ne!(pairing.code, pairing.pairing_id);

        let token = crate::transport::PairingToken::parse(&pairing.code).expect("a valid code");
        assert_eq!(token.pairing_id(), pairing.pairing_id);
        let stored = node
            .peers()
            .get(&pairing.pairing_id)
            .expect("the peer must be stored immediately");
        assert!(stored.psk_matches(&token.psk()));
        assert_eq!(stored.name, "laptop");
    }

    /// The code is a secret; nothing may render it but `to_code`.
    #[test]
    fn the_pairing_code_is_redacted_in_debug_output() {
        let (node, _dir) = node();
        let pairing = node.pair_create("laptop").unwrap();
        let rendered = format!("{pairing:?}");
        assert!(!rendered.contains(&pairing.code), "{rendered}");
        assert!(rendered.contains(&pairing.pairing_id));
    }

    #[test]
    fn unpairing_removes_the_key_from_the_listeners_candidates() {
        let (node, _dir) = node();
        let pairing = node.pair_create("laptop").unwrap();
        assert_eq!(node.peers().psks().len(), 1);

        assert!(node.unpair(&pairing.pairing_id).unwrap());
        assert!(node.peers().psks().is_empty());
        assert!(!node.unpair(&pairing.pairing_id).unwrap());
    }

    #[test]
    fn two_pairings_are_independent() {
        let (node, _dir) = node();
        let first = node.pair_create("a").unwrap();
        let second = node.pair_create("b").unwrap();
        assert_ne!(first.pairing_id, second.pairing_id);
        assert_eq!(node.peers().len(), 2);
    }

    #[test]
    fn a_missing_name_still_reads_as_something() {
        assert_eq!(placeholder_name("   "), "unnamed device");
        assert_eq!(placeholder_name(" laptop "), "laptop");
    }

    #[test]
    fn a_listen_address_is_never_loopback() {
        let (node, _dir) = node();
        if let Some(addr) = node.listen_addr() {
            let parsed: SocketAddr = addr.parse().expect("a socket address");
            assert!(!parsed.ip().is_loopback(), "{addr}");
        }
    }

    /// Nothing is visible without discovery, and that is a normal answer.
    #[test]
    fn a_node_without_discovery_sees_nobody() {
        let (node, _dir) = node();
        assert!(node.seen().is_empty());
        assert!(node.find("whoever").is_none());
        // Republishing must not panic or fail when there is nothing to publish.
        node.republish();
    }
}
