//! This host's own network addresses.
//!
//! Two questions, one answer source: where should a peer dial this device, and
//! is that address on the wire in fact ours? `getifaddrs(3)` behind a
//! maintained crate rather than a `SIOCGIFCONF` walk or the "connect a UDP
//! socket and read its local end" trick; it covers macOS and Android, which is
//! the whole of CLAUDE.md rule 7.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Duration;

/// How long an enumeration is reused. A laptop moving between Wi-Fi and a dock
/// changes its addresses in seconds, and this is only used to *discard*
/// candidates, so a stale entry costs one refused advertisement.
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Every address this host currently holds, loopback included.
///
/// Loopback is deliberately kept: two daemons on one host talk over it, and
/// telling our own advertisement apart from theirs is exactly what
/// [`is_local`] is for.
#[must_use]
pub fn local_ips() -> Vec<IpAddr> {
    match if_addrs::get_if_addrs() {
        Ok(addrs) => addrs.iter().map(if_addrs::Interface::ip).collect(),
        Err(e) => {
            // Never fatal: without the list nothing is treated as local, which
            // is the same behaviour as before this check existed.
            tracing::debug!(reason = %e, "could not enumerate local interfaces");
            Vec::new()
        }
    }
}

/// Where a peer on another device should dial this host, when that can be
/// determined.
///
/// Loopback is skipped: a peer elsewhere cannot use it. When the host has no
/// routable address the answer is `None` rather than a guess — the user is told
/// to supply the address themselves.
#[must_use]
pub fn routable_ip() -> Option<IpAddr> {
    local_ips()
        .into_iter()
        .filter(|ip| !ip.is_loopback())
        // IPv4 first: it is what a user can read out over the phone.
        .min_by_key(|ip| u8::from(ip.is_ipv6()))
}

/// A cached "is this address one of ours".
#[derive(Debug, Default)]
pub struct LocalAddrs {
    cached: Mutex<Option<(i64, Vec<IpAddr>)>>,
}

impl LocalAddrs {
    /// Whether `ip` belongs to this host, re-enumerating at most every
    /// [`CACHE_TTL`].
    pub fn is_local(&self, ip: IpAddr, now_ms: i64) -> bool {
        let mut cached = match self.cached.lock() {
            Ok(guard) => guard,
            // A cache, not a decision: a poisoned lock must not propagate a
            // panic into a discovery convenience.
            Err(poisoned) => poisoned.into_inner(),
        };
        let fresh = cached.as_ref().is_some_and(|(taken_at, _)| {
            now_ms.saturating_sub(*taken_at) < CACHE_TTL.as_millis() as i64
        });
        if !fresh {
            *cached = Some((now_ms, local_ips()));
        }
        cached.as_ref().is_some_and(|(_, ips)| ips.contains(&ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// Every host has loopback, including this container.
    #[test]
    fn loopback_is_local_and_a_lan_address_is_not() {
        let addrs = LocalAddrs::default();
        let now = crate::now_ms();
        assert!(addrs.is_local(IpAddr::V4(Ipv4Addr::LOCALHOST), now));
        // 192.0.2.0/24 is TEST-NET-1: reserved for documentation, never routed.
        assert!(!addrs.is_local(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), now));
    }

    #[test]
    fn a_routable_address_is_never_loopback() {
        if let Some(ip) = routable_ip() {
            assert!(!ip.is_loopback(), "{ip}");
        }
    }
}
