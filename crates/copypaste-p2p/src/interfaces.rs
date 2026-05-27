//! Host network-interface enumeration and filtering for mDNS-SD advertising.
//!
//! Wave F.L12 — before advertising the CopyPaste service over mDNS-SD we must
//! pick *real* LAN interfaces to bind/announce on. Letting `mdns-sd` discover
//! "all interfaces" causes the daemon to advertise loopback and virtual
//! adapters (Docker bridges, VPN tunnels, VirtualBox/VMware host-only nets),
//! which produce unroutable advertisements and noise on the wire. This module
//! enumerates interfaces via the `if-addrs` crate and applies a conservative
//! filter so only addresses a peer can plausibly reach are advertised.
//!
//! The filter logic is split out into [`is_advertisable_addr`] /
//! [`is_advertisable_interface`] (pure functions over already-enumerated data)
//! so it can be unit-tested without depending on the host's live NIC state.

use std::net::IpAddr;

use if_addrs::{get_if_addrs, IfOperStatus, Interface};
use tracing::{debug, warn};

/// Name prefixes/substrings that identify virtual / software interfaces we
/// never want to advertise CopyPaste on. Matched case-insensitively against
/// the interface name. Kept intentionally conservative — a false negative
/// (advertising on an unusual-but-real NIC) is harmless, whereas a false
/// positive could hide a legitimate LAN interface.
const VIRTUAL_IFACE_PREFIXES: &[&str] = &[
    // Docker / container bridges
    "docker",
    "br-",
    "veth",
    "cni",
    "flannel",
    "cali",
    "weave",
    // VPN / tunnels
    "tun",
    "tap",
    "utun",
    "wg",
    "ppp",
    "ipsec",
    "gif",
    "stf",
    // Virtualization host-only / bridged adapters
    "vboxnet",
    "vmnet",
    "vmware",
    "vnic",
    "hyper-v",
    "vethernet",
    // Misc software interfaces
    "bridge",
    "virbr",
    "zt",  // ZeroTier
    "ham", // Hamachi
];

/// Returns `true` if `name` looks like a virtual / software interface that
/// should not be advertised on.
fn is_virtual_iface_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    VIRTUAL_IFACE_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix) || lower.contains(prefix))
}

/// Returns `true` if `addr` is a real, routable-on-LAN address worth
/// advertising. Excludes loopback, unspecified, link-local and IPv4 multicast.
///
/// Link-local addresses are excluded because they require zone/scope IDs that
/// mDNS advertisements don't carry reliably across peers; real LAN reachability
/// uses the host's globally- or privately-routable address instead.
pub fn is_advertisable_addr(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            !v4.is_loopback()
                && !v4.is_unspecified()
                && !v4.is_link_local()
                && !v4.is_multicast()
                && !v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                // fe80::/10 link-local — `Ipv6Addr::is_unicast_link_local` is
                // unstable, so test the prefix directly.
                && (v6.segments()[0] & 0xffc0) != 0xfe80
        }
    }
}

/// Returns `true` if the operational status permits advertising.
///
/// macOS / BSD do not expose RFC 2863 oper-status, so `if-addrs` reports
/// [`IfOperStatus::Unknown`] there — we must treat `Unknown` as usable or we'd
/// filter out every interface on those platforms. Only the explicitly-down
/// states are rejected.
fn oper_status_is_usable(status: &IfOperStatus) -> bool {
    !matches!(
        status,
        IfOperStatus::Down | IfOperStatus::NotPresent | IfOperStatus::LowerLayerDown
    )
}

/// Decide whether a single enumerated [`Interface`] should be advertised on.
///
/// Excludes loopback, virtual/software interfaces (by name), down interfaces,
/// and non-advertisable addresses (link-local / unspecified / multicast).
pub fn is_advertisable_interface(iface: &Interface) -> bool {
    if iface.is_loopback() {
        return false;
    }
    if !oper_status_is_usable(&iface.oper_status) {
        return false;
    }
    if is_virtual_iface_name(&iface.name) {
        return false;
    }
    is_advertisable_addr(iface.ip())
}

/// Enumerate host interfaces and return the set of addresses suitable for
/// advertising the CopyPaste service over mDNS-SD.
///
/// On enumeration failure we log and return an empty `Vec` (matching the
/// "log and skip" philosophy used elsewhere in discovery) — the caller falls
/// back to letting `mdns-sd` auto-detect, so an empty result never breaks
/// discovery, it merely loses the filtering benefit for that run.
pub fn usable_advertise_addrs() -> Vec<IpAddr> {
    match get_if_addrs() {
        Ok(ifaces) => {
            let addrs: Vec<IpAddr> = ifaces
                .iter()
                .filter(|i| is_advertisable_interface(i))
                .map(|i| i.ip())
                .collect();
            debug!(
                total = ifaces.len(),
                usable = addrs.len(),
                "filtered network interfaces for mDNS advertising"
            );
            addrs
        }
        Err(e) => {
            warn!(error = %e, "interface enumeration failed; mDNS will auto-detect addresses");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use if_addrs::{IfAddr, Ifv4Addr, Ifv6Addr};
    use std::net::{Ipv4Addr, Ipv6Addr};

    // ── interface constructors (synthetic, no live NIC state) ────────────────

    fn v4_iface(name: &str, ip: Ipv4Addr, status: IfOperStatus) -> Interface {
        Interface {
            name: name.to_string(),
            addr: IfAddr::V4(Ifv4Addr {
                ip,
                netmask: Ipv4Addr::new(255, 255, 255, 0),
                prefixlen: 24,
                broadcast: None,
            }),
            index: Some(1),
            oper_status: status,
            is_p2p: false,
            #[cfg(windows)]
            adapter_name: String::new(),
        }
    }

    fn v6_iface(name: &str, ip: Ipv6Addr, status: IfOperStatus) -> Interface {
        Interface {
            name: name.to_string(),
            addr: IfAddr::V6(Ifv6Addr {
                ip,
                netmask: Ipv6Addr::from(0u128),
                prefixlen: 64,
                broadcast: None,
            }),
            index: Some(1),
            oper_status: status,
            is_p2p: false,
            #[cfg(windows)]
            adapter_name: String::new(),
        }
    }

    // ── is_advertisable_addr ─────────────────────────────────────────────────

    #[test]
    fn private_lan_ipv4_is_advertisable() {
        assert!(is_advertisable_addr(IpAddr::V4(Ipv4Addr::new(
            192, 168, 1, 50
        ))));
        assert!(is_advertisable_addr(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))));
    }

    #[test]
    fn loopback_ipv4_is_not_advertisable() {
        assert!(!is_advertisable_addr(IpAddr::V4(Ipv4Addr::new(
            127, 0, 0, 1
        ))));
    }

    #[test]
    fn link_local_ipv4_is_not_advertisable() {
        assert!(!is_advertisable_addr(IpAddr::V4(Ipv4Addr::new(
            169, 254, 5, 5
        ))));
    }

    #[test]
    fn unspecified_ipv4_is_not_advertisable() {
        assert!(!is_advertisable_addr(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    }

    #[test]
    fn global_ipv6_is_advertisable() {
        let g: Ipv6Addr = "2001:db8::1".parse().unwrap();
        assert!(is_advertisable_addr(IpAddr::V6(g)));
    }

    #[test]
    fn loopback_and_link_local_ipv6_not_advertisable() {
        assert!(!is_advertisable_addr(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        let ll: Ipv6Addr = "fe80::1".parse().unwrap();
        assert!(!is_advertisable_addr(IpAddr::V6(ll)));
    }

    // ── is_virtual_iface_name ────────────────────────────────────────────────

    #[test]
    fn virtual_interface_names_are_detected() {
        for name in [
            "docker0",
            "br-abc123",
            "veth1234",
            "utun3",
            "tun0",
            "vboxnet0",
            "vmnet1",
        ] {
            assert!(is_virtual_iface_name(name), "{name} should be virtual");
        }
    }

    #[test]
    fn real_interface_names_are_not_virtual() {
        for name in ["en0", "eth0", "wlan0", "enp3s0", "wlp2s0"] {
            assert!(!is_virtual_iface_name(name), "{name} should be real");
        }
    }

    // ── is_advertisable_interface (the core F.L12 filter) ────────────────────

    #[test]
    fn filter_excludes_loopback_interface() {
        let lo = v4_iface("lo0", Ipv4Addr::new(127, 0, 0, 1), IfOperStatus::Up);
        assert!(!is_advertisable_interface(&lo));
    }

    #[test]
    fn filter_excludes_virtual_interface_even_with_routable_ip() {
        // A Docker bridge often carries a private-but-virtual 172.17/16 addr.
        let docker = v4_iface("docker0", Ipv4Addr::new(172, 17, 0, 1), IfOperStatus::Up);
        assert!(!is_advertisable_interface(&docker));
    }

    #[test]
    fn filter_excludes_down_interface() {
        let down = v4_iface("en5", Ipv4Addr::new(192, 168, 9, 9), IfOperStatus::Down);
        assert!(!is_advertisable_interface(&down));
    }

    #[test]
    fn filter_keeps_real_up_lan_interface() {
        let en0 = v4_iface("en0", Ipv4Addr::new(192, 168, 1, 23), IfOperStatus::Up);
        assert!(is_advertisable_interface(&en0));
    }

    #[test]
    fn filter_keeps_interface_with_unknown_oper_status() {
        // macOS / BSD report Unknown; must NOT be filtered out.
        let en0 = v4_iface("en0", Ipv4Addr::new(192, 168, 1, 23), IfOperStatus::Unknown);
        assert!(is_advertisable_interface(&en0));
    }

    #[test]
    fn filter_excludes_link_local_real_interface() {
        let en0 = v4_iface("en0", Ipv4Addr::new(169, 254, 1, 1), IfOperStatus::Up);
        assert!(!is_advertisable_interface(&en0));
    }

    #[test]
    fn filter_over_mixed_list_keeps_only_real_lan_addrs() {
        let ifaces = [
            v4_iface("lo0", Ipv4Addr::new(127, 0, 0, 1), IfOperStatus::Up),
            v6_iface("lo0", Ipv6Addr::LOCALHOST, IfOperStatus::Up),
            v4_iface("docker0", Ipv4Addr::new(172, 17, 0, 1), IfOperStatus::Up),
            v4_iface("utun0", Ipv4Addr::new(10, 8, 0, 2), IfOperStatus::Up),
            v4_iface("en0", Ipv4Addr::new(192, 168, 1, 23), IfOperStatus::Up),
            v6_iface(
                "en0",
                "2001:db8::1234".parse().unwrap(),
                IfOperStatus::Unknown,
            ),
            v4_iface("en1", Ipv4Addr::new(10, 0, 0, 5), IfOperStatus::Down),
        ];

        let kept: Vec<IpAddr> = ifaces
            .iter()
            .filter(|i| is_advertisable_interface(i))
            .map(|i| i.ip())
            .collect();

        assert_eq!(
            kept,
            [
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 23)),
                IpAddr::V6("2001:db8::1234".parse().unwrap()),
            ],
            "only the real, up, non-virtual LAN addresses should survive"
        );
    }
}
