mod support;

use copypaste_ipc::{DiscoveredDevice, PeerInfo};
use serde_json::json;

#[test]
fn peer_online_is_a_fail_closed_presence_projection_on_the_wire() {
    let current = support::populated_peer();
    let encoded = serde_json::to_value(&current).unwrap();
    assert_eq!(encoded["online"], true);

    for (state, observed_at_ms, fresh_until_ms, expected) in [
        ("online", 0, 0, false),
        ("online", i64::MAX, i64::MAX, false),
        ("online", 0, i64::MAX, true),
        ("unknown", 0, i64::MAX, false),
        ("offline", 0, i64::MAX, false),
    ] {
        let mut value = encoded.clone();
        value["online"] = json!(true);
        value["details"]["presence"]["state"] = json!(state);
        value["details"]["presence"]["observed_at_ms"] = json!(observed_at_ms);
        value["details"]["presence"]["fresh_until_ms"] = json!(fresh_until_ms);
        let decoded: PeerInfo = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.online, expected);
        assert_eq!(serde_json::to_value(decoded).unwrap()["online"], expected);
    }

    let mut contradictory = encoded;
    contradictory["online"] = json!(false);
    let decoded: PeerInfo = serde_json::from_value(contradictory).unwrap();
    assert!(decoded.online);
}

#[test]
fn discovered_details_remain_an_optional_wire_field() {
    let device = DiscoveredDevice {
        discovery_id: "peer-1".into(),
        name: "Phone".into(),
        addr: "192.0.2.1:47654".into(),
        last_seen_ms: 1,
        paired: false,
        details: None,
    };
    let encoded = serde_json::to_value(&device).unwrap();
    assert!(encoded.get("details").is_none());
    assert!(serde_json::from_value::<DiscoveredDevice>(encoded)
        .unwrap()
        .details
        .is_none());
}
