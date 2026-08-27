use copypaste_ipc::{
    BackupData, CloudStatusData, CloudSyncData, ConfigApplied, ConfigData, DeviceDetails,
    DeviceObservationProvenance, DeviceObservationTrust, DevicePresence, DevicePresenceObservation,
    DiagnosticCounters, DiscoveredData, DiscoveredDevice, ErrorCode, EventData, EventKind,
    ExportData, ExportItem, ImagePreview, ImportData, Item, ItemPage, Method, PairingInviteData,
    PairingProgressData, PairingRole, PairingState, PeerInfo, PrivateModeData, Request, Response,
    ResponseData, SensitiveFinding, SensitiveSpan, StatusData, SyncResult, PROTOCOL_VERSION,
};
use serde_json::{json, Value};

fn item() -> Item {
    Item {
        id: "item-1".into(),
        content: "hello".into(),
        content_type: "text/plain".into(),
        created_at: 1,
        pinned: false,
        is_sensitive: false,
        sensitive_finding: None,
        origin_device_id: "device-1".into(),
        origin_device_name: Some("Laptop".into()),
        source_app_bundle_id: None,
        source_app_name: None,
        too_large_to_sync: false,
        truncated: false,
    }
}

fn export_item() -> ExportItem {
    ExportItem {
        content: "hello".into(),
        content_type: "text/plain".into(),
        created_at: 1,
        pinned: false,
        is_sensitive: false,
    }
}

fn peer() -> PeerInfo {
    PeerInfo {
        pairing_id: "peer-1".into(),
        name: "Phone".into(),
        last_addr: Some("192.0.2.1:47654".into()),
        last_seen_ms: 1,
        online: false,
        details: Some(DeviceDetails {
            presence: Some(DevicePresenceObservation {
                state: DevicePresence::Online,
                last_seen_ms: 1,
                provenance: DeviceObservationProvenance::Observed,
                trust: DeviceObservationTrust::Local,
                observed_at_ms: 0,
                fresh_until_ms: Some(i64::MAX),
            }),
            ..DeviceDetails::default()
        }),
    }
}

#[test]
fn peer_online_is_a_fail_closed_presence_projection_on_the_wire() {
    let current = peer();
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

fn sync_result() -> SyncResult {
    SyncResult {
        pairing_id: "peer-1".into(),
        name: "Phone".into(),
        sent: 1,
        received: 2,
        skipped_too_large: Some(0),
        duration_ms: Some(42),
        error: None,
        error_code: None,
    }
}

fn variant_tag(data: &ResponseData) -> &'static str {
    match data {
        ResponseData::Status(_) => "status",
        ResponseData::Export(_) => "export",
        ResponseData::Import(_) => "import",
        ResponseData::Backup(_) => "backup",
        ResponseData::Discovered(_) => "discovered",
        ResponseData::Config(_) => "config",
        ResponseData::Event(_) => "event",
        ResponseData::Page(_) => "page",
        ResponseData::Item(_) => "item",
        ResponseData::ImagePreview(_) => "image_preview",
        ResponseData::Count(_) => "count",
        ResponseData::PairingInvite(_) => "pairing_invite",
        ResponseData::PairingProgress(_) => "pairing_progress",
        ResponseData::Peers(_) => "peers",
        ResponseData::Sync(_) => "sync",
        ResponseData::CloudStatus(_) => "cloud_status",
        ResponseData::CloudSync(_) => "cloud_sync",
        ResponseData::PrivateMode(_) => "private_mode",
        ResponseData::Empty {} => "empty",
    }
}

fn assert_tagged_round_trip(data: ResponseData) {
    let expected_tag = variant_tag(&data);
    let encoded = serde_json::to_value(&data).expect("response data serialises");
    let object = encoded
        .as_object()
        .expect("response data is a wrapper object");
    assert_eq!(object.len(), 1, "{encoded}");
    assert!(object.contains_key(expected_tag), "{encoded}");

    let decoded: ResponseData =
        serde_json::from_value(encoded.clone()).expect("response data deserialises");
    assert_eq!(variant_tag(&decoded), expected_tag);
    assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
}

#[test]
fn every_response_data_variant_has_a_distinct_round_trip() {
    let variants = vec![
        ResponseData::Status(StatusData {
            device_name: "Laptop".into(),
            version: "2.0.0".into(),
            protocol_version: PROTOCOL_VERSION,
            listen_addr: Some("192.0.2.1:47654".into()),
            device_details: None,
            item_count: 1,
            capture_running: true,
            clipboard_backend: "fake".into(),
            private_mode: false,
            private_mode_epoch: 0,
            counters: DiagnosticCounters::default(),
            settings_health: None,
        }),
        ResponseData::Export(ExportData {
            items: vec![export_item()],
            skipped_non_text: 1,
            skipped_sensitive: 2,
            skipped_undecryptable: 3,
        }),
        ResponseData::Import(ImportData {
            inserted: 1,
            skipped: 2,
            skipped_duplicate: 1,
            skipped_empty: 1,
            skipped_too_large: 0,
            pins_failed: 1,
        }),
        ResponseData::Backup(BackupData { size_bytes: 3 }),
        ResponseData::Discovered(DiscoveredData {
            devices: vec![DiscoveredDevice {
                discovery_id: "peer-1".into(),
                name: "Phone".into(),
                addr: "192.0.2.1:47654".into(),
                last_seen_ms: 1,
                paired: true,
                details: None,
            }],
        }),
        ResponseData::Config(ConfigApplied {
            config: ConfigData::default(),
            restart_required: vec!["lan_visibility".into()],
        }),
        ResponseData::Event(EventData {
            event: EventKind::Items,
            item_count: 1,
            captured: true,
            swept: 0,
        }),
        ResponseData::Page(ItemPage {
            items: vec![item()],
            skipped_undecryptable: 1,
            next_cursor: Some("cursor".into()),
        }),
        ResponseData::Item(item()),
        ResponseData::ImagePreview(ImagePreview {
            png_base64: "iVBORw0KGgo=".into(),
            width: 1,
            height: 1,
        }),
        ResponseData::Count(1),
        ResponseData::PairingInvite(PairingInviteData {
            code: "ABCD-EFGH".into(),
            pairing_id: "peer-1".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
            expires_in_secs: 120,
        }),
        ResponseData::PairingProgress(PairingProgressData {
            pairing_id: Some("peer-1".into()),
            role: Some(PairingRole::Initiator),
            state: PairingState::AwaitingConfirmation,
            expires_in_ms: Some(60_000),
            sas: Some("123456".into()),
            peer_device_id: Some("device-1".into()),
            peer_name: Some("Phone".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code: None,
        }),
        ResponseData::Peers(vec![peer()]),
        ResponseData::Sync(vec![sync_result()]),
        ResponseData::CloudStatus(CloudStatusData {
            configured: true,
            signed_in: true,
            key_ready: true,
            email: Some("user@example.com".into()),
            last_sync_ms: Some(1),
            last_error: None,
            poll_interval_secs: 30,
            unreadable_uploads: 2,
        }),
        ResponseData::CloudSync(CloudSyncData {
            uploaded: 1,
            tombstoned: 2,
            downloaded: 3,
            applied: 4,
            skipped_sensitive: 5,
            skipped_undecryptable: 6,
            skipped_forged: 7,
            skipped_future: 8,
            skipped_too_large: 9,
        }),
        ResponseData::PrivateMode(PrivateModeData {
            private_mode: true,
            private_mode_epoch: 7,
        }),
        ResponseData::Empty {},
    ];

    assert_eq!(variants.len(), 19);
    for variant in variants {
        assert_tagged_round_trip(variant);
    }
}

#[test]
fn retired_pairing_response_shape_is_not_part_of_the_wire_contract() {
    let retired = json!({
        "pairing": {
            "code": "ABCD-EFGH",
            "pairing_id": "peer-1",
            "listen_addr": "192.0.2.1:47654"
        }
    });
    assert!(serde_json::from_value::<ResponseData>(retired).is_err());
}

#[test]
fn collection_variants_round_trip_when_empty() {
    for variant in [
        ResponseData::Export(ExportData {
            items: Vec::new(),
            skipped_non_text: 0,
            skipped_sensitive: 0,
            skipped_undecryptable: 0,
        }),
        ResponseData::Discovered(DiscoveredData {
            devices: Vec::new(),
        }),
        ResponseData::Config(ConfigApplied {
            config: ConfigData::default(),
            restart_required: Vec::new(),
        }),
        ResponseData::Page(ItemPage {
            items: Vec::new(),
            skipped_undecryptable: 0,
            next_cursor: None,
        }),
        ResponseData::Peers(Vec::new()),
        ResponseData::Sync(Vec::new()),
    ] {
        assert_tagged_round_trip(variant);
    }
}

#[test]
fn empty_sync_and_peer_lists_decode_as_their_own_variants() {
    let sync: ResponseData = serde_json::from_value(json!({"sync": []})).unwrap();
    let peers: ResponseData = serde_json::from_value(json!({"peers": []})).unwrap();
    assert!(matches!(sync, ResponseData::Sync(results) if results.is_empty()));
    assert!(matches!(peers, ResponseData::Peers(results) if results.is_empty()));
}

#[test]
fn unknown_error_code_does_not_invalidate_the_response() {
    let wire = json!({
        "id": 7,
        "ok": false,
        "error": "a newer daemon refused the request",
        "error_code": "future_daemon_state"
    });
    let response: Response = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(response.error_code, None);
    assert_eq!(
        response.raw_error_code.as_deref(),
        Some("future_daemon_state")
    );
    assert_eq!(serde_json::to_value(response).unwrap(), wire);
}

#[test]
fn known_error_code_still_maps_to_the_stable_enum() {
    let response: Response = serde_json::from_value(json!({
        "id": 7,
        "ok": false,
        "error": "no such item",
        "error_code": "not_found"
    }))
    .unwrap();
    assert_eq!(response.error_code, Some(ErrorCode::NotFound));
    assert_eq!(response.raw_error_code, None);
}

#[test]
fn unsupported_content_is_a_non_retryable_wire_error() {
    let response = Response::err(
        8,
        ErrorCode::UnsupportedContent,
        "that representation is unavailable",
    );
    let wire = serde_json::to_value(&response).unwrap();
    assert_eq!(wire["error_code"], "unsupported_content");

    let decoded: Response = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded.error_code, Some(ErrorCode::UnsupportedContent));
    assert!(!decoded.error_code.unwrap().retryable());
}

#[test]
fn sync_error_code_is_additive_and_omitted_when_absent() {
    let old_wire = json!({
        "pairing_id": "peer-1",
        "name": "Phone",
        "sent": 0,
        "received": 0,
        "error": "the peer stopped responding"
    });
    let old: SyncResult = serde_json::from_value(old_wire.clone()).unwrap();
    assert_eq!(old.error_code, None);
    assert_eq!(old.skipped_too_large, None);
    assert_eq!(serde_json::to_value(old).unwrap(), old_wire);

    let current = SyncResult {
        error_code: Some(ErrorCode::PeerUnreachable),
        ..sync_result()
    };
    assert_eq!(
        serde_json::to_value(current).unwrap()["error_code"],
        "peer_unreachable"
    );
}

#[test]
fn sync_size_refusal_count_is_additive_and_strict_when_present() {
    let current = SyncResult {
        skipped_too_large: Some(3),
        ..sync_result()
    };
    assert_eq!(
        serde_json::to_value(current).unwrap()["skipped_too_large"],
        3
    );

    for value in [
        json!(null),
        json!(-1),
        json!(1.5),
        json!("3"),
        json!(4_294_967_296u64),
    ] {
        let invalid = json!({
            "pairing_id": "peer-1",
            "name": "Phone",
            "sent": 0,
            "received": 0,
            "error": null,
            "skipped_too_large": value,
        });
        assert!(serde_json::from_value::<SyncResult>(invalid).is_err());
    }
}

#[test]
fn export_request_fields_default_to_safe_values() {
    let request: Request = serde_json::from_value(json!({
        "id": 1,
        "protocol_version": 2,
        "method": "export",
        "params": {}
    }))
    .unwrap();
    assert_eq!(request.protocol_version, PROTOCOL_VERSION);
    assert!(matches!(
        request.method,
        Method::Export {
            limit: 0,
            include_sensitive: false
        }
    ));
}

#[test]
fn device_name_request_is_typed_and_round_trips() {
    let request = Request {
        id: 9,
        protocol_version: PROTOCOL_VERSION,
        method: Method::SetDeviceName {
            name: "Kitchen Mac".into(),
        },
    };
    let wire = serde_json::to_value(&request).unwrap();
    assert_eq!(wire["method"], "set_device_name");
    assert_eq!(wire["params"]["name"], "Kitchen Mac");

    let decoded: Request = serde_json::from_value(wire).unwrap();
    assert!(matches!(
        decoded.method,
        Method::SetDeviceName { name } if name == "Kitchen Mac"
    ));
}

#[test]
fn omitted_import_sensitivity_defaults_to_false() {
    let wire: Value = json!({
        "content": "hello",
        "content_type": "text/plain",
        "created_at": 1,
        "pinned": false
    });
    let item: ExportItem = serde_json::from_value(wire).unwrap();
    assert!(!item.is_sensitive);
}

#[test]
fn an_item_with_no_truncated_flag_reads_as_a_whole_body() {
    let wire: Value = json!({
        "id": "item-1",
        "content": "hello",
        "content_type": "text/plain",
        "created_at": 1,
        "pinned": false,
        "is_sensitive": false
    });
    let item: Item = serde_json::from_value(wire).unwrap();
    assert!(!item.truncated);
    assert!(item.sensitive_finding.is_none());
}

#[test]
fn an_inert_sensitive_finding_round_trips_as_additive_metadata() {
    let mut item = item();
    item.sensitive_finding = Some(SensitiveFinding {
        label: "email".into(),
        spans: vec![SensitiveSpan { start: 5, end: 22 }],
        spans_truncated: false,
        redacted_preview: "mail ***REDACTED***".into(),
    });

    let wire = serde_json::to_value(&item).unwrap();
    assert_eq!(wire["sensitive_finding"]["label"], "email");
    assert_eq!(wire["sensitive_finding"]["spans"][0]["start"], 5);
    assert_eq!(
        wire["sensitive_finding"]["redacted_preview"],
        "mail ***REDACTED***"
    );
    let back: Item = serde_json::from_value(wire).unwrap();
    assert_eq!(back.sensitive_finding, item.sensitive_finding);
}

#[test]
fn a_bounded_list_body_says_so_on_the_wire() {
    let mut content = "x".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES * 2);
    assert!(copypaste_ipc::limits::bound_preview(&mut content));

    let mut cut = item();
    cut.content = content;
    cut.truncated = true;
    let wire = serde_json::to_value(&cut).unwrap();
    assert_eq!(wire["truncated"], json!(true));
    assert_eq!(
        wire["content"].as_str().unwrap().len(),
        copypaste_ipc::limits::LIST_PREVIEW_BYTES
    );

    let back: Item = serde_json::from_value(wire).unwrap();
    assert!(back.truncated);
}
