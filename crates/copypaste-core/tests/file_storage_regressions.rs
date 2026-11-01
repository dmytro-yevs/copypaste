use copypaste_core::{file_reference, file_transfer, FileMetadata, Keyring, Store};
use copypaste_ipc::ConfigData;

#[test]
fn restoring_a_backup_keeps_deferred_files_copyable_and_servable() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keyring::from_secret(&[76; 32]);
    let store = Store::open(&dir.path().join("history.db"), &keys.db_key()).unwrap();
    let path = dir.path().join("original.bin");
    std::fs::write(&path, b"original file bytes").unwrap();
    let row = file_reference::import(
        &store,
        &keys,
        path.to_str().unwrap(),
        "original.bin",
        "application/octet-stream",
        &ConfigData::default(),
        None,
    )
    .unwrap();
    let reference = store.file_reference(&row.id).unwrap().unwrap();
    let backup = dir.path().join("history.backup");
    store.backup_to(&backup).unwrap();
    store.restore_from(&backup, &keys.db_key()).unwrap();
    assert_eq!(store.file_reference(&row.id).unwrap(), Some(reference));
    let mut source = file_reference::source(&store, &row.id).unwrap().unwrap();
    assert_eq!(source.read_chunk().unwrap(), b"original file bytes");
    source.verify().unwrap();
    let restored = store.get(&row.id).unwrap().unwrap();
    let copy = file_transfer::prepare_copy(&store, &keys.item_key(), &restored, u64::MAX)
        .unwrap()
        .unwrap();
    assert_eq!(copy.payload.source_reference(), path.to_str());
}

#[test]
fn secrecy_changes_keep_retained_file_bytes_in_the_retention_budget() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keyring::from_secret(&[77; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let path = dir.path().join("retained.bin");
    std::fs::write(&path, vec![42; 32 * 1024]).unwrap();
    let row = file_transfer::import(
        &store,
        &keys,
        &path,
        FileMetadata::new("retained.bin", "application/octet-stream").unwrap(),
        copypaste_core::now_ms(),
        None,
        None,
        None,
        &ConfigData::default(),
        true,
    )
    .unwrap();
    store.set_secret(&keys, &row.id, true).unwrap();
    store.set_secret(&keys, &row.id, false).unwrap();
    copypaste_core::ingest_into(
        &store,
        &keys,
        "newest",
        "text",
        copypaste_core::now_ms() + 100,
        &ConfigData::default(),
    )
    .unwrap();
    assert_eq!(store.evict_over_byte_cap(2048).unwrap(), 1);
    assert!(store.get(&row.id).unwrap().is_none());
}
