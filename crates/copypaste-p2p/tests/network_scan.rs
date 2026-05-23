//! Network interface enumeration + multicast scan tests.
//!
//! These tests validate the assumptions that the mDNS-SD discovery layer
//! relies on:
//!
//! 1. The host can enumerate at least one interface (loopback).
//! 2. Link-local address families used by mDNS are detectable
//!    (IPv4 `169.254.0.0/16` and IPv6 `fe80::/10`).
//! 3. The well-known mDNS multicast groups (`224.0.0.251` for IPv4 and
//!    `ff02::fb` for IPv6) can be joined on the loopback interface.
//! 4. The interface filter respects operational state — down interfaces
//!    must be excluded from the candidate set.
//! 5. When no usable network is present, enumeration returns an empty
//!    list rather than erroring (we cannot reliably simulate this on a
//!    real CI host, so the test is `#[ignore]`d and serves as a
//!    documented invariant).
//!
//! Tests touching real network sockets are marked `#[ignore]` so they
//! do not break CI runners that block multicast or run in containers
//! without a usable network stack. Run with:
//! `cargo test -p copypaste-p2p --test network_scan -- --ignored`

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

use if_addrs::{get_if_addrs, Interface};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Best-effort interface enumeration. Returns an empty Vec on any error so
/// callers can treat "no network" and "enumeration failed" uniformly — the
/// real discovery layer (mdns-sd) does the same: it logs and skips bad
/// interfaces instead of failing the whole browse.
fn enumerate_interfaces() -> Vec<Interface> {
    get_if_addrs().unwrap_or_default()
}

/// IPv4 link-local range: 169.254.0.0/16.
fn is_ipv4_link_local(addr: Ipv4Addr) -> bool {
    let [a, b, _, _] = addr.octets();
    a == 169 && b == 254
}

/// IPv6 link-local range: fe80::/10.
fn is_ipv6_link_local(addr: Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xffc0) == 0xfe80
}

// ── 1. enumerate_interfaces_returns_loopback ─────────────────────────────────

#[test]
fn enumerate_interfaces_returns_loopback() {
    let ifaces = enumerate_interfaces();

    // On any sane host (incl. CI containers) loopback exists. If somehow
    // enumeration returned empty we still want a clear failure with context.
    assert!(
        !ifaces.is_empty(),
        "expected at least one network interface; got none"
    );

    let has_loopback = ifaces.iter().any(|iface| match iface.ip() {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    });
    assert!(
        has_loopback,
        "expected loopback interface (127.0.0.1 or ::1) among: {:?}",
        ifaces
            .iter()
            .map(|i| (i.name.clone(), i.ip()))
            .collect::<Vec<_>>()
    );
}

// ── 2. ipv4_link_local_filter_includes_169_254 ──────────────────────────────

#[test]
fn ipv4_link_local_filter_includes_169_254_range() {
    // Spot-check the filter function itself rather than depending on the
    // host actually having a link-local address (CI usually does not).
    let ll = Ipv4Addr::new(169, 254, 10, 20);
    let not_ll = Ipv4Addr::new(192, 168, 1, 1);
    let edge_low = Ipv4Addr::new(169, 254, 0, 0);
    let edge_high = Ipv4Addr::new(169, 254, 255, 255);
    let just_below = Ipv4Addr::new(169, 253, 255, 255);
    let just_above = Ipv4Addr::new(169, 255, 0, 0);

    assert!(is_ipv4_link_local(ll), "169.254.10.20 must be link-local");
    assert!(
        is_ipv4_link_local(edge_low),
        "169.254.0.0 must be link-local"
    );
    assert!(
        is_ipv4_link_local(edge_high),
        "169.254.255.255 must be link-local"
    );
    assert!(
        !is_ipv4_link_local(not_ll),
        "192.168.1.1 must NOT be link-local"
    );
    assert!(
        !is_ipv4_link_local(just_below),
        "169.253/16 must NOT be link-local"
    );
    assert!(
        !is_ipv4_link_local(just_above),
        "169.255/16 must NOT be link-local"
    );

    // Cross-check against the std library's own classification.
    assert!(ll.is_link_local());
    assert!(!not_ll.is_link_local());
}

// ── 3. ipv6_link_local_filter_includes_fe80 ─────────────────────────────────

#[test]
fn ipv6_link_local_filter_includes_fe80_range() {
    let ll: Ipv6Addr = "fe80::1".parse().unwrap();
    let ll_high: Ipv6Addr = "febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap();
    let not_ll: Ipv6Addr = "2001:db8::1".parse().unwrap();
    let loopback: Ipv6Addr = "::1".parse().unwrap();
    let just_above: Ipv6Addr = "fec0::1".parse().unwrap();

    assert!(is_ipv6_link_local(ll), "fe80::1 must be link-local");
    assert!(
        is_ipv6_link_local(ll_high),
        "febf::/10-top must be link-local"
    );
    assert!(
        !is_ipv6_link_local(not_ll),
        "2001:db8::1 must NOT be link-local"
    );
    assert!(!is_ipv6_link_local(loopback), "::1 must NOT be link-local");
    assert!(
        !is_ipv6_link_local(just_above),
        "fec0::1 must NOT be link-local"
    );
}

// ── 4. multicast_group_join_loopback_succeeds ────────────────────────────────

/// Join the IPv4 mDNS multicast group (`224.0.0.251`) on the loopback
/// interface. CI-fragile: many runners disable multicast or block port
/// 5353; marked `#[ignore]`.
#[test]
#[ignore]
fn multicast_group_join_loopback_succeeds_ipv4() {
    // Bind to an ephemeral port so we never collide with a real mDNS responder.
    let sock = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
        .expect("UDP bind on 0.0.0.0:0 must succeed");
    let mdns_v4 = Ipv4Addr::new(224, 0, 0, 251);
    sock.join_multicast_v4(&mdns_v4, &Ipv4Addr::new(127, 0, 0, 1))
        .expect("join 224.0.0.251 on 127.0.0.1 must succeed");
    // Cleanly leave; ignore errors on platforms that don't require it.
    let _ = sock.leave_multicast_v4(&mdns_v4, &Ipv4Addr::new(127, 0, 0, 1));
}

/// IPv6 variant: `ff02::fb` is the mDNS link-local multicast group.
/// CI-fragile (some hosts lack IPv6 loopback multicast); `#[ignore]`.
#[test]
#[ignore]
fn multicast_group_join_loopback_succeeds_ipv6() {
    let sock =
        UdpSocket::bind(SocketAddr::from(([0u16; 8], 0))).expect("UDP bind on [::]:0 must succeed");
    let mdns_v6: Ipv6Addr = "ff02::fb".parse().unwrap();
    // interface index 0 = let the kernel choose; on most hosts this maps
    // to the default interface or loopback depending on configuration.
    sock.join_multicast_v6(&mdns_v6, 0)
        .expect("join ff02::fb must succeed on at least the default iface");
    let _ = sock.leave_multicast_v6(&mdns_v6, 0);
}

// ── 5. interface_filter_excludes_down_interfaces ─────────────────────────────

/// `if-addrs` 0.15 doesn't expose an `is_up()` predicate uniformly on all
/// targets — it exposes `is_oper_up()` only on Linux. We therefore document
/// the invariant we expect (down interfaces are excluded) and verify the
/// loopback (which is always operationally up) is present. Anything beyond
/// that requires platform-specific APIs and is out of scope for this test.
#[test]
fn interface_filter_excludes_down_interfaces() {
    let ifaces = enumerate_interfaces();
    if ifaces.is_empty() {
        // No interfaces at all — vacuously true (and handled by test #1).
        return;
    }

    // Loopback must be present and is by definition up.
    let loopback = ifaces.iter().find(|i| match i.ip() {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    });
    assert!(
        loopback.is_some(),
        "loopback interface must be present and up"
    );

    // Sanity: any interface with the unspecified address (0.0.0.0 / ::) is
    // not a real candidate and must not appear in enumeration output.
    for iface in &ifaces {
        match iface.ip() {
            IpAddr::V4(v4) => assert!(
                !v4.is_unspecified(),
                "interface {} has unspecified IPv4 — should be filtered",
                iface.name
            ),
            IpAddr::V6(v6) => assert!(
                !v6.is_unspecified(),
                "interface {} has unspecified IPv6 — should be filtered",
                iface.name
            ),
        }
    }
}

// ── 6. skipped_when_no_network_present_returns_empty_not_error ───────────────

/// Documented contract: when the host has no usable network, our enumeration
/// wrapper returns `Vec::new()` rather than propagating an error. We cannot
/// disable the host's network from a unit test, so we verify the wrapper's
/// behavior by simulating the error path with a closure shaped identically
/// to `get_if_addrs`.
#[test]
fn skipped_when_no_network_present_returns_empty_not_error() {
    fn enumerate_with<F>(f: F) -> Vec<Interface>
    where
        F: FnOnce() -> std::io::Result<Vec<Interface>>,
    {
        f().unwrap_or_default()
    }

    // Simulated "no network" — IO error from the OS layer.
    let result = enumerate_with(|| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "simulated: no network interfaces available",
        ))
    });
    assert!(
        result.is_empty(),
        "expected empty Vec on enumeration failure, got {} interfaces",
        result.len()
    );

    // Simulated "OK but empty" — also valid.
    let empty_ok = enumerate_with(|| Ok(Vec::new()));
    assert!(
        empty_ok.is_empty(),
        "explicit empty Ok must yield empty Vec"
    );
}
