//! The daemon's history, as a cloud round sees it.
//!
//! Reads and writes go through the same [`copypaste_core::StoreSource`] the
//! peer transport uses, so the `is_sensitive = 0` filter and the comparator
//! behind it are shared rather than reimplemented (INV-C2).
//!
//! # The two cursors
//!
//! Why the upload floor is not the download watermark is on
//! [`CloudSource::upload_floor`]. Here: [`crate::cloud::poll`] advances the
//! floor only after a round completes, and only to the instant that round
//! *started*, so an item captured mid-round is still offered by the next one.
//!
//! A version can also appear *below* the floor — a peer applies rows carrying
//! the sender's stamp, and `Store::delete` tombstones without restamping. Both
//! call [`crate::cloud::note_version_written`] to pull the floor back; without
//! it, peer items and deletes of old items never reach the account. The cleaner
//! fix for the delete half is for `copypaste-core::Store::delete` to restamp on
//! mutation, which is what `CloudItem::created_at` asks of writers anyway.

use std::sync::{Arc, Mutex};

use copypaste_cloud::sync::{CloudSource, LocalItem, SyncError};
use copypaste_core::sync::blocking;
use copypaste_core::RemoteVersion;
use tracing::warn;

use crate::cloud::{KEY_UPLOAD_FLOOR, KEY_WATERMARK, KEY_WATERMARK_ITEM};
use crate::AppState;

/// Local versions offered to one push.
///
/// A bound on the query and on the memory one round holds, not on what
/// eventually uploads: the floor does not advance until a round completes, so a
/// larger backlog simply takes several rounds to drain.
const UPLOAD_SCAN_LIMIT: i64 = 500;

pub struct StoreSource {
    state: Arc<AppState>,
    /// The shared sync view. Held rather than rebuilt per call so one round
    /// opens and merges through one source, and so this file cannot grow a
    /// second answer to "what does this device hold".
    shared: copypaste_core::StoreSource,
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
            shared: crate::sync::store_source(&state),
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
                .store
                .versions_since(since_ms, UPLOAD_SCAN_LIMIT)
                .map_err(source_error)?;

            *self.last_offer.lock().unwrap_or_else(|e| e.into_inner()) = Offer {
                truncated: rows.len() as i64 >= UPLOAD_SCAN_LIMIT,
                max_created_at: rows.last().map_or(since_ms, |row| row.created_at),
            };

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                // A tombstone has no payload to open, and carries none on the
                // wire either (manifest 05 T-4).
                let content = if row.deleted {
                    Vec::new()
                } else {
                    match self.shared.open_bytes(&row) {
                        Some(content) => content,
                        None => continue,
                    }
                };
                items.push(LocalItem {
                    origin_device_id: copypaste_core::origin_or(
                        &row.origin_device_id,
                        self.state.meta.device_id(),
                    )
                    .to_string(),
                    item_id: row.id,
                    content,
                    content_type: row.content_type,
                    payload_metadata: row.payload_metadata,
                    created_at: row.created_at,
                    deleted: row.deleted,
                });
            }
            Ok(items)
        })
    }

    fn apply_remote(&self, item: LocalItem) -> Result<bool, SyncError> {
        blocking(|| {
            let text = copypaste_ipc::content_type::is_text(&item.content_type)
                .then(|| String::from_utf8_lossy(&item.content));
            self.shared
                .apply_version(&RemoteVersion {
                    item_id: &item.item_id,
                    content: text.as_deref().unwrap_or(""),
                    binary_content: (!item.deleted
                        && !copypaste_ipc::content_type::is_text(&item.content_type))
                    .then_some(item.content.as_slice()),
                    payload_metadata: item.payload_metadata.as_deref(),
                    content_type: &item.content_type,
                    created_at: item.created_at,
                    deleted: item.deleted,
                    // The cloud row carries no hash — see `RemoteVersion`.
                    content_hash: None,
                    origin_device_id: &item.origin_device_id,
                })
                .map_err(|e| SyncError::Source(e.message()))
        })
    }

    fn watermark(&self) -> Result<i64, SyncError> {
        blocking(|| {
            self.state
                .meta
                .state_ms(KEY_WATERMARK)
                .map_err(source_error)
        })
    }

    fn upload_floor(&self) -> Result<i64, SyncError> {
        blocking(|| {
            self.state
                .meta
                .state_ms(KEY_UPLOAD_FLOOR)
                .map_err(source_error)
        })
    }

    /// The tie-break half of the download cursor.
    ///
    /// Overridden — with [`CloudSource::set_watermark_keyset`], because a
    /// source that persists one half and not the other has two cursors that
    /// disagree. The default is millisecond-only, which re-offers the boundary
    /// millisecond at the start of every round; that is free when the boundary
    /// holds a handful of rows and is a stall when it holds more than a page,
    /// because a bound over a non-unique key cannot be paged past (INV-N1,
    /// AT-24). Two columns of work, as the trait's own doc says.
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        blocking(|| {
            self.state
                .meta
                .state(KEY_WATERMARK_ITEM)
                .map_err(source_error)
        })
    }

    /// Persist the millisecond alone, forgetting any item id.
    ///
    /// Clearing is the point. This is called when the cursor moves to a
    /// millisecond whose last row is not identified, and leaving the previous
    /// round's id beside a newer millisecond would make the pull query skip
    /// every row at that millisecond sorting below it.
    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        blocking(|| {
            self.state
                .meta
                .set_state_all(&[
                    (KEY_WATERMARK, &ms.max(0).to_string()),
                    (KEY_WATERMARK_ITEM, ""),
                ])
                .map_err(source_error)
        })
    }

    /// Persist both halves, in one transaction.
    ///
    /// Not two calls: a crash between them leaves a millisecond from one round
    /// beside an item id from another, and the pull query trusts the pair.
    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        // An empty id would be stored as "absent" by `set_state_all` anyway;
        // routing it through `set_watermark` says so rather than relying on
        // that.
        if item_id.is_empty() {
            return self.set_watermark(ms);
        }
        blocking(|| {
            self.state
                .meta
                .set_state_all(&[
                    (KEY_WATERMARK, &ms.max(0).to_string()),
                    (KEY_WATERMARK_ITEM, item_id),
                ])
                .map_err(source_error)
        })
    }
}

fn source_error(e: impl std::fmt::Debug) -> SyncError {
    warn!(error = ?e, "the history database could not be read for a cloud round");
    SyncError::Source(copypaste_core::sync::MSG_STORE)
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
        source
            .set_watermark(copypaste_core::now_ms() + 60_000)
            .unwrap();

        assert_eq!(source.upload_floor().unwrap(), 0);
        assert_eq!(
            source
                .local_changes_since(source.upload_floor().unwrap())
                .unwrap()
                .len(),
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
                payload_metadata: None,
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
        assert_eq!(source.watermark_item_id().unwrap(), None);
        source.set_watermark(1_700_000_000_000).unwrap();
        assert_eq!(source.watermark().unwrap(), 1_700_000_000_000);
    }

    /// Both halves of the keyset survive a restart. Without the id, a boundary
    /// millisecond holding more than one page of rows is re-offered every round
    /// and the rows behind it never arrive (INV-N1, AT-24).
    #[test]
    fn both_halves_of_the_cursor_are_persisted_and_read_back() {
        let (source, state, dir) = source("alpha");
        source
            .set_watermark_keyset(1_700_000_000_000, "item-b")
            .unwrap();
        assert_eq!(source.watermark().unwrap(), 1_700_000_000_000);
        assert_eq!(
            source.watermark_item_id().unwrap().as_deref(),
            Some("item-b")
        );
        drop(state);

        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        let source = StoreSource::new(restarted);
        assert_eq!(source.watermark().unwrap(), 1_700_000_000_000);
        assert_eq!(
            source.watermark_item_id().unwrap().as_deref(),
            Some("item-b")
        );
    }

    /// The halves must never disagree. A millisecond written on its own has to
    /// clear the id, or the pull query pages past rows at that millisecond that
    /// sort below a name from an older round.
    #[test]
    fn advancing_the_millisecond_alone_forgets_the_previous_item_id() {
        let (source, state, _dir) = source("alpha");
        source.set_watermark_keyset(1_000, "item-b").unwrap();
        source.set_watermark(2_000).unwrap();

        assert_eq!(source.watermark().unwrap(), 2_000);
        assert_eq!(source.watermark_item_id().unwrap(), None);
        assert_eq!(state.meta.state(KEY_WATERMARK_ITEM).unwrap(), None);
    }

    /// An empty id is the same statement as "no id", not a third state.
    #[test]
    fn an_empty_item_id_is_stored_as_absent() {
        let (source, _state, _dir) = source("alpha");
        source.set_watermark_keyset(1_000, "item-b").unwrap();
        source.set_watermark_keyset(2_000, "").unwrap();
        assert_eq!(source.watermark().unwrap(), 2_000);
        assert_eq!(source.watermark_item_id().unwrap(), None);
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
            payload_metadata: None,
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

    #[test]
    fn a_file_versions_metadata_round_trips_between_cloud_and_local_storage() {
        let (source, state, _dir) = source("file-metadata");
        let metadata = r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#;
        source
            .apply_remote(LocalItem {
                item_id: "file-from-cloud".into(),
                content: b"%PDF-binary".to_vec(),
                content_type: copypaste_ipc::content_type::FILE.into(),
                payload_metadata: Some(metadata.into()),
                created_at: 1_700_000_000_000,
                deleted: false,
                origin_device_id: "device-a".into(),
            })
            .unwrap();

        let row = state.store.get("file-from-cloud").unwrap().unwrap();
        assert_eq!(row.payload_metadata.as_deref(), Some(metadata));
        let offered = source.local_changes_since(0).unwrap();
        assert_eq!(offered[0].payload_metadata.as_deref(), Some(metadata));
        assert_eq!(offered[0].content, b"%PDF-binary");
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
