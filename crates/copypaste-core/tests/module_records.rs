use copypaste_core::storage::organization::{ModuleRecord, Record, RecordKey};
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
        .put_module_record(&ka.item_key(), "example.tools", "grant/1", r#"{"n":1}"#)
        .unwrap());
    a.put_module_record(&ka.item_key(), "example.tools", "keep", "true")
        .unwrap();
    a.put_module_record(&ka.item_key(), "example.tools", "host.preferences", "{}")
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

    b.put_module_record(&kb.item_key(), "example.tools", "grant/1", r#"{"n":2}"#)
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
