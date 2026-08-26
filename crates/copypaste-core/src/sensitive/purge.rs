//! Re-deciding, for rows already in the search index, the one question that was
//! only ever asked once.
//!
//! The three FTS-exclusion layers in [`crate::storage`] all run at capture, off a
//! verdict the ruleset gave at that moment. A rule added afterwards never sees
//! those rows again: their plaintext stays in `clipboard_fts` — the one table
//! not under the item AEAD — for as long as the item lives. Two detector fixes
//! landed on 2026-07-30 (security review F-1, F-2), so this is a live gap, not a
//! hypothetical one. `AGENTS.md` rule 4 has promised this pass since before it
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

use rusqlite::Transaction;

use crate::sensitive::Detector;
use crate::storage::{
    indexed_texts_in, purge_from_index_in, purge_index_of_unsearchable_in, IndexedText, Store,
    StoreError,
};

/// Rows per read page. Large enough that the ~10k-row default history is a
/// handful of statements, small enough that no page is a meaningful allocation.
const PAGE_ROWS: u32 = 512;

/// Bytes per read page, and the bound that actually holds. Rows alone do not
/// bound anything: an item may be 4 MiB, so 512 of them is 2 GiB held to look at
/// one row at a time. A history of small notes still pages by rows; one of large
/// pastes pages by this.
const PAGE_BYTES: usize = 1024 * 1024;

/// What one pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeReport {
    /// Index rows examined.
    pub scanned: u64,
    /// Index rows removed. Non-zero means the ruleset has changed under a
    /// history, which is worth a log line at the call site.
    pub purged: u64,
}

/// Drop every search-index row that belongs to an already-classified item, and
/// every one whose text the *current* ruleset classifies above the floor.
///
/// Two predicates because they catch different failures. The first (manifest 03
/// S4) catches an index row that should never have been
/// written for a row already known to be sensitive; the second catches the row
/// nobody knew about, which is the one a detector fix creates. Neither implies
/// the other: a classified item's text may no longer match above the floor, and
/// a high-confidence match may sit on a row classified clean.
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
    purge_index(store, detector)
}

/// Purge restored index rows inside the caller's transaction.
///
/// This is the restore boundary: a failure rolls the table replacement and
/// every purge delete back together, so sensitive plaintext is never committed
/// searchable while a restore reports failure.
pub fn purge_indexed_secrets_in_transaction(
    tx: &Transaction<'_>,
    detector: &Detector,
) -> rusqlite::Result<PurgeReport> {
    purge_index(tx, detector)
}

trait PurgeIndex {
    type Error;

    fn purge_unsearchable(&self) -> Result<u64, Self::Error>;
    fn indexed_texts(
        &self,
        after: i64,
        max_rows: u32,
        max_bytes: usize,
    ) -> Result<Vec<IndexedText>, Self::Error>;
    fn purge_rowids(&self, rowids: &[i64]) -> Result<u64, Self::Error>;
}

impl PurgeIndex for Store {
    type Error = StoreError;

    fn purge_unsearchable(&self) -> Result<u64, Self::Error> {
        self.purge_index_of_unsearchable()
    }

    fn indexed_texts(
        &self,
        after: i64,
        max_rows: u32,
        max_bytes: usize,
    ) -> Result<Vec<IndexedText>, Self::Error> {
        Store::indexed_texts(self, after, max_rows, max_bytes)
    }

    fn purge_rowids(&self, rowids: &[i64]) -> Result<u64, Self::Error> {
        self.purge_from_index(rowids)
    }
}

impl PurgeIndex for Transaction<'_> {
    type Error = rusqlite::Error;

    fn purge_unsearchable(&self) -> Result<u64, Self::Error> {
        purge_index_of_unsearchable_in(self)
    }

    fn indexed_texts(
        &self,
        after: i64,
        max_rows: u32,
        max_bytes: usize,
    ) -> Result<Vec<IndexedText>, Self::Error> {
        indexed_texts_in(self, after, max_rows, max_bytes)
    }

    fn purge_rowids(&self, rowids: &[i64]) -> Result<u64, Self::Error> {
        purge_from_index_in(self, rowids)
    }
}

fn purge_index<I: PurgeIndex>(index: &I, detector: &Detector) -> Result<PurgeReport, I::Error> {
    let mut report = PurgeReport::default();
    report.purged += index.purge_unsearchable()?;

    let mut after = 0i64;
    loop {
        let page = index.indexed_texts(after, PAGE_ROWS, PAGE_BYTES)?;
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
            report.purged += index.purge_rowids(&doomed)?;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{
        fts_row_count, item, plant_fts_bytes, plant_fts_row, raw_row_count, store, T0,
    };
    use crate::storage::NewItem;

    fn detector() -> Detector {
        Detector::new().expect("ruleset compiles")
    }

    /// A distinct credential payload per row. Distinct because these fixtures
    /// are index rows the pass must each recognise, and random-looking because
    /// every rule above the floor now gates on its value's entropy (§5.6) — a
    /// repeated-character payload is a placeholder and is meant to be rejected.
    fn token_payload(seed: i64, len: usize) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut state =
            0x9e37_79b9_7f4a_7c15_u64 ^ (seed as u64).wrapping_mul(0x2545_f491_4f6c_dd1d);
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                char::from(ALPHABET[(state % ALPHABET.len() as u64) as usize])
            })
            .collect()
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

    /// `AGENTS.md` rule 4. The re-derived verdict may take an index entry and
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
                    &format!("token ghp_{}", token_payload(n, 36)),
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

    /// A row whose stored text is not valid UTF-8 must not end the pass.
    ///
    /// Typed as `String`, one such row made the page read fail — and because the
    /// pages are ordered by `rowid`, every *later* page failed with it, so a
    /// genuine secret sitting behind it stayed indexed for as long as the bad row
    /// did. It is now decoded lossily: the pass continues, and a credential
    /// inside the undecodable row is itself still found, because a replacement
    /// character cannot split an ASCII token.
    #[test]
    fn an_undecodable_row_neither_stops_the_pass_nor_hides_behind_it() {
        let s = store();
        let holder = s
            .insert(NewItem {
                is_sensitive: false,
                search_text: None,
                ..item("holder", T0)
            })
            .unwrap();
        let mut bytes = b"caption ".to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe]);
        bytes.extend_from_slice(b" AKIAIOSFODNN7EXAMPLE tail");
        plant_fts_bytes(&s, &holder.id, &bytes);
        let behind = missed_at_capture(&s, &format!("ghp_{}", token_payload(1, 36)), T0 + 60_000);
        let ordinary = missed_at_capture(&s, "the release checklist", T0 + 120_000);
        assert_eq!(fts_row_count(&s, &holder.id), 1);

        let report = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(report.scanned, 3, "the pass stopped at the undecodable row");
        assert_eq!(report.purged, 2);

        assert_eq!(fts_row_count(&s, &holder.id), 0, "the bad row kept its key");
        assert_eq!(fts_row_count(&s, &behind), 0, "a secret hid behind it");
        assert_eq!(fts_row_count(&s, &ordinary), 1);
        assert!(s.get(&holder.id).unwrap().is_some(), "no item was deleted");
    }

    /// Rows alone do not bound a page: the item cap is 4 MiB, so `PAGE_ROWS` of
    /// them is 2 GiB held to look at one row at a time. The byte budget is what
    /// makes peak memory a number, and a row larger than the whole budget must
    /// still make progress or the cursor never advances.
    #[test]
    fn a_page_is_bounded_by_bytes_as_well_as_rows() {
        let s = store();
        let big = PAGE_BYTES / 4;
        for n in 0..12i64 {
            missed_at_capture(&s, &format!("{n:04} {}", "n".repeat(big)), T0 + n * 60_000);
        }
        let oversized = PAGE_BYTES * 2;
        missed_at_capture(&s, &"z".repeat(oversized), T0 + 12 * 60_000);

        let mut after = 0i64;
        let mut pages = 0;
        let mut rows = 0;
        loop {
            let page = s.indexed_texts(after, PAGE_ROWS, PAGE_BYTES).unwrap();
            if page.is_empty() {
                break;
            }
            let bytes: usize = page.iter().map(|row| row.text.len()).sum();
            assert!(
                bytes <= PAGE_BYTES || page.len() == 1,
                "page of {} rows held {bytes} bytes",
                page.len()
            );
            after = page[page.len() - 1].rowid;
            rows += page.len();
            pages += 1;
            assert!(pages <= 16, "the cursor is not advancing");
        }
        assert_eq!(rows, 13);
        assert!(pages > 1, "the byte budget never split a page");

        // The whole pass still walks every row, oversized one included.
        assert_eq!(purge_indexed_secrets(&s, &detector()).unwrap().scanned, 13);
    }

    /// A history past the 1 000-row mark is the shape DMY-162 asks about: the
    /// pass must walk all of it, take every secret in it, and leave every
    /// ordinary row searchable — with the page bounds holding throughout.
    #[test]
    fn a_history_beyond_a_thousand_rows_is_walked_whole() {
        let s = store();
        let rows = 1_200i64;
        let mut leaked = Vec::new();
        for n in 0..rows {
            let created_at = T0 + n * 60_000;
            if n % 7 == 0 {
                leaked.push(missed_at_capture(
                    &s,
                    &format!("AccountKey={}==", token_payload(n, 86)),
                    created_at,
                ));
            } else {
                missed_at_capture(&s, &format!("meeting note {n}"), created_at);
            }
        }

        let report = purge_indexed_secrets(&s, &detector()).unwrap();
        assert_eq!(report.scanned, rows as u64);
        assert_eq!(report.purged, leaked.len() as u64);
        for id in &leaked {
            assert_eq!(fts_row_count(&s, id), 0, "{id} stayed indexed");
        }
        assert_eq!(s.count().unwrap(), rows as u64, "no item was deleted");
        assert_eq!(s.search("meeting", 5).unwrap().len(), 5);
        assert_eq!(purge_indexed_secrets(&s, &detector()).unwrap().purged, 0);
    }

    /// Manifest 03 S4: an index row planted against an
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

    /// Inert findings remain searchable across startup rescans. Detection is
    /// still available to masking consumers through `scan_all`.
    #[test]
    fn the_purge_keeps_inert_findings_in_the_index() {
        let s = store();
        let inert = missed_at_capture(&s, "mail alice.smith@example.com about it", T0);
        let det = detector();
        assert!(!det
            .scan_all("mail alice.smith@example.com about it")
            .is_empty());
        assert!(!det.is_sensitive("mail alice.smith@example.com about it"));
        assert!(!det.may_auto_wipe("mail alice.smith@example.com about it"));

        assert_eq!(purge_indexed_secrets(&s, &det).unwrap().purged, 0);
        assert_eq!(fts_row_count(&s, &inert), 1);
        assert!(s.get(&inert).unwrap().is_some());
    }
}
