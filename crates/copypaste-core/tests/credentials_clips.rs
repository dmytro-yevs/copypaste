use copypaste_core::history_item::{open_item, ItemOrigin};
use copypaste_core::sync::{apply_remote_version, RemoteVersion};
use copypaste_core::{ingest_into, HistoryQuery, Keyring, Store};
use copypaste_ipc::{ConfigData, DeviceClass, SemanticKind};

const KEY: &str =
    "-----BEGIN OPENSSH PRIVATE KEY-----\nsynthetic-private-key\n-----END OPENSSH PRIVATE KEY-----";

#[test]
fn credentials_keep_their_type_without_exposing_searchable_content() {
    let keys = Keyring::from_secret(&[81; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let config = ConfigData {
        skip_secret: false,
        ..Default::default()
    };
    let item = ingest_into(&store, &keys, KEY, "text", 1000, &config)
        .unwrap()
        .into_item();
    assert!(item.clipboard_privacy().secret);
    assert!(store.search("synthetic", 10).unwrap().is_empty());
    assert!(store.semantic_work("test").unwrap().is_none());
    let query = HistoryQuery {
        semantic_kinds: vec![SemanticKind::Credentials],
        ..Default::default()
    };
    let page = store
        .query_history_bounded(&query, None, 10, usize::MAX)
        .unwrap();
    assert_eq!(page.items[0].id, item.id);
    let origin = ItemOrigin {
        device_id: "here".into(),
        device_name: None,
        device_class: DeviceClass::Unknown,
    };
    let (preview, _) = open_item(item.clone(), &origin, "here", &keys.item_key(), true).unwrap();
    assert!(preview.content.is_empty());
    assert_eq!(preview.semantic_kind, Some(SemanticKind::Credentials));
    // The cached list path must retain the label while keeping the body hidden.
    let cached = store
        .query_history_previews_with_semantic(
            &query,
            None,
            10,
            usize::MAX,
            Some("here"),
            None,
            (&keys.item_key(), &keys.preview_key()),
        )
        .unwrap();
    let row = &cached.items[0];
    let preview = row
        .preview
        .clone()
        .unwrap()
        .into_item(&row.row, &origin, "here");
    assert!(preview.content.is_empty());
    assert_eq!(preview.semantic_kind, Some(SemanticKind::Credentials));
    store.set_secret(&keys, &item.id, false).unwrap();
    let recopied = ingest_into(&store, &keys, KEY, "text", 2000, &config)
        .unwrap()
        .into_item();
    assert_eq!(recopied.id, item.id);
    assert!(!recopied.clipboard_privacy().secret);
    assert_eq!(
        store
            .query_history_bounded(&query, None, 10, usize::MAX)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn incoming_credentials_are_classified_even_without_new_sender_metadata() {
    let keys = Keyring::from_secret(&[82; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let incoming = RemoteVersion {
        item_id: "incoming-key",
        content: KEY,
        binary_content: None,
        payload_metadata: None,
        content_type: "text",
        created_at: 1000,
        deleted: false,
        content_hash: None,
        origin_device_id: "peer",
        app_bundle_id: None,
        app_name: None,
    };
    assert!(apply_remote_version(&store, &keys, "here", &incoming).unwrap());
    let row = store.get("incoming-key").unwrap().unwrap();
    assert!(row.clipboard_privacy().secret);
    assert!(store.search("synthetic", 10).unwrap().is_empty());
    let query = HistoryQuery {
        semantic_kinds: vec![SemanticKind::Credentials],
        ..Default::default()
    };
    assert_eq!(
        store
            .query_history_bounded(&query, None, 10, usize::MAX)
            .unwrap()
            .items
            .len(),
        1
    );
    let exported = copypaste_core::export(&store, &keys, 0).unwrap();
    let restored = Store::open_in_memory(&keys.db_key()).unwrap();
    copypaste_core::import(&restored, &keys, &ConfigData::default(), exported.items).unwrap();
    assert!(restored.list(10, 0).unwrap()[0].clipboard_privacy().secret);
    assert_eq!(
        restored
            .query_history_bounded(&query, None, 10, usize::MAX)
            .unwrap()
            .items
            .len(),
        1
    );
}
