//! The daemon's history, as a sync session sees it.
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
//!   carry no origin; this is the only place both `origin_device_id`s exist, and
//!   the only layer that sees a local write that landed while the session was in
//!   flight. Skipping the re-check is how the two halves of a session disagree.
//!
//! [`summaries`]: SyncSource::summaries
//! [`fetch`]: SyncSource::fetch
//! [`apply`]: SyncSource::apply

use std::sync::Arc;

use copypaste_p2p::protocol::{ItemSummary, SyncItem};
use copypaste_p2p::sync::{merge_decision, MergeDecision, SyncError, SyncSource};
use tracing::{debug, warn};

use crate::p2p::meta::IncomingVersion;
use crate::AppState;

/// Fixed, pathless sentences. A `SyncError::Source` is logged by the peer and
/// shown to a user, so it follows the same rule as an IPC error (`CLAUDE.md`
/// rule 4).
const MSG_METADATA: &str = "the history database could not be read";
const MSG_ENCRYPT: &str = "the incoming item could not be encrypted";

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
        self.state.p2p.meta.device_id().to_string()
    }

    fn device_name(&self) -> String {
        self.state.p2p.meta.device_name().to_string()
    }

    fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError> {
        blocking(|| {
            self.state.p2p.meta.summaries().map_err(|e| {
                warn!(error = ?e, "could not read item summaries for a sync session");
                SyncError::Source(MSG_METADATA.to_string())
            })
        })
    }

    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        blocking(|| {
            let rows = self.state.p2p.meta.fetch(ids).map_err(|e| {
                warn!(error = ?e, "could not read items for a sync session");
                SyncError::Source(MSG_METADATA.to_string())
            })?;

            let key = self.state.keyring.item_key();
            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                // A tombstone has no payload to open, and carries none on the
                // wire either (manifest 05 rule T-4).
                let content = if row.deleted {
                    String::new()
                } else {
                    let (Some(ciphertext), Some(nonce)) =
                        (row.content_ciphertext.as_ref(), row.nonce.as_ref())
                    else {
                        warn!(id = %row.item_id, "live item has no payload; not sending it");
                        continue;
                    };
                    // The item id is the AAD, exactly as `server::to_wire` uses
                    // it. One unreadable row must not fail the whole session —
                    // the rest of the history is still worth syncing.
                    match copypaste_core::decrypt(ciphertext, nonce, &key, &row.item_id) {
                        Ok(plaintext) => String::from_utf8_lossy(&plaintext).into_owned(),
                        Err(e) => {
                            warn!(id = %row.item_id, error = ?e, "skipping an item that failed to decrypt");
                            continue;
                        }
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
            let meta = &self.state.p2p.meta;
            let local = meta.local_version(&item.item_id).map_err(|e| {
                warn!(error = ?e, "could not read the local version of an incoming item");
                SyncError::Source(MSG_METADATA.to_string())
            })?;

            // The re-check. `plan` compared summaries, which have no origin and
            // were taken before this session started writing; this comparison
            // uses the true origin on both sides and the row as it stands now.
            if let Some(local) = &local {
                if merge_decision(
                    &local.summary,
                    &local.origin_device_id,
                    &item.summary(),
                    &item.origin_device_id,
                ) == MergeDecision::KeepLocal
                {
                    debug!(id = %item.item_id, "local version wins; not applying");
                    return Ok(false);
                }
            }

            // Sensitivity is decided *here*, by this device's detector, not by
            // whatever the sender thought. A tombstone has no content to judge,
            // so it inherits the flag from the row it is deleting — otherwise
            // deleting a flagged item would publish its hash to every peer.
            let is_sensitive = if item.deleted {
                local.as_ref().is_some_and(|l| l.is_sensitive)
            } else {
                self.state.detector.is_sensitive(&item.content)
            };

            let sealed = if item.deleted {
                None
            } else {
                let key = self.state.keyring.item_key();
                Some(
                    copypaste_core::encrypt(item.content.as_bytes(), &key, &item.item_id).map_err(
                        |e| {
                            warn!(error = ?e, "could not seal an incoming item");
                            SyncError::Source(MSG_ENCRYPT.to_string())
                        },
                    )?,
                )
            };
            let (nonce, ciphertext) = match &sealed {
                Some((nonce, ciphertext)) => (Some(nonce.as_slice()), Some(ciphertext.as_slice())),
                None => (None, None),
            };

            let stored = meta
                .apply(&IncomingVersion {
                    item_id: &item.item_id,
                    content_ciphertext: ciphertext,
                    nonce,
                    content_type: &item.content_type,
                    content_hash: &item.content_hash,
                    created_at: item.created_at,
                    deleted: item.deleted,
                    is_sensitive,
                    origin_device_id: &item.origin_device_id,
                    search_text: if is_sensitive || item.deleted {
                        None
                    } else {
                        Some(&item.content)
                    },
                })
                .map_err(|e| {
                    warn!(error = ?e, "could not store an incoming item");
                    SyncError::Source(MSG_METADATA.to_string())
                })?;

            if stored {
                debug!(id = %item.item_id, deleted = item.deleted, "applied a peer's version");
            }
            Ok(stored)
        })
    }
}

/// Run a short blocking database call without stalling the reactor.
///
/// Every method above is SQLite plus an AEAD, and `SyncSource` is a synchronous
/// trait called from inside an async session, so the work happens on whatever
/// thread is driving the session. On the daemon's multi-threaded runtime
/// `block_in_place` hands the other tasks to a different worker for the
/// duration; on a current-thread runtime — which is what `#[tokio::test]`
/// builds — it would panic, so there the call simply runs inline.
fn blocking<T>(f: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture;
    use crate::p2p::tests::test_state;

    fn add(state: &Arc<AppState>, content: &str) -> String {
        capture::ingest(state, content, "text")
            .expect("ingest")
            .into_item()
            .id
    }

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

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [MSG_METADATA, MSG_ENCRYPT] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
