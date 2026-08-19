//! The daemon's history, as a cloud round sees it.
//!
//! Everything about *rows* is [`copypaste_cloud::sync::StoreView`], shared with
//! the Tauri app so the upload scan, the truncation bookkeeping and the page
//! merge cannot come to mean two things. What is here is what only the daemon
//! knows: that a round belongs to one account, where device state lives, and
//! that every one of those calls is SQLite on a reactor thread.
//!
//! # The two cursors
//!
//! Why the upload floor is not the download watermark is on
//! [`CloudSource::upload_floor`]. Here: [`crate::cloud::poll`] advances the
//! floor only after a round completes, and only to the instant that round
//! *started*, so an item captured mid-round is still offered by the next one.
//!
//! A version can also appear *below* the floor — a peer applies rows carrying
//! the sender's stamp. Local deletes restamp, but a keyset floor can still
//! skip a same-millisecond tombstone until [`crate::cloud::note_version_written`]
//! clears the boundary; without that, peer items and deletes never reach the
//! account.

use std::sync::Arc;

use copypaste_cloud::sync::{
    floor_after_round, Applied, CloudSource, LocalItem, StoreView, SyncError, UnreadableUploads,
};
use copypaste_core::sync::blocking;
use tracing::warn;

use crate::cloud::{
    Driver, UploadFloor, KEY_UNREADABLE_UPLOADS, KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM,
    KEY_WATERMARK, KEY_WATERMARK_ITEM,
};
use crate::AppState;

pub struct StoreSource {
    state: Arc<AppState>,
    expected: Option<Arc<Driver>>,
    /// The shared sync view. Held rather than rebuilt per call so one round
    /// opens and merges through one source, and so this file cannot grow a
    /// second answer to "what does this device hold".
    view: StoreView,
}

impl StoreSource {
    pub fn new(state: Arc<AppState>) -> Self {
        let device_id = state.meta.device_id().to_string();
        Self {
            view: StoreView::new(crate::sync::store_source(&state), device_id),
            state,
            expected: None,
        }
    }

    pub(crate) fn for_round(state: Arc<AppState>, expected: Arc<Driver>) -> Self {
        let mut source = Self::new(state);
        source.expected = Some(expected);
        source
    }

    fn with_account<T>(
        &self,
        action: impl FnOnce() -> Result<T, SyncError>,
    ) -> Result<T, SyncError> {
        match self.expected.as_ref() {
            Some(expected) => self.state.cloud.with_current_driver(expected, action),
            None => action(),
        }
    }

    /// One account check and one blocking scope, whatever the call reads.
    ///
    /// Both were per row before, and a 500-row page took 500 of each — nested,
    /// because the batch wrapped its own `blocking` around the per-row ones.
    fn read<T>(&self, action: impl FnOnce() -> Result<T, SyncError>) -> Result<T, SyncError> {
        self.with_account(|| blocking(action))
    }

    /// Where the upload floor may move now that a round has completed.
    pub fn commit_upload_floor(&self, round_started_ms: i64) -> Result<(), SyncError> {
        let offer = self.view.offer();
        let next = floor_after_round(&offer, round_started_ms);
        self.read(|| {
            self.state
                .cloud
                .commit_upload_floor(
                    &self.state.meta,
                    &UploadFloor {
                        created_at: offer.started.created_at,
                        item_id: offer.started.item_id.clone(),
                    },
                    offer.started_epoch,
                    &UploadFloor {
                        created_at: next.created_at,
                        item_id: next.item_id.clone(),
                    },
                )
                .map_err(source_error)
        })
    }
}

impl CloudSource for StoreSource {
    fn device_id(&self) -> String {
        self.view.device_id().to_string()
    }

    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
        self.local_changes_after(since_ms, None)
    }

    fn local_changes_after(
        &self,
        since_ms: i64,
        after_item_id: Option<&str>,
    ) -> Result<Vec<LocalItem>, SyncError> {
        self.read(|| {
            let started_epoch = self.state.cloud.upload_floor_epoch();
            let previous = UnreadableUploads::decode(
                self.state
                    .meta
                    .state(KEY_UNREADABLE_UPLOADS)
                    .map_err(source_error)?
                    .as_deref(),
            );
            if previous.reset_floor {
                crate::cloud::note_version_written(&self.state, 0);
            }
            let scan = self.view.scan(
                &self.state.store,
                since_ms,
                after_item_id,
                started_epoch,
                &previous,
            )?;
            if scan.unreadable != previous {
                self.state
                    .meta
                    .set_state(KEY_UNREADABLE_UPLOADS, &scan.unreadable.encode())
                    .map_err(source_error)?;
            }
            self.state
                .cloud
                .note_unreadable_uploads(scan.unreadable.total);
            Ok(scan.items)
        })
    }

    fn apply_remote(&self, item: LocalItem) -> Result<Applied, SyncError> {
        self.apply_remote_batch(vec![item])?
            .pop()
            .ok_or(SyncError::Source(copypaste_core::sync::MSG_STORE))
    }

    /// One page, one account check, one blocking scope, one write transaction.
    fn apply_remote_batch(&self, items: Vec<LocalItem>) -> Result<Vec<Applied>, SyncError> {
        let floor = items.iter().map(|item| item.created_at).min();
        let outcomes = self.read(|| self.view.apply_page(items))?;
        // The peer cursors are lowered once for the page rather than once per
        // row: the call is a `min`, so the oldest stamp is the only one that
        // moves it.
        if let Some(floor) = floor {
            if outcomes.iter().any(|o| matches!(o, Applied::Merged)) {
                // No peer to hold back: a row from the cloud is new to all of
                // them.
                self.state.p2p.node().cursors().note_local(floor);
            }
        }
        Ok(outcomes)
    }

    fn watermark(&self) -> Result<i64, SyncError> {
        self.read(|| {
            self.state
                .meta
                .state_ms(KEY_WATERMARK)
                .map_err(source_error)
        })
    }

    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.read(|| {
            self.state
                .meta
                .state_ms(KEY_UPLOAD_FLOOR)
                .map_err(source_error)
        })
    }

    fn upload_floor_item_id(&self) -> Result<Option<String>, SyncError> {
        self.read(|| {
            self.state
                .meta
                .state(KEY_UPLOAD_FLOOR_ITEM)
                .map_err(source_error)
        })
    }

    fn requeue_local_winner(&self, incoming: &LocalItem) -> Result<bool, SyncError> {
        let stamp = self.read(|| self.view.requeue_stamp(incoming))?;
        if let Some(stamp) = stamp {
            crate::cloud::note_version_written(&self.state, stamp);
            return Ok(true);
        }
        Ok(false)
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
        self.read(|| {
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
        self.read(|| {
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
        self.read(|| {
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
    fn upload_keyset_drains_a_full_round_started_boundary_without_replaying_it() {
        let (source, _state, _dir) = source("alpha");
        let stamp = 1_700_000_000_000;
        for n in 0..=copypaste_cloud::sync::UPLOAD_SCAN_LIMIT {
            source
                .apply_remote(LocalItem {
                    item_id: format!("boundary-{n:04}"),
                    content: format!("row {n}").into_bytes(),
                    content_type: "text".into(),
                    created_at: stamp,
                    deleted: false,
                    origin_device_id: "peer".into(),
                    payload_metadata: None,
                })
                .unwrap();
        }

        let first = source.local_changes_after(0, None).unwrap();
        assert_eq!(first.len() as i64, copypaste_cloud::sync::UPLOAD_SCAN_LIMIT);
        let last = first.last().unwrap().item_id.clone();
        source.commit_upload_floor(stamp).unwrap();

        assert_eq!(source.upload_floor().unwrap(), stamp);
        assert_eq!(
            source.upload_floor_item_id().unwrap().as_deref(),
            Some(last.as_str())
        );
        let second = source.local_changes_after(stamp, Some(&last)).unwrap();
        assert_eq!(second.len(), 1, "the first 500 rows were replayed");
        assert_ne!(second[0].item_id, last);
    }

    #[test]
    fn peer_write_during_a_round_cannot_be_overwritten_by_its_stale_floor() {
        let (source, state, _dir) = source("alpha");
        let started = 20_000;
        state.meta.set_state_ms(KEY_UPLOAD_FLOOR, started).unwrap();
        source.local_changes_after(started, None).unwrap();

        crate::cloud::note_version_written(&state, 1_000);
        source.commit_upload_floor(started + 1_000).unwrap();

        assert_eq!(source.upload_floor().unwrap(), 1_000);
        assert_eq!(source.upload_floor_item_id().unwrap(), None);
    }

    #[test]
    fn a_cloud_applied_older_version_pulls_the_p2p_relay_floor_back() {
        let (source, state, _dir) = source("relay-floor");
        let cursors = state.p2p.node().cursors();
        cursors.record_session("peer-1", 5_000);
        assert_eq!(cursors.get("peer-1").relay_floor_ms, None);

        let old_stamp = 1_000;
        assert_eq!(
            source
                .apply_remote(LocalItem {
                    item_id: "relayed-through-cloud".into(),
                    content: b"older than the peer cursor".to_vec(),
                    content_type: "text".into(),
                    payload_metadata: None,
                    created_at: old_stamp,
                    deleted: false,
                    origin_device_id: "device-c".into(),
                })
                .unwrap(),
            Applied::Merged
        );

        assert_eq!(
            cursors.get("peer-1").relay_floor_ms,
            Some(old_stamp),
            "a cloud-applied row left the peer's relay floor above it"
        );
        assert_eq!(
            cursors.get("peer-1").advertise_from(5_000),
            old_stamp,
            "the next session with that peer would not re-advertise the row"
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

        assert_eq!(source.apply_remote(incoming()).unwrap(), Applied::Merged);
        assert!(
            matches!(
                source.apply_remote(incoming()).unwrap(),
                Applied::Declined(_)
            ),
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
        assert_eq!(plain.as_slice(), b"from another device");
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

        assert!(matches!(
            source.apply_remote(mine).unwrap(),
            Applied::Declined(_)
        ));
        assert_eq!(state.store.count().unwrap(), 1);
    }

    #[test]
    fn a_local_lww_winner_is_reoffered_but_a_normal_self_echo_is_not() {
        let (source, state, _dir) = source("alpha");
        let id = add(&state, "mine");
        let local = source
            .local_changes_after(0, None)
            .unwrap()
            .into_iter()
            .find(|item| item.item_id == id)
            .unwrap();
        let passed_floor = local.created_at + 1;
        state
            .meta
            .set_state_ms(KEY_UPLOAD_FLOOR, passed_floor)
            .unwrap();

        let mut stale = local.clone();
        stale.created_at -= 1;
        stale.origin_device_id = "device-b".into();
        let Applied::Declined(stale) = source.apply_remote(stale).unwrap() else {
            panic!("the older remote version should lose")
        };
        source.requeue_local_winner(&stale).unwrap();
        assert_eq!(
            source.upload_floor().unwrap(),
            local.created_at,
            "the local LWW winner was not scheduled for republishing"
        );
        assert!(source
            .local_changes_after(local.created_at, None)
            .unwrap()
            .iter()
            .any(|item| item.item_id == id));

        state
            .meta
            .set_state_ms(KEY_UPLOAD_FLOOR, passed_floor)
            .unwrap();
        let Applied::Declined(local) = source.apply_remote(local).unwrap() else {
            panic!("the self echo should be unchanged")
        };
        source.requeue_local_winner(&local).unwrap();
        assert_eq!(
            source.upload_floor().unwrap(),
            passed_floor,
            "a normal self echo scheduled a duplicate upload"
        );
    }
}

/// The failures DMY-155 found in the upload scan and the page merge, and the
/// measurements that say the fixes hold.
#[cfg(test)]
mod round_tests {
    use super::*;
    use crate::testutil::{add, test_state};
    use copypaste_core::NewItem;

    fn source(name: &str) -> (StoreSource, Arc<AppState>, tempfile::TempDir) {
        let (state, dir) = test_state(name);
        (StoreSource::new(Arc::clone(&state)), state, dir)
    }

    /// A row this device's own key will not open. Sealed under a second
    /// keyring, which is what a corrupted payload or a rotated device key
    /// looks like from the merge's side.
    fn add_unreadable(state: &Arc<AppState>, id: &str, created_at: i64) {
        let stranger = copypaste_core::Keyring::from_secret(&[0xAB; 32]);
        let (nonce, ciphertext) =
            copypaste_core::encrypt(b"sealed elsewhere", &stranger.item_key(), id).unwrap();
        state
            .store
            .insert(NewItem {
                id: id.to_string(),
                content_ciphertext: ciphertext,
                nonce,
                content_type: "text".into(),
                content_hash: copypaste_core::compute_content_hash(b"sealed elsewhere"),
                is_sensitive: false,
                search_text: None,
                created_at,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
    }

    /// The upload half of INV-N3. A row that will not open cannot be uploaded,
    /// and the scan used to drop it on the floor: the round then advanced the
    /// upload floor past it, so it was never offered again and nothing
    /// anywhere said so.
    ///
    /// What must hold now: the readable rows still upload, the floor still
    /// advances (an unreadable row must not stall every row behind it), the
    /// row stays on a retry list, and the count reaches the status a client
    /// reads.
    #[tokio::test]
    async fn a_row_that_will_not_open_is_counted_and_kept_rather_than_dropped() {
        let (source, state, _dir) = source("alpha");
        add_unreadable(&state, "unreadable", 1_000);
        let readable = add(&state, "an ordinary snippet");

        let offered = source.local_changes_after(0, None).unwrap();
        assert_eq!(
            offered
                .iter()
                .map(|i| i.item_id.as_str())
                .collect::<Vec<_>>(),
            [readable.as_str()],
            "an unopenable row reached the upload path"
        );
        assert_eq!(
            state.cloud.status().unreadable_uploads,
            1,
            "the row was skipped without saying so"
        );

        // The floor advances — the row will never start opening, and holding
        // the floor below it would stall every later row for good.
        let started = copypaste_core::now_ms() + 1;
        source.commit_upload_floor(started).unwrap();
        assert!(source.upload_floor().unwrap() >= started);

        // And it is still retried, from above the floor, so a device whose key
        // situation is repaired offers the row rather than having lost it.
        let after = source.local_changes_after(started, None).unwrap();
        assert!(
            after.is_empty(),
            "an unreadable row was offered as if it had opened"
        );
        assert_eq!(state.cloud.status().unreadable_uploads, 1);
    }

    /// The retry half. Once the row can be read, the next scan offers it even
    /// though the floor has long since passed its stamp.
    #[tokio::test]
    async fn a_row_that_becomes_readable_is_offered_again_from_the_retry_list() {
        let (source, state, _dir) = source("alpha");
        add_unreadable(&state, "unreadable", 1_000);
        source.local_changes_after(0, None).unwrap();
        let started = copypaste_core::now_ms() + 1;
        source.commit_upload_floor(started).unwrap();
        assert_eq!(state.cloud.status().unreadable_uploads, 1);

        // Repaired: the same id, now sealed under this device's key.
        let key = state.keyring.item_key();
        let (nonce, ciphertext) =
            copypaste_core::encrypt(b"readable now", &key, "unreadable").unwrap();
        state
            .store
            .upsert(&copypaste_core::IncomingItem {
                id: "unreadable",
                content_ciphertext: Some(&ciphertext),
                nonce: Some(&nonce),
                content_type: "text",
                content_hash: &copypaste_core::compute_content_hash(b"readable now"),
                created_at: 1_000,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "",
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
                search_text: Some("readable now"),
                payload_metadata: None,
            })
            .unwrap();

        let offered = source.local_changes_after(started, None).unwrap();
        assert!(
            offered.iter().any(|item| item.item_id == "unreadable"),
            "a repaired row below the floor was lost"
        );
        assert_eq!(
            state.cloud.status().unreadable_uploads,
            0,
            "the count did not clear once the row could be read"
        );
    }

    /// Nothing about the row reaches a client: not the id, not the content,
    /// not a path (`AGENTS.md` rule 4).
    #[tokio::test]
    async fn the_unreadable_record_never_carries_a_row_identity_to_a_client() {
        let (source, state, _dir) = source("alpha");
        add_unreadable(&state, "a-very-distinctive-id", 1_000);
        source.local_changes_after(0, None).unwrap();

        let status = state.cloud.status();
        let frame = serde_json::to_string(&status).unwrap();
        assert!(!frame.contains("a-very-distinctive-id"), "{frame}");
        assert!(!frame.contains("sealed elsewhere"), "{frame}");
        assert_eq!(status.unreadable_uploads, 1);
    }

    /// A 500-row page, driven the way `CloudSync::pull` drives it.
    ///
    /// Before: `apply_remote_batch` wrapped its own `blocking` around 500
    /// per-row calls, each of which took the account guard, its own `blocking`
    /// scope, one pooled connection to read the local version and a second to
    /// write it under its own IMMEDIATE transaction — about 1,000 checkouts and
    /// 500 commits for one page, on a pool four deep.
    ///
    /// After: one account guard, one blocking scope, one read of every local
    /// version and one write transaction. What is asserted here is the
    /// behaviour that must survive the change; the counts are in the commit's
    /// evidence.
    #[tokio::test]
    async fn a_five_hundred_row_page_merges_through_one_batch_with_every_row_answered() {
        let (source, state, _dir) = source("alpha");
        let page: Vec<LocalItem> = (0..500)
            .map(|n| LocalItem {
                item_id: format!("page-{n:04}"),
                content: format!("row {n}").into_bytes(),
                content_type: "text".into(),
                payload_metadata: None,
                created_at: 1_700_000_000_000 + i64::from(n),
                deleted: false,
                origin_device_id: "device-b".into(),
            })
            .collect();

        let outcomes = source.apply_remote_batch(page.clone()).unwrap();
        assert_eq!(outcomes.len(), 500, "the page was not answered for in full");
        assert!(outcomes.iter().all(|o| *o == Applied::Merged));
        assert_eq!(state.store.count().unwrap(), 500);

        // Replaying the page changes nothing: the comparator, not the batching,
        // is what makes that true (INV-I1).
        let replayed = source.apply_remote_batch(page).unwrap();
        assert_eq!(replayed.len(), 500);
        assert!(replayed.iter().all(|o| matches!(o, Applied::Declined(_))));
        assert_eq!(state.store.count().unwrap(), 500);
    }

    /// Two versions of one id inside a page must be decided against each
    /// other, not both against the row on disk — the write has not happened
    /// yet when the second one is prepared.
    #[tokio::test]
    async fn two_versions_of_one_item_in_a_page_keep_the_newer() {
        let (source, state, _dir) = source("alpha");
        let version = |created_at: i64, body: &str| LocalItem {
            item_id: "same-id".into(),
            content: body.as_bytes().to_vec(),
            content_type: "text".into(),
            payload_metadata: None,
            created_at,
            deleted: false,
            origin_device_id: "device-b".into(),
        };

        let outcomes = source
            .apply_remote_batch(vec![version(2_000, "newer"), version(1_000, "older")])
            .unwrap();
        assert_eq!(outcomes[0], Applied::Merged);
        assert!(matches!(outcomes[1], Applied::Declined(_)));
        assert_eq!(
            state.store.version("same-id").unwrap().unwrap().created_at,
            2_000
        );
    }
}
