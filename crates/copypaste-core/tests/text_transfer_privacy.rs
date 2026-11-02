use copypaste_core::{Keyring, PayloadMetadata, Store};
use copypaste_ipc::{ClipboardPrivacy, ConfigData, ExportData, ExportItem};

#[test]
fn portable_text_round_trip_preserves_secret_classification_without_search_leaks() {
    let keys = Keyring::from_secret(&[71; 32]);
    let source = Store::open_in_memory(&keys.db_key()).unwrap();
    let metadata = PayloadMetadata {
        privacy: ClipboardPrivacy {
            secret: true,
            transient: true,
        },
        ..Default::default()
    };
    let row = copypaste_core::ingest::ingest_into_with_capture_source_metadata(
        &source,
        &keys,
        "synthetic confidential transfer",
        "text",
        copypaste_core::now_ms(),
        None,
        None,
        Some(&metadata),
        &ConfigData::default(),
    )
    .unwrap()
    .into_item();
    source.set_pinned(&row.id, true).unwrap();
    let exported = copypaste_core::export(&source, &keys, 0).unwrap();
    let wire = serde_json::to_vec(&exported).unwrap();
    let decoded: ExportData = serde_json::from_slice(&wire).unwrap();
    let target_keys = Keyring::from_secret(&[72; 32]);
    let target = Store::open_in_memory(&target_keys.db_key()).unwrap();
    copypaste_core::import(
        &target,
        &target_keys,
        &ConfigData::default(),
        decoded.items.clone(),
    )
    .unwrap();
    let imported = target.list(10, 0).unwrap();
    assert_eq!(imported[0].clipboard_privacy(), metadata.privacy);
    assert!(imported[0].pinned);
    assert!(target.search("confidential", 10).unwrap().is_empty());
    assert!(target.semantic_work("test").unwrap().is_none());

    // Importing into an existing ordinary row must not lose the incoming flag.
    let existing = Store::open_in_memory(&target_keys.db_key()).unwrap();
    copypaste_core::ingest::ingest_into(
        &existing,
        &target_keys,
        "synthetic confidential transfer",
        "text",
        row.created_at,
        &ConfigData::default(),
    )
    .unwrap();
    copypaste_core::import(
        &existing,
        &target_keys,
        &ConfigData::default(),
        decoded.items,
    )
    .unwrap();
    assert!(existing.list(10, 0).unwrap()[0].clipboard_privacy().secret);
    assert!(existing.search("confidential", 10).unwrap().is_empty());
}

#[test]
fn legacy_text_exports_remain_ordinary_and_keep_their_wire_shape() {
    let legacy = r#"{"content":"legacy","content_type":"text","created_at":1,"pinned":false}"#;
    let item: ExportItem = serde_json::from_str(legacy).unwrap();
    assert!(item.privacy.is_empty());
    assert_eq!(
        serde_json::to_value(&item).unwrap(),
        serde_json::from_str::<serde_json::Value>(legacy).unwrap()
    );
}
