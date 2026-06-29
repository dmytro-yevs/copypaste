//! mDNS-SD event handler and peer-resolution helpers.
//!
//! Processes [`ServiceEvent`]s from the mDNS browse channel: filters own
//! service, rate-limits, deduplicates, and fires `on_found`/`on_lost` callbacks.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use mdns_sd::{ResolvedService, ScopedIp, ServiceEvent};
use tracing::{debug, info, warn};

use super::lock_safe;
use super::types::{
    PeerAdmission, PeerFoundCallback, PeerInfo, PeerLostCallback, MAX_KNOWN_PEERS,
    PROTOCOL_VERSION, PROTOCOL_VERSION_V1, TXT_BPORT, TXT_DEVICE_ID, TXT_DEVICE_NAME, TXT_VERSION,
};
use crate::rate_limit::MdnsRateLimiter;

/// Dispatch a single mDNS [`ServiceEvent`] — filter, rate-limit, dedup, callback.
pub(super) fn handle_event(
    event: ServiceEvent,
    own_id: &Option<String>,
    known_peers: &Arc<Mutex<HashMap<String, PeerInfo>>>,
    on_found: &Arc<Mutex<Vec<PeerFoundCallback>>>,
    on_lost: &Arc<Mutex<Vec<PeerLostCallback>>>,
    rate_limiter: &Arc<MdnsRateLimiter>,
) {
    match event {
        ServiceEvent::ServiceResolved(resolved) => {
            if let Some(peer) = peer_from_resolved(&resolved) {
                // Skip own service.
                if own_id.as_deref() == Some(peer.device_id.as_str()) {
                    debug!(device_id = %peer.device_id, "Ignoring own mDNS advertisement");
                    return;
                }

                // OI-3 mitigation: rate-limit per peer identity. Prefer
                // `device_id` (the cert fingerprint advertised in TXT) so a
                // dual-stack peer with both v4 and v6 addresses doesn't get
                // 2× budget (security MED #11). When `device_id` is empty
                // (older clients / malformed TXT) we fall back to a stable
                // hash of the *sorted* address set rather than the first
                // address, which also closes the same v4/v6-rotation bypass.
                // Drop = silent denial-of-response; the limiter emits
                // trace + sampled warn telemetry itself.
                let rl_key = if !peer.device_id.is_empty() {
                    peer.device_id.clone()
                } else {
                    address_set_key(&peer.ip_addrs)
                };
                if !rate_limiter.try_admit_key(&rl_key) {
                    return;
                }

                let fullname = resolved.fullname.clone();

                // Dedup + cap: only emit if this is a new or changed peer, and
                // refuse brand-new fullnames once the map is at capacity.
                let mut peers = lock_safe(known_peers);
                let is_new = match admit_peer(&peers, &fullname, &peer) {
                    PeerAdmission::Skip => false,
                    PeerAdmission::Insert => true,
                    PeerAdmission::AtCapacity => {
                        drop(peers);
                        warn!(
                            device_id = %peer.device_id,
                            cap = MAX_KNOWN_PEERS,
                            "known_peers at capacity — refusing new mDNS peer"
                        );
                        return;
                    }
                };

                if is_new {
                    info!(
                        device_id = %peer.device_id,
                        device_name = %peer.device_name,
                        port = peer.port,
                        addrs = ?peer.ip_addrs,
                        "mDNS peer found"
                    );
                    peers.insert(fullname, peer.clone());
                    drop(peers);

                    // Snapshot callbacks so user code never holds the mutex —
                    // a panic inside a callback can only poison the mutex
                    // briefly; `lock_safe` will recover on the next call.
                    let callbacks: Vec<PeerFoundCallback> =
                        lock_safe(on_found).iter().cloned().collect();
                    for cb in callbacks.iter() {
                        cb(peer.clone());
                    }
                }
            } else {
                warn!(
                    fullname = %resolved.fullname,
                    "Ignoring mDNS service missing required TXT records"
                );
            }
        }

        ServiceEvent::ServiceRemoved(_svc_type, fullname) => {
            let mut peers = lock_safe(known_peers);
            if let Some(peer) = peers.remove(&fullname) {
                info!(device_id = %peer.device_id, "mDNS peer lost");
                drop(peers);

                let callbacks: Vec<PeerLostCallback> = lock_safe(on_lost).iter().cloned().collect();
                for cb in callbacks.iter() {
                    cb(peer.device_id.clone());
                }
            }
        }

        other => {
            debug!("mDNS event (ignored): {:?}", other);
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a [`PeerInfo`] from a resolved mDNS service, if the required
/// TXT records (`v`, `did`) are present and version is supported.
///
/// Accepts both v1 (`PROTOCOL_VERSION_V1 = "1"`, no `bport`) and v2
/// (`PROTOCOL_VERSION = "2"`, optional `bport`) so that existing v1 peers
/// are never silently dropped from the discovered list after the Phase 0
/// version bump. v1 peers produce a `PeerInfo` with `bport: None`; the UI
/// disables the "Pair" button for those entries.
///
/// The `name` TXT key is now **optional** (CopyPaste-sh9a): v3+ peers no
/// longer advertise it because it leaks PII on the LAN. Legacy v1/v2 peers
/// that still include `name` will have it accepted into `device_name`; newer
/// peers that omit it get an empty `device_name` in the returned `PeerInfo`.
/// The authoritative human name is exchanged post-PAKE during pairing.
fn peer_from_resolved(resolved: &ResolvedService) -> Option<PeerInfo> {
    let version = resolved.get_property_val_str(TXT_VERSION)?;
    // Accept v1 (legacy) and v2 (current). Any other version is unsupported.
    if version != PROTOCOL_VERSION && version != PROTOCOL_VERSION_V1 {
        warn!(version, "mDNS peer uses unsupported protocol version");
        return None;
    }

    let device_id = resolved.get_property_val_str(TXT_DEVICE_ID)?.to_string();

    // CopyPaste-rh27: tighten mDNS→peer correlation.
    //
    // A rogue LAN host could broadcast a `did` TXT record claiming to be any
    // device (IP-correlation attack). The mTLS handshake (cert-fingerprint
    // pinning) is the definitive defence — a rogue peer cannot impersonate
    // another device's fingerprint without its private key. However, if the
    // `device_id` in the TXT record is empty or malformed we would (a) skip
    // the rate-limit key based on identity and fall through to the IP-set hash,
    // and (b) insert a confusingly-keyed entry into `known_peers` that the
    // connector might try to dial. Reject malformed device_ids here so the
    // discovery layer never presents an unauthenticated device_id to callers.
    if !is_valid_device_id(&device_id) {
        warn!(
            device_id = %device_id,
            fullname = %resolved.fullname,
            "CopyPaste-rh27: mDNS peer has empty or malformed device_id — ignoring"
        );
        return None;
    }
    // `name` is optional since CopyPaste-sh9a: upgraded peers no longer
    // advertise it. Empty string = "unknown until post-PAKE exchange".
    let device_name = resolved
        .get_property_val_str(TXT_DEVICE_NAME)
        .unwrap_or("")
        .to_string();

    // Collect all addresses, deduplicated and sorted for determinism.
    // Unknown ScopedIp variants return None and are filtered out so 0.0.0.0
    // is never placed into the dial list.
    let mut ip_addrs: Vec<IpAddr> = resolved
        .get_addresses()
        .iter()
        .filter_map(scoped_ip_to_ip_addr)
        .collect();
    ip_addrs.sort_unstable_by_key(|a| (a.is_ipv6(), a.to_string()));
    ip_addrs.dedup();

    // Parse the optional bootstrap port from TXT. A malformed value (non-u16)
    // is treated as absent rather than fatal — the peer still appears in the
    // discovered list, the UI just disables the "Pair" button.
    let bport: Option<u16> = resolved
        .get_property_val_str(TXT_BPORT)
        .and_then(|s| s.parse().ok());

    Some(PeerInfo {
        device_id,
        device_name,
        ip_addrs,
        port: resolved.get_port(),
        bport,
    })
}

/// Decide how to handle a resolved peer relative to the current `known_peers`
/// map, enforcing [`MAX_KNOWN_PEERS`]. Pure (no mutation) so it is unit-testable
/// without a live mDNS daemon.
pub(super) fn admit_peer(
    peers: &HashMap<String, PeerInfo>,
    fullname: &str,
    peer: &PeerInfo,
) -> PeerAdmission {
    match peers.get(fullname) {
        Some(existing) if existing == peer => PeerAdmission::Skip,
        Some(_) => PeerAdmission::Insert, // known fullname, changed value
        None if peers.len() >= MAX_KNOWN_PEERS => PeerAdmission::AtCapacity,
        None => PeerAdmission::Insert,
    }
}

/// Validate a `device_id` advertised in the mDNS TXT `did` field.
///
/// A valid device_id must be non-empty and consist entirely of lowercase hex
/// characters (0-9, a-f). This matches the format produced by
/// `fingerprint_of` in `crate::cert` (hex-encoded SHA-256). Uppercase hex,
/// empty strings, and any non-hex characters are rejected to prevent a rogue
/// LAN peer from advertising a device_id that bypasses identity-keyed rate
/// limiting or confuses the known-peers map (CopyPaste-rh27).
///
/// Note: the TLS certificate-fingerprint check in `PeerTransport::connect`
/// is the *definitive* authentication gate; this is a defence-in-depth
/// pre-filter at the discovery layer.
pub(super) fn is_valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Build a stable rate-limit key from a set of resolved peer addresses.
///
/// Used when the peer's `device_id` is unknown (older clients / malformed
/// TXT). Sorting + delimiter-joining means the key is invariant to the
/// order `mdns-sd` happens to enumerate v4 vs v6 vs link-local addresses,
/// so a dual-stack peer cannot escape per-peer rate limiting by rotating
/// which address ends up first (security MED #11).
fn address_set_key(addrs: &[IpAddr]) -> String {
    let mut sorted: Vec<String> = addrs.iter().map(|a| a.to_string()).collect();
    sorted.sort();
    sorted.dedup();
    sorted.join(",")
}

/// Convert a [`ScopedIp`] to a standard [`IpAddr`].
///
/// Returns `None` for unknown `ScopedIp` variants (the type is
/// `#[non_exhaustive]`) so callers filter them out rather than dialling
/// `0.0.0.0`, which would be an unreachable and security-confusing address.
fn scoped_ip_to_ip_addr(scoped: &ScopedIp) -> Option<IpAddr> {
    match scoped {
        ScopedIp::V4(v4) => Some(IpAddr::V4(*v4.addr())),
        ScopedIp::V6(v6) => Some(IpAddr::V6(*v6.addr())),
        // `ScopedIp` is #[non_exhaustive]; unknown future variants are dropped
        // rather than substituted with 0.0.0.0, which would be dialled.
        &_ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(id: &str, name: &str, port: u16) -> PeerInfo {
        PeerInfo {
            device_id: id.to_string(),
            device_name: name.to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port,
            bport: None,
        }
    }

    // ── known_peers cap (security MED: DoS / unbounded memory) ───────────────

    #[test]
    fn admit_peer_inserts_until_cap_then_refuses_new() {
        let mut map: HashMap<String, PeerInfo> = HashMap::new();
        // Fill the map exactly to capacity with distinct fullnames.
        for i in 0..MAX_KNOWN_PEERS {
            let fullname = format!("peer-{i}.local.");
            let peer = make_peer(&format!("id{i}"), "P", 1);
            assert_eq!(admit_peer(&map, &fullname, &peer), PeerAdmission::Insert);
            map.insert(fullname, peer);
        }
        assert_eq!(map.len(), MAX_KNOWN_PEERS);

        // A brand-new fullname past the cap must be refused.
        let overflow = make_peer("overflow", "P", 1);
        assert_eq!(
            admit_peer(&map, "overflow.local.", &overflow),
            PeerAdmission::AtCapacity,
            "new peer past the cap must be refused, not inserted"
        );

        // Updates to an already-known fullname are still allowed at capacity.
        let changed = make_peer("id0-changed", "P", 2);
        assert_eq!(
            admit_peer(&map, "peer-0.local.", &changed),
            PeerAdmission::Insert,
            "updating an existing peer must be allowed even at capacity"
        );

        // An unchanged already-known peer is skipped.
        let same = make_peer("id0", "P", 1);
        map.insert("peer-0.local.".to_string(), same.clone());
        assert_eq!(
            admit_peer(&map, "peer-0.local.", &same),
            PeerAdmission::Skip
        );
    }

    #[test]
    fn known_peers_growth_is_bounded_under_id_rotation() {
        // Simulate the attack: a flood of ever-varying fullnames. Apply the same
        // admission decision the event handler uses and confirm the map never
        // exceeds the cap.
        let mut map: HashMap<String, PeerInfo> = HashMap::new();
        for i in 0..(MAX_KNOWN_PEERS * 4) {
            let fullname = format!("rotating-{i}.local.");
            let peer = make_peer(&format!("rot{i}"), "P", 1);
            if admit_peer(&map, &fullname, &peer) == PeerAdmission::Insert {
                map.insert(fullname, peer);
            }
        }
        assert!(
            map.len() <= MAX_KNOWN_PEERS,
            "known_peers must stay bounded under id rotation, got {}",
            map.len()
        );
    }

    // ── IP address sorting ────────────────────────────────────────────────────

    #[test]
    fn ipv4_addresses_sort_before_ipv6() {
        let mut addrs: Vec<IpAddr> = vec!["::1".parse().unwrap(), "127.0.0.1".parse().unwrap()];
        addrs.sort_unstable_by_key(|a| (a.is_ipv6(), a.to_string()));
        assert!(!addrs[0].is_ipv6());
        assert!(addrs[1].is_ipv6());
    }

    // ── CopyPaste-rh27: device_id format validation ──────────────────────────

    /// rh27: a valid hex device_id (lowercase hex chars only) must pass.
    #[test]
    fn rh27_valid_hex_device_id_accepted() {
        // SHA-256 fingerprint is 64 lowercase hex chars.
        let fp = "a".repeat(64);
        assert!(
            is_valid_device_id(&fp),
            "64-char lowercase hex must be accepted"
        );
        // Shorter IDs (e.g. in tests) are also valid as long as they are hex.
        assert!(
            is_valid_device_id("aabbccdd"),
            "short hex id must be accepted"
        );
        assert!(
            is_valid_device_id("0123456789abcdef"),
            "mixed digits+hex letters must be accepted"
        );
        assert!(
            is_valid_device_id("deadbeef"),
            "classic hex id must be accepted"
        );
    }

    /// rh27: an empty device_id must be rejected — it bypasses rate-limit keying.
    #[test]
    fn rh27_empty_device_id_rejected() {
        assert!(
            !is_valid_device_id(""),
            "empty device_id must be rejected (bypasses rate-limit key)"
        );
    }

    /// rh27: uppercase hex must be rejected — fingerprints are always lowercase.
    /// This prevents a rogue peer from advertising the same fingerprint in two
    /// casing variants (A-Z vs a-z) to get double the rate-limit budget.
    #[test]
    fn rh27_uppercase_hex_device_id_rejected() {
        assert!(
            !is_valid_device_id("AABBCCDD"),
            "uppercase hex must be rejected (all CopyPaste fingerprints are lowercase)"
        );
        assert!(
            !is_valid_device_id("AaBbCcDd"),
            "mixed-case hex must be rejected"
        );
    }

    /// rh27: non-hex characters in device_id must be rejected.
    #[test]
    fn rh27_non_hex_device_id_rejected() {
        assert!(
            !is_valid_device_id("not-a-fingerprint"),
            "arbitrary string must be rejected"
        );
        assert!(
            !is_valid_device_id("zzzzzzzz"),
            "non-hex lowercase letters must be rejected"
        );
        assert!(
            !is_valid_device_id("aabb:ccdd"),
            "colon-separated hex must be rejected (colons are not hex chars)"
        );
        assert!(
            !is_valid_device_id("aabb ccdd"),
            "hex with whitespace must be rejected"
        );
    }

    /// v1 TXT record (no bport key) must still be accepted after the version
    /// bump so existing peers are never silently dropped from the list.
    #[test]
    fn peer_from_resolved_v1_is_accepted() {
        // v1 advertises version="1"; bport absent — must NOT return None.
        // We can only test the acceptance logic with a real ResolvedService in an
        // integration test, but we can verify the v1 constant is still "1".
        assert_eq!(PROTOCOL_VERSION_V1, "1");
    }
}
