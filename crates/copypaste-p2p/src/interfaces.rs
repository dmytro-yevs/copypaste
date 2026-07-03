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

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
///
/// Matches by **prefix only** (fix/p2p-c-review #5). The earlier
/// `|| lower.contains(prefix)` fallback was unsafe for the short tokens
/// (`tun`, `tap`, `zt`, `gif`, `wg`): a substring match would hide a
/// legitimate NIC whose name merely *contains* one of them (e.g. a vendor NIC
/// named `engtun0`, or any interface with `wg` somewhere in the middle).
/// Every entry in [`VIRTUAL_IFACE_PREFIXES`] is a genuine name *prefix*, so
/// `starts_with` is both sufficient and strictly safer.
fn is_virtual_iface_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    VIRTUAL_IFACE_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
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

/// Probe the OS routing table to discover the default-route source IP.
///
/// Opens a **connected** UDP socket to `probe_target` and reads `local_addr()`.
/// A connected UDP socket does not send any data — `connect()` on UDP merely
/// sets the kernel's routing destination used by `getsockname()` (i.e.
/// `local_addr()`). This is the standard no-new-dep trick for learning the
/// outgoing interface on multi-homed hosts without parsing routing tables.
///
/// `probe_target` is deliberately a public anycast address (`1.1.1.1:53`) so
/// the kernel selects the default-route interface; use any reachable LAN host
/// when you only want to prefer a specific subnet.
///
/// Returns `None` when the host has no default route (offline, sandboxed CI)
/// or the socket call fails for any reason — callers fall back gracefully.
fn probe_default_route_source(probe_target: std::net::SocketAddrV4) -> Option<Ipv4Addr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // connect() on UDP is non-blocking and sends no data — it only sets the
    // kernel's idea of the destination for routing/getsockname purposes.
    sock.connect(probe_target).ok()?;
    match sock.local_addr().ok()? {
        std::net::SocketAddr::V4(v4) => {
            let ip = *v4.ip();
            // Only use the probed result when it is itself a usable LAN address;
            // if the kernel returns 0.0.0.0 (no route) or loopback, discard.
            if ip.is_unspecified() || ip.is_loopback() {
                None
            } else {
                Some(ip)
            }
        }
        std::net::SocketAddr::V6(_) => None,
    }
}

/// Pick the single LAN-routable host address to advertise to a peer, from an
/// already-enumerated list of usable addresses.
///
/// **Egress-interface preference (HW-B2 fix):** on multi-homed hosts the first
/// enumerated IPv4 address is nondeterministic and may belong to a NIC that is
/// unreachable from the peer's subnet. Instead we probe the OS routing table by
/// connecting a no-op UDP socket to a public anycast target and reading
/// `local_addr()` — this yields the IP the kernel would use for default-route
/// traffic, which is the address most likely reachable by a LAN peer. If that
/// probed IP appears in `usable`, it is preferred; otherwise we fall back to
/// the first IPv4 in `usable`, then the first address of any family, then
/// `fallback`.
///
/// IPv4 is still preferred over IPv6 in all paths because link-local/zone-id
/// handling for IPv6 is fragile across the pairing and dial paths.
///
/// When `usable` is empty (no real LAN interface — e.g. an offline machine, a
/// CI sandbox, or a single-host loopback test) the caller's loopback fallback
/// is returned so same-host pairing still works.
///
/// Split out as a pure function over an explicit address list so the selection
/// policy can be unit-tested without a live NIC.
pub fn pick_advertise_host(usable: &[IpAddr], fallback: IpAddr) -> IpAddr {
    if usable.is_empty() {
        return fallback;
    }

    // Probe the OS routing table: connected UDP socket → local_addr() reveals
    // the source IP the kernel selects for default-route traffic (no data sent).
    //
    // CopyPaste-8ebg.65: the probe target is intentionally hardcoded to
    // 1.1.1.1:53 (Cloudflare anycast) — any routable public IP works equally
    // well since the socket is UDP and `connect()` never actually sends a
    // packet (see `probe_default_route_source`'s doc comment); no real
    // network traffic reaches this address, so it is not a privacy/telemetry
    // concern. It is nonetheless overridable via `COPYPASTE_P2P_ROUTE_PROBE`
    // (`ip:port`) for restricted/offline test environments where even a
    // routing-table lookup toward a specific public anycast IP is undesired.
    let probe_target = std::env::var("COPYPASTE_P2P_ROUTE_PROBE")
        .ok()
        .and_then(|s| s.parse::<std::net::SocketAddrV4>().ok())
        .unwrap_or_else(|| std::net::SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 53));
    if let Some(egress_ip) = probe_default_route_source(probe_target) {
        let egress = IpAddr::V4(egress_ip);
        if usable.contains(&egress) {
            debug!(egress_ip = %egress_ip, "pick_advertise_host: using probed default-route source IP");
            return egress;
        }
        debug!(
            egress_ip = %egress_ip,
            "pick_advertise_host: probed IP not in usable list, falling back"
        );
    }

    // Fallback: first IPv4 in usable, then first of any family.
    usable
        .iter()
        .find(|ip| ip.is_ipv4())
        .or_else(|| usable.first())
        .copied()
        .unwrap_or(fallback)
}

/// Build the `host:port` sync-listener address to advertise to a peer, choosing
/// a real LAN-routable host address from [`usable_advertise_addrs`].
///
/// This is the single source of truth used by BOTH the pairing QR `addr_hint`
/// and the in-band P2P sync-listener address persisted to `peers.json`: a peer
/// (a real phone on the same Wi-Fi, not just an emulator on loopback) must
/// receive a host-reachable address it can dial, never `127.0.0.1`.
///
/// Falls back to `127.0.0.1:<port>` ONLY when the host exposes no usable LAN
/// interface, so single-host / loopback-test pairing still functions. The
/// listener itself binds `0.0.0.0`, so it is reachable on every interface —
/// this only decides which concrete host the advertisement carries.
pub fn advertise_sync_addr(port: u16) -> SocketAddr {
    let usable = usable_advertise_addrs();
    let host = pick_advertise_host(&usable, IpAddr::V4(Ipv4Addr::LOCALHOST));
    SocketAddr::new(host, port)
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

    // ── pick_advertise_host / advertise_sync_addr (LAN sync-addr policy) ─────

    /// IPv4 is preferred even when an IPv6 address comes first in the list.
    #[test]
    fn pick_advertise_host_prefers_ipv4() {
        let usable = [
            IpAddr::V6("2001:db8::5".parse().unwrap()),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)),
        ];
        let host = pick_advertise_host(&usable, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(host, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)));
    }

    /// With only an IPv6 LAN address available, that address is used (not the
    /// loopback fallback).
    #[test]
    fn pick_advertise_host_uses_ipv6_when_no_ipv4() {
        let g: Ipv6Addr = "2001:db8::9".parse().unwrap();
        let usable = [IpAddr::V6(g)];
        let host = pick_advertise_host(&usable, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(host, IpAddr::V6(g));
    }

    /// Empty usable list → caller's loopback fallback (single-host / loopback
    /// test still works). This is the ONLY path that yields loopback.
    #[test]
    fn pick_advertise_host_falls_back_to_loopback_when_empty() {
        let host = pick_advertise_host(&[], IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(host, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    /// The advertised sync address must NEVER be loopback when a real LAN
    /// interface exists — this is the exact regression behind the
    /// emulator-only / loopback-only Android sync bug.
    #[test]
    fn advertised_host_is_lan_not_loopback_when_lan_present() {
        let usable = [IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))];
        let host = pick_advertise_host(&usable, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(!host.is_loopback(), "must advertise a routable LAN host");
        assert_eq!(host, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)));
    }

    /// `advertise_sync_addr` carries the requested port through unchanged and
    /// always yields a parseable `host:port` (host depends on the live NIC
    /// state, so we only assert the port and parseability here).
    #[test]
    fn advertise_sync_addr_carries_port() {
        let addr = advertise_sync_addr(54321);
        assert_eq!(addr.port(), 54321);
        // Round-trips through its string form (what gets written to peers.json
        // / the QR addr_hint and re-parsed by the connector).
        let reparsed: SocketAddr = addr.to_string().parse().unwrap();
        assert_eq!(reparsed, addr);
    }
}
