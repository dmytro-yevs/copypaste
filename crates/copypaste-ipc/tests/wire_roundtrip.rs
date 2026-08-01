use copypaste_ipc::{
    BackupData, CloudStatusData, CloudSyncData, ConfigApplied, ConfigData, DiagnosticCounters,
    DiscoveredData, DiscoveredDevice, ErrorCode, EventData, EventKind, ExportData, ExportItem,
    ImagePreview, ImportData, Item, ItemPage, Method, PairingData, PeerInfo, PrivateModeData,
    Request, Response, ResponseData, StatusData, SyncResult, PROTOCOL_VERSION,
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
        origin_device_id: "device-1".into(),
        origin_device_name: Some("Laptop".into()),
        source_app_bundle_id: None,
        source_app_name: None,
        too_large_to_sync: false,
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
        online: true,
    }
}

fn sync_result() -> SyncResult {
    SyncResult {
        pairing_id: "peer-1".into(),
        name: "Phone".into(),
        sent: 1,
        received: 2,
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
        ResponseData::Pairing(_) => "pairing",
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
            version: "2.0.0".into(),
            protocol_version: PROTOCOL_VERSION,
            item_count: 1,
            capture_running: true,
            clipboard_backend: "fake".into(),
            private_mode: false,
            counters: DiagnosticCounters::default(),
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
        }),
        ResponseData::Backup(BackupData { size_bytes: 3 }),
        ResponseData::Discovered(DiscoveredData {
            devices: vec![DiscoveredDevice {
                discovery_id: "peer-1".into(),
                name: "Phone".into(),
                addr: "192.0.2.1:47654".into(),
                last_seen_ms: 1,
                paired: true,
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
        ResponseData::Pairing(PairingData {
            code: "ABCD-EFGH".into(),
            pairing_id: "peer-1".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
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
        ResponseData::PrivateMode(PrivateModeData { private_mode: true }),
        ResponseData::Empty {},
    ];

    assert_eq!(variants.len(), 18);
    for variant in variants {
        assert_tagged_round_trip(variant);
    }
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
fn export_request_fields_default_to_safe_values() {
    let request: Request = serde_json::from_value(json!({
        "id": 1,
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
