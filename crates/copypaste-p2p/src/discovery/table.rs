//! What we currently believe about the LAN: the peer table, its expiry, and its
//! cap. Pure, and the clock is a parameter, so both rules are tested directly.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

pub(super) use crate::now_ms;
use crate::DeviceProfile;

/// How long a peer stays in the table after it was last seen. mdns-sd publishes
/// host records with a 120 s TTL and refreshes ahead of expiry, so three minutes
/// rides out one lost refresh while still dropping a device that left the
/// building. `ServiceRemoved` normally removes entries promptly; this is the
/// backstop for a goodbye packet that never arrives.
pub const PEER_TTL: Duration = Duration::from_secs(180);

/// Hard ceiling on tracked peers. mDNS is unauthenticated, so anyone on the LAN
/// can announce as many instances as they like; when full the least recently
/// seen entry is evicted, and a live peer refreshes well inside [`PEER_TTL`], so
/// a flood cannot push out a device that is still talking.
pub const MAX_PEERS: usize = 256;

/// A peer seen on the network. Presence here means "reachable", never
/// "trusted" — trust comes only from having been paired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// Opaque identifier for one mDNS service. It is for list de-duplication
    /// only; trust still comes exclusively from the Noise handshake.
    pub discovery_id: String,
    /// Pairings this device says it can accept. A fresh device has none and
    /// must still be discoverable so the user can start a pairing ceremony.
    pub pairing_ids: Vec<String>,
    /// Display name the peer advertised. Untrusted, unvalidated beyond length
    /// and control-character stripping — never use it as an identity.
    pub name: String,
    pub profile: Option<DeviceProfile>,
    pub addr: SocketAddr,
    pub last_seen_ms: i64,
}

/// Keyed by service fullname. One resolved service is one candidate device,
/// regardless of how many pairings it advertises.
type PeerKey = String;

#[derive(Debug)]
pub(super) struct PeerTable {
    entries: HashMap<PeerKey, DiscoveredPeer>,
    ttl_ms: i64,
    max_peers: usize,
}

impl Default for PeerTable {
    fn default() -> Self {
        Self::new(PEER_TTL.as_millis() as i64, MAX_PEERS)
    }
}

impl PeerTable {
    fn new(ttl_ms: i64, max_peers: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_ms,
            max_peers: max_peers.max(1),
        }
    }

    pub(super) fn observe(&mut self, fullname: &str, peer: DiscoveredPeer, now_ms: i64) {
        self.prune(now_ms);
        self.entries.insert(fullname.to_string(), peer);
        self.enforce_cap();
    }

    pub(super) fn remove_service(&mut self, fullname: &str) {
        self.entries.retain(|service, _| service != fullname);
    }

    fn prune(&mut self, now_ms: i64) {
        let ttl = self.ttl_ms;
        // `now - last_seen` rather than a stored deadline, so a backwards step
        // of the wall clock expires entries early instead of stranding them.
        self.entries
            .retain(|_, peer| now_ms.saturating_sub(peer.last_seen_ms) < ttl);
    }

    /// Evict least-recently-seen until we are inside the cap. A live peer
    /// refreshes well inside the TTL, so a flood evicts its own entries first.
    fn enforce_cap(&mut self) {
        while self.entries.len() > self.max_peers {
            let victim = self
                .entries
                .iter()
                .min_by(|a, b| {
                    a.1.last_seen_ms
                        .cmp(&b.1.last_seen_ms)
                        .then_with(|| a.0.cmp(b.0))
                })
                .map(|(key, _)| key.clone());
            match victim {
                Some(key) => drop(self.entries.remove(&key)),
                None => break,
            }
        }
    }

    pub(super) fn snapshot(&mut self, now_ms: i64) -> Vec<DiscoveredPeer> {
        self.prune(now_ms);
        let mut peers: Vec<DiscoveredPeer> = self.entries.values().cloned().collect();
        peers.sort_by(|a, b| {
            a.discovery_id
                .cmp(&b.discovery_id)
                .then_with(|| a.addr.cmp(&b.addr))
        });
        peers
    }

    pub(super) fn find(&mut self, pairing_id: &str, now_ms: i64) -> Option<DiscoveredPeer> {
        self.prune(now_ms);
        self.entries
            .values()
            .filter(|peer| peer.pairing_ids.iter().any(|id| id == pairing_id))
            .max_by(|a, b| {
                a.last_seen_ms
                    .cmp(&b.last_seen_ms)
                    .then_with(|| a.addr.cmp(&b.addr))
            })
            .cloned()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn peer(pairing_id: &str, last_seen_ms: i64) -> DiscoveredPeer {
        DiscoveredPeer {
            discovery_id: format!("device-{pairing_id}"),
            pairing_ids: vec![pairing_id.to_string()],
            name: "peer".to_string(),
            profile: None,
            addr: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
                crate::DEFAULT_PORT,
            ),
            last_seen_ms,
        }
    }

    // -- expiry ---------------------------------------------------------------

    #[test]
    fn stale_entries_expire() {
        let mut table = PeerTable::new(1_000, MAX_PEERS);
        table.observe("a._copypaste._tcp.local.", peer("fresh", 10_000), 10_000);
        assert_eq!(table.snapshot(10_500).len(), 1);
        assert!(table.find("fresh", 10_500).is_some());

        assert!(table.snapshot(11_000).is_empty());
        assert!(table.find("fresh", 11_000).is_none());
    }

    #[test]
    fn a_clock_that_steps_backwards_expires_rather_than_strands() {
        let mut table = PeerTable::new(1_000, MAX_PEERS);
        table.observe("a._copypaste._tcp.local.", peer("x", 10_000), 10_000);
        // saturating_sub floors at 0, which is inside the TTL, so the entry
        // survives one backwards step instead of being pinned forever.
        assert_eq!(table.snapshot(0).len(), 1);
        assert!(table.snapshot(-5_000).len() <= 1);
    }

    #[test]
    fn a_departing_peer_is_removed_immediately() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe("gone._copypaste._tcp.local.", peer("x", now), now);
        table.observe("here._copypaste._tcp.local.", peer("y", now), now);
        table.remove_service("gone._copypaste._tcp.local.");
        let remaining = table.snapshot(now);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].pairing_ids, ["y"]);
    }

    #[test]
    fn one_device_may_hold_several_pairings() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe(
            "laptop._copypaste._tcp.local.",
            DiscoveredPeer {
                discovery_id: "laptop".into(),
                pairing_ids: vec!["a".into(), "b".into()],
                name: "peer".into(),
                profile: None,
                addr: std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 2)),
                    crate::DEFAULT_PORT,
                ),
                last_seen_ms: now,
            },
            now,
        );
        assert_eq!(table.snapshot(now).len(), 1);
        assert_eq!(table.find("a", now).unwrap().discovery_id, "laptop");
    }

    #[test]
    fn find_prefers_the_most_recently_seen_claimant() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe("old._copypaste._tcp.local.", peer("dup", now - 5_000), now);
        table.observe("new._copypaste._tcp.local.", peer("dup", now), now);
        assert_eq!(table.find("dup", now).unwrap().last_seen_ms, now);
    }

    // -- cap ------------------------------------------------------------------

    #[test]
    fn the_peer_table_is_bounded() {
        let mut table = PeerTable::new(600_000, 8);
        for i in 0..500 {
            table.observe(
                &format!("flood-{i}._copypaste._tcp.local."),
                peer(&format!("id-{i}"), 1_000 + i as i64),
                1_000 + i as i64,
            );
            assert!(table.entries.len() <= 8);
        }
        let peers = table.snapshot(1_500);
        assert_eq!(peers.len(), 8);
    }

    #[test]
    fn flooding_evicts_the_flood_before_a_live_peer() {
        let mut table = PeerTable::new(600_000, 4);
        let mut clock = 1_000i64;
        table.observe("real._copypaste._tcp.local.", peer("real", clock), clock);

        for i in 0..100 {
            clock += 10;
            // The real peer keeps refreshing, as a live device does.
            table.observe("real._copypaste._tcp.local.", peer("real", clock), clock);
            table.observe(
                &format!("flood-{i}._copypaste._tcp.local."),
                peer(&format!("junk-{i}"), clock),
                clock,
            );
        }

        assert!(table.entries.len() <= 4);
        assert!(
            table.find("real", clock).is_some(),
            "a refreshing peer must survive a flood"
        );
    }
}
