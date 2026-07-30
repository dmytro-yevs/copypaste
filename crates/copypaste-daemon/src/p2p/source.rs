//! The daemon's history, as a peer sync session sees it.
//!
//! Three rules the session depends on and cannot check for itself:
//!
//! * **[`summaries`] never lists a sensitive item.** That is what makes "a
//!   sensitive item never leaves the device" a property of the protocol:
//!   `serve_items` refuses anything outside the advertised set, so an item that
//!   is not in this list cannot be requested out of us. [`fetch`] filters again,
//!   and the session filters a third time.
//! * **[`fetch`] returns plaintext**, decrypted under the local item key. The
//!   sender's ciphertext is useless to a peer — the AEAD binds the item id to a
//!   key derived from *this* device's secret — so content crosses the Noise
//!   channel in the clear and is re-sealed on the other side.
//! * **[`apply`] re-runs the merge.** The session decided from summaries, which
//!   carry no origin; the re-check needs both `origin_device_id`s and the row as
//!   it stands now. It lives in [`crate::merge`], with the cloud transport's,
//!   because one comparator for every transport is manifest 05 INV-C2.
//!
//! [`summaries`]: SyncSource::summaries
//! [`fetch`]: SyncSource::fetch
//! [`apply`]: SyncSource::apply

use std::sync::Arc;

use copypaste_p2p::protocol::{ItemSummary, SyncItem};
use copypaste_p2p::sync::{SyncError, SyncSource};
use tracing::warn;

use crate::merge::{apply_remote_version, open_version, RemoteVersion, MSG_STORE};
use crate::meta::blocking;
use crate::AppState;

/// A [`SyncSource`] over the daemon's store.
///
/// Holds the whole `AppState` rather than a `Store`: applying an item needs the
/// keyring to seal it and the detector to decide whether it is sensitive here,
/// which is not necessarily what the sender decided.
pub struct StoreSource {
    state: Arc<AppState>,
}

impl StoreSource {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl SyncSource for StoreSource {
    fn device_id(&self) -> String {
        self.state.meta.device_id().to_string()
    }

    fn device_name(&self) -> String {
        self.state.meta.device_name().to_string()
    }

    fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError> {
        blocking(|| {
            self.state.meta.summaries().map_err(|e| {
                warn!(error = ?e, "could not read item summaries for a sync session");
                SyncError::Source(MSG_STORE.to_string())
            })
        })
    }

    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        blocking(|| {
            let rows = self.state.meta.fetch(ids).map_err(|e| {
                warn!(error = ?e, "could not read items for a sync session");
                SyncError::Source(MSG_STORE.to_string())
            })?;

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                // A tombstone has no payload to open, and carries none on the
                // wire either (manifest 05 rule T-4).
                let content = if row.deleted {
                    String::new()
                } else {
                    match open_version(&self.state, &row) {
                        Some(content) => content,
                        None => continue,
                    }
                };

                items.push(SyncItem {
                    item_id: row.item_id,
                    content,
                    content_type: row.content_type,
                    created_at: row.created_at,
                    deleted: row.deleted,
                    content_hash: row.content_hash,
                    origin_device_id: row.origin_device_id,
                });
            }
            Ok(items)
        })
    }

    fn apply(&self, item: SyncItem) -> Result<bool, SyncError> {
        blocking(|| {
            let created_at = item.created_at;
            let applied = apply_remote_version(
                &self.state,
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
            )
            .map_err(|e| SyncError::Source(e.message().to_string()))?;

            if applied {
                // The item now exists here but carries the *sender's* stamp,
                // which is routinely older than this device's cloud upload
                // cursor. Without this it would never be forwarded to the
                // account.
                crate::cloud::note_version_written(&self.state, created_at);
            }
            Ok(applied)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{add, test_state};

    #[test]
    fn a_locally_captured_item_is_advertised_with_this_device_as_its_origin() {
        let (state, _dir) = test_state("alpha");
        let id = add(&state, "shared thing");
        let source = StoreSource::new(Arc::clone(&state));

        let summaries = source.summaries().unwrap();
        assert!(summaries.iter().any(|s| s.item_id == id));

        let items = source.fetch(std::slice::from_ref(&id)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "shared thing");
        assert_eq!(items[0].origin_device_id, source.device_id());
        assert_eq!(items[0].content_hash, summaries[0].content_hash);
    }

    #[test]
    fn a_sensitive_item_is_neither_advertised_nor_served() {
        let (state, _dir) = test_state("alpha");
        let id = add(&state, "AKIAIOSFODNN7EXAMPLE");
        let source = StoreSource::new(Arc::clone(&state));

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
    fn an_applied_item_is_re_encrypted_under_the_local_key() {
        let (state, dir) = test_state("beta");
        let source = StoreSource::new(Arc::clone(&state));

        let applied = source
            .apply(SyncItem {
                item_id: "peer-item".into(),
                content: "from the other device".into(),
                content_type: "text".into(),
                created_at: 1_700_000_000_000,
                deleted: false,
                content_hash: "abc123".into(),
                origin_device_id: "device-a".into(),
            })
            .unwrap();
        assert!(applied);

        // Readable through the daemon's own key...
        let row = state.store.get("peer-item").unwrap().expect("stored");
        let key = state.keyring.item_key();
        let plain = copypaste_core::decrypt(&row.content_ciphertext, &row.nonce, &key, "peer-item")
            .expect("the local key must open it");
        assert_eq!(String::from_utf8(plain).unwrap(), "from the other device");

        // ...and not present as plaintext on disk.
        let db = std::fs::read(dir.path().join("copypaste-v2.db")).unwrap();
        assert!(
            !db.windows(21).any(|w| w == b"from the other device"),
            "plaintext reached the database file"
        );
    }

    #[test]
    fn applying_the_same_version_twice_is_a_no_op() {
        let (state, _dir) = test_state("beta");
        let source = StoreSource::new(Arc::clone(&state));
        let item = SyncItem {
            item_id: "peer-item".into(),
            content: "same".into(),
            content_type: "text".into(),
            created_at: 1_700_000_000_000,
            deleted: false,
            content_hash: "abc123".into(),
            origin_device_id: "device-a".into(),
        };

        assert!(source.apply(item.clone()).unwrap());
        assert!(
            !source.apply(item).unwrap(),
            "a replayed version must not be re-applied"
        );
    }

    #[test]
    fn an_older_remote_version_loses_to_what_is_stored() {
        let (state, _dir) = test_state("beta");
        let source = StoreSource::new(Arc::clone(&state));
        let base = SyncItem {
            item_id: "peer-item".into(),
            content: "newer".into(),
            content_type: "text".into(),
            created_at: 2_000_000,
            deleted: false,
            content_hash: "hash-new".into(),
            origin_device_id: "device-a".into(),
        };
        assert!(source.apply(base.clone()).unwrap());

        let older = SyncItem {
            content: "older".into(),
            created_at: 1_000_000,
            content_hash: "hash-old".into(),
            ..base
        };
        assert!(!source.apply(older).unwrap());
        let row = state.store.get("peer-item").unwrap().unwrap();
        assert_eq!(row.created_at, 2_000_000);
    }

    #[test]
    fn an_incoming_secret_is_flagged_by_this_devices_detector() {
        let (state, _dir) = test_state("beta");
        let source = StoreSource::new(Arc::clone(&state));

        assert!(source
            .apply(SyncItem {
                item_id: "leaky".into(),
                content: "AKIAIOSFODNN7EXAMPLE".into(),
                content_type: "text".into(),
                created_at: 1_700_000_000_000,
                deleted: false,
                content_hash: "hash-secret".into(),
                origin_device_id: "device-a".into(),
            })
            .unwrap());

        let row = state.store.get("leaky").unwrap().expect("stored");
        assert!(row.is_sensitive, "the local detector must have the say");
        assert!(
            state
                .store
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
        let (state, _dir) = test_state("beta");
        let id = add(&state, "doomed");
        let source = StoreSource::new(Arc::clone(&state));
        let local = source
            .summaries()
            .unwrap()
            .into_iter()
            .find(|s| s.item_id == id)
            .unwrap();

        assert!(source
            .apply(SyncItem {
                item_id: id.clone(),
                content: String::new(),
                content_type: "text".into(),
                created_at: local.created_at + 1,
                deleted: true,
                content_hash: local.content_hash,
                origin_device_id: source.device_id(),
            })
            .unwrap());

        assert!(state.store.get(&id).unwrap().is_none());
        let summary = source
            .summaries()
            .unwrap()
            .into_iter()
            .find(|s| s.item_id == id)
            .expect("the tombstone is still a version");
        assert!(summary.deleted);
    }
}
