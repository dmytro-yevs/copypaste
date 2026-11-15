use copypaste_core::storage::organization::{
    ModuleRecord, ModuleRecordLimits, ModuleStorageConfig, Record, RecordKey,
    MAX_LIBRARY_RECORD_VALUE_BYTES,
};
use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_ipc::ConfigData;
use copypaste_p2p::sync::SyncSource;

fn device(secret: u8, name: &str) -> (Store, Keyring, StoreSource) {
    let key = Keyring::from_secret(&[secret; 32]);
    let store = Store::open_in_memory(&key.db_key()).unwrap();
    let source = StoreSource::new(
        store.clone(),
        std::sync::Arc::new(Keyring::from_secret(&[secret; 32])),
        name.into(),
        name.into(),
        ConfigData::default(),
    );
    (store, key, source)
}

/// One P2P session: everything `from` advertises is fetched and applied.
fn session(from: &StoreSource, to: &StoreSource) -> Vec<bool> {
    let ids = from
        .summaries(0)
        .unwrap()
        .into_iter()
        .map(|summary| summary.item_id)
        .collect::<Vec<_>>();
    to.apply_batch(from.fetch(&ids).unwrap()).unwrap()
}

#[test]
fn module_records_sync_sealed_and_the_newest_write_or_deletion_wins() {
    let (a, ka, source_a) = device(70, "a");
    let (b, kb, source_b) = device(71, "b");
    assert!(a
        .put_module_record(
            &ka.item_key(),
            "example.tools",
            "grant/1",
            r#"{"n":1}"#,
            Default::default()
        )
        .unwrap());
    a.put_module_record(
        &ka.item_key(),
        "example.tools",
        "keep",
        "true",
        Default::default(),
    )
    .unwrap();
    a.put_module_record(
        &ka.item_key(),
        "example.tools",
        "host.preferences",
        "{}",
        Default::default(),
    )
    .unwrap();

    let sealed: Vec<u8> = a
        .versions_since(0, 100)
        .unwrap()
        .into_iter()
        .flat_map(|row| row.content_ciphertext)
        .collect();
    assert!(!sealed.windows(7).any(|window| window == b"grant/1"));

    assert!(session(&source_a, &source_b)
        .into_iter()
        .all(|stored| stored));
    assert_eq!(
        b.module_record("example.tools", "grant/1").unwrap(),
        Some(ModuleRecord {
            module_id: "example.tools".into(),
            key: "grant/1".into(),
            value_json: r#"{"n":1}"#.into(),
            deleted: false,
        })
    );
    assert_eq!(
        b.list(10, 0).unwrap().len(),
        0,
        "records never enter history"
    );

    b.put_module_record(
        &kb.item_key(),
        "example.tools",
        "grant/1",
        r#"{"n":2}"#,
        Default::default(),
    )
    .unwrap();
    b.delete_module_record(&kb.item_key(), "example.tools", "keep")
        .unwrap();
    session(&source_b, &source_a);
    assert_eq!(
        a.module_record("example.tools", "grant/1")
            .unwrap()
            .map(|record| record.value_json),
        Some(r#"{"n":2}"#.into())
    );
    assert_eq!(a.module_record("example.tools", "keep").unwrap(), None);
    assert!(matches!(
        a.organization_record(RecordKey::ModuleRecord {
            module_id: "example.tools",
            key: "keep",
        })
        .unwrap(),
        Some(Record::ModuleRecord(ModuleRecord { deleted: true, .. }))
    ));
    let keys: Vec<String> = a
        .module_records("example.tools", None, None, 10)
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.key)
        .collect();
    assert_eq!(keys, ["grant/1", "host.preferences"]);
}

#[test]
fn declared_storage_searches_keeps_replaced_values_and_withholds_large_records() {
    let (a, ka, source_a) = device(72, "a");
    let (b, kb, source_b) = device(73, "b");
    let config = ModuleStorageConfig {
        search_fields: vec!["title".into(), "body".into()],
        keep_conflicts: true,
    };
    let limits = ModuleRecordLimits {
        max_records: 10,
        max_value_bytes: MAX_LIBRARY_RECORD_VALUE_BYTES,
    };
    let put = |store: &Store, key: &Keyring, record: &str, value: &str| {
        assert!(store
            .put_module_record(&key.item_key(), "example.library", record, value, limits)
            .unwrap());
    };
    put(
        &a,
        &ka,
        "s/1",
        r#"{"title":"Home address","body":"Main street"}"#,
    );
    a.configure_module_storage("example.library", &config)
        .unwrap();
    put(
        &a,
        &ka,
        "s/2",
        r#"{"title":"Greeting","body":"Hello there"}"#,
    );
    let found = |store: &Store, query: &str| -> Vec<String> {
        store
            .search_module_records("example.library", query, None, 10)
            .unwrap()
            .into_iter()
            .map(|record| record.key)
            .collect()
    };
    assert_eq!(found(&a, "addr"), ["s/1"], "indexed when configured later");
    assert_eq!(found(&a, "hello"), ["s/2"]);

    b.configure_module_storage("example.library", &config)
        .unwrap();
    session(&source_a, &source_b);
    assert_eq!(found(&b, "street"), ["s/1"], "arriving records are indexed");
    put(&b, &kb, "s/2", r#"{"title":"Greeting","body":"Hi from b"}"#);
    // Stamps are milliseconds; the later write must win.
    std::thread::sleep(std::time::Duration::from_millis(5));
    put(&a, &ka, "s/2", r#"{"title":"Greeting","body":"Hi from a"}"#);
    session(&source_a, &source_b);
    let kept = b.module_record_conflicts("example.library", 10).unwrap();
    assert_eq!(kept.len(), 1);
    assert!(kept[0].value_json.contains("from b"), "{kept:?}");
    b.clear_module_record_conflicts("example.library", &[kept[0].id])
        .unwrap();
    assert!(b
        .module_record_conflicts("example.library", 10)
        .unwrap()
        .is_empty());

    let large = format!(r#"{{"title":"Large","body":"{}"}}"#, "x".repeat(70 * 1024));
    put(&a, &ka, "s/3", &large);
    let offered = |kinds| -> bool {
        let id = a
            .organization_record(RecordKey::ModuleRecord {
                module_id: "example.library",
                key: "s/3",
            })
            .unwrap()
            .is_some();
        id && a.summaries_since_for(0, None, 100, kinds).unwrap().len()
            > a.summaries_since_for(0, None, 100, 2).unwrap().len()
    };
    assert!(offered(copypaste_p2p::ORGANIZATION_KINDS));
    assert!(!a
        .put_module_record(
            &ka.item_key(),
            "example.library",
            "s/4",
            &large,
            Default::default()
        )
        .unwrap());
}
