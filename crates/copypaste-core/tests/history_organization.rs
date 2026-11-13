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
    transfer(source, keys, target, target_keys, origin, |_| true);
}

fn transfer(
    source: &Store,
    keys: &Keyring,
    target: &Store,
    target_keys: &Keyring,
    origin: &str,
    only: impl Fn(&copypaste_core::StoredItem) -> bool,
) {
    for row in source.versions_since(0, 1000).unwrap().into_iter().rev() {
        if !only(&row) {
            continue;
        }
        let bytes = if row.deleted {
            Default::default()
        } else {
            open_version_bytes(keys, &row).unwrap()
        };
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
                binary_content: (binary && !row.deleted).then_some(bytes.as_slice()),
                payload_metadata: None,
                content_type: &row.content_type,
                created_at: row.created_at,
                deleted: row.deleted,
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
fn page_item_tags_are_name_ordered_and_bounded() {
    let (store, key) = store(41);
    let id = clip(&store, &key, "tagged", 100);
    store.set_pinned_synced(&id, true, &key.item_key()).unwrap();
    let mut names: Vec<String> = (0..=copypaste_ipc::MAX_ITEM_TAG_IDS)
        .map(|index| format!("tag {index:02}"))
        .collect();
    let ids: Vec<String> = names
        .iter()
        .rev()
        .map(|name| tag(&store, &key, name))
        .collect();
    for tag_id in &ids {
        assign(&store, &key, &id, tag_id, true);
    }
    let catalogue = store.history_tags(&[]).unwrap().tags;
    let shown: Vec<String> = store.item_tag_ids(std::slice::from_ref(&id)).unwrap()[&id]
        .iter()
        .map(|tag_id| {
            catalogue
                .iter()
                .find(|tag| &tag.id == tag_id)
                .unwrap()
                .name
                .clone()
        })
        .collect();
    names.truncate(copypaste_ipc::MAX_ITEM_TAG_IDS);
    assert_eq!(shown, names);
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
fn a_legacy_record_copy_in_history_leaves_one_synced_version_per_id() {
    use copypaste_p2p::sync::SyncSource;
    let (store, key) = store(50);
    let id = clip(&store, &key, "pinned", 1_000);
    store.set_pinned_synced(&id, true, &key.item_key()).unwrap();
    let record = store
        .summaries(10)
        .unwrap()
        .into_iter()
        .find(|row| row.id != id)
        .unwrap()
        .id;
    // A build without organization records stored a peer's older copy of the
    // same record as an ordinary binary clip.
    let text = format!(r#"{{"kind":"pin","clip_id":"{id}","pinned":true,"order":1.0}}"#);
    store
        .insert(copypaste_core::NewItem {
            id: record.clone(),
            content_ciphertext: copypaste_core::binary::seal(
                text.as_bytes(),
                &key.item_key(),
                &record,
            )
            .unwrap(),
            nonce: Vec::new(),
            content_type: "application/vnd.copypaste.organization+json".into(),
            content_hash: copypaste_core::compute_content_hash(text.as_bytes()),
            search_text: None,
            created_at: 500,
            app_bundle_id: None,
            app_name: None,
            payload_metadata: None,
        })
        .unwrap();

    let source = copypaste_core::StoreSource::new(
        store,
        std::sync::Arc::new(key),
        "device".into(),
        "Device".into(),
        ConfigData::default(),
    );
    let advertised = source.summaries(0).unwrap();
    let ids = advertised
        .iter()
        .map(|summary| summary.item_id.clone())
        .collect::<Vec<_>>();
    let served = source
        .fetch(&ids)
        .unwrap()
        .into_iter()
        .map(|item| (item.item_id.clone(), item.summary()))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(served.len(), advertised.len(), "one id has two versions");
    for summary in advertised {
        assert_eq!(served.get(&summary.item_id), Some(&summary));
    }
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
    store
        .backup_to(&backup, &key.db_key(), |_, _| true)
        .unwrap();
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

use copypaste_core::storage::organization::{
    AiGrantRecord, ClipNoteRecord, ContentRuleRecord, PipelineRecord, PreferenceRecord, Provenance,
    ProvenanceKind, Record, RecordKey, RuleOrigin, RulePlacementRecord, SetMemberRecord,
    SnippetBodyRecord, SnippetCollectionRecord, SnippetRecord, TransformRef,
};

const ORGANIZATION: &str = "application/vnd.copypaste.organization+json";

fn write(store: &Store, key: &Keyring, record: Record) {
    store
        .write_organization_record(&key.item_key(), record)
        .unwrap();
}

fn record_id(key: &str) -> String {
    use sha2::Digest;
    format!(
        "org:v1:{}",
        hex::encode(sha2::Sha256::digest(key.as_bytes()))
    )
}

fn record(store: &Store, key: RecordKey<'_>) -> Option<Record> {
    store.organization_record(key).unwrap()
}

fn source(store: &Store, secret: u8, device: &str) -> copypaste_core::StoreSource {
    copypaste_core::StoreSource::new(
        store.clone(),
        std::sync::Arc::new(Keyring::from_secret(&[secret; 32])),
        device.into(),
        device.into(),
        ConfigData::default(),
    )
}

/// One P2P session: everything `from` advertises is fetched and applied.
fn p2p(from: &copypaste_core::StoreSource, to: &copypaste_core::StoreSource) -> Vec<bool> {
    use copypaste_p2p::sync::SyncSource;
    let ids = from
        .summaries(0)
        .unwrap()
        .into_iter()
        .map(|summary| summary.item_id)
        .collect::<Vec<_>>();
    to.apply_batch(from.fetch(&ids).unwrap()).unwrap()
}

fn snippet(id: &str, title: &str) -> SnippetRecord {
    SnippetRecord {
        id: id.into(),
        collection_id: Some("work".into()),
        title: title.into(),
        abbreviation: Some(";Sig".into()),
        template: true,
        pinned: true,
        pin_order: Some(1.5),
        order_key: "a0".into(),
        concealed: false,
        deleted_at: None,
        purged: false,
        created_at: 10,
        created_device: "device-a".into(),
    }
}

fn body(id: &str, revision: char, text: &str) -> SnippetBodyRecord {
    SnippetBodyRecord {
        id: id.into(),
        revision: revision.to_string().repeat(32),
        ancestors: Vec::new(),
        text: text.into(),
        rich_html: None,
    }
}

fn grant(revoked: bool, name: &str) -> AiGrantRecord {
    AiGrantRecord {
        id: "grant-1".into(),
        owner_device: "device-a".into(),
        name: name.into(),
        client_name: "Editor".into(),
        client_executable: None,
        scopes: vec!["read_history".into()],
        lookback: "7d".into(),
        created_at: 10,
        last_used_day: None,
        revoked,
        revoked_at: revoked.then_some(20),
        deleted: false,
    }
}

fn note(clip_id: &str, hidden: bool, changed_at: i64, base: i64, keep: bool) -> ClipNoteRecord {
    ClipNoteRecord {
        clip_id: clip_id.into(),
        hidden_from_ai: hidden,
        hidden_changed_at: changed_at,
        base_stamp: base,
        keep,
        expires_at: None,
        expiry_duration_ms: None,
        provenance: None,
    }
}

#[test]
fn every_library_kind_round_trips_through_a_p2p_session_into_its_projection() {
    let (a, ka) = store(60);
    let (b, _) = store(61);
    let clip_id = clip(&a, &ka, "described clip", 100);
    let records = [
        Record::Snippet(snippet("s1", "Signature")),
        Record::SnippetBody(body("s1", 'a', "Kind regards")),
        Record::SnippetCollection(SnippetCollectionRecord {
            id: "work".into(),
            name: "Work".into(),
            icon: "briefcase".into(),
            accent: "blue".into(),
            order_key: "a0".into(),
            deleted_at: None,
        }),
        Record::ContentRule(ContentRuleRecord {
            id: "builtin.credentials".into(),
            origin: RuleOrigin::Builtin,
            builtin_version: Some(1),
            customized_fields: vec!["actions".into()],
            name: "Credentials".into(),
            definition_json: r#"{"match":"all","conditions":[]}"#.into(),
            device_scope: Vec::new(),
            schema_version: 1,
            effective_from: 50,
            deleted_at: None,
            base_stamp: None,
        }),
        Record::ContentRulePlacement(RulePlacementRecord {
            id: "builtin.credentials".into(),
            enabled: true,
            order_key: "a0".into(),
        }),
        Record::Pipeline(PipelineRecord {
            id: "p1".into(),
            name: "Clean URL".into(),
            description: None,
            steps_json: r#"[{"transform_id":"url.clean","version":1}]"#.into(),
            deleted_at: None,
        }),
        Record::Preference(PreferenceRecord {
            namespace: "snippets".into(),
            key: "record_pasted".into(),
            value_json: "true".into(),
        }),
        Record::SetMember(SetMemberRecord {
            set: "ai_policy.excluded_apps".into(),
            member: "com.example.vault".into(),
            present: true,
        }),
        Record::AiGrant(grant(false, "Editor grant")),
        Record::ClipNote(ClipNoteRecord {
            provenance: Some(Provenance {
                kind: ProvenanceKind::Transformed,
                from_clip_id: Some("origin".into()),
                transform_snapshot: Some(vec![TransformRef {
                    transform_id: "url.clean".into(),
                    version: 1,
                }]),
                pipeline_name: None,
                grant_name: None,
            }),
            ..note(&clip_id, true, 5, 0, true)
        }),
    ];
    for record in &records {
        write(&a, &ka, record.clone());
    }
    assert!(p2p(&source(&a, 60, "a"), &source(&b, 61, "b"))
        .into_iter()
        .all(|stored| stored));
    let keys = [
        RecordKey::Snippet("s1"),
        RecordKey::SnippetBody("s1"),
        RecordKey::SnippetCollection("work"),
        RecordKey::ContentRule("builtin.credentials"),
        RecordKey::ContentRulePlacement("builtin.credentials"),
        RecordKey::Pipeline("p1"),
        RecordKey::Preference {
            namespace: "snippets",
            key: "record_pasted",
        },
        RecordKey::SetMember {
            set: "ai_policy.excluded_apps",
            member: "com.example.vault",
        },
        RecordKey::AiGrant("grant-1"),
        RecordKey::ClipNote(&clip_id),
    ];
    for (key, expected) in keys.into_iter().zip(&records) {
        assert_eq!(record(&b, key).as_ref(), Some(expected), "{key:?}");
    }
    assert_eq!(
        b.list(10, 0).unwrap().len(),
        1,
        "records never enter history"
    );
    assert!(b.search("Kind regards", 10).unwrap().is_empty());
}

#[test]
fn a_record_of_an_unknown_kind_is_declined_and_never_becomes_a_clip() {
    let (store, key) = store(62);
    let text = r#"{"kind":"future_kind","id":"x"}"#;
    let id = record_id("future_kind:x");
    let stored = apply_remote_version(
        &store,
        &key,
        "receiver",
        &RemoteVersion {
            item_id: &id,
            content: "",
            binary_content: Some(text.as_bytes()),
            payload_metadata: None,
            content_type: ORGANIZATION,
            created_at: 10,
            deleted: false,
            content_hash: None,
            origin_device_id: "newer-peer",
            app_bundle_id: None,
            app_name: None,
        },
    )
    .unwrap();
    assert!(!stored);
    assert!(store.version(&id).unwrap().is_none());
    assert_eq!(store.count().unwrap(), 0);
}

#[test]
fn a_concurrent_unhide_or_unrevoke_loses_and_is_written_back() {
    let (a, ka) = store(63);
    let (b, kb) = store(64);
    let clip_id = clip(&a, &ka, "private", 100);
    sync(&a, &ka, &b, &kb, "a");
    write(
        &a,
        &ka,
        Record::ClipNote(note(&clip_id, false, 0, 0, false)),
    );
    write(&a, &ka, Record::AiGrant(grant(false, "Original")));
    sync(&a, &ka, &b, &kb, "a");

    write(
        &a,
        &ka,
        Record::ClipNote(note(&clip_id, true, 2_000, 1_000, false)),
    );
    write(&a, &ka, Record::AiGrant(grant(true, "Original")));
    // Later on the other device, before it learned of the hide and revocation.
    std::thread::sleep(std::time::Duration::from_millis(5));
    write(
        &b,
        &kb,
        Record::ClipNote(note(&clip_id, false, 0, 1_000, true)),
    );
    write(&b, &kb, Record::AiGrant(grant(false, "Renamed")));

    sync(&b, &kb, &a, &ka, "b");
    assert_eq!(
        record(&a, RecordKey::ClipNote(&clip_id)),
        Some(Record::ClipNote(note(&clip_id, true, 2_000, 1_000, false)))
    );
    assert_eq!(
        record(&a, RecordKey::AiGrant("grant-1")),
        Some(Record::AiGrant(grant(true, "Original")))
    );
    sync(&a, &ka, &b, &kb, "a");
    assert_eq!(
        record(&b, RecordKey::ClipNote(&clip_id)),
        Some(Record::ClipNote(note(&clip_id, true, 2_000, 1_000, false)))
    );
    assert_eq!(
        record(&b, RecordKey::AiGrant("grant-1")),
        Some(Record::AiGrant(grant(true, "Original")))
    );
}

#[test]
fn a_purge_stays_purged_whatever_the_stamp_order() {
    let (a, ka) = store(65);
    let (b, kb) = store(66);
    write(&a, &ka, Record::Snippet(snippet("s1", "Address")));
    write(
        &a,
        &ka,
        Record::SnippetBody(body("s1", 'a', "1 Main Street")),
    );
    sync(&a, &ka, &b, &kb, "a");

    let stub = SnippetRecord {
        title: String::new(),
        abbreviation: None,
        purged: true,
        ..snippet("s1", "")
    };
    write(&a, &ka, Record::Snippet(stub.clone()));
    write(&a, &ka, Record::SnippetBody(body("s1", 'b', "")));
    // An edit made later on a device that has not seen the purge.
    std::thread::sleep(std::time::Duration::from_millis(5));
    write(&b, &kb, Record::Snippet(snippet("s1", "Home address")));
    write(
        &b,
        &kb,
        Record::SnippetBody(body("s1", 'c', "2 Main Street")),
    );

    sync(&b, &kb, &a, &ka, "b");
    sync(&a, &ka, &b, &kb, "a");
    for store in [&a, &b] {
        assert_eq!(
            record(store, RecordKey::Snippet("s1")),
            Some(Record::Snippet(stub.clone()))
        );
        let Some(Record::SnippetBody(body)) = record(store, RecordKey::SnippetBody("s1")) else {
            panic!("the body projection is missing");
        };
        assert!(body.text.is_empty());
    }
}

#[test]
fn clip_notes_wait_for_their_clip_and_only_notes_accept_a_tombstone() {
    let (a, ka) = store(67);
    let (b, kb) = store(68);
    let clip_id = clip(&a, &ka, "noted", 100);
    write(&a, &ka, Record::ClipNote(note(&clip_id, true, 1, 0, false)));
    write(
        &a,
        &ka,
        Record::ClipNote(note("never-arrives", true, 1, 0, false)),
    );
    transfer(&a, &ka, &b, &kb, "a", |row| {
        row.content_type == ORGANIZATION
    });
    assert!(record(&b, RecordKey::ClipNote(&clip_id)).is_some());
    sync(&a, &ka, &b, &kb, "a");

    let month = 31 * 24 * 60 * 60 * 1000;
    assert_eq!(
        b.prune_orphan_clip_notes(copypaste_core::now_ms()).unwrap(),
        0
    );
    assert_eq!(
        b.prune_orphan_clip_notes(copypaste_core::now_ms() + month)
            .unwrap(),
        1
    );
    assert!(record(&b, RecordKey::ClipNote("never-arrives")).is_none());
    assert!(record(&b, RecordKey::ClipNote(&clip_id)).is_some());

    assert!(a.tombstone_clip_note(&clip_id).unwrap());
    sync(&a, &ka, &b, &kb, "a");
    assert!(record(&b, RecordKey::ClipNote(&clip_id)).is_none());

    write(&a, &ka, Record::Snippet(snippet("s1", "Kept")));
    sync(&a, &ka, &b, &kb, "a");
    let snippet_row = b.version(&record_id("snippet:s1")).unwrap().unwrap();
    let forged = apply_remote_version(
        &b,
        &kb,
        "receiver",
        &RemoteVersion {
            item_id: &snippet_row.id,
            content: "",
            binary_content: None,
            payload_metadata: None,
            content_type: ORGANIZATION,
            created_at: snippet_row.created_at + 1,
            deleted: true,
            content_hash: Some(&snippet_row.content_hash),
            origin_device_id: "a",
            app_bundle_id: None,
            app_name: None,
        },
    )
    .unwrap();
    assert!(!forged, "only a clip note may be tombstoned");
    assert!(record(&b, RecordKey::Snippet("s1")).is_some());
}

#[test]
fn deleting_a_clip_tombstones_its_note_with_one_version_everywhere() {
    let (a, ka) = store(69);
    let (b, kb) = store(70);
    let clip_id = clip(&a, &ka, "short lived", 100);
    write(&a, &ka, Record::ClipNote(note(&clip_id, true, 1, 0, false)));
    sync(&a, &ka, &b, &kb, "a");
    let note_id = record_id(&format!("clip_note:{clip_id}"));
    let written = a
        .organization_record_stamp(RecordKey::ClipNote(&clip_id))
        .unwrap();
    assert_eq!(
        written,
        Some(a.version(&note_id).unwrap().unwrap().created_at)
    );

    assert!(a.delete(&clip_id).unwrap());
    sync(&a, &ka, &b, &kb, "a");

    let versions = [&a, &b].map(|store| {
        let row = store.version(&note_id).unwrap().unwrap();
        (row.deleted, row.created_at, row.content_hash)
    });
    assert!(versions[0].0, "the note is tombstoned with its clip");
    assert_eq!(versions[0], versions[1]);
    for store in [&a, &b] {
        assert!(record(store, RecordKey::ClipNote(&clip_id)).is_none());
        assert!(store
            .organization_record_stamp(RecordKey::ClipNote(&clip_id))
            .unwrap()
            .is_none());
    }
}
