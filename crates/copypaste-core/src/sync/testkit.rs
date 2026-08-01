//! A store, a keyring, a detector and a device id — one device, for the merge
//! and source tests. Test-only.

use std::sync::Arc;

use super::{RemoteVersion, StoreSource};
use crate::{Detector, Keyring, Store};

pub(super) struct Fixture {
    pub store: Store,
    pub keyring: Arc<Keyring>,
    pub detector: Arc<Detector>,
    pub here: String,
}

/// A device whose secret is derived from `name`, so two fixtures built with
/// different names cannot open each other's rows — which is what makes
/// "re-encrypted under the local key" a meaningful assertion.
pub(super) fn fixture_named(name: &str) -> Fixture {
    let mut secret = [0u8; 32];
    for (slot, byte) in secret.iter_mut().zip(name.bytes().cycle()) {
        *slot = byte;
    }
    let keyring = Keyring::from_secret(&secret);
    let store = Store::open_in_memory(&keyring.db_key()).expect("in-memory store");
    let here = store.device_identity(name).expect("identity").device_id;
    Fixture {
        store,
        keyring: Arc::new(keyring),
        detector: Arc::new(Detector::new().expect("detector")),
        here,
    }
}

pub(super) fn fixture() -> Fixture {
    fixture_named("test-device")
}

impl Fixture {
    pub(super) fn source(&self) -> StoreSource {
        StoreSource::new(
            self.store.clone(),
            Arc::clone(&self.keyring),
            Arc::clone(&self.detector),
            self.here.clone(),
            "test-device".to_string(),
        )
    }
}

/// A live version from another device, with no hash of its own — the cloud
/// shape, which is the one that exercises the recompute.
pub(super) fn version<'a>(
    item_id: &'a str,
    content: &'a str,
    created_at: i64,
) -> RemoteVersion<'a> {
    RemoteVersion {
        item_id,
        content,
        binary_content: None,
        payload_metadata: None,
        content_type: "text",
        created_at,
        deleted: false,
        content_hash: None,
        origin_device_id: "device-a",
    }
}
