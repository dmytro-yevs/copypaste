//! A device's history, as a peer sync session sees it.
//!
//! Three rules the session depends on and cannot check for itself:
//!
//! * **[`summaries`] never lists a live sensitive item.** A sensitive tombstone
//!   is listed because it has no payload and is required to delete a stale peer
//!   copy. `serve_items` refuses anything outside the advertised set, while
//!   [`fetch`] filters again and the session filters a third time.
//! * **[`fetch`] returns plaintext**, decrypted under the local item key. The
//!   sender's ciphertext is useless to a peer — the AEAD binds the item id to a
//!   key derived from *this* device's secret — so content crosses the Noise
//!   channel in the clear and is re-sealed on the other side.
//! * **[`apply`] re-runs the merge.** The session decided from summaries, which
//!   carry no origin; the re-check needs both `origin_device_id`s and the row as
//!   it stands now.
//!
//! [`summaries`]: SyncSource::summaries
//! [`fetch`]: SyncSource::fetch
//! [`apply`]: SyncSource::apply

use std::sync::Arc;

use copypaste_p2p::protocol::{ItemSummary, SyncItem};
use copypaste_p2p::sync::{SyncError, SyncSource};
use tracing::warn;

use super::merge::{
    apply_remote_p2p_version_with_pin_stamp, apply_remote_version, open_version, MergeError,
    RemoteVersion,
};
use super::MSG_STORE;
use crate::sensitive::Detector;
use crate::storage::{origin_or, Store, StoredItem};
use crate::Keyring;

/// A [`SyncSource`] over a [`Store`].
///
/// Holds the keyring and the detector as well as the store: applying an item
/// needs the keyring to re-seal it and the detector to decide whether it is
/// sensitive *here*, which is not necessarily what the sender decided.
pub struct StoreSource {
    store: Store,
    keyring: Arc<Keyring>,
    detector: Arc<Detector>,
    device_id: String,
    device_name: String,
    storage_quota_bytes: u64,
    on_applied: Option<Arc<dyn Fn(i64) + Send + Sync>>,
}

impl std::fmt::Debug for StoreSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreSource")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl StoreSource {
    #[must_use]
    pub fn new(
        store: Store,
        keyring: Arc<Keyring>,
        detector: Arc<Detector>,
        device_id: String,
        device_name: String,
        storage_quota_bytes: u64,
    ) -> Self {
        Self {
            store,
            keyring,
            detector,
            device_id,
            device_name,
            storage_quota_bytes,
            on_applied: None,
        }
    }

    /// Called with the stamp of every version this source stores.
    ///
    /// The daemon uses it to pull the cloud upload floor back: an item that
    /// arrived from a peer carries the *sender's* stamp, routinely older than
    /// this device's upload cursor, so without it a peer's row would never be
    /// forwarded to the account. It is a hook rather than part of the merge
    /// because a client with no cloud account has nothing to tell.
    #[must_use]
    pub fn on_applied(mut self, f: impl Fn(i64) + Send + Sync + 'static) -> Self {
        self.on_applied = Some(Arc::new(f));
        self
    }

    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Decrypt one stored row for sending. See [`open_version`].
    #[must_use]
    pub fn open(&self, row: &StoredItem) -> Option<String> {
        open_version(&self.keyring, row)
    }

    /// Merge one remote version in, whichever transport carried it.
    pub fn apply_version(&self, incoming: &RemoteVersion<'_>) -> Result<bool, MergeError> {
        let applied = apply_remote_version(
            &self.store,
            &self.keyring,
            &self.detector,
            &self.device_id,
            incoming,
        )?;
        if applied {
            self.store
                .evict_over_byte_cap(self.storage_quota_bytes)
                .map_err(|e| {
                    warn!(error = ?e, "could not enforce storage quota after a remote merge");
                    MergeError::Store
                })?;
            if let Some(hook) = &self.on_applied {
                hook(incoming.created_at);
            }
        }
        Ok(applied)
    }

    /// The row as it will travel, or `None` when it must not.
    fn to_wire(&self, row: StoredItem) -> Option<SyncItem> {
        // A tombstone has no payload to open, and carries none on the wire
        // either (manifest 05 rule T-4).
        let content = if row.deleted {
            String::new()
        } else {
            self.open(&row)?
        };
        Some(SyncItem {
            content,
            content_type: row.content_type,
            created_at: row.created_at,
            deleted: row.deleted,
            content_hash: row.content_hash,
            origin_device_id: origin_or(&row.origin_device_id, &self.device_id).to_string(),
            pinned: row.pinned,
            pin_order: row.pin_order,
            pin_updated_at: row.pin_updated_at,
            item_id: row.id,
        })
    }
}

fn store_error(e: crate::StoreError, what: &str) -> SyncError {
    warn!(error = ?e, "{what}");
    SyncError::Source(MSG_STORE.to_string())
}

impl SyncSource for StoreSource {
    fn device_id(&self) -> String {
        self.device_id.clone()
    }

    fn device_name(&self) -> String {
        self.device_name.clone()
    }

    fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError> {
        super::blocking(|| {
            let rows = self
                .store
                .summaries(i64::MAX)
                .map_err(|e| store_error(e, "could not read item summaries for a sync session"))?;
            Ok(rows
                .into_iter()
                .map(|version| ItemSummary {
                    item_id: version.id,
                    created_at: version.created_at,
                    deleted: version.deleted,
                    content_hash: version.content_hash,
                    origin_device_id: origin_or(&version.origin_device_id, &self.device_id)
                        .to_string(),
                    pinned: version.pinned,
                    pin_order: version.pin_order,
                    pin_updated_at: version.pin_updated_at,
                })
                .collect())
        })
    }

    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        super::blocking(|| {
            let rows = self
                .store
                .versions(ids)
                .map_err(|e| store_error(e, "could not read items for a sync session"))?;
            Ok(rows
                .into_iter()
                .filter_map(|row| self.to_wire(row))
                .collect())
        })
    }

    fn apply(&self, item: SyncItem) -> Result<bool, SyncError> {
        super::blocking(|| {
            let result = apply_remote_p2p_version_with_pin_stamp(
                &self.store,
                &self.keyring,
                &self.detector,
                &self.device_id,
                &RemoteVersion {
                    item_id: &item.item_id,
                    content: &item.content,
                    content_type: &item.content_type,
                    created_at: item.created_at,
                    deleted: item.deleted,
                    // The peer protocol carries the sender's hash, and a
                    // tombstone's hash is the deleted item's — so it is passed
                    // through rather than recomputed from the empty content.
                    content_hash: Some(&item.content_hash),
                    origin_device_id: &item.origin_device_id,
                },
                item.pinned,
                item.pin_order,
                item.pin_updated_at,
            )
            .map_err(|e| SyncError::Source(e.message().to_string()))?;
            if result.content {
                if let Some(hook) = &self.on_applied {
                    hook(item.created_at);
                }
            }
            Ok(result.any())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::fts_row_count;
    use crate::sync::testkit::{fixture, fixture_named};
    use crate::NewItem;

    fn add(f: &crate::sync::testkit::Fixture, id: &str, content: &str, created_at: i64) -> String {
        let key = f.keyring.item_key();
        let (nonce, ciphertext) = crate::encrypt(content.as_bytes(), &key, id).unwrap();
        let is_sensitive = f.detector.is_sensitive(content);
        f.store
            .insert(NewItem {
                id: id.to_string(),
                content_ciphertext: ciphertext,
                nonce,
                content_type: "text".into(),
                content_hash: crate::storage::compute_content_hash(content.as_bytes()),
                is_sensitive,
                search_text: if is_sensitive {
                    None
                } else {
                    Some(content.to_string())
                },
                created_at,
            })
            .expect("insert")
            .id
    }

    fn peer_item(item_id: &str, content: &str, created_at: i64) -> SyncItem {
        SyncItem {
            item_id: item_id.into(),
            content: content.into(),
            content_type: "text".into(),
            created_at,
            deleted: false,
            content_hash: crate::storage::compute_content_hash(content.as_bytes()),
            origin_device_id: "device-a".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        }
    }

    #[test]
    fn a_locally_captured_item_is_advertised_with_this_device_as_its_origin() {
        let f = fixture();
        let id = add(&f, "mine", "shared thing", 1_000);
        let source = f.source();

        let summaries = source.summaries().unwrap();
        assert!(summaries.iter().any(|s| s.item_id == id));

        let items = source.fetch(std::slice::from_ref(&id)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "shared thing");
        assert_eq!(
            items[0].origin_device_id,
            source.device_id(),
            "an unstamped origin must leave the device as this device's"
        );
        assert_eq!(items[0].content_hash, summaries[0].content_hash);
        assert_eq!(summaries[0].origin_device_id, source.device_id());
        assert!(!summaries[0].pinned);
        assert_eq!(summaries[0].pin_order, None);
    }

    #[test]
    fn a_sensitive_item_is_neither_advertised_nor_served() {
        let f = fixture();
        let id = add(&f, "leaky", "AKIAIOSFODNN7EXAMPLE", 1_000);
        let source = f.source();

        assert!(
            !source.summaries().unwrap().iter().any(|s| s.item_id == id),
            "a sensitive item reached the advertised set"
        );
        assert!(
            source.fetch(&[id]).unwrap().is_empty(),
            "a sensitive item was served on request"
        );
    }

    #[test]
    fn a_sensitive_tombstone_is_advertised_and_served_empty() {
        let f = fixture();
        let id = add(&f, "secret", "AKIAIOSFODNN7EXAMPLE", 1_000);
        assert!(f.store.delete(&id).unwrap());
        let source = f.source();

        assert!(source
            .summaries()
            .unwrap()
            .iter()
            .any(|summary| summary.item_id == id && summary.deleted));
        let item = source
            .fetch(std::slice::from_ref(&id))
            .unwrap()
            .pop()
            .unwrap();
        assert!(item.deleted);
        assert!(item.content.is_empty());
        assert!(item.content_hash.is_empty());
    }

    #[test]
    fn an_applied_item_is_re_encrypted_under_the_local_key() {
        let f = fixture_named("beta");
        let source = f.source();

        assert!(source
            .apply(peer_item(
                "peer-item",
                "from the other device",
                1_700_000_000_000
            ))
            .unwrap());

        let row = f.store.get("peer-item").unwrap().expect("stored");
        let plain = crate::decrypt(
            &row.content_ciphertext,
            &row.nonce,
            &f.keyring.item_key(),
            "peer-item",
        )
        .expect("the local key must open it");
        assert_eq!(String::from_utf8(plain).unwrap(), "from the other device");
        assert_eq!(row.origin_device_id, "device-a");
    }

    #[test]
    fn applying_the_same_version_twice_is_a_no_op() {
        let f = fixture_named("beta");
        let source = f.source();
        let item = peer_item("peer-item", "same", 1_700_000_000_000);

        assert!(source.apply(item.clone()).unwrap());
        assert!(
            !source.apply(item).unwrap(),
            "a replayed version must not be re-applied"
        );
    }

    #[test]
    fn remote_merges_enforce_the_byte_quota_without_losing_fts_or_tombstones() {
        let f = fixture_named("beta");
        let source = StoreSource::new(
            f.store.clone(),
            Arc::clone(&f.keyring),
            Arc::clone(&f.detector),
            f.here.clone(),
            "test-device".to_string(),
            0,
        );

        let oldest = peer_item("oldest", "first remote item", 1_000);
        assert!(source.apply(oldest).unwrap());
        assert_eq!(fts_row_count(&f.store, "oldest"), 1);

        assert!(source
            .apply(peer_item("newest", "second remote item", 2_000))
            .unwrap());
        assert!(f.store.get("oldest").unwrap().is_none());
        assert_eq!(fts_row_count(&f.store, "oldest"), 0);
        assert!(f.store.get("newest").unwrap().is_some());

        assert!(source
            .apply(peer_item("deleted", "will become a tombstone", 3_000))
            .unwrap());
        let live = f.store.version("deleted").unwrap().unwrap();
        assert!(source
            .apply(SyncItem {
                item_id: "deleted".into(),
                content: String::new(),
                content_type: "text".into(),
                created_at: 4_000,
                deleted: true,
                content_hash: live.content_hash,
                origin_device_id: "device-a".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            })
            .unwrap());
        assert!(f.store.version("deleted").unwrap().unwrap().deleted);
        assert_eq!(fts_row_count(&f.store, "deleted"), 0);
    }

    #[test]
    fn a_p2p_pin_and_order_replace_the_local_state() {
        let f = fixture_named("beta");
        let source = f.source();
        let mut remote = peer_item("peer-item", "pinned elsewhere", 1_700_000_000_000);
        remote.pinned = true;
        remote.pin_order = Some(3.0);

        assert!(source.apply(remote).unwrap());

        let stored = f.store.version("peer-item").unwrap().unwrap();
        assert!(stored.pinned);
        assert_eq!(stored.pin_order, Some(3.0));
    }

    #[test]
    fn an_older_remote_version_loses_to_what_is_stored() {
        let f = fixture_named("beta");
        let source = f.source();
        assert!(source
            .apply(peer_item("peer-item", "newer", 2_000_000))
            .unwrap());
        assert!(!source
            .apply(peer_item("peer-item", "older", 1_000_000))
            .unwrap());
        assert_eq!(
            f.store.get("peer-item").unwrap().unwrap().created_at,
            2_000_000
        );
    }

    #[test]
    fn an_incoming_secret_is_flagged_by_this_devices_detector() {
        let f = fixture_named("beta");
        let source = f.source();
        assert!(source
            .apply(peer_item(
                "leaky",
                "AKIAIOSFODNN7EXAMPLE",
                1_700_000_000_000
            ))
            .unwrap());

        let row = f.store.get("leaky").unwrap().expect("stored");
        assert!(row.is_sensitive, "the local detector must have the say");
        assert!(
            f.store
                .search("AKIAIOSFODNN7EXAMPLE", 10)
                .unwrap()
                .is_empty(),
            "an applied secret reached the search index"
        );
        // And it does not go back out again.
        assert!(!source
            .summaries()
            .unwrap()
            .iter()
            .any(|s| s.item_id == "leaky"));
    }

    #[test]
    fn an_incoming_tombstone_deletes_a_local_item() {
        let f = fixture_named("beta");
        let id = add(&f, "doomed", "doomed", 1_000);
        let source = f.source();
        let local = source
            .summaries()
            .unwrap()
            .into_iter()
            .find(|s| s.item_id == id)
            .unwrap();

        assert!(source
            .apply(SyncItem {
                content: String::new(),
                deleted: true,
                created_at: local.created_at + 1,
                content_hash: local.content_hash,
                origin_device_id: source.device_id(),
                ..peer_item(&id, "", 0)
            })
            .unwrap());

        assert!(f.store.get(&id).unwrap().is_none());
        let summary = source
            .summaries()
            .unwrap()
            .into_iter()
            .find(|s| s.item_id == id)
            .expect("the tombstone is still a version");
        assert!(summary.deleted);
    }

    /// The hook exists so the daemon can pull its cloud upload floor back to a
    /// peer item's own stamp; it must fire on a stored version and on nothing
    /// else.
    #[test]
    fn the_applied_hook_reports_only_versions_that_were_stored() {
        use std::sync::Mutex;
        let f = fixture_named("beta");
        let seen: Arc<Mutex<Vec<i64>>> = Arc::default();
        let recorder = Arc::clone(&seen);
        let source = f
            .source()
            .on_applied(move |ms| recorder.lock().unwrap().push(ms));

        let item = peer_item("peer-item", "once", 5_000);
        assert!(source.apply(item.clone()).unwrap());
        assert!(!source.apply(item).unwrap());
        assert_eq!(*seen.lock().unwrap(), vec![5_000]);
    }

    #[test]
    fn a_row_this_device_cannot_decrypt_is_skipped_rather_than_failing_the_session() {
        let f = fixture_named("alpha");
        // Sealed under another device's key: the AEAD must fail closed.
        let other = fixture_named("beta");
        let key = other.keyring.item_key();
        let (nonce, ciphertext) = crate::encrypt(b"not ours", &key, "foreign").unwrap();
        f.store
            .insert(NewItem {
                id: "foreign".into(),
                content_ciphertext: ciphertext,
                nonce,
                content_type: "text".into(),
                content_hash: crate::storage::compute_content_hash(b"not ours"),
                is_sensitive: false,
                search_text: None,
                created_at: 1_000,
            })
            .unwrap();
        add(&f, "readable", "readable", 2_000);

        let source = f.source();
        let served = source
            .fetch(&["foreign".to_string(), "readable".to_string()])
            .unwrap();
        assert_eq!(served.len(), 1);
        assert_eq!(served[0].item_id, "readable");
    }
}
