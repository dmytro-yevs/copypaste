use copypaste_ipc::{ConfigData, ExportItem, ImportData};
use tracing::{info, warn};

use crate::retention;
use crate::storage::{Store, StoreError};
use crate::{
    ingest::ingest_into_batched, now_ms, CryptoError, Detector, IngestError, Ingested, Keyring,
};

/// Ceiling on one import batch. The IPC frame cap bounds the bytes; this bounds
/// the work, so a single request cannot hold the database mutex for minutes.
pub const MAX_IMPORT_ITEMS: usize = 10_000;

/// Imported timestamps beyond this are untrusted metadata, not a future event.
/// Keeping one would make a version that P2P and cloud correctly refuse become
/// permanently local-only.
const MAX_IMPORT_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

/// Import failures.
///
/// The `Display` text carries no detail for the same reason
/// [`IngestError`]'s does not: these strings can reach a client, and the
/// underlying `StoreError` may carry a database path, which discloses the local
/// username (AGENTS.md rule 4).
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("there is nothing to import")]
    Empty,
    #[error("too many items in one import")]
    TooMany,
    #[error("an item could not be encrypted")]
    Crypto(#[from] CryptoError),
    #[error("the items could not be stored")]
    Storage(#[from] StoreError),
}

/// Put exported items back, through the same ingest a capture takes.
///
/// The caller's `is_sensitive` is a **floor, never a ceiling**: every item is
/// re-run through the detector, so a hand-edited export cannot reintroduce a
/// credential marked clean (manifest 04, PG-26). Nothing is written until the
/// whole batch has passed the two bounds above.
pub fn import(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    settings: &ConfigData,
    items: Vec<ExportItem>,
) -> Result<ImportData, ImportError> {
    let ingress_settings = settings.clone();
    let retention_settings = settings.clone();
    import_with_current_retention(
        store,
        detector,
        keyring,
        &ingress_settings,
        move || retention_settings.clone(),
        items,
    )
}

/// Import with a live retention policy for the batch's terminal sweep.
///
/// `settings` is deliberately still the ingress snapshot: its size and
/// sensitive-content decisions applied to every item before it reached this
/// batch.
pub fn import_with_current_retention(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    settings: &ConfigData,
    current_settings: impl Fn() -> ConfigData,
    items: Vec<ExportItem>,
) -> Result<ImportData, ImportError> {
    if items.is_empty() {
        return Err(ImportError::Empty);
    }
    if items.len() > MAX_IMPORT_ITEMS {
        return Err(ImportError::TooMany);
    }

    let mut result = ImportData {
        inserted: 0,
        skipped: 0,
        skipped_duplicate: 0,
        skipped_empty: 0,
        skipped_too_large: 0,
        pins_failed: 0,
    };
    let mut pin: Vec<String> = Vec::new();
    let batch = retention::batch_with_current_policy(store, settings.clone(), &current_settings);
    let mut fatal: Option<ImportError> = None;

    for item in items {
        let created_at = item
            .created_at
            .min(now_ms().saturating_add(MAX_IMPORT_FUTURE_SKEW_MS));
        match ingest_into_batched(
            &batch,
            detector,
            keyring,
            &item.content,
            &item.content_type,
            created_at,
            item.is_sensitive,
        ) {
            Ok(Ingested::Stored(stored)) => {
                result.inserted += 1;
                if item.pinned {
                    pin.push(stored.id);
                }
            }
            Ok(Ingested::Duplicate(existing)) => {
                result.skipped += 1;
                result.skipped_duplicate += 1;
                if item.pinned {
                    pin.push(existing.id);
                }
            }
            Err(IngestError::Empty) => {
                result.skipped += 1;
                result.skipped_empty += 1;
            }
            Err(IngestError::TooLarge) => {
                result.skipped += 1;
                result.skipped_too_large += 1;
            }
            Err(IngestError::Crypto(e)) => {
                fatal = Some(ImportError::Crypto(e));
                break;
            }
            Err(IngestError::Storage(e)) => {
                fatal = Some(ImportError::Storage(e));
                break;
            }
        }
    }

    // DMY-156: pin intent is not proportional to `result.inserted`. A duplicate
    // writes no new row and still owes the pin the file named, so this loop
    // runs before every exit — including the fatal one, whose rows are already
    // committed.
    for item_id in &pin {
        match batch.store().set_pinned(item_id, true) {
            Ok(true) => {}
            Ok(false) => {
                warn!("an imported pin names an item that is no longer live");
                result.pins_failed += 1;
            }
            Err(e) => {
                warn!(error = ?e, "could not restore a pin on an imported item");
                result.pins_failed += 1;
            }
        }
    }

    // DMY-156: only rows this batch *added* can put the history over a bound —
    // a duplicate bump and a pin change move no row count and no byte total. An
    // import that added nothing therefore owes no sweep, and running one applies
    // a lowered cap to history the user never asked to trim. A failed pin blocks
    // it too: the row it names is still unpinned, so I9 would not protect it.
    if result.inserted > 0 && result.pins_failed == 0 {
        retention::finish(batch);
    } else {
        retention::disarm(batch);
    }

    info!(
        inserted = result.inserted,
        skipped = result.skipped,
        skipped_duplicate = result.skipped_duplicate,
        skipped_empty = result.skipped_empty,
        skipped_too_large = result.skipped_too_large,
        pins_failed = result.pins_failed,
        "imported history"
    );
    if let Some(e) = fatal {
        return Err(e);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc, Mutex};

    use super::*;
    use crate::transfer::export;
    use crate::transfer::testkit::{fixture, fixture_named, Fixture};

    fn item(content: &str) -> ExportItem {
        ExportItem {
            content: content.to_string(),
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            created_at: 1_700_000_000_000,
            pinned: false,
            is_sensitive: false,
        }
    }

    fn import_into(f: &Fixture, items: Vec<ExportItem>) -> Result<ImportData, ImportError> {
        import(&f.store, &f.detector, &f.keyring, &f.settings, items)
    }

    #[test]
    fn an_import_terminal_sweep_reads_the_policy_published_after_admission() {
        let dir = tempfile::TempDir::new().unwrap();
        let keyring = Arc::new(Keyring::from_secret(&[5u8; 32]));
        let db_key = keyring.db_key();
        let store = Store::open(&dir.path().join("history.db"), &db_key).unwrap();
        let detector = Arc::new(Detector::new().unwrap());
        let ingress = ConfigData {
            history_limit: 1,
            ..ConfigData::default()
        };
        crate::ingest::ingest_into(
            &store,
            &detector,
            &keyring,
            "first",
            copypaste_ipc::content_type::TEXT,
            1_700_000_000_000,
            &ingress,
        )
        .unwrap();

        let current = Arc::new(Mutex::new(ingress.clone()));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        store.before_next_retention(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        let worker = {
            let store = store.clone();
            let detector = Arc::clone(&detector);
            let keyring = Arc::clone(&keyring);
            let current = Arc::clone(&current);
            let ingress = ingress.clone();
            std::thread::spawn(move || {
                import_with_current_retention(
                    &store,
                    &detector,
                    &keyring,
                    &ingress,
                    || current.lock().unwrap().clone(),
                    vec![item("second")],
                )
            })
        };
        entered_rx.recv().unwrap();
        current.lock().unwrap().history_limit = 2;
        release_tx.send(()).unwrap();

        worker.join().unwrap().unwrap();
        assert_eq!(store.count().unwrap(), 2);
        assert_eq!(store.search("first", 10).unwrap().len(), 1);
        assert_eq!(store.search("second", 10).unwrap().len(), 1);
    }

    #[test]
    fn an_export_round_trips_onto_a_device_that_cannot_read_the_original() {
        let source = fixture_named("alpha");
        source.add("first");
        source.add("second");
        let exported = export(&source.store, &source.keyring, 0, false).unwrap();

        let target = fixture_named("beta");
        let result = import_into(&target, exported.items).unwrap();
        assert_eq!(result.inserted, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.skipped_duplicate, 0);
        assert_eq!(result.skipped_empty, 0);
        assert_eq!(result.skipped_too_large, 0);
        assert_eq!(target.contents(), ["first", "second"]);
    }

    /// PG-26. An export edited to claim a credential is clean must not import as
    /// clean: the detector is re-run and the two are OR-ed. This is the property
    /// that matters most in the module — an import is the one path by which a
    /// user's own file decides what the database believes.
    #[test]
    fn an_import_cannot_smuggle_a_credential_in_marked_clean() {
        let f = fixture();
        import_into(&f, vec![item("AKIAIOSFODNN7EXAMPLE")]).unwrap();

        let row = f.store.list(10, 0).unwrap().remove(0);
        assert!(row.is_sensitive, "the detector did not re-run on import");
        assert!(
            f.store
                .search("AKIAIOSFODNN7EXAMPLE", 10)
                .unwrap()
                .is_empty(),
            "an imported credential reached the search index"
        );
    }

    /// An exported sensitive classification remains binding even if the current
    /// detector no longer recognises the text.
    #[test]
    fn an_exported_sensitive_item_never_reaches_fts() {
        let f = fixture();
        let mut claimed = item("just a note");
        claimed.is_sensitive = true;
        import_into(&f, vec![claimed]).unwrap();

        assert!(f.store.list(10, 0).unwrap().remove(0).is_sensitive);
        assert!(f.store.search("note", 10).unwrap().is_empty());
    }

    #[test]
    fn a_far_future_import_stamp_is_clamped_before_sync_can_refuse_it() {
        let f = fixture();
        let mut future = item("ordinary note");
        future.created_at = now_ms().saturating_add(MAX_IMPORT_FUTURE_SKEW_MS * 2);
        import_into(&f, vec![future]).unwrap();

        let created_at = f.store.list(10, 0).unwrap().remove(0).created_at;
        assert!(created_at <= now_ms().saturating_add(MAX_IMPORT_FUTURE_SKEW_MS));
    }

    #[test]
    fn timestamps_and_pins_survive_a_round_trip() {
        let source = fixture_named("alpha");
        let id = source.add("keep me");
        source.store.set_pinned(&id, true).unwrap();
        let created_at = source.store.get(&id).unwrap().unwrap().created_at;

        let target = fixture_named("beta");
        let exported = export(&source.store, &source.keyring, 0, false).unwrap();
        import_into(&target, exported.items).unwrap();

        let row = target.store.list(10, 0).unwrap().remove(0);
        assert_eq!(row.created_at, created_at, "the original stamp was lost");
        assert!(row.pinned, "the pin was lost");
    }

    #[test]
    fn an_empty_or_oversized_batch_is_refused_before_anything_is_written() {
        let f = fixture();
        assert!(matches!(
            import_into(&f, Vec::new()),
            Err(ImportError::Empty)
        ));

        let batch = vec![item("x"); MAX_IMPORT_ITEMS + 1];
        assert!(matches!(import_into(&f, batch), Err(ImportError::TooMany)));
        assert_eq!(f.store.count().unwrap(), 0);
    }

    /// An entry that cannot be stored at all is counted, not fatal: losing the
    /// rest of the file over one blank line is data loss (AGENTS.md rule 4).
    #[test]
    fn a_mixed_batch_reports_every_skip_reason_and_the_rest_still_lands() {
        let mut f = fixture();
        f.settings.max_text_size_bytes = 16;
        import_into(&f, vec![item("already here")]).unwrap();
        let result = import_into(
            &f,
            vec![
                item("already here"),
                item("   "),
                item(&"x".repeat(17)),
                item("keeps going"),
            ],
        )
        .unwrap();

        assert_eq!(result.inserted, 1);
        assert_eq!(result.skipped, 3);
        assert_eq!(result.skipped_duplicate, 1);
        assert_eq!(result.skipped_empty, 1);
        assert_eq!(result.skipped_too_large, 1);
        assert_eq!(f.contents(), ["already here", "keeps going"]);
    }

    #[test]
    fn an_all_empty_batch_is_counted_as_empty() {
        let f = fixture();
        let result = import_into(&f, vec![item(""), item(" \n\t")]).unwrap();

        assert_eq!(result.inserted, 0);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.skipped_duplicate, 0);
        assert_eq!(result.skipped_empty, 2);
        assert_eq!(result.skipped_too_large, 0);
        assert_eq!(f.store.count().unwrap(), 0);
    }

    #[test]
    fn an_all_over_limit_batch_is_counted_as_too_large() {
        let mut f = fixture();
        f.settings.max_text_size_bytes = copypaste_ipc::MIN_TEXT_SIZE_BYTES;
        let oversized = "x".repeat(copypaste_ipc::MIN_TEXT_SIZE_BYTES as usize + 1);
        let result = import_into(&f, vec![item(&oversized), item(&oversized)]).unwrap();

        assert_eq!(result.inserted, 0);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.skipped_duplicate, 0);
        assert_eq!(result.skipped_empty, 0);
        assert_eq!(result.skipped_too_large, 2);
        assert_eq!(f.store.count().unwrap(), 0);
    }

    /// Importing the same file twice is one history, not two — the dedup window
    /// is applied around each item's own stamp, which is why re-importing an old
    /// file collapses rather than doubling.
    #[test]
    fn importing_the_same_file_twice_does_not_double_the_history() {
        let f = fixture();
        let batch = vec![item("first"), item("second")];
        assert_eq!(import_into(&f, batch.clone()).unwrap().inserted, 2);

        let again = import_into(&f, batch).unwrap();
        assert_eq!(again.inserted, 0);
        assert_eq!(again.skipped, 2);
        assert_eq!(again.skipped_duplicate, 2);
        assert_eq!(again.skipped_empty, 0);
        assert_eq!(again.skipped_too_large, 0);
        assert_eq!(f.store.count().unwrap(), 2);
    }

    /// An import writes and never deletes on its own account. What it *can*
    /// evict is what any ingest evicts — the history cap and age-based retention
    /// — so a batch larger than the cap trims the history exactly as the same
    /// number of captures would, and by no other rule.
    #[test]
    fn an_import_deletes_nothing_beyond_the_users_own_cap() {
        let mut f = fixture();
        f.add("already here");
        f.settings.history_limit = 3;

        import_into(&f, vec![item("a"), item("b")]).unwrap();
        assert_eq!(f.store.count().unwrap(), 3, "nothing existing was removed");

        import_into(&f, vec![item("c")]).unwrap();
        assert_eq!(f.store.count().unwrap(), 3, "the cap is the only evictor");
    }

    /// The amplification this seam exists for: retention is a bound on the
    /// history, and running it per item made a 200-entry file run 200 sweeps —
    /// 600 write transactions — to reach the history one sweep leaves.
    #[test]
    fn a_file_is_swept_once_however_many_entries_it_has() {
        let f = fixture();
        let batch: Vec<ExportItem> = (0..200).map(|n| item(&format!("entry {n}"))).collect();

        crate::retention::sweeps::take();
        import_into(&f, batch).unwrap();
        assert_eq!(crate::retention::sweeps::take(), 1);
    }

    /// The bound has to end up in the same place, or the seam has traded
    /// correctness for speed. Same items, same cap: one file through the batch
    /// path against the same items ingested one at a time.
    #[test]
    fn a_batch_import_leaves_the_history_a_per_item_ingest_would() {
        // Distinct stamps, or the cap tie-breaks on a random uuid and the two
        // sides keep an arbitrary seven each.
        let entries: Vec<(String, i64)> = (0..40)
            .map(|n| (format!("entry {n}"), 1_700_000_000_000 + n * 60_000))
            .collect();

        let mut batched = fixture();
        batched.settings.history_limit = 7;
        let items: Vec<ExportItem> = entries
            .iter()
            .map(|(content, created_at)| ExportItem {
                created_at: *created_at,
                ..item(content)
            })
            .collect();
        import_into(&batched, items).unwrap();

        let mut per_item = fixture();
        per_item.settings.history_limit = 7;
        for (content, created_at) in &entries {
            crate::ingest::ingest_into(
                &per_item.store,
                &per_item.detector,
                &per_item.keyring,
                content,
                copypaste_ipc::content_type::TEXT,
                *created_at,
                &per_item.settings,
            )
            .unwrap();
        }

        assert_eq!(batched.store.count().unwrap(), 7);
        assert_eq!(batched.contents(), per_item.contents());
    }

    /// A pin named by the file must survive the sweep the same import runs.
    #[test]
    fn a_restored_pin_outlives_the_batch_sweep() {
        let mut f = fixture();
        f.settings.history_limit = 2;

        let mut oldest = item("pin me");
        oldest.pinned = true;
        oldest.created_at = 1_700_000_000_000;
        let mut newer = item("ordinary");
        newer.created_at = 1_700_000_600_000;
        let mut newest = item("newest");
        newest.created_at = 1_700_001_200_000;

        import_into(&f, vec![oldest, newer, newest]).unwrap();

        let rows = f.store.list(10, 0).unwrap();
        assert!(
            rows.iter().any(|row| row.pinned),
            "the pin was lost to the sweep: {:?}",
            f.contents()
        );
    }

    /// DMY-156 B1: an import that commits zero rows must never run retention
    /// or delete pre-existing history. A blank input under a lowered cap is
    /// the exact scenario the review reproduced.
    #[test]
    fn a_zero_write_import_never_deletes_existing_history() {
        let mut f = fixture();
        f.add("old");
        f.add("new");
        assert_eq!(f.store.count().unwrap(), 2);

        f.settings.history_limit = 1;

        let result = import_into(&f, vec![item(""), item(" \n\t")]).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(
            f.store.count().unwrap(),
            2,
            "an all-blank import must not touch existing rows"
        );
    }

    /// DMY-156 B1: same as above, but with oversized items and a lowered byte
    /// quota — another zero-write scenario that must not trigger eviction.
    #[test]
    fn a_zero_write_oversized_import_never_deletes_existing_history() {
        let mut f = fixture();
        f.add("keep me");
        f.settings.max_text_size_bytes = copypaste_ipc::MIN_TEXT_SIZE_BYTES;
        f.settings.storage_quota_bytes = 0;

        let oversized = "x".repeat(copypaste_ipc::MIN_TEXT_SIZE_BYTES as usize + 1);
        let result = import_into(&f, vec![item(&oversized)]).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(
            f.store.count().unwrap(),
            1,
            "an all-oversized import must not evict under a byte quota"
        );
    }

    /// DMY-156 B1: all-duplicate is another zero-write scenario. The dedup
    /// path writes nothing new, so the sweep must not run.
    #[test]
    fn an_all_duplicate_import_does_not_evict_under_a_lowered_cap() {
        let mut f = fixture();
        f.add("alpha");
        f.add("bravo");
        assert_eq!(f.store.count().unwrap(), 2);

        f.settings.history_limit = 1;
        let result = import_into(&f, vec![item("alpha"), item("bravo")]).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(
            f.store.count().unwrap(),
            2,
            "an all-duplicate import must not apply the lowered cap"
        );
    }

    /// DMY-156: `inserted` counts new rows, not durable intent. An import whose
    /// every item deduplicates still owes the pins the file named, and skipping
    /// the pin loop on `inserted == 0` dropped them — so re-importing a file to
    /// restore a lost pin did nothing at all.
    #[test]
    fn an_all_duplicate_import_still_restores_the_pins_the_file_named() {
        let mut f = fixture();
        let alpha = f.add("alpha");
        f.add("bravo");
        f.settings.history_limit = 1;

        crate::retention::sweeps::take();
        let result = import_into(
            &f,
            vec![
                ExportItem {
                    pinned: true,
                    ..item("alpha")
                },
                item("bravo"),
            ],
        )
        .unwrap();

        assert_eq!(result.inserted, 0);
        assert_eq!(result.skipped_duplicate, 2);
        assert!(
            f.store.get(&alpha).unwrap().unwrap().pinned,
            "the duplicate's pin was not restored"
        );
        assert_eq!(f.store.count().unwrap(), 2, "nothing may be evicted");
        assert_eq!(crate::retention::sweeps::take(), 0);
    }

    /// DMY-156: the same, with a fatal item behind the duplicate. The early
    /// return on `inserted == 0` skipped the pin loop here too, so the error
    /// took the pin with it.
    #[test]
    fn a_fatal_item_after_a_duplicate_still_restores_the_duplicates_pin() {
        let f = fixture();
        let alpha = f.add("alpha");
        crate::storage::test_support::reject_writes(&f.store, "no_inserts", "INSERT", "1");

        let error = import_into(
            &f,
            vec![
                ExportItem {
                    pinned: true,
                    ..item("alpha")
                },
                item("bravo"),
            ],
        )
        .unwrap_err();

        assert!(matches!(error, ImportError::Storage(_)), "{error:?}");
        assert!(
            f.store.get(&alpha).unwrap().unwrap().pinned,
            "a fatal item must not take a prior item's pin with it"
        );
        assert_eq!(f.store.count().unwrap(), 1);
    }

    /// DMY-156: a pin that cannot be written is counted on the result, not
    /// logged and swallowed. The rows stay — data loss is the worse outcome —
    /// and no retention runs, because the row the pin names is still unpinned
    /// and I9 would not protect it.
    #[test]
    fn a_failed_pin_update_is_reported_and_evicts_nothing() {
        let mut f = fixture();
        f.add("already here");
        f.settings.history_limit = 1;
        crate::storage::test_support::reject_writes(&f.store, "no_pins", "UPDATE OF pinned", "1");

        crate::retention::sweeps::take();
        let result = import_into(
            &f,
            vec![ExportItem {
                pinned: true,
                ..item("imported")
            }],
        )
        .unwrap();

        assert_eq!(result.inserted, 1);
        assert_eq!(result.pins_failed, 1, "the lost pin was not reported");
        assert_eq!(
            f.store.count().unwrap(),
            2,
            "a failed pin must not cost the rows that did land"
        );
        assert_eq!(crate::retention::sweeps::take(), 0);
        assert!(f.store.list(10, 0).unwrap().iter().all(|row| !row.pinned));
    }

    /// DMY-156 B1: zero-write with a pinned row (I9). If the sweep ran it
    /// could not touch the pinned row, but the other one would be evicted
    /// by a cap of 1. Both must survive.
    #[test]
    fn a_zero_write_import_preserves_pinned_rows_and_their_neighbours() {
        let mut f = fixture();
        let id = f.add("pinned");
        f.store.set_pinned(&id, true).unwrap();
        f.add("unpinned");
        assert_eq!(f.store.count().unwrap(), 2);

        f.settings.history_limit = 1;
        let result = import_into(&f, vec![item("")]).unwrap();
        assert_eq!(result.inserted, 0);
        assert_eq!(f.store.count().unwrap(), 2);
    }

    /// DMY-156 B1: verify the sweep count is zero for a zero-write import.
    #[test]
    fn a_zero_write_import_runs_no_sweep() {
        let f = fixture();
        f.add("existing");

        crate::retention::sweeps::take();
        import_into(&f, vec![item("")]).unwrap();
        assert_eq!(
            crate::retention::sweeps::take(),
            0,
            "a zero-write import must not run any sweep"
        );
    }

    /// DMY-156 B2: a fatal error after prior writes must still restore pins for
    /// the items already committed, and the sweep those rows owe still runs —
    /// after the pins, so I9 protects them. The second insert is refused by an
    /// injected trigger, which is the failure the earlier version of this test
    /// only described.
    #[test]
    fn a_fatal_later_item_still_restores_pins_for_prior_writes() {
        let mut f = fixture();
        f.settings.history_limit = 1;
        crate::storage::test_support::reject_writes(
            &f.store,
            "no_second_insert",
            "INSERT",
            "(SELECT COUNT(*) FROM clipboard_items) >= 1",
        );

        crate::retention::sweeps::take();
        let error = import_into(
            &f,
            vec![
                ExportItem {
                    pinned: true,
                    created_at: 1_700_000_000_000,
                    ..item("pin me")
                },
                ExportItem {
                    created_at: 1_700_000_600_000,
                    ..item("never lands")
                },
            ],
        )
        .unwrap_err();

        assert!(matches!(error, ImportError::Storage(_)), "{error:?}");
        let rows = f.store.list(10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].pinned,
            "the committed row lost its pin to the error"
        );
        assert_eq!(
            crate::retention::sweeps::take(),
            1,
            "a batch that did write rows still owes its sweep"
        );
    }

    #[test]
    fn import_errors_disclose_no_path() {
        for message in [
            ImportError::Empty.to_string(),
            ImportError::TooMany.to_string(),
            ImportError::Storage(StoreError::InvalidKey).to_string(),
            ImportError::Crypto(CryptoError::AuthFailed).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
