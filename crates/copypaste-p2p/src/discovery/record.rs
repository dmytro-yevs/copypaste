//! The TXT record: what we advertise, and what we are willing to believe.
//!
//! Only non-secret material goes in it:
//!
//! | key | value |
//! |---|---|
//! | `v` | discovery record version, currently `1` |
//! | `n` | device display name, UTF-8 |
//! | `av`/`pv`/`pf`/`dc`/`os`/`ov`/`m` | bounded self-reported device profile |
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
use sha2::{Digest, Sha256};
use tracing::debug;

use super::names::{
    is_valid_pairing_id, sanitise_display_name, sanitise_host_label, sanitise_instance,
};
use super::table::DiscoveredPeer;
use super::DiscoveryError;
use crate::SERVICE_TYPE;
use crate::{DeviceClass, DevicePlatform, DeviceProfile};

/// Version stamped into the TXT record, so a future format change can be
/// ignored by old builds instead of misread by them.
const TXT_VERSION: &str = "1";

const TXT_KEY_VERSION: &str = "v";
const TXT_KEY_NAME: &str = "n";
const TXT_KEY_APP_VERSION: &str = "av";
const TXT_KEY_PROTOCOL_VERSION: &str = "pv";
const TXT_KEY_PLATFORM: &str = "pf";
const TXT_KEY_DEVICE_CLASS: &str = "dc";
const TXT_KEY_OS_NAME: &str = "os";
const TXT_KEY_OS_VERSION: &str = "ov";
const TXT_KEY_MODEL: &str = "m";
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
    let hostname = format!(
        "{}.local.",
        sanitise_host_label(device_name).unwrap_or_else(|| "copypaste".to_string())
    );

    let txt = advertisement_attributes(device_name, pairing_ids)?;
    let info = ServiceInfo::new(SERVICE_TYPE, &instance, &hostname, (), port, txt)?;
    Ok(info.enable_addr_auto())
}

/// Encode the shared CopyPaste TXT schema for an mDNS transport that is not
/// backed by `mdns-sd` (Android NSD).
pub fn advertisement_attributes(
    device_name: &str,
    pairing_ids: &[String],
) -> Result<HashMap<String, String>, DiscoveryError> {
    let display = sanitise_display_name(device_name).ok_or(DiscoveryError::InvalidDeviceName)?;

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

    // The pairing id remains the only field derived from the token; profile
    // values are ordinary self-reported metadata (see the module docs).
    let mut txt: HashMap<String, String> = HashMap::new();
    txt.insert(TXT_KEY_VERSION.to_string(), TXT_VERSION.to_string());
    txt.insert(TXT_KEY_NAME.to_string(), display);
    add_profile_properties(&mut txt, &DeviceProfile::current());
    for (i, id) in advertised.iter().enumerate() {
        txt.insert(format!("{TXT_KEY_PAIRING_PREFIX}{i}"), (*id).clone());
    }

    Ok(txt)
}

fn add_profile_properties(txt: &mut HashMap<String, String>, profile: &DeviceProfile) {
    if let Some(value) = &profile.app_version {
        txt.insert(TXT_KEY_APP_VERSION.to_string(), value.clone());
    }
    if let Some(value) = profile.protocol_version {
        txt.insert(TXT_KEY_PROTOCOL_VERSION.to_string(), value.to_string());
    }
    txt.insert(
        TXT_KEY_PLATFORM.to_string(),
        profile.platform.wire_name().to_string(),
    );
    txt.insert(
        TXT_KEY_DEVICE_CLASS.to_string(),
        profile.device_class.wire_name().to_string(),
    );
    for (key, value) in [
        (TXT_KEY_OS_NAME, profile.os_name.as_ref()),
        (TXT_KEY_OS_VERSION, profile.os_version.as_ref()),
        (TXT_KEY_MODEL, profile.model.as_ref()),
    ] {
        if let Some(value) = value {
            txt.insert(key.to_string(), value.clone());
        }
    }
}

/// What we were able to read out of somebody's TXT record.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advertisement {
    name: String,
    pairing_ids: Vec<String>,
    profile: Option<DeviceProfile>,
}

/// Parse a TXT record. Returns `None` for a record that is not ours or not this
/// version — a foreign service on `_copypaste._tcp` is simply ignored.
///
/// Everything here is attacker-controlled, so each field is length-bounded and
/// character-checked before it can reach a log line or the peer table.
fn parse_advertisement(
    txt: &HashMap<String, String>,
    fallback_name: &str,
) -> Option<Advertisement> {
    if txt.get(TXT_KEY_VERSION)? != TXT_VERSION {
        return None;
    }

    let name = txt
        .get(TXT_KEY_NAME)
        .map(String::as_str)
        .and_then(sanitise_display_name)
        .or_else(|| sanitise_display_name(fallback_name))
        .unwrap_or_else(|| "unknown".to_string());

    // Collect `p<n>` in index order so the result is deterministic regardless
    // of how the peer ordered its strings.
    let mut indexed: Vec<(usize, String)> = Vec::new();
    for (key, value) in txt {
        let Some(index) = key.strip_prefix(TXT_KEY_PAIRING_PREFIX) else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        if !is_valid_pairing_id(value) {
            continue;
        }
        indexed.push((index, value.clone()));
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

    Some(Advertisement {
        name,
        pairing_ids,
        profile: parse_profile(txt),
    })
}

fn parse_profile(txt: &HashMap<String, String>) -> Option<DeviceProfile> {
    let app_version = profile_text(txt, TXT_KEY_APP_VERSION);
    let protocol_version = txt
        .get(TXT_KEY_PROTOCOL_VERSION)
        .map(String::as_str)
        .and_then(|value| value.parse::<u32>().ok());
    let platform = txt
        .get(TXT_KEY_PLATFORM)
        .map(String::as_str)
        .map(DevicePlatform::from_wire_name)
        .unwrap_or_default();
    let device_class = txt
        .get(TXT_KEY_DEVICE_CLASS)
        .map(String::as_str)
        .map(DeviceClass::from_wire_name)
        .unwrap_or_default();
    let os_name = profile_text(txt, TXT_KEY_OS_NAME);
    let os_version = profile_text(txt, TXT_KEY_OS_VERSION);
    let model = profile_text(txt, TXT_KEY_MODEL);

    (app_version.is_some()
        || protocol_version.is_some()
        || platform != DevicePlatform::Unknown
        || device_class != DeviceClass::Unknown
        || os_name.is_some()
        || os_version.is_some()
        || model.is_some())
    .then_some(DeviceProfile {
        app_version,
        protocol_version,
        platform,
        device_class,
        os_name,
        os_version,
        model,
    })
}

fn profile_text(txt: &HashMap<String, String>, key: &str) -> Option<String> {
    txt.get(key).and_then(|value| sanitise_display_name(value))
}

fn attributes_from_txt(txt: &TxtProperties) -> HashMap<String, String> {
    txt.iter()
        .map(|property| (property.key().to_string(), property.val_str().to_string()))
        .collect()
}

/// Parse a resolved service whose TXT attributes came from a platform-native
/// mDNS implementation. The same validation and discovery-id derivation apply
/// to every transport.
pub fn peer_from_record(
    service_name: &str,
    host: IpAddr,
    port: u16,
    attributes: &HashMap<String, String>,
    last_seen_ms: i64,
) -> Option<DiscoveredPeer> {
    let advertisement = parse_advertisement(attributes, service_name)?;
    (port != 0).then(|| DiscoveredPeer {
        discovery_id: discovery_id(service_name),
        pairing_ids: advertisement.pairing_ids,
        name: advertisement.name,
        profile: advertisement.profile,
        addr: SocketAddr::new(host, port),
        last_seen_ms,
    })
}

/// A candidate device from one resolved mDNS service. This deliberately
/// returns a peer even with no pairing ids: discovery must find a fresh device
/// before either side has created its first pairing.
pub(super) fn peer_from_resolved(
    resolved: &ResolvedService,
    now_ms: i64,
) -> Option<DiscoveredPeer> {
    let addrs: Vec<IpAddr> = resolved.addresses.iter().map(|a| a.to_ip_addr()).collect();
    let ip = best_addr(&addrs)?;
    peer_from_record(
        &resolved.fullname,
        ip,
        resolved.port,
        &attributes_from_txt(&resolved.txt_properties),
        now_ms,
    )
}

/// Keeps the mDNS fullname (which is attacker-controlled and can be long) out
/// of the UI/IPC surface while remaining stable for this advertisement.
fn discovery_id(fullname: &str) -> String {
    let digest = Sha256::digest(fullname.as_bytes());
    hex::encode(&digest[..16])
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

    fn attrs(info: &ServiceInfo) -> HashMap<String, String> {
        attributes_from_txt(info.get_properties())
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

        let parsed = parse_advertisement(&attrs(&info), "fallback").expect("record is ours");

        assert_eq!(parsed.name, "Dmitriy's Laptop");
        assert_eq!(parsed.pairing_ids, ids);
        let profile = parsed.profile.expect("current profile");
        assert_eq!(profile.protocol_version, Some(crate::PROTOCOL_VERSION));
        assert_eq!(profile.platform, DevicePlatform::current());
        assert_eq!(info.get_port(), 47_654);
        assert!(info.get_fullname().ends_with(SERVICE_TYPE));
    }

    #[test]
    fn txt_round_trip_survives_a_renamed_instance() {
        let ids = vec!["pair-one".to_string()];
        // What `republish` does after a conflict rename: instance differs from
        // the display name, but the display name still round-trips.
        let info = build_service_info("Laptop (2)", "Laptop", &ids, 47_654).unwrap();
        let parsed = parse_advertisement(&attrs(&info), "fallback").unwrap();
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
        assert!(parse_advertisement(&attrs(&info), "other").is_none());

        let mut txt = HashMap::new();
        txt.insert("v".to_string(), "99".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "future", "future.local.", (), 1, txt).unwrap();
        assert!(parse_advertisement(&attrs(&info), "future").is_none());
    }

    #[test]
    fn platform_records_use_the_canonical_parser() {
        let ids = vec!["pair-one".to_string()];
        let attributes = advertisement_attributes("Android", &ids).unwrap();
        let peer = peer_from_record(
            "phone._copypaste._tcp.local.",
            "192.0.2.1".parse().unwrap(),
            47_654,
            &attributes,
            10_000,
        )
        .expect("canonical record");

        assert_eq!(peer.name, "Android");
        assert_eq!(peer.pairing_ids, ids);
        assert_eq!(peer.last_seen_ms, 10_000);
        assert_eq!(
            peer.profile.expect("current profile").platform,
            DevicePlatform::current()
        );

        let mut invalid = attributes;
        invalid.insert("v".into(), "99".into());
        assert!(peer_from_record(
            "phone._copypaste._tcp.local.",
            "192.0.2.1".parse().unwrap(),
            47_654,
            &invalid,
            10_000,
        )
        .is_none());
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

        let parsed = parse_advertisement(&attrs(&info), "fallback").unwrap();
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
        let parsed = parse_advertisement(&attrs(&info), "greedy").unwrap();
        assert_eq!(parsed.pairing_ids.len(), MAX_PAIRING_IDS_PER_PEER);
    }

    #[test]
    fn advertised_pairing_ids_are_capped_and_deduplicated() {
        let mut ids: Vec<String> = (0..MAX_ADVERTISED_PAIRING_IDS * 2)
            .map(|i| format!("id-{i}"))
            .collect();
        ids.push("id-0".to_string());
        let info = build_service_info("Laptop", "Laptop", &ids, 1).unwrap();
        let parsed = parse_advertisement(&attrs(&info), "Laptop").unwrap();
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

        // Pin the closed key set so new LAN disclosure needs a decision.
        let mut keys: Vec<String> = info
            .get_properties()
            .iter()
            .map(|p| p.key().to_string())
            .collect();
        keys.sort();
        let allowed = [
            "av", "dc", "m", "n", "os", "ov", "p0", "p1", "pf", "pv", "v",
        ];
        assert!(keys.iter().all(|key| allowed.contains(&key.as_str())));
        for required in ["av", "dc", "n", "p0", "p1", "pf", "pv", "v"] {
            assert!(keys.iter().any(|key| key == required), "missing {required}");
        }
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
