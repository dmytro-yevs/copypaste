use copypaste_core::rules::builtin;
use copypaste_core::storage::expiry::Clocks;
use copypaste_core::storage::organization::{
    ContentRuleRecord, Record, RuleOrigin, RulePlacementRecord,
};
use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_ipc::{ConfigData, HistoryQuery, HistoryTagMutation};
use copypaste_p2p::sync::SyncSource;

const MIN: i64 = 60 * 1000;
const DAY: i64 = 24 * 60 * MIN;
const PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----";

struct Device {
    store: Store,
    keys: Keyring,
    source: StoreSource,
}

fn device(secret: u8, name: &str) -> Device {
    let keys = Keyring::from_secret(&[secret; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let id = store.device_identity(name).unwrap().device_id;
    let source = StoreSource::new(
        store.clone(),
        std::sync::Arc::new(Keyring::from_secret(&[secret; 32])),
        id,
        name.into(),
        ConfigData::default(),
    );
    Device {
        store,
        keys,
        source,
    }
}

impl Device {
    fn capture_at(&self, text: &str, created_at: i64) -> String {
        copypaste_core::ingest::ingest_into(
            &self.store,
            &self.keys,
            text,
            "text",
            created_at,
            &ConfigData::default(),
        )
        .unwrap()
        .into_item()
        .id
    }

    fn write(&self, record: Record) {
        self.store
            .write_organization_record(&self.keys.item_key(), record)
            .unwrap();
    }

    fn add_rule(&self, id: &str, definition: &str, effective_from: i64) {
        self.write(Record::ContentRule(ContentRuleRecord {
            id: id.into(),
            origin: RuleOrigin::User,
            builtin_version: None,
            customized_fields: Vec::new(),
            name: id.into(),
            definition_json: definition.into(),
            device_scope: Vec::new(),
            schema_version: 1,
            effective_from,
            deleted_at: None,
            base_stamp: None,
        }));
        self.write(Record::ContentRulePlacement(RulePlacementRecord {
            id: id.into(),
            enabled: true,
            order_key: "m".into(),
        }));
    }

    /// Every built-in, the credential expiry one turned on.
    fn seed_builtins(&self, now: i64) {
        for rule in builtin::builtins() {
            self.write(Record::ContentRule(rule.record(now)));
            self.write(Record::ContentRulePlacement(RulePlacementRecord {
                enabled: true,
                ..rule.placement()
            }));
        }
    }

    fn annotations(&self, id: &str) -> copypaste_ipc::payload::ItemAnnotations {
        self.store
            .item_annotations(&[id.to_owned()])
            .unwrap()
            .remove(id)
            .unwrap_or_default()
    }

    fn live(&self) -> usize {
        self.store
            .list_from_bounded(None, 500, usize::MAX)
            .unwrap()
            .items
            .len()
    }

    fn tombstones(&self) -> usize {
        self.store
            .summaries(10_000)
            .unwrap()
            .iter()
            .filter(|version| version.deleted)
            .count()
    }
}

/// One P2P session from `from` to `to`, offering `ids` in this order, or
/// everything `from` holds.
fn send_ids(from: &Device, to: &Device, ids: Option<Vec<String>>) {
    let ids = ids.unwrap_or_else(|| {
        from.source
            .summaries(0)
            .unwrap()
            .into_iter()
            .map(|summary| summary.item_id)
            .collect()
    });
    to.source
        .apply_batch(from.source.fetch(&ids).unwrap())
        .unwrap();
}

fn send(from: &Device, to: &Device) {
    send_ids(from, to, None);
}

fn at(wall_ms: i64) -> Clocks {
    Clocks {
        wall_ms,
        boot_ms: None,
    }
}

#[test]
fn a_received_clip_gets_local_protection_and_rule_tags_but_keeps_its_content() {
    let (laptop, phone) = (device(61, "laptop"), device(62, "phone"));
    let now = copypaste_core::now_ms();
    phone
        .store
        .mutate_history_tags(
            &phone.keys.item_key(),
            &HistoryTagMutation::Save {
                id: None,
                name: "Alpha".into(),
            },
        )
        .unwrap();
    let tag = phone.store.history_tags(&[]).unwrap().tags[0].id.clone();
    phone.add_rule(
        "protect-alpha",
        &format!(
            r#"{{"match":"all","conditions":[{{"type":"content_text","op":"contains","value":"alpha"}}],
            "actions":[{{"type":"mark_secret"}},{{"type":"hide_from_integrations"}},{{"type":"add_tags","tag_ids":["{tag}"]}},
            {{"type":"transform","transform_id":"case.upper"}},{{"type":"delete_after","duration_ms":600000}}]}}"#
        ),
        now - MIN,
    );
    let id = laptop.capture_at("alpha notes", now);
    let other = laptop.capture_at("beta notes", now);
    send(&laptop, &phone);

    let annotations = phone.annotations(&id);
    assert!(annotations.rule_secret && annotations.hidden_from_integrations);
    assert_eq!(annotations.rule_tag_ids, vec![tag.clone()]);
    let deadline = annotations.expires_at.expect("a local deadline");
    assert!((now + 10 * MIN..=copypaste_core::now_ms() + 10 * MIN).contains(&deadline));
    assert_eq!(phone.annotations(&other), Default::default());
    // Not transformed, out of the search index, and found by its rule tag
    // although it is not pinned.
    let row = phone.store.get(&id).unwrap().unwrap();
    let text = copypaste_core::decrypt(
        &row.content_ciphertext,
        &row.nonce,
        &phone.keys.item_key(),
        &id,
    )
    .unwrap();
    assert_eq!(text.as_slice(), b"alpha notes");
    assert!(phone.store.search("alpha", 10).unwrap().is_empty());
    let tagged = phone
        .store
        .query_history_bounded(
            &HistoryQuery {
                tag_ids: vec![tag],
                ..Default::default()
            },
            None,
            10,
            usize::MAX,
        )
        .unwrap();
    assert_eq!(
        tagged.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
        [&id]
    );
    assert_eq!(laptop.annotations(&id), Default::default());

    // The receiver's expiry deletes its own copy only.
    let swept = phone.store.expire_due(at(deadline)).unwrap();
    assert_eq!((swept.tombstoned.len(), swept.purged_locally), (0, 1));
    send(&phone, &laptop);
    assert!(laptop.store.get(&id).unwrap().is_some());
    assert_eq!(laptop.tombstones(), 0);
}

#[test]
fn a_clip_arriving_after_its_deadline_is_never_stored_and_an_unknown_tombstone_is_taken() {
    let (laptop, phone) = (device(63, "laptop"), device(64, "phone"));
    let now = copypaste_core::now_ms();
    let id = laptop.capture_at("one-time code 482913", now - 20 * MIN);
    laptop
        .store
        .set_clip_expiry(
            &laptop.keys.item_key(),
            &id,
            5 * MIN,
            false,
            at(now - 10 * MIN),
        )
        .unwrap()
        .expect("an expiring clip");
    let gone = laptop.capture_at("deleted before it ever synced", now);
    assert!(laptop.store.delete(&gone).unwrap());

    // The note first, so the clip arrives with its deadline already known.
    let ids: Vec<String> = laptop
        .source
        .summaries(0)
        .unwrap()
        .into_iter()
        .map(|summary| summary.item_id)
        .filter(|item| item != &id)
        .collect();
    send_ids(&laptop, &phone, Some(ids));
    send(&laptop, &phone);
    assert!(phone.store.version(&id).unwrap().is_none());
    assert!(phone.store.get(&id).unwrap().is_none());
    assert!(phone.store.version(&gone).unwrap().unwrap().deleted);

    send(&laptop, &phone);
    assert!(
        phone.store.version(&id).unwrap().is_none(),
        "never re-stored"
    );
}

#[test]
fn pairing_with_old_credential_history_deletes_nothing_on_either_device() {
    let (laptop, phone) = (device(65, "laptop"), device(66, "phone"));
    let now = copypaste_core::now_ms();
    let key = laptop.keys.item_key();
    let ids: Vec<String> = (0..100)
        .map(|index| {
            laptop.capture_at(
                &format!("{PRIVATE_KEY}\nb3BlbnNzaC1rZXktdjE{index}\n"),
                now - 30 * DAY + index * MIN,
            )
        })
        .collect();
    laptop.store.set_pinned_synced(&ids[0], true, &key).unwrap();
    laptop.store.keep_clip(&key, &ids[1], false).unwrap();
    for device in [&laptop, &phone] {
        device.seed_builtins(now);
        device.add_rule(
            "expire-everything",
            r#"{"match":"all","conditions":[{"type":"size","unit":"bytes","op":"greater_than","value":0}],
            "actions":[{"type":"delete_after","duration_ms":60000}]}"#,
            now,
        );
    }

    // Copied after the rules took effect: the receiver protects this one.
    let fresh = laptop.capture_at(&format!("{PRIVATE_KEY}\nZnJlc2g\n"), now + MIN);

    send(&laptop, &phone);
    send(&phone, &laptop);
    send(&laptop, &phone);

    assert!(phone.annotations(&fresh).expires_at.is_some());
    assert!(laptop.annotations(&fresh).expires_at.is_none());
    for (device, purged) in [(&laptop, 0), (&phone, 1)] {
        assert_eq!(device.live(), 101);
        for id in &ids {
            assert!(device.annotations(id).expires_at.is_none(), "{id}");
        }
        let swept = device.store.expire_due(at(now + DAY)).unwrap();
        assert_eq!((swept.tombstoned.len(), swept.purged_locally), (0, purged));
        assert_eq!(device.live(), 101 - purged);
    }
    send(&phone, &laptop);
    assert_eq!((laptop.tombstones(), phone.tombstones()), (0, 0));
    assert_eq!(laptop.live(), 101);
}
