//! mDNS-discovery address resolution/refresh helpers used by the connector
//! loop's dial-failure and steady-state paths.
//!
//! Split out of the former flat `p2p/connector.rs` (ADR-017,
//! CopyPaste-vp63.48) — moved verbatim, no behavior change.

use std::net::SocketAddr;

use copypaste_p2p::discovery::DiscoveryService;

/// Resolve a fresh dial address for `fingerprint` from the mDNS discovery
/// service.
///
/// Iterates the current snapshot of discovered peers and returns a
/// `SocketAddr` for the first peer whose `device_id` matches `fingerprint`
/// (exact string match after the caller has already normalised both sides to
/// canonical form).  The first IPv4 address is preferred over IPv6 to maximise
/// compatibility; if only IPv6 addresses are present the first one is used.
///
/// Returns `None` when:
/// - discovery has no peers at all,
/// - no peer's `device_id` matches `fingerprint`, or
/// - the matching peer has an empty `ip_addrs` list.
///
/// This is **best-effort**: the discovery snapshot may be stale (mDNS
/// re-announcement period is typically 1–5 minutes) and is never guaranteed to
/// reflect a peer's current address.  The connector must not rely on it as the
/// sole source of truth — it is a fallback consulted only after a persisted
/// address fails.
pub(in crate::p2p) fn resolve_addr_from_discovery(
    discovery: &DiscoveryService,
    fingerprint: &str,
) -> Option<SocketAddr> {
    // `resolve_peer` matches by device_id (already the right semantic).
    let peer = discovery.resolve_peer(fingerprint)?;
    // Prefer IPv4 for broadest compatibility; fall back to the first address
    // regardless of family if no IPv4 is found.  `ip_addrs` is sorted IPv4-
    // first by `peer_from_resolved`, so `find` over a non-empty vec is O(n)
    // with n typically ≤ 2.
    let ip = peer
        .ip_addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| peer.ip_addrs.first())?;
    Some(SocketAddr::new(*ip, peer.port))
}

/// IP-correlated fallback for [`resolve_addr_from_discovery`].
///
/// The device_id-keyed lookup above never matches a real peer: mDNS advertises
/// a per-device UUID as `device_id`, but a paired peer is keyed by its SHA-256
/// cert fingerprint — the two are different strings, so `resolve_peer` returns
/// `None` and the connector keeps dialing a stale persisted port forever.
///
/// On a LAN the host IP uniquely identifies a peer, so when the persisted dial
/// address fails we correlate by IP instead: find the discovered peer that
/// advertises the same IP as the address that just failed and adopt its freshly
/// announced sync port. This is what self-heals the common failure mode — both
/// peers bind an **ephemeral** sync-listener port that drifts on every
/// daemon/app restart, leaving the port persisted at pairing time stale.
pub(in crate::p2p) fn resolve_addr_from_discovery_by_ip(
    discovery: &DiscoveryService,
    failed_addr: SocketAddr,
) -> Option<SocketAddr> {
    let want_ip = failed_addr.ip();
    discovery
        .peers()
        .into_iter()
        .find(|p| p.ip_addrs.contains(&want_ip))
        .map(|p| SocketAddr::new(want_ip, p.port))
}

/// Proactively refresh a paired peer's `name`, `address`, and `local_ip` from
/// the live mDNS discovery snapshot.
///
/// Called every connector tick for each dialable peer, regardless of
/// connection state.  Correlates by the IP component of the peer's persisted
/// `address` — the mDNS `device_id` is a UUID, never a cert fingerprint, so
/// fingerprint-keyed lookup ([`resolve_addr_from_discovery`]) would never match.
///
/// When a matching mDNS peer is found and any of its fields (name, sync port,
/// IP) differ from what is persisted, [`crate::peers::update_peer_meta`] rewrites
/// `peers.json` in place (atomic 0600 rename).  The next [`crate::ipc`]
/// `list_peers` poll then surfaces the fresh values to the UI.
///
/// # Out-of-scope fields
/// `model`, `os_version`, `app_version`, and `public_ip` are learned in-band
/// over the bootstrap channel at pairing time and are NOT carried by mDNS TXT
/// records — they are untouched here.  Refreshing them reactively would require
/// a new wire-protocol extension and is deferred to a future release.
pub(in crate::p2p) fn refresh_peer_meta_from_discovery(
    peers_path: &std::path::Path,
    fingerprint: &str,
    persisted_addr: SocketAddr,
    discovery: &DiscoveryService,
) {
    let want_ip = persisted_addr.ip();
    let Some(discovered) = discovery
        .peers()
        .into_iter()
        .find(|p| p.ip_addrs.contains(&want_ip))
    else {
        // Peer not in the current mDNS snapshot — nothing to refresh.
        return;
    };

    let fresh_addr = SocketAddr::new(want_ip, discovered.port);
    let fresh_name = discovered.device_name.as_str();
    let local_ip_str = want_ip.to_string();

    match crate::peers::update_peer_meta(
        peers_path,
        fingerprint,
        fresh_name,
        fresh_addr,
        &local_ip_str,
    ) {
        Ok(true) => {
            tracing::debug!(
                %fingerprint,
                %fresh_addr,
                name = %fresh_name,
                "connector: refreshed peer name+addr from mDNS"
            );
        }
        Ok(false) => {} // Nothing changed — no log noise.
        Err(e) => {
            tracing::warn!(
                %fingerprint,
                error = %e,
                "connector: failed to persist mDNS meta refresh"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mDNS address refresh (P2P audit P2 #3) ───────────────────────────────

    /// `resolve_addr_from_discovery` returns `None` when the discovery service
    /// has no matching peer (empty).
    #[test]
    fn resolve_addr_from_discovery_returns_none_when_empty() {
        let discovery = DiscoveryService::new();
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(
            result.is_none(),
            "empty discovery must yield None for any fingerprint"
        );
    }

    /// `resolve_addr_from_discovery` returns `None` when no discovered peer
    /// has a matching `device_id` (fingerprint).
    #[test]
    fn resolve_addr_from_discovery_returns_none_for_unknown_peer() {
        use copypaste_p2p::discovery::PeerInfo;

        let discovery = DiscoveryService::new();
        // Manually inject a peer with a different fingerprint via on_peer_found
        // callback simulation: insert directly into known_peers (test-internal).
        discovery.inject_peer_for_test(
            "bob.local.",
            PeerInfo {
                device_id: "1122334455".to_string(),
                device_name: "Bob".to_string(),
                ip_addrs: vec!["192.168.1.10".parse().unwrap()],
                port: 51000,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(result.is_none(), "non-matching peer must yield None");
    }

    /// `resolve_addr_from_discovery` returns a valid `SocketAddr` when a
    /// discovered peer's `device_id` matches the queried fingerprint and it has
    /// at least one routable IP address.
    #[test]
    fn resolve_addr_from_discovery_returns_addr_for_matching_peer() {
        use copypaste_p2p::discovery::PeerInfo;

        let discovery = DiscoveryService::new();
        discovery.inject_peer_for_test(
            "alice.local.",
            PeerInfo {
                device_id: "aabbccdd".to_string(),
                device_name: "Alice".to_string(),
                ip_addrs: vec!["192.168.1.99".parse().unwrap()],
                port: 51515,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "aabbccdd");
        assert!(result.is_some(), "matching peer must yield Some addr");
        let addr = result.unwrap();
        assert_eq!(addr.port(), 51515);
        assert_eq!(addr.ip().to_string(), "192.168.1.99");
    }

    /// `resolve_addr_from_discovery` prefers IPv4 over IPv6 when both are
    /// present (IPv4 is listed first after the sort in `peer_from_resolved`).
    #[test]
    fn resolve_addr_from_discovery_prefers_ipv4() {
        use copypaste_p2p::discovery::PeerInfo;

        let discovery = DiscoveryService::new();
        discovery.inject_peer_for_test(
            "carol.local.",
            PeerInfo {
                device_id: "ccddee".to_string(),
                device_name: "Carol".to_string(),
                ip_addrs: vec!["192.168.2.5".parse().unwrap(), "::1".parse().unwrap()],
                port: 9000,
                bport: None,
            },
        );
        let result = resolve_addr_from_discovery(&discovery, "ccddee");
        assert!(result.is_some());
        let addr = result.unwrap();
        assert!(!addr.ip().is_ipv6(), "must prefer IPv4 when available");
    }
}
