use copypaste_core::sync::{apply_remote_version, RemoteVersion};
use copypaste_core::{Keyring, PayloadMetadata, Store, StoredItem};
use copypaste_ipc::ConfigData;

fn record(store: &Store, keys: &Keyring) -> StoredItem {
    copypaste_core::ingest::ingest_into(
        store,
        keys,
        "toggle searchable content",
        "text",
        1000,
        &ConfigData::default(),
    )
    .unwrap()
    .into_item()
}

fn send(source: &Store, target: &Store, target_keys: &Keyring, item: &StoredItem) -> bool {
    let metadata = source.payload_metadata_for_sync(item).unwrap();
    apply_remote_version(
        target,
        target_keys,
        "target",
        &RemoteVersion {
            item_id: &item.id,
            content: "toggle searchable content",
            binary_content: None,
            payload_metadata: metadata.as_deref(),
            content_type: "text",
            created_at: item.created_at,
            deleted: false,
            content_hash: Some(&item.content_hash),
            origin_device_id: "source",
            app_bundle_id: None,
            app_name: None,
        },
    )
    .unwrap()
}

#[test]
fn toggling_preserves_identity_and_pins_and_updates_search_atomically() {
    let keys = Keyring::from_secret(&[31; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let original = record(&store, &keys);
    store.set_pinned(&original.id, true).unwrap();
    let pinned = store.get(&original.id).unwrap().unwrap();
    assert!(store.set_secret(&keys, &original.id, true).unwrap());
    let secret = store.get(&original.id).unwrap().unwrap();
    assert!(secret.clipboard_privacy().secret);
    assert!(store.search("searchable", 10).unwrap().is_empty());
    assert!(store.semantic_work("test").unwrap().is_none());
    assert!(store.set_secret(&keys, &original.id, false).unwrap());
    let visible = store.get(&original.id).unwrap().unwrap();
    assert!(!visible.clipboard_privacy().secret);
    assert_eq!(visible.content_ciphertext, original.content_ciphertext);
    assert_eq!(visible.nonce, original.nonce);
    assert_eq!(visible.content_hash, original.content_hash);
    assert_eq!(visible.pin_order, pinned.pin_order);
    assert_eq!(visible.pin_updated_at, pinned.pin_updated_at);
    assert!(visible.pinned);
    assert!(visible.created_at > secret.created_at);
    assert_eq!(store.search("searchable", 10).unwrap()[0].id, original.id);
    assert!(store.semantic_work("test").unwrap().is_some());
    assert!(!store.set_secret(&keys, "missing", true).unwrap());
    store.delete(&original.id).unwrap();
    assert!(!store.set_secret(&keys, &original.id, false).unwrap());
}

#[test]
fn explicit_choice_syncs_both_ways_and_older_producer_hints_do_not_restore_secrecy() {
    let keys = Keyring::from_secret(&[32; 32]);
    let peer_keys = Keyring::from_secret(&[33; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let peer = Store::open_in_memory(&peer_keys.db_key()).unwrap();
    let item = record(&store, &keys);
    assert!(send(&store, &peer, &peer_keys, &item));
    store.set_secret(&keys, &item.id, true).unwrap();
    let secret = store.get(&item.id).unwrap().unwrap();
    assert!(send(&store, &peer, &peer_keys, &secret));
    assert!(
        peer.get(&item.id)
            .unwrap()
            .unwrap()
            .clipboard_privacy()
            .secret
    );
    peer.set_secret(&peer_keys, &item.id, false).unwrap();
    let public = peer.get(&item.id).unwrap().unwrap();
    assert!(send(&peer, &store, &keys, &public));
    let received = store.get(&item.id).unwrap().unwrap();
    assert!(!copypaste_core::sync::InstantClipboard::default().apply(
        &store,
        &keys,
        &received,
        ConfigData::default,
        |_| panic!("a privacy action must not replace the system clipboard"),
    ));
    assert!(
        !store
            .get(&item.id)
            .unwrap()
            .unwrap()
            .clipboard_privacy()
            .secret
    );
    assert_eq!(store.search("searchable", 10).unwrap().len(), 1);
    // A peer can recopy a stale secret version with a later content timestamp.
    let mut stale = secret;
    stale.created_at = public.created_at + 100;
    assert!(send(&store, &peer, &peer_keys, &stale));
    let merged = peer.get(&item.id).unwrap().unwrap();
    assert!(!merged.clipboard_privacy().secret);
    assert!(merged.created_at > stale.created_at);
    assert_eq!(peer.search("searchable", 10).unwrap().len(), 1);
    assert!(send(&peer, &store, &keys, &merged));
    assert!(!send(&peer, &store, &keys, &merged));
    assert!(
        !store
            .get(&item.id)
            .unwrap()
            .unwrap()
            .clipboard_privacy()
            .secret
    );
}

#[test]
fn failed_decryption_does_not_declassify_or_restore_search() {
    let keys = Keyring::from_secret(&[34; 32]);
    let wrong = Keyring::from_secret(&[35; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let item = record(&store, &keys);
    store.set_secret(&keys, &item.id, true).unwrap();
    let before = store.get(&item.id).unwrap().unwrap();
    assert!(store.set_secret(&wrong, &item.id, false).is_err());
    assert_eq!(store.get(&item.id).unwrap().unwrap(), before);
    assert!(store.search("searchable", 10).unwrap().is_empty());
}

#[test]
fn manual_choice_is_retained_on_recapture() {
    let keys = Keyring::from_secret(&[36; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let item = record(&store, &keys);
    store.set_secret(&keys, &item.id, false).unwrap();
    let mut metadata = PayloadMetadata::default();
    metadata.privacy.secret = true;
    let config = ConfigData {
        skip_secret: false,
        ..Default::default()
    };
    let recopy =
        copypaste_core::ingest::ingest_into_with_capture_source_metadata_with_current_retention(
            &store,
            &keys,
            "toggle searchable content",
            "text",
            copypaste_core::now_ms() + 1,
            None,
            None,
            Some(&metadata),
            &config,
            || config.clone(),
        )
        .unwrap()
        .into_item();
    assert_eq!(recopy.id, item.id);
    assert!(!recopy.clipboard_privacy().secret);
    assert_eq!(store.search("searchable", 10).unwrap().len(), 1);
}
