//! The daemon's history, as a cloud round sees it.
//!
//! The seam is `copypaste_cloud::sync::CloudSource`. Everything below reads and
//! writes through [`crate::meta`] — the same view the peer transport uses, with
//! the same `is_sensitive = 0` filter deciding what may leave the device and the
//! same comparator deciding what arrives ([`crate::merge`]).
//!
//! # Two cursors, not one
//!
//! [`CloudSource::watermark`] is what this device has reconciled *down* from
//! the account; [`CloudSource::upload_floor`] is what it has offered *up*. The
//! seam keeps them apart for the two reasons written on `upload_floor` — a
//! watermark dragged forward by another device's clock strands local items, and
//! a fresh sign-in has a backlog to send (manifest 05 §4.9, BUG C2).
//!
//! Both live in `sync_device_state`. [`crate::cloud::poll`] advances the floor
//! only after a round completes, and only to the instant that round *started*,
//! so an item captured while a round was in flight is still offered by the next
//! one.
//!
//! One case the floor cannot see on its own: an item that arrives from a *peer*
//! carries the sender's `created_at`, which may be well behind this device's
//! floor, so it would never be forwarded to the account. The peer transport
//! therefore lowers the floor to the stamp it just applied — see
//! [`crate::cloud::note_version_written`].
//!
//! # One thing the cursor cannot see
//!
//! `Store::delete` tombstones a row without restamping `created_at`, so
//! deleting an item older than the cursor produces a version no `created_at`
//! query can find, and that delete does not propagate over the cloud path. It
//! propagates over the peer path, which advertises full state rather than a
//! window. The fix belongs in `copypaste-core::Store::delete` — restamp on
//! mutation, which is exactly what `CloudItem::created_at` already documents as
//! a requirement on writers — and not in a second bookkeeping table here, which
//! would be one more thing to drift out of step with the rows it describes.

use std::sync::{Arc, Mutex};

use copypaste_cloud::sync::{CloudSource, LocalItem, SyncError};
use tracing::warn;

use crate::cloud::{KEY_UPLOAD_FLOOR, KEY_WATERMARK};
use crate::merge::{apply_remote_version, open_version, RemoteVersion, MSG_STORE};
use crate::meta::blocking;
use crate::AppState;

/// Local versions offered to one push.
///
/// A bound on the query and on the memory one round holds, not on what
/// eventually uploads: the floor does not advance until a round completes, so a
/// larger backlog simply takes several rounds to drain.
const UPLOAD_SCAN_LIMIT: i64 = 500;

pub struct StoreSource {
    state: Arc<AppState>,
    last_offer: Mutex<Offer>,
}

/// What the last push was actually shown.
#[derive(Default, Clone, Copy)]
struct Offer {
    /// The scan hit [`UPLOAD_SCAN_LIMIT`], so there is more behind it.
    truncated: bool,
    /// The newest stamp in the batch — the furthest the floor may move when it
    /// was truncated.
    max_created_at: i64,
}

impl StoreSource {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            last_offer: Mutex::new(Offer::default()),
        }
    }

    /// Where the upload floor may move now that a round has completed.
    ///
    /// Normally the instant the round began — everything older was offered. But
    /// when the scan was truncated, only the batch that was *shown* has been
    /// offered, so the floor may move no further than its newest stamp;
    /// jumping to the round's start would silently drop every row the limit cut
    /// off, which is exactly how a large backlog would lose all but its first
    /// page.
    pub fn next_floor(&self, round_started_ms: i64) -> i64 {
        let offer = *self.last_offer.lock().unwrap_or_else(|e| e.into_inner());
        if offer.truncated {
            offer.max_created_at.min(round_started_ms)
        } else {
            round_started_ms
        }
    }
}

impl CloudSource for StoreSource {
    fn device_id(&self) -> String {
        self.state.meta.device_id().to_string()
    }

    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
        blocking(|| {
            let rows = self
                .state
                .meta
                .versions_since(since_ms, UPLOAD_SCAN_LIMIT)
                .map_err(source_error)?;

            *self
                .last_offer
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Offer {
                truncated: rows.len() as i64 >= UPLOAD_SCAN_LIMIT,
                max_created_at: rows.last().map_or(since_ms, |row| row.created_at),
            };

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                // A tombstone has no payload to open, and carries none on the
                // wire either (manifest 05 T-4).
                let content = if row.deleted {
                    String::new()
                } else {
                    match open_version(&self.state, &row) {
                        Some(content) => content,
                        None => continue,
                    }
                };
                items.push(LocalItem {
                    item_id: row.item_id,
                    content: content.into_bytes(),
                    content_type: row.content_type,
                    created_at: row.created_at,
                    deleted: row.deleted,
                    origin_device_id: row.origin_device_id,
                });
            }
            Ok(items)
        })
    }

    fn apply_remote(&self, item: LocalItem) -> Result<bool, SyncError> {
        blocking(|| {
            // Lossy for the same reason the peer path is: clipboard content is
            // text, and one row that is not valid UTF-8 must not fail a round.
            let content = String::from_utf8_lossy(&item.content);
            apply_remote_version(
                &self.state,
                &RemoteVersion {
                    item_id: &item.item_id,
                    content: &content,
                    content_type: &item.content_type,
                    created_at: item.created_at,
                    deleted: item.deleted,
                    // The cloud row carries no hash — see `RemoteVersion`.
                    content_hash: None,
                    origin_device_id: &item.origin_device_id,
                },
            )
            .map_err(|e| SyncError::Source(e.message()))
        })
    }

    fn watermark(&self) -> Result<i64, SyncError> {
        blocking(|| self.state.meta.state_ms(KEY_WATERMARK).map_err(source_error))
    }

    fn upload_floor(&self) -> Result<i64, SyncError> {
        blocking(|| {
            self.state
                .meta
                .state_ms(KEY_UPLOAD_FLOOR)
                .map_err(source_error)
        })
    }

    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        blocking(|| {
            self.state
                .meta
                .set_state_ms(KEY_WATERMARK, ms)
                .map_err(source_error)
        })
    }
}

fn source_error(e: crate::meta::MetaError) -> SyncError {
    warn!(error = ?e, "the history database could not be read for a cloud round");
    SyncError::Source(MSG_STORE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{add, test_state};

    fn source(name: &str) -> (StoreSource, Arc<AppState>, tempfile::TempDir) {
        let (state, dir) = test_state(name);
        (StoreSource::new(Arc::clone(&state)), state, dir)
    }

    fn ids(items: &[LocalItem]) -> Vec<&str> {
        items.iter().map(|i| i.item_id.as_str()).collect()
    }

    #[test]
    fn a_local_item_is_offered_in_the_plain_with_this_device_as_its_origin() {
        let (source, state, _dir) = source("alpha");
        let id = add(&state, "shared clipboard text");

        let items = source.local_changes_since(0).unwrap();
        assert_eq!(ids(&items), [id.as_str()]);
        assert_eq!(items[0].content, b"shared clipboard text");
        assert_eq!(items[0].origin_device_id, source.device_id());
        assert!(!items[0].deleted);
    }

    /// The first layer of "a sensitive item never leaves the device": it is not
    /// in the outbound query at all, so the driver's guard is a second layer
    /// rather than the only one (AT-56 / `CopyPaste-20yw`).
    #[test]
    fn a_sensitive_item_is_never_offered_for_upload() {
        let (source, state, _dir) = source("alpha");
        add(&state, "an ordinary snippet");
        let secret = add(&state, "AKIAIOSFODNN7EXAMPLE");

        let items = source.local_changes_since(0).unwrap();
        assert!(
            !ids(&items).contains(&secret.as_str()),
            "a sensitive item reached the upload path: {:?}",
            ids(&items)
        );
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn a_deleted_item_is_offered_as_a_tombstone_with_no_content() {
        let (source, state, _dir) = source("alpha");
        let id = add(&state, "doomed");
        assert!(state.store.delete(&id).unwrap());

        let items = source.local_changes_since(0).unwrap();
        assert_eq!(ids(&items), [id.as_str()]);
        assert!(items[0].deleted);
        assert!(items[0].content.is_empty(), "a tombstone carried content");
    }

    /// The backlog sweep: signing in has to offer the history that already
    /// exists, not only what is captured afterwards (manifest 05 §4.9, BUG C2).
    /// The floor is the cursor push uses, and it is not the watermark.
    #[test]
    fn the_upload_floor_starts_at_zero_and_is_independent_of_the_watermark() {
        let (source, state, _dir) = source("alpha");
        add(&state, "captured before signing in");

        // Another device's rows dragged the download watermark past our own
        // item's stamp — clock skew, or simply a busier peer.
        source.set_watermark(copypaste_core::now_ms() + 60_000).unwrap();

        assert_eq!(source.upload_floor().unwrap(), 0);
        assert_eq!(
            source.local_changes_since(source.upload_floor().unwrap()).unwrap().len(),
            1,
            "the item was stranded below the download cursor"
        );

        // Once a round has completed, the floor moves and the item stops being
        // re-offered.
        let after = copypaste_core::now_ms() + 1;
        state.meta.set_state_ms(KEY_UPLOAD_FLOOR, after).unwrap();
        assert_eq!(source.upload_floor().unwrap(), after);
        assert!(source
            .local_changes_since(source.upload_floor().unwrap())
            .unwrap()
            .is_empty());
    }

    /// An item that arrived from a peer carries the *sender's* stamp, which can
    /// be behind this device's floor. Without the peer path lowering the floor
    /// it would never be forwarded to the account.
    #[test]
    fn a_peer_write_lowers_the_upload_floor_so_it_still_reaches_the_account() {
        let (source, state, _dir) = source("alpha");
        state
            .meta
            .set_state_ms(KEY_UPLOAD_FLOOR, copypaste_core::now_ms())
            .unwrap();

        let old_stamp = 1_700_000_000_000;
        source
            .apply_remote(LocalItem {
                item_id: "from-a-peer".into(),
                content: b"arrived over the peer transport".to_vec(),
                content_type: "text".into(),
                created_at: old_stamp,
                deleted: false,
                origin_device_id: "device-b".into(),
            })
            .unwrap();
        crate::cloud::note_version_written(&state, old_stamp);

        let offered = source
            .local_changes_since(source.upload_floor().unwrap())
            .unwrap();
        assert!(
            offered.iter().any(|i| i.item_id == "from-a-peer"),
            "a peer's item was stranded below the upload floor"
        );
    }

    #[test]
    fn the_watermark_round_trips_and_starts_at_zero() {
        let (source, _state, _dir) = source("alpha");
        assert_eq!(source.watermark().unwrap(), 0);
        source.set_watermark(1_700_000_000_000).unwrap();
        assert_eq!(source.watermark().unwrap(), 1_700_000_000_000);
    }

    /// A round trip through the seam: a remote version arrives, is stored under
    /// the local key, and re-applying it changes nothing.
    #[test]
    fn a_remote_version_is_applied_once_and_then_absorbed() {
        let (source, state, _dir) = source("beta");
        let incoming = || LocalItem {
            item_id: "from-the-cloud".into(),
            content: b"from another device".to_vec(),
            content_type: "text".into(),
            created_at: 1_700_000_000_000,
            deleted: false,
            origin_device_id: "device-a".into(),
        };

        assert!(source.apply_remote(incoming()).unwrap());
        assert!(
            !source.apply_remote(incoming()).unwrap(),
            "a replayed version must not be re-applied (INV-I1)"
        );

        let row = state.store.get("from-the-cloud").unwrap().expect("stored");
        let plain = copypaste_core::decrypt(
            &row.content_ciphertext,
            &row.nonce,
            &state.keyring.item_key(),
            "from-the-cloud",
        )
        .expect("the local key must open it");
        assert_eq!(String::from_utf8(plain).unwrap(), "from another device");
    }

    /// A round's own upload, echoed back by the account, must be a no-op — and
    /// it is absorbed by the ordering rather than by a "did I send this?" check
    /// (INV-I2).
    #[test]
    fn this_devices_own_item_coming_back_changes_nothing() {
        let (source, state, _dir) = source("alpha");
        let id = add(&state, "mine");
        let offered = source.local_changes_since(0).unwrap();
        let mine = offered.into_iter().find(|i| i.item_id == id).unwrap();

        assert!(!source.apply_remote(mine).unwrap());
        assert_eq!(state.store.count().unwrap(), 1);
    }
}
