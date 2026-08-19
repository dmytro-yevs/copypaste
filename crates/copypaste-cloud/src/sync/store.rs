//! The half of a `CloudSource` that is the same on every host.
//!
//! The daemon and the Tauri app differ only in who guards a round and where
//! device state lives. The upload scan, the truncation bookkeeping and the page
//! merge were written twice, and two copies of the upload floor rules is how
//! the floor comes to mean two things. Nothing here takes a lock or decides
//! when a round may run: it is handed a [`Store`] and answers about rows.

use std::sync::{Arc, Mutex};

use copypaste_core::{RemoteVersion, Store, StoreSource};
use tracing::warn;
use zeroize::Zeroizing;

use super::source::{Applied, LocalItem};
use super::unreadable::{Sweep, UnreadableUploads, UploadFloor};
use super::SyncError;

/// Local versions offered to one push.
///
/// A bound on the query and on the memory one round holds, not on what
/// eventually uploads: the floor does not advance until a round completes, so a
/// larger backlog simply takes several rounds to drain.
pub const UPLOAD_SCAN_LIMIT: i64 = 500;

/// What the last upload scan was actually shown.
#[derive(Clone, Debug, Default)]
pub struct Offer {
    /// The scan hit [`UPLOAD_SCAN_LIMIT`], so there is more behind it.
    pub truncated: bool,
    /// The last offered row's full keyset — the furthest the floor may move
    /// when the scan was truncated.
    pub last: UploadFloor,
    /// The cursor this scan started from. It lets completion detect a peer
    /// write that lowered the floor while the round was in flight.
    pub started: UploadFloor,
    /// Detects a write that landed at the same keyset boundary, where comparing
    /// only the floor pair cannot prove the scan saw it.
    pub started_epoch: u64,
}

/// The result of one upload scan.
pub struct Scan {
    pub items: Vec<LocalItem>,
    pub offer: Offer,
    pub unreadable: UnreadableUploads,
}

/// A device's history, as both hosts' cloud sources read and write it.
pub struct StoreView {
    shared: StoreSource,
    device_id: String,
    last_offer: Mutex<Offer>,
}

impl StoreView {
    #[must_use]
    pub fn new(shared: StoreSource, device_id: String) -> Self {
        Self {
            shared,
            device_id,
            last_offer: Mutex::new(Offer::default()),
        }
    }

    #[must_use]
    pub fn shared(&self) -> &StoreSource {
        &self.shared
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// What the last [`StoreView::scan`] was shown.
    #[must_use]
    pub fn offer(&self) -> Offer {
        self.last_offer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The local versions after a keyset position, in the plain.
    ///
    /// A row that will not open under this device's own key cannot be uploaded
    /// and will not start opening, and holding the floor below it stalls every
    /// later row. So it is skipped, its id recorded, and retried at the head of
    /// the next scan — offered if it opens now, forgotten if it has gone,
    /// counted otherwise (INV-N7). Past the id bound a keyset walk carries the
    /// same guarantee (INV-N7a): that is the second page a scan reads.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    pub fn scan(
        &self,
        store: &Store,
        since_ms: i64,
        after_item_id: Option<&str>,
        started_epoch: u64,
        previously_unreadable: &UnreadableUploads,
    ) -> Result<Scan, SyncError> {
        // A record that would not parse lost ids whose rows the floor has
        // already passed. Rescanning from the start is the only way back to
        // them (B2), and it makes the walk redundant for this round.
        let restart = previously_unreadable.reset_floor;
        let (since_ms, after_item_id) = if restart {
            (0, None)
        } else {
            (since_ms, after_item_id)
        };
        let floor = UploadFloor {
            created_at: since_ms,
            item_id: after_item_id.map(str::to_owned),
        };

        let mut unreadable = UnreadableUploads::default();
        let mut items = Vec::new();

        let retried: Vec<String> = previously_unreadable.ids.iter().cloned().collect();
        if !retried.is_empty() {
            for chunk in retried.chunks(UPLOAD_SCAN_LIMIT as usize) {
                for row in store.versions(chunk).map_err(source_error)? {
                    match self.to_local(&row) {
                        Some(item) => items.push(item),
                        // A row that is not in the answer at all has been
                        // deleted or pruned; forgetting it is the only correct
                        // outcome, so nothing is done for the absent case.
                        None => {
                            unreadable.track(&row.id);
                        }
                    }
                }
            }
        }

        let carried = (!restart)
            .then(|| previously_unreadable.sweep.clone())
            .flatten();
        if let Some(sweep) = carried {
            let walked = self.walk_left_behind(
                store,
                sweep,
                &floor,
                previously_unreadable,
                &mut unreadable,
                &mut items,
            )?;
            unreadable.sweep = walked;
        }

        let rows = store
            .versions_after(since_ms, after_item_id, UPLOAD_SCAN_LIMIT)
            .map_err(source_error)?;
        let offer = Offer {
            truncated: rows.len() as i64 >= UPLOAD_SCAN_LIMIT,
            last: rows.last().map_or_else(|| floor.clone(), keyset_of),
            started: floor,
            started_epoch,
        };

        items.reserve(rows.len());
        // The position *before* the row being read, which is where a walk has
        // to resume from to read that row again.
        let mut behind = offer.started.clone();
        for row in &rows {
            if !previously_unreadable.ids.contains(&row.id) {
                match self.to_local(row) {
                    Some(item) => items.push(item),
                    None => {
                        if !unreadable.track(&row.id) {
                            unreadable.sweep_from(&behind);
                        }
                    }
                }
            }
            behind = keyset_of(row);
        }
        unreadable.settle_total();
        if unreadable.total > 0 {
            warn!(
                rows = unreadable.total,
                "some local rows could not be prepared for upload and were not offered"
            );
        }

        *self
            .last_offer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offer.clone();

        Ok(Scan {
            items,
            offer,
            unreadable,
        })
    }

    /// One page of the walk over rows the id bound had no room for.
    ///
    /// Bounded above by `floor`: everything at or past it belongs to the main
    /// scan, and reading it here would offer the same row twice. Reaching that
    /// boundary — or running out of rows — ends the cycle, which settles the
    /// count and sends the cursor back to `from`. `None` retires the walk: a
    /// cycle that counted nothing has nothing left behind to come back for.
    fn walk_left_behind(
        &self,
        store: &Store,
        mut sweep: Sweep,
        floor: &UploadFloor,
        previously_unreadable: &UnreadableUploads,
        found: &mut UnreadableUploads,
        items: &mut Vec<LocalItem>,
    ) -> Result<Option<Sweep>, SyncError> {
        let page = store
            .versions_after(
                sweep.next.created_at,
                sweep.next.item_id.as_deref(),
                UPLOAD_SCAN_LIMIT,
            )
            .map_err(source_error)?;
        let mut cycled = page.len() < UPLOAD_SCAN_LIMIT as usize;
        for row in &page {
            let at = keyset_of(row);
            if at > *floor {
                cycled = true;
                break;
            }
            sweep.next = at;
            if previously_unreadable.ids.contains(&row.id) {
                continue;
            }
            match self.to_local(row) {
                Some(item) => items.push(item),
                None => found.record_walked(&row.id, &mut sweep),
            }
        }
        if !cycled {
            return Ok(Some(sweep));
        }
        sweep.settled = sweep.seen;
        sweep.seen = 0;
        sweep.next.clone_from(&sweep.from);
        Ok((sweep.settled > 0).then_some(sweep))
    }

    /// Merge a page of remote versions, answering positionally.
    ///
    /// One call into the store for the page: one read of every local version,
    /// then one write transaction. See
    /// [`copypaste_core::sync::apply_remote_versions`].
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    pub fn apply_page(&self, items: Vec<LocalItem>) -> Result<Vec<Applied>, SyncError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let texts: Vec<Option<String>> = items
            .iter()
            .map(|item| {
                copypaste_ipc::content_type::is_text(&item.content_type)
                    .then(|| String::from_utf8_lossy(&item.content).into_owned())
            })
            .collect();
        let versions: Vec<RemoteVersion<'_>> = items
            .iter()
            .zip(&texts)
            .map(|(item, text)| remote_version(item, text.as_deref()))
            .collect();

        let applied = self
            .shared
            .apply_versions(&versions)
            .map_err(|e| SyncError::Source(e.message()))?;
        if applied.len() != items.len() {
            return Err(SyncError::Source(copypaste_core::sync::MSG_STORE));
        }
        Ok(items
            .into_iter()
            .zip(applied)
            .map(|(item, stored)| {
                if stored {
                    Applied::Merged
                } else {
                    Applied::Declined(item)
                }
            })
            .collect())
    }

    /// The stamp to lower the upload floor to so a declined row's local winner
    /// is republished, or `None` when the local copy is a plain self-echo.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    pub fn requeue_stamp(&self, incoming: &LocalItem) -> Result<Option<i64>, SyncError> {
        let text = copypaste_ipc::content_type::is_text(&incoming.content_type)
            .then(|| String::from_utf8_lossy(&incoming.content));
        copypaste_core::local_winner_stamp(
            self.shared.store(),
            &self.device_id,
            &remote_version(incoming, text.as_deref()),
        )
        .map_err(|e| SyncError::Source(e.message()))
    }

    /// One stored row in the plain, or `None` when it will not open.
    ///
    /// A tombstone has no payload to open, and carries none on the wire either
    /// (manifest 05 T-4).
    fn to_local(&self, row: &copypaste_core::StoredItem) -> Option<LocalItem> {
        let content = if row.deleted {
            Zeroizing::new(Vec::new())
        } else {
            self.shared.open_bytes(row).ok()?
        };
        Some(LocalItem {
            origin_device_id: copypaste_core::origin_or(&row.origin_device_id, &self.device_id)
                .to_string(),
            item_id: row.id.clone(),
            content,
            content_type: row.content_type.clone(),
            payload_metadata: row.payload_metadata.clone(),
            created_at: row.created_at,
            deleted: row.deleted,
        })
    }
}

fn keyset_of(row: &copypaste_core::StoredItem) -> UploadFloor {
    UploadFloor {
        created_at: row.created_at,
        item_id: Some(row.id.clone()),
    }
}

/// Where the upload floor may move now that a round has completed.
///
/// Normally the instant the round began — everything older was offered. But
/// when the scan was truncated, only the batch that was *shown* has been
/// offered, so the floor may move no further than its newest stamp; jumping to
/// the round's start would silently drop every row the limit cut off, which is
/// exactly how a large backlog would lose all but its first page.
#[must_use]
pub fn floor_after_round(offer: &Offer, round_started_ms: i64) -> UploadFloor {
    if offer.truncated && offer.last.created_at <= round_started_ms {
        offer.last.clone()
    } else {
        UploadFloor {
            created_at: round_started_ms,
            item_id: None,
        }
    }
}

/// The `LocalItem`/`RemoteVersion` conversion both hosts made identically.
///
/// `text` is the lossy decode the caller already made, kept out of here so a
/// page allocates its decodes once.
fn remote_version<'a>(item: &'a LocalItem, text: Option<&'a str>) -> RemoteVersion<'a> {
    RemoteVersion {
        item_id: &item.item_id,
        content: text.unwrap_or(""),
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
    }
}

fn source_error(e: impl std::fmt::Debug) -> SyncError {
    warn!(error = ?e, "the history database could not be read for a cloud round");
    SyncError::Source(copypaste_core::sync::MSG_STORE)
}

/// Shared so a caller can hand the view out without cloning the store.
pub type SharedStoreView = Arc<StoreView>;

#[cfg(test)]
mod tests {
    use copypaste_core::{Detector, IncomingItem, Keyring};

    use super::*;

    const SECRET: [u8; 32] = [3u8; 32];
    const FOREIGN: [u8; 32] = [9u8; 32];

    /// A device, plus the keyring of the device whose rows it cannot open.
    struct Device {
        store: Store,
        here: String,
        keyring: Keyring,
        foreign: Keyring,
    }

    fn device() -> Device {
        let keyring = Keyring::from_secret(&SECRET);
        let store = Store::open_in_memory(&keyring.db_key()).expect("store");
        let here = store.device_identity("here").expect("identity").device_id;
        Device {
            store,
            here,
            keyring,
            foreign: Keyring::from_secret(&FOREIGN),
        }
    }

    impl Device {
        /// A fresh view over the same history. `StoreView` holds no state a
        /// scan reads back, so building one is exactly what a restart leaves:
        /// the persisted record, and nothing else.
        fn view(&self) -> StoreView {
            let shared = StoreSource::new(
                self.store.clone(),
                Arc::new(Keyring::from_secret(&SECRET)),
                Arc::new(Detector::new().expect("detector")),
                self.here.clone(),
                "here".to_string(),
                copypaste_ipc::ConfigData::default(),
            );
            StoreView::new(shared, self.here.clone())
        }

        /// A row at a fixed keyset position, sealed under this device's key or
        /// under the foreign one. Sealing under `foreign` is what an unreadable
        /// row is: the AEAD fails closed under this device's key. Writing the
        /// same id again with `readable` is the repair, and it deliberately
        /// keeps `created_at` — a repair that moved the row would be found by
        /// the main scan and prove nothing about what is behind the floor.
        fn put(&self, id: &str, created_at: i64, readable: bool) {
            let content = format!("clipboard {id}");
            let key = if readable {
                self.keyring.item_key()
            } else {
                self.foreign.item_key()
            };
            let (nonce, ciphertext) =
                copypaste_core::encrypt(content.as_bytes(), &key, id).expect("seal");
            let hash = copypaste_core::compute_content_hash(content.as_bytes());
            let stored = self
                .store
                .upsert(&IncomingItem {
                    id,
                    content_ciphertext: Some(&ciphertext),
                    nonce: Some(&nonce),
                    content_type: "text",
                    content_hash: &hash,
                    created_at,
                    deleted: false,
                    is_sensitive: false,
                    origin_device_id: "",
                    pinned: false,
                    pin_order: None,
                    pin_updated_at: 0,
                    search_text: None,
                    payload_metadata: None,
                })
                .expect("upsert");
            assert!(stored, "the dedup index refused {id}");
        }

        /// One round, driven exactly as both adapters drive it: decode the
        /// persisted record, scan from the stored floor, persist what the scan
        /// answered, then commit the floor. The view is rebuilt every time, so
        /// every round in these tests is also a restart.
        fn round(&self, floor: &mut UploadFloor, persisted: &mut Option<String>) -> Vec<String> {
            let view = self.view();
            let previous = UnreadableUploads::decode(persisted.as_deref());
            let (since, after) = if previous.reset_floor {
                (0, None)
            } else {
                (floor.created_at, floor.item_id.clone())
            };
            let scan = view
                .scan(&self.store, since, after.as_deref(), 0, &previous)
                .expect("scan");
            *persisted = Some(scan.unreadable.encode());
            *floor = floor_after_round(&scan.offer, ROUND_STARTED_MS);
            scan.items.into_iter().map(|item| item.item_id).collect()
        }
    }

    /// Well past every row's stamp, so an untruncated round moves the floor to
    /// the round's own start exactly as a live one would.
    const ROUND_STARTED_MS: i64 = 10_000;

    /// B3, reproduced. 600 rows this device cannot open is more than the retry
    /// bound *and* more than one scan page, so the floor passes rows whose ids
    /// were never recorded. Repairing one of those rows afterwards has to end
    /// with it offered: a bounded id set alone cannot promise that, and the
    /// keyset walk is what does.
    #[test]
    fn a_repaired_row_past_the_retry_bound_is_still_offered() {
        let device = device();
        for n in 1..=600 {
            device.put(&format!("row-{n:04}"), n, false);
        }

        let mut floor = UploadFloor::default();
        let mut persisted = None;
        for _ in 0..6 {
            device.round(&mut floor, &mut persisted);
        }
        assert_eq!(
            floor,
            UploadFloor {
                created_at: ROUND_STARTED_MS,
                item_id: None,
            },
            "the floor must still advance, or every later clip stalls behind the backlog"
        );

        device.put("row-0600", 600, true);
        let mut offered = Vec::new();
        for _ in 0..4 {
            offered.extend(device.round(&mut floor, &mut persisted));
            if offered.iter().any(|id| id == "row-0600") {
                break;
            }
        }
        assert!(
            offered.iter().any(|id| id == "row-0600"),
            "a repaired row behind the floor was stranded: {offered:?}"
        );
    }

    /// The other half of the same guarantee: the count settles on the real
    /// number once a cycle has walked the whole backlog, rather than resting at
    /// the retry bound and under-reporting for good.
    #[test]
    fn the_count_settles_on_the_whole_backlog_not_the_retry_bound() {
        let device = device();
        for n in 1..=600 {
            device.put(&format!("row-{n:04}"), n, false);
        }

        let mut floor = UploadFloor::default();
        let mut persisted = None;
        let mut total = 0;
        for _ in 0..8 {
            device.round(&mut floor, &mut persisted);
            total = UnreadableUploads::decode(persisted.as_deref()).total;
        }
        assert_eq!(total, 600);
    }

    /// 300 rows this device cannot open, settled: 256 the id list tracks and
    /// 44 the walk carries. Two rounds are what it takes to count both halves.
    fn a_backlog_just_past_the_bound() -> (Device, UploadFloor, Option<String>) {
        let device = device();
        for n in 1..=300 {
            device.put(&format!("row-{n:04}"), n, false);
        }

        let mut floor = UploadFloor::default();
        let mut persisted = None;
        for _ in 0..2 {
            device.round(&mut floor, &mut persisted);
        }
        let record = UnreadableUploads::decode(persisted.as_deref());
        assert_eq!((record.total, record.ids.len()), (300, 256));
        (device, floor, persisted)
    }

    /// INV-N7b, the over-count. A row the walk finds belongs either to the id
    /// list or to the cycle's count, and repairing tracked rows is what makes
    /// room for it to be taken by both: the round after the repair reported
    /// 244 items for the 200 that were really unreadable.
    #[test]
    fn the_count_never_exceeds_the_rows_that_are_really_unreadable() {
        let (device, mut floor, mut persisted) = a_backlog_just_past_the_bound();
        for n in 1..=100 {
            device.put(&format!("row-{n:04}"), n, true);
        }

        let mut reported = Vec::new();
        for _ in 0..4 {
            device.round(&mut floor, &mut persisted);
            reported.push(UnreadableUploads::decode(persisted.as_deref()).total);
        }
        assert!(
            reported.iter().all(|total| *total <= 200),
            "more items reported than are unreadable: {reported:?}"
        );
        assert_eq!(reported.last(), Some(&200), "{reported:?}");
    }

    /// A cycle that counts nothing retires the walk, and once a repair frees
    /// room that is because the id list took the swept rows over rather than
    /// because they are gone. Retiring must not strand them: the id list is
    /// retried by direct lookup, so one repaired afterwards is still offered.
    #[test]
    fn a_swept_row_the_id_list_took_over_is_still_offered_when_repaired() {
        let (device, mut floor, mut persisted) = a_backlog_just_past_the_bound();
        for n in 1..=100 {
            device.put(&format!("row-{n:04}"), n, true);
        }
        for _ in 0..2 {
            device.round(&mut floor, &mut persisted);
        }
        let record = UnreadableUploads::decode(persisted.as_deref());
        assert_eq!(
            record.sweep, None,
            "the walk had nothing left to come back for"
        );
        assert!(
            record.ids.contains("row-0300"),
            "the swept rows were dropped"
        );

        device.put("row-0300", 300, true);
        let offered = device.round(&mut floor, &mut persisted);
        assert!(
            offered.iter().any(|id| id == "row-0300"),
            "a row handed to the id list was stranded when the walk retired: {offered:?}"
        );
    }

    /// INV-N7b, the stale cycle. A cycle spanning several rounds settles on a
    /// figure for its region, and repairing rows there left that older, larger
    /// figure on screen for the whole of the next cycle: 1,500 reported while
    /// 500 were really unreadable. Every round rebuilds the view, so the low
    /// figures here are read back from a part-walked cycle, not held in memory.
    #[test]
    fn a_repair_between_cycles_is_not_reported_at_the_last_cycles_figure() {
        let device = device();
        for n in 1..=1500 {
            device.put(&format!("row-{n:04}"), n, false);
        }

        let mut floor = UploadFloor::default();
        let mut persisted = None;
        let mut settled = 0;
        for _ in 0..5 {
            device.round(&mut floor, &mut persisted);
            settled = UnreadableUploads::decode(persisted.as_deref()).total;
        }
        assert_eq!(settled, 1500, "a completed cycle must be exact");

        // Rows 1..=256 are the tracked ids; the walk carries the rest. Repairing
        // the first 1,000 rows it carries leaves 500 unreadable.
        for n in 257..=1256 {
            device.put(&format!("row-{n:04}"), n, true);
        }

        let mut reported = Vec::new();
        let mut walked_mid_cycle = false;
        for _ in 0..3 {
            device.round(&mut floor, &mut persisted);
            let record = UnreadableUploads::decode(persisted.as_deref());
            walked_mid_cycle |= record
                .sweep
                .as_ref()
                .is_some_and(|sweep| sweep.next > sweep.from);
            reported.push(record.total);
        }
        assert!(
            reported.iter().all(|total| *total <= 500),
            "more items reported than are unreadable: {reported:?}"
        );
        assert!(
            walked_mid_cycle,
            "no round read back a part-walked cycle: {reported:?}"
        );
        assert_eq!(reported.last(), Some(&500), "{reported:?}");
    }

    /// A restart carries nothing but the persisted record, so the walk's
    /// position has to live in it. 1,500 rows make a cycle span several rounds,
    /// which is the case a restart can lose: a device restarted more often than
    /// a cycle takes would re-read the first page for ever and never reach the
    /// far end. Every round here rebuilds the view, so every round is a restart.
    #[test]
    fn a_restart_between_every_round_still_reaches_the_far_end_of_the_walk() {
        let device = device();
        for n in 1..=1500 {
            device.put(&format!("row-{n:04}"), n, false);
        }

        let mut floor = UploadFloor::default();
        let mut persisted = None;
        for _ in 0..4 {
            device.round(&mut floor, &mut persisted);
        }
        let carried = UnreadableUploads::decode(persisted.as_deref())
            .sweep
            .expect("a backlog past the bound must leave a walk behind");
        assert!(
            carried.next > carried.from,
            "a cycle spanning rounds must persist where it got to: {carried:?}"
        );

        device.put("row-1500", 1500, true);
        let mut offered = Vec::new();
        for _ in 0..8 {
            offered.extend(device.round(&mut floor, &mut persisted));
            if offered.iter().any(|id| id == "row-1500") {
                break;
            }
        }
        assert!(
            offered.iter().any(|id| id == "row-1500"),
            "the far end of the walk was never reached across restarts"
        );
    }

    #[test]
    fn a_truncated_scan_moves_the_floor_no_further_than_what_it_showed() {
        let offer = Offer {
            truncated: true,
            last: UploadFloor {
                created_at: 5_000,
                item_id: Some("item-z".into()),
            },
            ..Offer::default()
        };
        assert_eq!(floor_after_round(&offer, 9_000), offer.last);

        let complete = Offer {
            truncated: false,
            ..offer
        };
        assert_eq!(
            floor_after_round(&complete, 9_000),
            UploadFloor {
                created_at: 9_000,
                item_id: None,
            }
        );
    }
}
