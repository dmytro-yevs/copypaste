//! Shared types, constants, and callback type aliases for mDNS-SD discovery.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

/// Service type used for mDNS-SD advertisement and browsing.
pub const SERVICE_TYPE: &str = "_copypaste._tcp.local.";
/// TXT record version key.
pub(super) const TXT_VERSION: &str = "v";
/// TXT record device-id key.
pub(super) const TXT_DEVICE_ID: &str = "did";
/// TXT record device-name key.
pub(super) const TXT_DEVICE_NAME: &str = "name";
/// TXT record bootstrap-port key (LAN/SAS Phase 0).
///
/// Carries the TCP port of the ephemeral PAKE bootstrap listener used for
/// SAS-authenticated pairing (Phase 2). Absent on v1 peers — `peer_from_resolved`
/// accepts both so the discovered list is never gated on the peer version.
pub(super) const TXT_BPORT: &str = "bport";
/// Protocol version advertised in TXT records (Phase 0 bump: was "1").
///
/// v2 adds the `bport` TXT key. `peer_from_resolved` accepts both v1 and v2
/// so existing peers are never silently dropped from the discovered list.
pub(super) const PROTOCOL_VERSION: &str = "2";
/// Maximum length of a single DNS label per RFC 1035 §2.3.4.
/// mdns-sd asserts `s.len() < 64` in dns_parser.rs; enforce the limit here
/// before registering so we never trigger a background-thread panic.
pub(super) const DNS_LABEL_MAX: usize = 63;
/// The v1 protocol version string, accepted for backward compatibility.
///
/// v1 peers lack the `bport` TXT key; they appear in the discovered list but
/// the UI disables the "Pair" button because bootstrap is unavailable.
pub const PROTOCOL_VERSION_V1: &str = "1";

/// How often own mDNS service is unconditionally re-announced even without a
/// detected network-change event.
///
/// This self-heals stale IP advertisements that arise after a Wi-Fi roam, VPN
/// connect/disconnect, or DHCP renew — events that change the host IP without
/// triggering an explicit re-register. RFC 6762 §11.3 recommends re-announcing
/// at a multiple of the record TTL; 5 minutes is conservative enough to avoid
/// unnecessary multicast traffic while still recovering within one TTL period.
///
/// Platform-specific change detection (RTNetlink on Linux, SCDynamicStore on
/// macOS) would allow immediate re-announce; the periodic fallback here avoids
/// that platform-specific complexity while meeting the functional requirement.
pub const MDNS_REANNOUNCE_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum number of distinct peers retained in `known_peers`.
///
/// Security MED (DoS / unbounded memory): `known_peers` is keyed by the mDNS
/// fullname, which embeds the unauthenticated, rotatable TXT `did`. A LAN host
/// emitting endlessly-varying instance fullnames would otherwise grow the map
/// without bound (and id-rotation also dodges the per-key rate limiter). Once
/// this cap is reached we refuse to insert genuinely new peers rather than
/// evict existing ones — eviction would let an attacker flush legitimately
/// discovered peers. Updates to already-known fullnames are always allowed.
pub(super) const MAX_KNOWN_PEERS: usize = 256;

/// Information about a discovered peer.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerInfo {
    /// The peer's device ID (hex fingerprint of their public key).
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// All resolved IP addresses for the peer (sorted, deduplicated).
    pub ip_addrs: Vec<IpAddr>,
    /// TCP port the peer's P2P sync listener is on.
    pub port: u16,
    /// TCP port of the peer's PAKE bootstrap listener (LAN/SAS Phase 0).
    ///
    /// Present on v2 peers that advertise `bport` in their TXT record.
    /// `None` on v1 peers — the UI must disable the "Pair" button in that case
    /// because the bootstrap handshake cannot be initiated without this port.
    pub bport: Option<u16>,
}

pub(super) type PeerFoundCallback = Arc<dyn Fn(PeerInfo) + Send + Sync + 'static>;
pub(super) type PeerLostCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Decision for whether a resolved peer should be inserted into `known_peers`.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PeerAdmission {
    /// Already present and unchanged — no insert, no callback.
    Skip,
    /// New or changed peer that fits within the cap — insert and notify.
    Insert,
    /// A brand-new fullname but the map is full — refuse (DoS guard).
    AtCapacity,
}

/// Registration parameters stored when `register()` is called.
#[derive(Clone)]
pub(super) struct Registration {
    pub(super) port: u16,
    pub(super) device_id: String,
    // device_name is intentionally not stored here (CopyPaste-sh9a): the human
    // name is no longer included in the mDNS advertisement to avoid PII leakage
    // on the LAN. The name is retained in the daemon's own config and exchanged
    // post-PAKE during pairing. The public `register()` API still accepts the
    // name parameter for caller compatibility but does not persist it.
    /// Bootstrap port for SAS pairing (Phase 0). None = v1 advertisement.
    pub(super) bport: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MDNS_REANNOUNCE_INTERVAL ─────────────────────────────────────────────

    /// Guard that the re-announce interval is in a sane range: tight enough
    /// to recover from a network change within one TTL but not so short that
    /// it floods the LAN with multicast.
    #[test]
    fn mdns_reannounce_interval_is_in_valid_range() {
        // ≥ 60 s: no more multicast than once per minute.
        assert!(
            MDNS_REANNOUNCE_INTERVAL >= Duration::from_secs(60),
            "MDNS_REANNOUNCE_INTERVAL too short — would flood the LAN"
        );
        // ≤ 30 min: recovery within a reasonable time after network change.
        assert!(
            MDNS_REANNOUNCE_INTERVAL <= Duration::from_secs(1800),
            "MDNS_REANNOUNCE_INTERVAL too long — post-roam recovery delayed"
        );
    }

    // ── PeerInfo helpers ─────────────────────────────────────────────────────

    fn make_peer(id: &str, name: &str, port: u16) -> PeerInfo {
        PeerInfo {
            device_id: id.to_string(),
            device_name: name.to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port,
            bport: None,
        }
    }

    #[test]
    fn peer_info_equality() {
        assert_eq!(
            make_peer("aabb", "Alice", 51515),
            make_peer("aabb", "Alice", 51515)
        );
    }

    #[test]
    fn peer_info_inequality_on_port() {
        assert_ne!(
            make_peer("aabb", "Alice", 51515),
            make_peer("aabb", "Alice", 9999)
        );
    }

    #[test]
    fn peer_info_inequality_on_device_id() {
        assert_ne!(
            make_peer("aabb", "Alice", 51515),
            make_peer("1122", "Alice", 51515)
        );
    }

    // ── service type constant ────────────────────────────────────────────────

    #[test]
    fn service_type_has_correct_format() {
        assert!(SERVICE_TYPE.starts_with('_'));
        assert!(SERVICE_TYPE.ends_with(".local."));
        assert!(SERVICE_TYPE.contains("_tcp"));
        assert!(SERVICE_TYPE.contains("_copypaste"));
    }

    // ── LAN/SAS Phase 0: bport TXT key + PROTOCOL_VERSION "2" ───────────────

    /// PROTOCOL_VERSION must be "2" after the Phase 0 bump.
    #[test]
    fn protocol_version_is_2() {
        assert_eq!(PROTOCOL_VERSION, "2");
    }

    /// PeerInfo must carry an optional `bport` field for the bootstrap port.
    #[test]
    fn peer_info_has_bport_field() {
        let peer = PeerInfo {
            device_id: "aabb".to_string(),
            device_name: "Test".to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port: 51515,
            bport: Some(51516),
        };
        assert_eq!(peer.bport, Some(51516));

        let peer_no_bport = PeerInfo {
            device_id: "aabb".to_string(),
            device_name: "Test".to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port: 51515,
            bport: None,
        };
        assert_eq!(peer_no_bport.bport, None);
    }
}
