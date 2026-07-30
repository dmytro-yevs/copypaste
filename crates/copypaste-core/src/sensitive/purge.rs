//! Re-deciding, for rows already in the search index, the one question that was
//! only ever asked once.
//!
//! The three ADR-015 layers in [`crate::storage`] all run at capture, off a
//! verdict the ruleset gave at that moment. A rule added afterwards never sees
//! those rows again: their plaintext stays in `clipboard_fts` — the one table
//! not under the item AEAD — for as long as the item lives. Two detector fixes
//! landed on 2026-07-30 (security review F-1, F-2), so this is a live gap, not a
//! hypothetical one. `CLAUDE.md` rule 4 has promised this pass since before it
//! existed, and so do `copypaste-ipc`'s and the daemon's item payloads.
//!
//! # It removes from the index, and only from the index
//!
//! Re-deriving sensitivity necessarily disagrees with the user sometimes: a
//! ruleset that has grown will flag things they were content to keep. That is
//! survivable when the cost is "you can no longer find this by searching" and
//! unacceptable when it is "this is gone", so nothing here deletes a row,
//! tombstones one, or writes `is_sensitive`. Writing the flag would be the
//! quiet version of the same mistake: `is_sensitive = 1` is what
//! [`super::sweep_sensitive`] selects on, so a re-derived flag would hand a
//! changed ruleset a hard delete over data the user never reviewed.
//!
//! The item itself stays listable, readable, copyable and pinnable. Only the
//! plaintext copy in the index goes.
//!
//! # Why it can run on every open, and why there is no cursor
//!
//! Idempotent by construction: the second pass over a purged index finds
//! nothing, because the rows it would have removed are gone. It reads the index
//! rather than the history, so it needs no key and decrypts nothing, and its
//! cost is bounded by what is *searchable* — sensitive rows were never indexed,
//! and neither were tombstones.
//!
//! **Measured, release build, 2026-07-30.** A full pass over the 10,000-item
//! default history (`history_limit`, ~1 MB of indexed text) is **9.7 ms**; over
//! 11 MiB of indexed text — a hundred items near the 4 MiB item cap — **38 ms**.
//! Both exclude [`Detector::new`], which costs ~52 ms once and which the daemon
//! already builds and shares.
//!
//! That is what makes a bounded sweep with a persisted resume point the wrong
//! answer here. It was the right answer against the first measurement, which was
//! 140× worse; that turned out to be an undersized lazy-DFA cache on the
//! detector's prefilter and is now fixed at its source
//! (`PREFILTER_DFA_SIZE_LIMIT`), where it also pays back on every capture. A
//! persisted cursor would additionally need a stamp saying *which* ruleset had
//! already passed over each row, or it would skip exactly the rows the next
//! detector fix is for — a persisted verdict from a ruleset that has since
//! changed, which is the shape [`super::wipe`] rejects for the same reason.

use crate::sensitive::Detector;
use crate::storage::{Store, StoreError};

/// Rows per read page. Large enough that the ~10k-row default history is a
/// handful of statements, small enough that no page is a meaningful allocation.
const PAGE_ROWS: u32 = 512;

/// What one pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeReport {
    /// Index rows examined.
    pub scanned: u64,
    /// Index rows removed. Non-zero means the ruleset has changed under a
    /// history, which is worth a log line at the call site.
    pub purged: u64,
}

/// Drop every search-index row that belongs to an already-flagged item, and
/// every one whose text the *current* ruleset calls sensitive.
///
/// Two predicates because they catch different failures. The first (manifest 03
/// S4, v1's migration v13) catches an index row that should never have been
/// written for a row already known to be sensitive; the second catches the row
/// nobody knew about, which is the one a detector fix creates. Neither implies
/// the other: a flagged item's text may no longer match any rule, and a matching
/// text may sit on a row flagged clean.
///
/// Safe to run on every open and safe to run twice. Never removes a clipboard
/// item; see the module header.
///
/// # Errors
///
/// [`StoreError`] if a read or a delete fails. Deletes commit per page, so an
/// error partway leaves the pages already done purged and the rest for the next
/// run — which is correct, because each page's verdicts are independent.
pub fn purge_indexed_secrets(
    store: &Store,
    detector: &Detector,
) -> Result<PurgeReport, StoreError> {
    let mut report = PurgeReport::default();
    report.purged += store.purge_index_of_flagged()?;

    let mut after = 0i64;
    loop {
        let page = store.indexed_texts(after, PAGE_ROWS)?;
        if page.is_empty() {
            break;
        }
        after = page[page.len() - 1].rowid;
        report.scanned += page.len() as u64;

        let doomed: Vec<i64> = page
            .iter()
            .filter(|row| detector.is_sensitive(&row.text))
            .map(|row| row.rowid)
            .collect();
        if !doomed.is_empty() {
            tracing::info!(
                count = doomed.len(),
                "removing search-index entries the current ruleset calls sensitive"
            );
            report.purged += store.purge_from_index(&doomed)?;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{
        fts_row_count, item, plant_fts_row, raw_row_count, store, T0,
    };
    use crate::storage::NewItem;

    fn detector() -> Detector {
        Detector::new().expect("ruleset compiles")
    }

    /// A row indexed before its rule existed. Simulated the only way that is
    /// honest: insert it flagged as ordinary, exactly as a build with a smaller
    /// ruleset would have.
    fn missed_at_capture(store: &Store, text: &str, created_at: i64) -> String {
        let stored = store
            .insert(NewItem {
                is_sensitive: false,
                search_text: Some(text.to_string()),
                ..item(text, created_at)
            })
            .expect("insert");
        assert_eq!(
            fts_row_count(store, &stored.id),
            1,
            "the fixture must be in the index, or the test proves nothing"
        );
        stored.id
    }

    /// The whole point: a secret that predates the rule that catches it leaves
    /// the index, and stops being searchable.
    #[test]
    fn a_row_indexed_before_its_rule_existed_is_purged() {
        let s = store();
        let leaked = missed_at_capture(&s, "AKIAIOSFODNN7EXAMPLE", T0);
        let ordinary = missed_at_capture(&s, "shopping list: milk and bread", T0 + 60_000);
        assert_eq!(s.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().len(), 1);

        let report = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(report.scanned, 2);
        assert_eq!(report.purged, 1);

        assert_eq!(fts_row_count(&s, &leaked), 0);
        assert!(s.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().is_empty());
        assert_eq!(fts_row_count(&s, &ordinary), 1);
        assert_eq!(s.search("shopping", 10).unwrap().len(), 1);
    }

    /// `CLAUDE.md` rule 4. The re-derived verdict may take an index entry and
    /// may take nothing else — the item is still there, still readable, still
    /// listed.
    #[test]
    fn a_purged_row_keeps_its_clipboard_item() {
        let s = store();
        let leaked = missed_at_capture(&s, "AKIAIOSFODNN7EXAMPLE", T0);
        assert!(s.set_pinned(&leaked, true).unwrap());

        purge_indexed_secrets(&s, &detector()).unwrap();

        let kept = s.get(&leaked).unwrap().expect("the item must survive");
        assert_eq!(kept.content_ciphertext, b"ct:AKIAIOSFODNN7EXAMPLE");
        assert!(kept.pinned, "the pin must survive");
        assert_eq!(
            raw_row_count(&s, &leaked),
            1,
            "no tombstone, no hard delete"
        );
        assert_eq!(s.count().unwrap(), 1);
        assert_eq!(s.list(10, 0).unwrap().len(), 1);
    }

    /// The flag is not rewritten, because `is_sensitive = 1` is what the wipe
    /// sweep selects on and a re-derived flag would hand it a deletion nobody
    /// reviewed.
    #[test]
    fn a_purge_does_not_reflag_the_row_for_the_wipe_sweep() {
        let s = store();
        let leaked = missed_at_capture(&s, "AKIAIOSFODNN7EXAMPLE", T0);
        purge_indexed_secrets(&s, &detector()).unwrap();

        assert!(!s.get(&leaked).unwrap().unwrap().is_sensitive);
        assert!(
            !s.has_wipeable_sensitive(),
            "the purge must not arm the auto-wipe sweep"
        );
    }

    #[test]
    fn a_second_pass_finds_nothing_and_a_clean_history_is_untouched() {
        let s = store();
        missed_at_capture(&s, "AKIAIOSFODNN7EXAMPLE", T0);
        let ordinary = missed_at_capture(&s, "notes about the release", T0 + 60_000);

        let first = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(first.purged, 1);

        let second = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(second.scanned, 1);
        assert_eq!(second.purged, 0);
        assert_eq!(fts_row_count(&s, &ordinary), 1);

        let empty = store();
        assert_eq!(
            purge_indexed_secrets(&empty, &detector()).unwrap(),
            PurgeReport::default()
        );
    }

    /// The pass is paged; a history larger than one page must be walked to the
    /// end, and the cursor must not skip the row after a purge.
    #[test]
    fn every_page_is_walked_even_when_rows_are_removed_as_it_goes() {
        let s = store();
        let rows = PAGE_ROWS as i64 * 2 + 7;
        let mut leaked = Vec::new();
        for n in 0..rows {
            let created_at = T0 + n * 60_000;
            if n % 3 == 0 {
                leaked.push(missed_at_capture(
                    &s,
                    &format!("token ghp_{}{:028}", "A".repeat(8), n),
                    created_at,
                ));
            } else {
                missed_at_capture(&s, &format!("ordinary note number {n}"), created_at);
            }
        }

        let report = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(report.scanned, rows as u64);
        assert_eq!(report.purged, leaked.len() as u64);
        for id in &leaked {
            assert_eq!(fts_row_count(&s, id), 0, "{id} survived the sweep");
        }
        assert_eq!(s.count().unwrap(), rows as u64, "no item was deleted");
    }

    /// Manifest 03 S4, v1's migration v13: an index row planted against an
    /// already-flagged item goes, and the current ruleset has no say in it.
    #[test]
    fn a_stale_index_row_for_a_flagged_item_is_purged() {
        let s = store();
        let flagged = s
            .insert(NewItem {
                is_sensitive: true,
                search_text: None,
                ..item("a shape no rule matches any more", T0)
            })
            .unwrap();
        let ordinary = missed_at_capture(&s, "release checklist", T0 + 60_000);
        plant_fts_row(&s, &flagged.id, "a shape no rule matches any more");
        assert_eq!(fts_row_count(&s, &flagged.id), 1);
        let det = detector();
        assert!(
            !det.is_sensitive("a shape no rule matches any more"),
            "the fixture must be invisible to the detector, or this proves nothing"
        );

        assert_eq!(purge_indexed_secrets(&s, &det).unwrap().purged, 1);
        assert_eq!(fts_row_count(&s, &flagged.id), 0);
        assert_eq!(fts_row_count(&s, &ordinary), 1);
        assert!(s.get(&flagged.id).unwrap().is_some(), "the item survives");
    }

    /// The inclusive predicate, matching the write guard: an email address is
    /// flagged and kept out of the index, and is nowhere near the wipe floor.
    #[test]
    fn the_purge_uses_the_same_inclusive_predicate_as_the_write_guard() {
        let s = store();
        let flagged = missed_at_capture(&s, "mail alice.smith@example.com about it", T0);
        let det = detector();
        assert!(det.is_sensitive("mail alice.smith@example.com about it"));
        assert!(!det.may_auto_wipe("mail alice.smith@example.com about it"));

        assert_eq!(purge_indexed_secrets(&s, &det).unwrap().purged, 1);
        assert_eq!(fts_row_count(&s, &flagged), 0);
        assert!(s.get(&flagged).unwrap().is_some());
    }
}
