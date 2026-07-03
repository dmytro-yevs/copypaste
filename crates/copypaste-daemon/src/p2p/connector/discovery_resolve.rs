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
/// connection state.
///
/// # Correlation key (CopyPaste-8ebg.27)
/// Peers paired since [`crate::peers::PairedDevice::device_id`] was
/// introduced carry the peer's stable mDNS device UUID, learned in-band at
/// pairing time (`PeerMeta::device_id`). When present, that UUID is matched
/// directly against each discovered peer's `device_id` — this is stable
/// across DHCP renewal / network roaming, unlike the persisted IP.
///
/// Legacy peers paired before this field existed have `device_id == None`;
/// for those we fall back to the OLD IP-correlation heuristic (find the
/// discovered peer whose `ip_addrs` still contains the stale persisted IP).
/// That fallback only self-heals a port change on the SAME IP — it can never
/// heal an IP change — but it is the best available signal without a stable
/// identifier, and preserves prior behavior for peers that have not been
/// re-paired.
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

    // Look up the persisted record's stable device_id (if any) so we can
    // re-key on it instead of the (possibly stale) IP.
    let persisted_device_id = crate::peers::load_peers(peers_path)
        .into_iter()
        .find(|p| {
            crate::ipc::canonical_fingerprint(&p.fingerprint)
                == crate::ipc::canonical_fingerprint(fingerprint)
        })
        .and_then(|p| p.device_id);

    let discovered = match persisted_device_id {
        Some(device_id) if !device_id.is_empty() => {
            // Stable-key match: same mDNS device_id, regardless of IP.
            match discovery.resolve_peer(&device_id) {
                Some(p) => Some(p),
                // No match under the stable key — do NOT silently fall back to
                // IP correlation here: a device_id is present, so IP drift is
                // exactly the case this path exists to heal, and falling back
                // to IP would just reproduce the original bug for this peer.
                None => None,
            }
        }
        // Legacy peer (paired before device_id was persisted) — fall back to
        // the original IP-correlation heuristic.
        _ => discovery
            .peers()
            .into_iter()
            .find(|p| p.ip_addrs.contains(&want_ip)),
    };

    let Some(discovered) = discovered else {
        // Peer not in the current mDNS snapshot — nothing to refresh.
        return;
    };

    // Prefer the freshly discovered IP (heals IP drift for device_id-matched
    // peers); fall back to the first advertised IPv4/any address, and finally
    // to the persisted IP if the discovery record has no addresses at all.
    let fresh_ip = discovered
        .ip_addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| discovered.ip_addrs.first())
        .copied()
        .unwrap_or(want_ip);

    let fresh_addr = SocketAddr::new(fresh_ip, discovered.port);
    let fresh_name = discovered.device_name.as_str();
    let local_ip_str = fresh_ip.to_string();

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

    // ── CopyPaste-8ebg.27: device_id-keyed refresh survives IP changes ──────

    fn make_paired_device(
        fingerprint: &str,
        address: &str,
        device_id: Option<&str>,
    ) -> crate::peers::PairedDevice {
        crate::peers::PairedDevice {
            fingerprint: fingerprint.to_string(),
            name: String::new(),
            added_at: 1_700_000_000,
            address: Some(address.to_string()),
            sync_key_b64: None,
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_id: device_id.map(str::to_string),
            public_ip: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: None,
            password_file_enc: None,
            supabase_account_id: None,
        }
    }

    /// The bug this fix addresses: when the persisted peer has a stored
    /// `device_id`, `refresh_peer_meta_from_discovery` must find it under its
    /// NEW IP (DHCP renewal / roaming) via the stable device_id match — the
    /// old IP-only correlation would return `None` here and silently no-op.
    #[test]
    fn refresh_by_device_id_heals_ip_change() {
        use copypaste_p2p::discovery::PeerInfo;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[make_paired_device(
                "aa:bb:cc:dd",
                "192.168.1.50:5000", // stale IP — peer has since roamed
                Some("device-uuid-123"),
            )],
        )
        .unwrap();

        let discovery = DiscoveryService::new();
        // Peer re-announces under a brand-new IP after DHCP renewal, but the
        // same stable mDNS device_id.
        discovery.inject_peer_for_test(
            "alice.local.",
            PeerInfo {
                device_id: "device-uuid-123".to_string(),
                device_name: "Alice".to_string(),
                ip_addrs: vec!["192.168.9.77".parse().unwrap()],
                port: 6000,
                bport: None,
            },
        );

        let persisted_addr: SocketAddr = "192.168.1.50:5000".parse().unwrap();
        refresh_peer_meta_from_discovery(&path, "aabbccdd", persisted_addr, &discovery);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(
            loaded[0].address.as_deref(),
            Some("192.168.9.77:6000"),
            "device_id-keyed refresh must adopt the peer's new IP+port"
        );
        assert_eq!(loaded[0].local_ip.as_deref(), Some("192.168.9.77"));
    }

    /// Legacy peers (paired before `device_id` was persisted, so it is `None`)
    /// must keep working via the old IP-correlation fallback: a port change on
    /// the SAME IP is still healed.
    #[test]
    fn refresh_falls_back_to_ip_for_legacy_peer_without_device_id() {
        use copypaste_p2p::discovery::PeerInfo;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[make_paired_device("aa:bb:cc:ee", "192.168.1.60:5001", None)],
        )
        .unwrap();

        let discovery = DiscoveryService::new();
        discovery.inject_peer_for_test(
            "bob.local.",
            PeerInfo {
                device_id: "some-other-uuid".to_string(),
                device_name: "Bob".to_string(),
                ip_addrs: vec!["192.168.1.60".parse().unwrap()],
                port: 7000, // port drifted, same IP
                bport: None,
            },
        );

        let persisted_addr: SocketAddr = "192.168.1.60:5001".parse().unwrap();
        refresh_peer_meta_from_discovery(&path, "aabbccee", persisted_addr, &discovery);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(
            loaded[0].address.as_deref(),
            Some("192.168.1.60:7000"),
            "legacy IP-correlation fallback must still heal a port change"
        );
    }
}
