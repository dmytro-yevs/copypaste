use copypaste_ipc::{ConfigData, ExportItem, ImportData};
use tracing::{info, warn};

use crate::storage::{Store, StoreError};
use crate::{
    ingest::ingest_into_with_sensitivity_floor, now_ms, CryptoError, Detector, IngestError,
    Ingested, Keyring,
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
/// username (CLAUDE.md rule 4).
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
    };
    let mut pin: Vec<String> = Vec::new();

    for item in items {
        // `ingest_into` recomputes sensitivity from the plaintext and takes the
        // item's own `created_at`, so a restored history keeps its order and a
        // file imported twice collapses rather than doubling.
        let created_at = item
            .created_at
            .min(now_ms().saturating_add(MAX_IMPORT_FUTURE_SKEW_MS));
        match ingest_into_with_sensitivity_floor(
            store,
            detector,
            keyring,
            &item.content,
            &item.content_type,
            created_at,
            item.is_sensitive,
            settings,
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
            // Empty and over-size are the two entries worth tolerating: neither
            // can be stored and neither is a reason to lose the other 9 999.
            Err(IngestError::Empty) => {
                result.skipped += 1;
                result.skipped_empty += 1;
            }
            Err(IngestError::TooLarge) => {
                result.skipped += 1;
                result.skipped_too_large += 1;
            }
            Err(IngestError::Crypto(e)) => return Err(ImportError::Crypto(e)),
            Err(IngestError::Storage(e)) => return Err(ImportError::Storage(e)),
        }
    }

    // Pins are applied after the fact rather than inside the ingest: the store
    // has one pin path (`set_pinned`, which also assigns `pin_order`) and a
    // second one here would be a second definition of what a pin is.
    for item_id in &pin {
        if let Err(e) = store.set_pinned(item_id, true) {
            warn!(error = ?e, "could not restore a pin on an imported item");
        }
    }

    info!(
        inserted = result.inserted,
        skipped = result.skipped,
        skipped_duplicate = result.skipped_duplicate,
        skipped_empty = result.skipped_empty,
        skipped_too_large = result.skipped_too_large,
        "imported history"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
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
    /// rest of the file over one blank line is data loss (CLAUDE.md rule 4).
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
        f.settings.max_item_bytes = 3;
        let result = import_into(&f, vec![item("four"), item("five!")]).unwrap();

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
