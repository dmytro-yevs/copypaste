//! Dialable-peer list + mtime-gated cache for the connector loop.
//!
//! Split out of the former flat `p2p/connector.rs` (ADR-017,
//! CopyPaste-vp63.48) — moved verbatim, no behavior change.

use std::net::SocketAddr;

use copypaste_p2p::transport::DeviceFingerprint;

/// A dialable paired peer resolved from `peers.json`.
#[derive(Clone)]
pub(in crate::p2p) struct DialablePeer {
    /// Canonical (colon-free, lowercase) cert fingerprint — the mTLS pin.
    pub(in crate::p2p) fingerprint: DeviceFingerprint,
    /// The peer's sync-listener socket address.
    pub(in crate::p2p) addr: SocketAddr,
}

/// CopyPaste-c1dd: mtime-gated cache for the dialable-peer list so the connector
/// loop does not re-read + re-parse `peers.json` from disk on every 3 s tick.
///
/// `peers.json` only changes when the user pairs/unpairs or when
/// `refresh_peer_meta_from_discovery` writes an updated peer record; both bump
/// the file mtime, which invalidates the cache. The steady state (no pairing
/// activity) reads only the cheap `fs::metadata` mtime and reuses the parsed
/// Vec, avoiding a full read+JSON-parse every tick.
#[derive(Default)]
pub(in crate::p2p) struct DialablePeersCache {
    /// Last observed file modification time; `None` until the first read.
    last_mtime: Option<std::time::SystemTime>,
    /// Cached parse result reused while the mtime is unchanged.
    cached: Vec<DialablePeer>,
}

impl DialablePeersCache {
    /// Return the dialable peers for `path`, re-reading + re-parsing from disk
    /// only when the file mtime has changed since the last call (or on the first
    /// call, or if the mtime cannot be read — fail safe by always re-reading).
    ///
    /// Returns an owned `Vec` (a cheap clone of the cached list — a handful of
    /// `String` + `SocketAddr` per peer) so the connector loop keeps its
    /// existing by-value iteration; the avoided cost is the per-tick file read +
    /// JSON parse, not the small Vec clone.
    pub(in crate::p2p) fn get(&mut self, path: &std::path::Path) -> Vec<DialablePeer> {
        let current_mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        // Re-read when: first call (last_mtime None), mtime changed, or mtime is
        // unavailable (treat as "may have changed" to never serve stale data).
        let stale = match (current_mtime, self.last_mtime) {
            (Some(now), Some(prev)) => now != prev,
            _ => true,
        };
        if stale {
            self.cached = dialable_peers_from_path(path);
            self.last_mtime = current_mtime;
        }
        self.cached.clone()
    }
}

/// Read `peers.json` and return the paired peers that carry a parseable sync
/// `address` — the set the connector may dial. Peers with no address (legacy
/// records, or a peer that never advertised one) are skipped: the connector
/// has nothing to dial and relies on the peer dialing us instead.
pub(in crate::p2p) fn dialable_peers_from_path(path: &std::path::Path) -> Vec<DialablePeer> {
    let stored = crate::peers::load_peers(path);
    let mut out = Vec::new();
    for dev in &stored {
        if dev.fingerprint.is_empty() {
            continue;
        }
        let Some(addr_str) = dev.address.as_deref() else {
            continue;
        };
        let addr = match addr_str.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(addr = %addr_str, error = %e, "skipping peer with unparseable sync address");
                continue;
            }
        };
        out.push(DialablePeer {
            fingerprint: copypaste_p2p::DeviceFingerprint(crate::ipc::canonical_fingerprint(
                &dev.fingerprint,
            )),
            addr,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Phase 3: the connector resolves only paired peers that carry a parseable
    /// sync `address` from `peers.json`; records with no address (or a malformed
    /// one) are skipped, and the fingerprint is normalised to canonical hex.
    #[test]
    fn dialable_peers_resolves_address_records_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_colon = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let fp_canonical = crate::ipc::canonical_fingerprint(&fp_colon);
        let json = format!(
            r#"[
                {{"fingerprint":"{fp_colon}","name":"A","added_at":1,"address":"127.0.0.1:4242"}},
                {{"fingerprint":"cd:cd","added_at":2}},
                {{"fingerprint":"ef:ef","added_at":3,"address":"not-an-addr"}}
            ]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(
            dialable.len(),
            1,
            "only the record with a valid address is dialable"
        );
        assert_eq!(dialable[0].fingerprint, fp_canonical);
        assert_eq!(dialable[0].addr, "127.0.0.1:4242".parse().unwrap());
    }

    /// A peer persisted with a real LAN sync address is considered dialable by
    /// the connector (the Android→macOS background-sync direction depends on the
    /// macOS daemon advertising — and the peer persisting — a routable LAN
    /// address, not loopback). The resolved `addr` round-trips exactly.
    #[test]
    fn peer_with_lan_address_is_dialable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_colon = std::iter::repeat_n("a1", 32).collect::<Vec<_>>().join(":");
        let fp_canonical = crate::ipc::canonical_fingerprint(&fp_colon);
        let json = format!(
            r#"[{{"fingerprint":"{fp_colon}","name":"Mac","added_at":1,"address":"192.168.1.50:43117"}}]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(dialable.len(), 1, "LAN-addressed peer must be dialable");
        assert_eq!(dialable[0].fingerprint, fp_canonical);
        assert_eq!(
            dialable[0].addr,
            "192.168.1.50:43117".parse::<SocketAddr>().unwrap()
        );
        assert!(
            !dialable[0].addr.ip().is_loopback(),
            "a real LAN peer address must not be loopback"
        );
    }

    /// Connector dial policy for the two non-LAN cases:
    /// * an EMPTY address record is skipped entirely (nothing to dial — the
    ///   connector relies on the peer dialing us instead);
    /// * a LOOPBACK address still parses and is therefore dialable, which keeps
    ///   single-host / loopback tests working (it simply fails and backs off on
    ///   a real cross-host LAN, which is harmless).
    #[test]
    fn dial_policy_skips_empty_addr_but_keeps_loopback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let fp_empty = std::iter::repeat_n("b2", 32).collect::<Vec<_>>().join(":");
        let fp_loop = std::iter::repeat_n("c3", 32).collect::<Vec<_>>().join(":");
        let fp_loop_canonical = crate::ipc::canonical_fingerprint(&fp_loop);
        // One record with no `address` key at all, one with a loopback address.
        let json = format!(
            r#"[
                {{"fingerprint":"{fp_empty}","name":"NoAddr","added_at":1}},
                {{"fingerprint":"{fp_loop}","name":"Loop","added_at":2,"address":"127.0.0.1:7000"}}
            ]"#
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let dialable = dialable_peers_from_path(&path);
        assert_eq!(
            dialable.len(),
            1,
            "only the loopback record is dialable; the address-less record is skipped"
        );
        assert_eq!(dialable[0].fingerprint, fp_loop_canonical);
        assert!(dialable[0].addr.ip().is_loopback());
    }
}
