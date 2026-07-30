//! One device with a history — the fixture both halves of the round trip use.
//! Test-only.

use copypaste_ipc::ConfigData;

use crate::{Detector, Keyring, Store};

pub(super) struct Fixture {
    pub store: Store,
    pub keyring: Keyring,
    pub detector: Detector,
    pub settings: ConfigData,
}

/// A device whose secret is derived from `name`, so two fixtures built with
/// different names cannot open each other's rows.
pub(super) fn fixture_named(name: &str) -> Fixture {
    let mut secret = [0u8; 32];
    for (slot, byte) in secret.iter_mut().zip(name.bytes().cycle()) {
        *slot = byte;
    }
    let keyring = Keyring::from_secret(&secret);
    Fixture {
        store: Store::open_in_memory(&keyring.db_key()).expect("in-memory store"),
        keyring,
        detector: Detector::new().expect("detector"),
        settings: ConfigData::default(),
    }
}

pub(super) fn fixture() -> Fixture {
    fixture_named("test-device")
}

impl Fixture {
    pub(super) fn add(&self, content: &str) -> String {
        self.add_typed(content, copypaste_ipc::content_type::TEXT)
    }

    pub(super) fn add_typed(&self, content: &str, content_type: &str) -> String {
        crate::ingest(
            &self.store,
            &self.detector,
            &self.keyring,
            content,
            content_type,
            &self.settings,
        )
        .expect("the fixture's own content must ingest")
        .into_item()
        .id
    }

    pub(super) fn contents(&self) -> Vec<String> {
        let key = self.keyring.item_key();
        let mut out: Vec<String> = self
            .store
            .list(1000, 0)
            .expect("the store must read")
            .into_iter()
            .map(|row| {
                let bytes = crate::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)
                    .expect("the local key must open it");
                String::from_utf8(bytes).expect("utf-8")
            })
            .collect();
        out.sort();
        out
    }
}
