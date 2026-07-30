//! The TXT record: what we advertise, and what we are willing to believe.
//!
//! Only non-secret material goes in it:
//!
//! | key | value |
//! |---|---|
//! | `v` | discovery record version, currently `1` |
//! | `n` | device display name, UTF-8 |
//! | `p0`…`pN` | one advertised `pairing_id` per key |
//!
//! # What the advertisement discloses about the token
//!
//! The token itself never appears — not whole, not truncated, not in the code
//! alphabet. The `pairing_id` that *is* advertised is nevertheless derived from
//! it: [`crate::PairingToken::pairing_id`] is a domain-separated BLAKE2s of the
//! token truncated to 128 bits. That is one-way, so the id is not a credential
//! and possession of it authenticates nothing — but it is not independent of the
//! token either, and two consequences follow and are accepted (security review
//! F-7, and `SECURITY.md` says the same):
//!
//! * Someone holding a candidate pairing code can compute its id and confirm
//!   offline which device on the LAN it belongs to, without touching the
//!   network.
//! * The ids are stable public identifiers, broadcast on every network the
//!   device joins, so they link a device across networks.
//!
//! Nothing else derived from the token goes in the record, and the key set is
//! closed: `advertisement_carries_the_pairing_id_and_nothing_else_of_the_token`
//! builds its record from a real [`crate::PairingToken`], so it can fail on both
//! halves of that claim rather than on strings a test invented.
//!
//! Both directions are pure: no sockets either way.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use mdns_sd::{ResolvedService, ServiceInfo, TxtProperties};
use tracing::debug;

use super::names::{
    is_valid_pairing_id, sanitise_display_name, sanitise_host_label, sanitise_instance,
};
use super::table::DiscoveredPeer;
use super::DiscoveryError;
use crate::SERVICE_TYPE;

/// Version stamped into the TXT record, so a future format change can be
/// ignored by old builds instead of misread by them.
const TXT_VERSION: &str = "1";

const TXT_KEY_VERSION: &str = "v";
const TXT_KEY_NAME: &str = "n";
/// Pairing id keys are this prefix followed by a decimal index: `p0`, `p1`, …
const TXT_KEY_PAIRING_PREFIX: &str = "p";

/// Ceiling on pairing ids accepted from a single advertisement, so one host
/// cannot fill the table on its own.
pub const MAX_PAIRING_IDS_PER_PEER: usize = 16;

/// Ceiling on pairing ids we advertise. Beyond this the record stops fitting
/// comfortably in one mDNS packet. Extra ids are dropped with a debug log
/// rather than raising an error: discovery is a convenience, and refusing to
/// advertise must never be able to break pairing.
pub const MAX_ADVERTISED_PAIRING_IDS: usize = 16;

/// Build the advertisement. `instance` is the mDNS instance label (which the
/// daemon may rename on conflict); `device_name` is what goes in TXT for humans
/// to read.
pub(super) fn build_service_info(
    instance: &str,
    device_name: &str,
    pairing_ids: &[String],
    port: u16,
) -> Result<ServiceInfo, DiscoveryError> {
    let instance = sanitise_instance(instance).ok_or(DiscoveryError::InvalidDeviceName)?;
    let display = sanitise_display_name(device_name).ok_or(DiscoveryError::InvalidDeviceName)?;
    let hostname = format!(
        "{}.local.",
        sanitise_host_label(device_name).unwrap_or_else(|| "copypaste".to_string())
    );

    // Deduplicate, keeping the caller's order, then cap.
    let mut advertised: Vec<&String> = Vec::new();
    for id in pairing_ids {
        if !is_valid_pairing_id(id) {
            return Err(DiscoveryError::InvalidPairingId);
        }
        if !advertised.contains(&id) {
            advertised.push(id);
        }
    }
    if advertised.len() > MAX_ADVERTISED_PAIRING_IDS {
        debug!(
            advertised = MAX_ADVERTISED_PAIRING_IDS,
            held = advertised.len(),
            "too many pairings to advertise; the rest still work with an explicit address"
        );
        advertised.truncate(MAX_ADVERTISED_PAIRING_IDS);
    }

    // Only these three kinds of key ever exist, and the pairing id is the only
    // one of the three derived from the token — see the module docs.
    let mut txt: HashMap<String, String> = HashMap::new();
    txt.insert(TXT_KEY_VERSION.to_string(), TXT_VERSION.to_string());
    txt.insert(TXT_KEY_NAME.to_string(), display);
    for (i, id) in advertised.iter().enumerate() {
        txt.insert(format!("{TXT_KEY_PAIRING_PREFIX}{i}"), (*id).clone());
    }

    // Addresses are left to the daemon: `enable_addr_auto` keeps the A/AAAA
    // records in step with the host as interfaces come and go, which a laptop
    // moving between Wi-Fi and a dock does constantly.
    let info = ServiceInfo::new(SERVICE_TYPE, &instance, &hostname, (), port, txt)?;
    Ok(info.enable_addr_auto())
}

/// What we were able to read out of somebody's TXT record.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advertisement {
    name: String,
    pairing_ids: Vec<String>,
}

/// Parse a TXT record. Returns `None` for a record that is not ours or not this
/// version — a foreign service on `_copypaste._tcp` is simply ignored.
///
/// Everything here is attacker-controlled, so each field is length-bounded and
/// character-checked before it can reach a log line or the peer table.
fn parse_advertisement(txt: &TxtProperties, fallback_name: &str) -> Option<Advertisement> {
    if txt.get_property_val_str(TXT_KEY_VERSION)? != TXT_VERSION {
        return None;
    }

    let name = txt
        .get_property_val_str(TXT_KEY_NAME)
        .and_then(sanitise_display_name)
        .or_else(|| sanitise_display_name(fallback_name))
        .unwrap_or_else(|| "unknown".to_string());

    // Collect `p<n>` in index order so the result is deterministic regardless
    // of how the peer ordered its strings.
    let mut indexed: Vec<(usize, String)> = Vec::new();
    for prop in txt.iter() {
        let Some(index) = prop.key().strip_prefix(TXT_KEY_PAIRING_PREFIX) else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let value = prop.val_str();
        if !is_valid_pairing_id(value) {
            continue;
        }
        indexed.push((index, value.to_string()));
    }
    indexed.sort_unstable();

    let mut pairing_ids: Vec<String> = Vec::new();
    for (_, id) in indexed {
        if pairing_ids.len() >= MAX_PAIRING_IDS_PER_PEER {
            break;
        }
        if !pairing_ids.contains(&id) {
            pairing_ids.push(id);
        }
    }

    Some(Advertisement { name, pairing_ids })
}

/// One [`DiscoveredPeer`] per pairing id the service claims, so `find` is a
/// direct lookup.
pub(super) fn peers_from_resolved(resolved: &ResolvedService, now_ms: i64) -> Vec<DiscoveredPeer> {
    let Some(advertisement) = parse_advertisement(&resolved.txt_properties, &resolved.fullname)
    else {
        return Vec::new();
    };
    if resolved.port == 0 {
        return Vec::new();
    }

    let addrs: Vec<IpAddr> = resolved.addresses.iter().map(|a| a.to_ip_addr()).collect();
    let Some(ip) = best_addr(&addrs) else {
        return Vec::new();
    };
    let addr = SocketAddr::new(ip, resolved.port);

    advertisement
        .pairing_ids
        .into_iter()
        .map(|pairing_id| DiscoveredPeer {
            pairing_id,
            name: advertisement.name.clone(),
            addr,
            last_seen_ms: now_ms,
        })
        .collect()
}

/// Prefer a routable IPv4 address: it is the one most likely to connect, and
/// picking deterministically keeps the table stable across re-resolutions.
fn best_addr(addrs: &[IpAddr]) -> Option<IpAddr> {
    let rank = |ip: &IpAddr| match ip {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => 0,
        IpAddr::V6(v6) if !v6.is_loopback() && !v6.is_unspecified() => 1,
        IpAddr::V4(_) => 2,
        IpAddr::V6(_) => 3,
    };
    addrs
        .iter()
        .min_by_key(|ip| (rank(ip), ip.to_string()))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::names::{instance_of, MAX_PAIRING_ID_LEN};
    use crate::transport::{PairingToken, TOKEN_LEN};
    use std::net::Ipv4Addr;

    /// Everything a TXT record puts on the wire, flattened, for the audit test.
    fn txt_bytes(info: &ServiceInfo) -> Vec<u8> {
        let mut out = Vec::new();
        for prop in info.get_properties().iter() {
            out.extend_from_slice(prop.key().as_bytes());
            out.push(b'=');
            out.extend_from_slice(prop.val().unwrap_or_default());
            out.push(0);
        }
        out
    }

    #[test]
    fn txt_round_trip() {
        let ids = vec![
            "pair-one".to_string(),
            "pair_two".to_string(),
            "P3".to_string(),
        ];
        let info = build_service_info("Dmitriy's Laptop", "Dmitriy's Laptop", &ids, 47_654)
            .expect("valid advertisement");

        let parsed =
            parse_advertisement(info.get_properties(), "fallback").expect("record is ours");

        assert_eq!(parsed.name, "Dmitriy's Laptop");
        assert_eq!(parsed.pairing_ids, ids);
        assert_eq!(info.get_port(), 47_654);
        assert!(info.get_fullname().ends_with(SERVICE_TYPE));
    }

    #[test]
    fn txt_round_trip_survives_a_renamed_instance() {
        let ids = vec!["pair-one".to_string()];
        // What `republish` does after a conflict rename: instance differs from
        // the display name, but the display name still round-trips.
        let info = build_service_info("Laptop (2)", "Laptop", &ids, 47_654).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "fallback").unwrap();
        assert_eq!(parsed.name, "Laptop");
        assert_eq!(
            instance_of(info.get_fullname()).as_deref(),
            Some("Laptop (2)")
        );
    }

    #[test]
    fn foreign_and_versionless_records_are_ignored() {
        let mut txt = HashMap::new();
        txt.insert("p0".to_string(), "pair-one".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "other", "other.local.", (), 1, txt).unwrap();
        assert!(parse_advertisement(info.get_properties(), "other").is_none());

        let mut txt = HashMap::new();
        txt.insert("v".to_string(), "99".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "future", "future.local.", (), 1, txt).unwrap();
        assert!(parse_advertisement(info.get_properties(), "future").is_none());
    }

    #[test]
    fn hostile_txt_fields_are_bounded_and_scrubbed() {
        let mut txt = HashMap::new();
        txt.insert("v".to_string(), TXT_VERSION.to_string());
        txt.insert("n".to_string(), "evil\u{7}\u{1b}[31mname".to_string());
        txt.insert("p0".to_string(), "ok-id".to_string());
        txt.insert("p1".to_string(), "../../etc/passwd".to_string());
        txt.insert("p2".to_string(), "x".repeat(MAX_PAIRING_ID_LEN + 1));
        txt.insert("p3".to_string(), String::new());
        txt.insert("pnotanumber".to_string(), "sneaky".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "evil", "evil.local.", (), 1, txt).unwrap();

        let parsed = parse_advertisement(info.get_properties(), "fallback").unwrap();
        assert_eq!(parsed.name, "evil[31mname");
        assert_eq!(parsed.pairing_ids, vec!["ok-id".to_string()]);
    }

    #[test]
    fn a_single_advertisement_cannot_flood_the_table() {
        let mut txt = HashMap::new();
        txt.insert("v".to_string(), TXT_VERSION.to_string());
        txt.insert("n".to_string(), "greedy".to_string());
        for i in 0..(MAX_PAIRING_IDS_PER_PEER * 4) {
            txt.insert(format!("p{i}"), format!("id-{i}"));
        }
        let info = ServiceInfo::new(SERVICE_TYPE, "greedy", "greedy.local.", (), 1, txt).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "greedy").unwrap();
        assert_eq!(parsed.pairing_ids.len(), MAX_PAIRING_IDS_PER_PEER);
    }

    #[test]
    fn advertised_pairing_ids_are_capped_and_deduplicated() {
        let mut ids: Vec<String> = (0..MAX_ADVERTISED_PAIRING_IDS * 2)
            .map(|i| format!("id-{i}"))
            .collect();
        ids.push("id-0".to_string());
        let info = build_service_info("Laptop", "Laptop", &ids, 1).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "Laptop").unwrap();
        assert_eq!(parsed.pairing_ids.len(), MAX_ADVERTISED_PAIRING_IDS);
        assert_eq!(parsed.pairing_ids[0], "id-0");
    }

    // -- the security property ------------------------------------------------

    /// The advertisement is public to anyone within radio range.
    ///
    /// Security review F-7: the claim this pins used to be "nothing derived
    /// from the token is advertised", which was false — the `pairing_id` is a
    /// truncated digest of it — and the test could not notice, because it built
    /// its record from strings like `"pair-one"` instead of from a pairing. The
    /// ids here come from real [`PairingToken`]s, the way `Node::republish`
    /// feeds `build_service_info` from the peer store, so both halves of the
    /// corrected claim are actually exercised: the id *is* advertised, and
    /// nothing that yields the token is.
    #[test]
    fn advertisement_carries_the_pairing_id_and_nothing_else_of_the_token() {
        let tokens = [PairingToken::generate(), PairingToken::generate()];
        let ids: Vec<String> = tokens.iter().map(PairingToken::pairing_id).collect();
        let info = build_service_info("Laptop", "Laptop", &ids, 47_654).unwrap();

        let rendered = String::from_utf8(txt_bytes(&info)).unwrap();
        let lowered = rendered.to_lowercase();

        // The derived id is advertised, and that is deliberate: it is what a
        // peer looks this device up by.
        for id in &ids {
            assert!(rendered.contains(id), "the pairing id must be advertised");
        }

        for token in &tokens {
            let psk = token.psk();
            let code = token.to_code();
            let bare = code.replace('-', "");

            // Not the token, in any rendering it has.
            for needle in [
                hex::encode(psk),
                hex::encode_upper(psk),
                code.clone(),
                code.to_lowercase(),
                bare.clone(),
                bare.to_lowercase(),
            ] {
                assert!(!rendered.contains(&needle), "the token reached the wire");
                assert!(!lowered.contains(&needle.to_lowercase()));
                assert!(!info.get_fullname().contains(&needle));
                assert!(!info.get_hostname().contains(&needle));
            }

            // Nor a prefix of it. "Truncated" is the specific thing the id must
            // not be: were `pairing_id` ever reduced to `hex::encode(&psk[..16])`,
            // this is what would catch it. Eight hex characters is the shortest
            // needle that cannot match the record by chance.
            for keep in [4usize, 8, 16, TOKEN_LEN] {
                let prefix = hex::encode(&psk[..keep]);
                assert!(
                    !lowered.contains(&prefix),
                    "a {keep}-byte prefix of the token reached the wire"
                );
            }
            for keep in [8usize, 16, 32] {
                assert!(
                    !lowered.contains(&bare[..keep].to_lowercase()),
                    "a prefix of the pairing code reached the wire"
                );
            }
        }

        for banned in ["psk", "token", "secret", "key"] {
            assert!(!lowered.contains(banned), "advertisement mentions {banned}");
        }

        // Pin the key set: only v, n and p<n> are ever published, so a fourth
        // kind of field cannot arrive without a decision.
        let mut keys: Vec<String> = info
            .get_properties()
            .iter()
            .map(|p| p.key().to_string())
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["n", "p0", "p1", "v"]);
    }

    #[test]
    fn address_choice_prefers_a_routable_ipv4() {
        let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let routable = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4));
        let v6: IpAddr = "fe80::1".parse().unwrap();
        assert_eq!(best_addr(&[loopback, routable, v6]), Some(routable));
        assert_eq!(best_addr(&[loopback, v6]), Some(v6));
        assert_eq!(best_addr(&[]), None);
    }
}
