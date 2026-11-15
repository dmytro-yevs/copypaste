use copypaste_core::storage::expiry::Clocks;
use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_ipc::ConfigData;

const MIN: i64 = 60 * 1000;
const DAY: i64 = 24 * 60 * MIN;

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
    fn capture(&self, text: &str) -> String {
        copypaste_core::ingest::ingest_into(
            &self.store,
            &self.keys,
            text,
            "text",
            copypaste_core::now_ms(),
            &ConfigData::default(),
        )
        .unwrap()
        .into_item()
        .id
    }

    fn expire_in(&self, id: &str, duration_ms: i64, clocks: Clocks) -> i64 {
        self.store
            .set_clip_expiry(&self.keys.item_key(), id, duration_ms, false, clocks)
            .unwrap()
            .expect("an expiring clip")
    }

    fn listed(&self) -> Vec<String> {
        self.store
            .list_from_bounded(None, 50, usize::MAX)
            .unwrap()
            .items
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    fn deadline(&self, id: &str) -> Option<i64> {
        self.store
            .item_annotations(&[id.to_owned()])
            .unwrap()
            .remove(id)
            .and_then(|annotations| annotations.expires_at)
    }
}

/// One P2P session from `from` to `to`.
fn send(from: &Device, to: &Device) {
    use copypaste_p2p::sync::SyncSource;
    let ids = from
        .source
        .summaries(0)
        .unwrap()
        .into_iter()
        .map(|summary| summary.item_id)
        .collect::<Vec<_>>();
    to.source
        .apply_batch(from.source.fetch(&ids).unwrap())
        .unwrap();
}

fn at(wall_ms: i64) -> Clocks {
    Clocks {
        wall_ms,
        boot_ms: None,
    }
}

#[test]
fn an_expired_clip_is_hidden_before_its_purge_and_the_setting_device_tombstones_it() {
    let laptop = device(41, "laptop");
    let id = laptop.capture("one-time code 482913");
    let kept = laptop.capture("kept");
    let now = copypaste_core::now_ms();
    laptop.expire_in(&id, 5 * MIN, at(now - 6 * MIN));

    assert_eq!(laptop.listed(), vec![kept.clone()]);
    assert!(laptop.store.search("482913", 10).unwrap().is_empty());
    assert!(laptop.store.get(&id).unwrap().is_some(), "not purged yet");

    // What a sweep after wake runs.
    let swept = laptop.store.expire_due(at(now)).unwrap();
    assert_eq!(swept.tombstoned, vec![id.clone()]);
    assert_eq!(swept.purged_locally, 0);
    assert!(laptop.store.version(&id).unwrap().unwrap().deleted);
    assert!(laptop.store.get(&kept).unwrap().is_some());
}

#[test]
fn a_receiver_purges_locally_drops_a_re_arrival_and_accepts_the_tombstone() {
    let (laptop, phone) = (device(42, "laptop"), device(43, "phone"));
    let id = laptop.capture("temporary");
    let now = copypaste_core::now_ms();
    let deadline = laptop.expire_in(&id, 5 * MIN, at(now));
    send(&laptop, &phone);
    assert_eq!(phone.deadline(&id), Some(deadline), "the deadline syncs");

    let swept = phone.store.expire_due(at(deadline)).unwrap();
    assert_eq!((swept.tombstoned.len(), swept.purged_locally), (0, 1));
    assert!(phone.store.version(&id).unwrap().is_none(), "no tombstone");

    // The laptop has not purged yet and offers the clip again.
    send(&laptop, &phone);
    assert!(phone.store.version(&id).unwrap().is_none());

    laptop.store.expire_due(at(deadline)).unwrap();
    send(&laptop, &phone);
    assert!(phone.store.version(&id).unwrap().is_none());
    assert!(phone.store.get(&id).unwrap().is_none());
}

#[test]
fn moving_the_wall_clock_back_does_not_extend_an_expiring_clip() {
    let laptop = device(44, "laptop");
    let id = laptop.capture("short lived");
    let now = copypaste_core::now_ms();
    // The store also reads the real boot clock while projecting the synced
    // note, so a fake one would race the host's uptime.
    let boot = copypaste_core::storage::expiry::boot_clock_ms().expect("a boot clock");
    laptop.expire_in(
        &id,
        5 * MIN,
        Clocks {
            wall_ms: now,
            boot_ms: Some(boot),
        },
    );
    let early = Clocks {
        wall_ms: now - DAY,
        boot_ms: Some(boot + 4 * MIN),
    };
    assert_eq!(laptop.store.expire_due(early).unwrap().tombstoned.len(), 0);
    let swept = laptop
        .store
        .expire_due(Clocks {
            wall_ms: now - DAY,
            boot_ms: Some(boot + 5 * MIN),
        })
        .unwrap();
    assert_eq!(swept.tombstoned, vec![id]);
}

#[test]
fn keep_clears_the_expiry_on_every_device_and_the_earlier_deadline_wins() {
    let (laptop, phone) = (device(45, "laptop"), device(46, "phone"));
    let id = laptop.capture("keep me");
    let now = copypaste_core::now_ms();
    laptop.expire_in(&id, 60 * MIN, at(now));
    let earlier = laptop.expire_in(&id, 10 * MIN, at(now));
    assert_eq!(laptop.expire_in(&id, 30 * MIN, at(now)), earlier);
    send(&laptop, &phone);
    assert_eq!(phone.deadline(&id), Some(earlier));

    let outcome = laptop
        .store
        .keep_clip(&laptop.keys.item_key(), &id, false)
        .unwrap()
        .unwrap();
    assert!(outcome.kept);
    assert_eq!(outcome.reoffer, vec![id.clone()], "it was withheld");
    send(&laptop, &phone);
    for device in [&laptop, &phone] {
        let annotations = device
            .store
            .item_annotations(std::slice::from_ref(&id))
            .unwrap()[&id]
            .clone();
        assert!(annotations.keep && annotations.expires_at.is_none());
        let swept = device.store.expire_due(at(now + DAY)).unwrap();
        assert_eq!((swept.tombstoned.len(), swept.purged_locally), (0, 0));
        assert!(device.store.get(&id).unwrap().is_some());
    }
    assert!(laptop
        .store
        .set_clip_expiry(&laptop.keys.item_key(), &id, MIN, false, at(now))
        .unwrap()
        .is_none());
}
