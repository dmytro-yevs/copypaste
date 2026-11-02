#![cfg(feature = "storage")]

use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_ipc::{ConfigData, HistoryTagMutation};
use copypaste_sync::{store::StoreView, unreadable::UnreadableUploads};
use std::sync::Arc;

#[test]
fn cloud_provider_scan_and_apply_carry_independent_organization_versions() {
    let source_keys = Arc::new(Keyring::from_secret(&[71; 32]));
    let target_keys = Arc::new(Keyring::from_secret(&[72; 32]));
    let source = Store::open_in_memory(&source_keys.db_key()).unwrap();
    let target = Store::open_in_memory(&target_keys.db_key()).unwrap();
    let id = copypaste_core::ingest::ingest_into(
        &source,
        &source_keys,
        "saved snippet",
        "text",
        100,
        &ConfigData::default(),
    )
    .unwrap()
    .into_item()
    .id;
    source
        .set_pinned_synced(&id, true, &source_keys.item_key())
        .unwrap();
    source
        .mutate_history_tags(
            &source_keys.item_key(),
            &HistoryTagMutation::Save {
                id: None,
                name: "Work".into(),
            },
        )
        .unwrap();
    let tag = source.history_tags(&[]).unwrap().tags.remove(0).id;
    source
        .mutate_history_tags(
            &source_keys.item_key(),
            &HistoryTagMutation::Assign {
                clip_ids: vec![id.clone()],
                tag_ids: vec![tag.clone()],
                assigned: true,
            },
        )
        .unwrap();
    let sender = StoreView::new(
        StoreSource::new(
            source.clone(),
            source_keys,
            "source".into(),
            "Source".into(),
            ConfigData::default(),
        ),
        "source".into(),
    );
    let receiver = StoreView::new(
        StoreSource::new(
            target.clone(),
            target_keys.clone(),
            "target".into(),
            "Target".into(),
            ConfigData::default(),
        ),
        "target".into(),
    );
    let scan = sender
        .scan(&source, 0, None, 0, &UnreadableUploads::default())
        .unwrap();
    assert_eq!(
        scan.items.len(),
        4,
        "one clip and three organization records"
    );
    receiver.apply_page(scan.items).unwrap();
    let stored = target.get(&id).unwrap().unwrap();
    assert!(stored.pinned);
    assert_eq!(stored.created_at, 100);
    assert_eq!(
        target
            .history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id],
        vec![tag]
    );
    target
        .set_pinned_synced(&id, false, &target_keys.item_key())
        .unwrap();
    let scan = receiver
        .scan(&target, 0, None, 0, &UnreadableUploads::default())
        .unwrap();
    sender.apply_page(scan.items).unwrap();
    assert!(!source.get(&id).unwrap().unwrap().pinned);
}
