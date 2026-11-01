use copypaste_core::sync::{apply_remote_version, open_version_bytes, RemoteVersion};
use copypaste_core::{Keyring, Store};
use copypaste_ipc::{ConfigData, HistoryQuery, HistoryTagMutation};

fn store(secret: u8) -> (Store, Keyring) {
    let key = Keyring::from_secret(&[secret; 32]);
    (Store::open_in_memory(&key.db_key()).unwrap(), key)
}

fn clip(store: &Store, key: &Keyring, text: &str, stamp: i64) -> String {
    copypaste_core::ingest::ingest_into(store, key, text, "text", stamp, &ConfigData::default())
        .unwrap()
        .into_item()
        .id
}

fn tag(store: &Store, key: &Keyring, name: &str) -> String {
    store
        .mutate_history_tags(
            &key.item_key(),
            &HistoryTagMutation::Save {
                id: None,
                name: name.into(),
            },
        )
        .unwrap();
    store
        .history_tags(&[])
        .unwrap()
        .tags
        .into_iter()
        .find(|tag| tag.name == name)
        .unwrap()
        .id
}

fn assign(store: &Store, key: &Keyring, id: &str, tag: &str, assigned: bool) {
    store
        .mutate_history_tags(
            &key.item_key(),
            &HistoryTagMutation::Assign {
                clip_ids: vec![id.into()],
                tag_ids: vec![tag.into()],
                assigned,
            },
        )
        .unwrap();
}

/// Uses the shared cloud-style encrypted merge; versions are deliberately sent
/// newest-first, so organization commonly arrives before the clip it describes.
fn sync(source: &Store, keys: &Keyring, target: &Store, target_keys: &Keyring, origin: &str) {
    for row in source.versions_since(0, 1000).unwrap().into_iter().rev() {
        let bytes = open_version_bytes(keys, &row).unwrap();
        let binary = copypaste_ipc::content_type::is_binary(&row.content_type);
        let content = if binary {
            ""
        } else {
            std::str::from_utf8(&bytes).unwrap()
        };
        apply_remote_version(
            target,
            target_keys,
            "receiver",
            &RemoteVersion {
                item_id: &row.id,
                content,
                binary_content: binary.then_some(bytes.as_slice()),
                payload_metadata: None,
                content_type: &row.content_type,
                created_at: row.created_at,
                deleted: false,
                content_hash: Some(&row.content_hash),
                origin_device_id: if row.origin_device_id.is_empty() {
                    origin
                } else {
                    &row.origin_device_id
                },
                app_bundle_id: None,
                app_name: None,
            },
        )
        .unwrap();
    }
}

#[test]
fn dates_filter_before_paging_and_bind_the_cursor() {
    let (store, key) = store(41);
    for stamp in 1..=20 {
        clip(&store, &key, &format!("needle {stamp}"), stamp);
    }
    let query = HistoryQuery {
        search: Some("needle".into()),
        captured_from_ms: Some(3),
        captured_before_ms: Some(7),
        ..Default::default()
    };
    let first = store
        .query_history_bounded(&query, None, 2, usize::MAX)
        .unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|row| row.created_at)
            .collect::<Vec<_>>(),
        [6, 5]
    );
    let cursor = first.next.unwrap();
    let next = store
        .query_history_bounded(&query, Some(&cursor), 2, usize::MAX)
        .unwrap();
    assert_eq!(
        next.items
            .iter()
            .map(|row| row.created_at)
            .collect::<Vec<_>>(),
        [4, 3]
    );
    let changed = HistoryQuery {
        captured_from_ms: Some(4),
        ..query
    };
    assert!(store
        .query_history_bounded(&changed, Some(&cursor), 2, usize::MAX)
        .is_err());
    assert!(store
        .query_history_bounded(
            &HistoryQuery {
                captured_from_ms: Some(7),
                captured_before_ms: Some(7),
                ..Default::default()
            },
            None,
            2,
            usize::MAX
        )
        .is_err());
}

#[test]
fn tags_survive_unpin_and_deletion_removes_them_from_filters() {
    let (store, key) = store(42);
    let id = clip(&store, &key, "saved command", 100);
    store.set_pinned_synced(&id, true, &key.item_key()).unwrap();
    let work = tag(&store, &key, "Work");
    assign(&store, &key, &id, &work, true);
    let query = HistoryQuery {
        tag_ids: vec![work.clone()],
        captured_from_ms: Some(100),
        captured_before_ms: Some(101),
        ..Default::default()
    };
    assert_eq!(
        store
            .query_history_bounded(&query, None, 10, usize::MAX)
            .unwrap()
            .items[0]
            .id,
        id
    );
    store
        .set_pinned_synced(&id, false, &key.item_key())
        .unwrap();
    assert!(store
        .query_history_bounded(&query, None, 10, usize::MAX)
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        store
            .history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id],
        vec![work.clone()]
    );
    store.set_pinned_synced(&id, true, &key.item_key()).unwrap();
    store
        .mutate_history_tags(
            &key.item_key(),
            &HistoryTagMutation::Save {
                id: Some(work.clone()),
                name: "ProjectTaxonomyUnique".into(),
            },
        )
        .unwrap();
    assert_eq!(
        store.history_tags(&[]).unwrap().tags[0].name,
        "ProjectTaxonomyUnique"
    );
    store
        .mutate_history_tags(&key.item_key(), &HistoryTagMutation::Delete { id: work })
        .unwrap();
    assert!(store
        .query_history_bounded(&query, None, 10, usize::MAX)
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        store
            .query_history_bounded(
                &HistoryQuery {
                    untagged_only: true,
                    ..Default::default()
                },
                None,
                10,
                usize::MAX
            )
            .unwrap()
            .items[0]
            .id,
        id
    );
    assert_eq!(store.get(&id).unwrap().unwrap().created_at, 100);
    assert_eq!(
        store.list(100, 0).unwrap().len(),
        1,
        "organization never enters history"
    );
    assert!(store
        .search("ProjectTaxonomyUnique", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn encrypted_sync_carries_tags_pin_order_and_unpin_without_recapturing_content() {
    let (a, ka) = store(43);
    let (b, kb) = store(44);
    let first = clip(&a, &ka, "first", 10);
    let second = clip(&a, &ka, "second", 20);
    for id in [&first, &second] {
        a.set_pinned_synced(id, true, &ka.item_key()).unwrap();
    }
    let work = tag(&a, &ka, "Work");
    assign(&a, &ka, &first, &work, true);
    a.reorder_pinned_synced(&[second.clone(), first.clone()], &ka.item_key())
        .unwrap();
    sync(&a, &ka, &b, &kb, "a");
    assert_eq!(
        b.list(10, 0)
            .unwrap()
            .iter()
            .map(|row| &row.id)
            .collect::<Vec<_>>(),
        vec![&second, &first]
    );
    assert_eq!(b.get(&first).unwrap().unwrap().created_at, 10);
    assert_eq!(
        b.history_tags(std::slice::from_ref(&first))
            .unwrap()
            .assignments[&first],
        vec![work.clone()]
    );
    b.set_pinned_synced(&first, false, &kb.item_key()).unwrap();
    sync(&b, &kb, &a, &ka, "b");
    assert!(!a.get(&first).unwrap().unwrap().pinned);
    assert_eq!(
        a.history_tags(std::slice::from_ref(&first))
            .unwrap()
            .assignments[&first],
        vec![work.clone()]
    );
    b.mutate_history_tags(&kb.item_key(), &HistoryTagMutation::Delete { id: work })
        .unwrap();
    sync(&b, &kb, &a, &ka, "b");
    sync(&a, &ka, &b, &kb, "a");
    assert!(a.history_tags(&[]).unwrap().tags.is_empty());
    assert!(b.history_tags(&[]).unwrap().tags.is_empty());
}

#[test]
fn independent_memberships_merge_and_replaying_a_removed_assignment_is_safe() {
    let (a, ka) = store(45);
    let (b, kb) = store(46);
    let id = clip(&a, &ka, "snippet", 10);
    a.set_pinned_synced(&id, true, &ka.item_key()).unwrap();
    let one = tag(&a, &ka, "One");
    let two = tag(&a, &ka, "Two");
    sync(&a, &ka, &b, &kb, "a");
    assign(&a, &ka, &id, &one, true);
    assign(&b, &kb, &id, &two, true);
    sync(&a, &ka, &b, &kb, "a");
    sync(&b, &kb, &a, &ka, "b");
    assert_eq!(
        a.history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id]
            .len(),
        2
    );
    assign(&a, &ka, &id, &one, false);
    sync(&a, &ka, &b, &kb, "a");
    sync(&b, &kb, &a, &ka, "b");
    assert_eq!(
        a.history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id],
        vec![two]
    );
    assert!(a
        .mutate_history_tags(
            &ka.item_key(),
            &HistoryTagMutation::Save {
                id: None,
                name: "one".into()
            }
        )
        .is_err());
}

#[test]
fn p2p_fetch_and_batch_apply_keep_organization_out_of_clipboard_notifications() {
    use copypaste_p2p::sync::SyncSource;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let (a, ka) = store(47);
    let (b, kb) = store(48);
    let id = clip(&a, &ka, "clipboard content", 100);
    a.set_pinned_synced(&id, true, &ka.item_key()).unwrap();
    let work = tag(&a, &ka, "Work");
    assign(&a, &ka, &id, &work, true);
    let source = copypaste_core::StoreSource::new(
        a.clone(),
        Arc::new(ka),
        "a".into(),
        "A".into(),
        ConfigData::default(),
    );
    let notifications = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&notifications);
    let target = copypaste_core::StoreSource::new(
        b.clone(),
        Arc::new(kb),
        "b".into(),
        "B".into(),
        ConfigData::default(),
    )
    .on_clip_received(move |_| {
        count.fetch_add(1, Ordering::Relaxed);
    });
    let ids = a
        .summaries(100)
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    let offered = source.fetch(&ids).unwrap();
    assert!(offered
        .iter()
        .any(|item| item.content.is_empty() && !item.binary_content.is_empty()));
    target.apply_batch(offered).unwrap();
    assert!(b.get(&id).unwrap().unwrap().pinned);
    assert_eq!(
        b.history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id],
        vec![work]
    );
    assert_eq!(notifications.load(Ordering::Relaxed), 1);
}

#[test]
fn encrypted_backup_restores_tag_catalogue_assignments_and_pins() {
    let (store, key) = store(49);
    let id = clip(&store, &key, "retained", 100);
    store.set_pinned_synced(&id, true, &key.item_key()).unwrap();
    let work = tag(&store, &key, "Work");
    assign(&store, &key, &id, &work, true);
    let directory = tempfile::tempdir().unwrap();
    let backup = directory.path().join("history.db");
    store.backup_to(&backup).unwrap();
    store
        .mutate_history_tags(
            &key.item_key(),
            &HistoryTagMutation::Delete { id: work.clone() },
        )
        .unwrap();
    store.restore_from(&backup, &key.db_key()).unwrap();
    assert_eq!(
        store
            .history_tags(std::slice::from_ref(&id))
            .unwrap()
            .assignments[&id],
        vec![work]
    );
    assert_eq!(store.history_tags(&[]).unwrap().tags[0].name, "Work");
    assert!(store.get(&id).unwrap().unwrap().pinned);
}
