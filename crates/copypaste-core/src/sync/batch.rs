//! [`apply_remote_version`] for a whole page.
//!
//! Two properties the row-at-a-time path gets for free and this one builds:
//!
//! * **The decision and the write see the same history.** Deciding from a read
//!   that a local delete or a newer peer version can land behind would let an
//!   unconditional UPSERT resurrect it, so the write transaction re-reads the
//!   summaries and refuses a snapshot that has moved (B6).
//! * **The answer does not depend on how the page was cut.** A simulated winner
//!   the dedup index then refuses never becomes a row, so anything that lost to
//!   it lost to nothing; those are re-applied one at a time afterwards (B7).

use std::collections::HashMap;

use tracing::warn;

use super::merge::{apply_remote_version, payload_is_refused, MergeError, RemoteVersion};
use super::prepare::{prepare_remote_version, Prepared};
use crate::sensitive::Detector;
use crate::storage::{IncomingItem, MergePageError, Store, Version};
use crate::Keyring;

/// How many times a moved snapshot is re-prepared before the page gives up.
///
/// One retry, not a loop: a second race under the write lock means the history
/// is being written continuously, and failing the round is safe where spinning
/// against a live writer is not.
const SNAPSHOT_ATTEMPTS: usize = 2;

/// [`apply_remote_version`] for a whole page, answering positionally.
///
/// The same comparator, seal and refusals; what changes is how often the store
/// is touched. A cloud page of 500 rows went through 1,000 pooled connection
/// checkouts and up to 500 IMMEDIATE write transactions when it was driven a
/// row at a time, on a pool four deep, so it spent most of its wall clock
/// queueing behind itself. Now: one read, then one write transaction.
///
/// # Errors
///
/// [`MergeError::Store`] or [`MergeError::Encrypt`]. A version this device
/// declines is `false`, never an error.
pub fn apply_remote_versions(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
) -> Result<Vec<bool>, MergeError> {
    if incoming.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<&str> = incoming.iter().map(|item| item.item_id).collect();
    let mut snapshot = store.version_summaries(&ids).map_err(|e| {
        warn!(error = ?e, "could not read the local versions of an incoming page");
        MergeError::Store
    })?;

    for _ in 0..SNAPSHOT_ATTEMPTS {
        let (prepared, slots) = prepare(keyring, detector, here, incoming, &snapshot)?;
        let writes: Vec<IncomingItem<'_>> = prepared.iter().map(Prepared::as_incoming).collect();
        match store.merge_page(&ids, &snapshot, &writes) {
            Ok(written) => {
                if written.len() != prepared.len() {
                    warn!("the database did not answer for every row of a merged page");
                    return Err(MergeError::Store);
                }
                let mut applied: Vec<bool> = slots
                    .iter()
                    .map(|slot| slot.is_some_and(|index| written[index]))
                    .collect();
                redo_after_refusals(
                    store,
                    keyring,
                    detector,
                    here,
                    incoming,
                    &slots,
                    &written,
                    &mut applied,
                )?;
                return Ok(applied);
            }
            Err(MergePageError::SnapshotChanged(authoritative)) => {
                warn!("a concurrent write changed the merge snapshot; re-preparing");
                snapshot = authoritative;
            }
            Err(MergePageError::Store(e)) => {
                warn!(error = ?e, "could not merge an incoming page");
                return Err(MergeError::Store);
            }
        }
    }

    warn!("the merge snapshot changed on every attempt");
    Err(MergeError::Store)
}

/// Decide the whole page against `local`, threading each id's own decisions
/// forward. `slots[i]` is `None` for a version that lost outright, otherwise
/// its index in the returned writes.
fn prepare(
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    local: &HashMap<String, Version>,
) -> Result<(Vec<Prepared>, Vec<Option<usize>>), MergeError> {
    let mut local = local.clone();
    let mut prepared: Vec<Prepared> = Vec::new();
    let mut slots: Vec<Option<usize>> = Vec::with_capacity(incoming.len());
    for item in incoming {
        if payload_is_refused(item) {
            slots.push(None);
            continue;
        }
        match prepare_remote_version(keyring, detector, here, item, None, local.get(item.item_id))?
        {
            Some(ready) => {
                // A second version of the same id later in this page must
                // compare against this decision, not against the row on disk.
                local.insert(item.item_id.to_string(), ready.as_version(here));
                slots.push(Some(prepared.len()));
                prepared.push(ready);
            }
            None => slots.push(None),
        }
    }
    Ok((prepared, slots))
}

/// Re-apply the versions that were decided against a winner the store then
/// refused (B7).
///
/// The dedup index refuses a row whose content already belongs to a different
/// id from the same origin in the same bucket. That refusal is per row, so the
/// page keeps its other writes — but a later version of the *refused* id was
/// compared against a winner that never became a row, and on its own page it
/// might well have been admitted. Only the versions that came out `false` are
/// redone: a `true` is a row that actually landed, and re-offering it would
/// have it decline against itself.
fn redo_after_refusals(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    slots: &[Option<usize>],
    written: &[bool],
    applied: &mut [bool],
) -> Result<(), MergeError> {
    let mut refused_at: HashMap<&str, usize> = HashMap::new();
    for (index, slot) in slots.iter().enumerate() {
        if slot.is_some_and(|slot| !written[slot]) {
            refused_at.entry(incoming[index].item_id).or_insert(index);
        }
    }
    if refused_at.is_empty() {
        return Ok(());
    }
    for (index, item) in incoming.iter().enumerate() {
        if applied[index]
            || refused_at
                .get(item.item_id)
                .is_none_or(|first| index <= *first)
        {
            continue;
        }
        applied[index] = apply_remote_version(store, keyring, detector, here, item)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{fixture_named, version, Fixture};
    use super::*;

    fn apply_page(f: &Fixture, page: &[RemoteVersion<'_>]) -> Vec<bool> {
        apply_remote_versions(&f.store, &f.keyring, &f.detector, &f.here, page).expect("merge")
    }

    /// Occupy the dedup key `(content_hash, created_at / 60000, origin)` under
    /// a different id, which is what makes the store refuse a later row with
    /// the same content in the same minute from the same device.
    fn occupy(f: &Fixture) {
        assert!(apply_page(f, &[version("occupant", "collides", 5_000)])[0]);
    }

    /// B7, reproduced. The first version of `target` wins the comparator and is
    /// then refused by the dedup index, so it never becomes a row — but the
    /// second version was declined against it. Sending the two on separate
    /// pages admits the second, and one page has to answer the same way or the
    /// history a device ends up with depends on how the transport cut its
    /// pages.
    #[test]
    fn a_version_declined_against_a_refused_winner_is_re_applied() {
        let batched = fixture_named("batched");
        occupy(&batched);
        let answers = apply_page(
            &batched,
            &[
                version("target", "collides", 9_000),
                version("target", "admissible", 8_000),
            ],
        );

        assert_eq!(
            answers,
            vec![false, true],
            "the refused winner must not decide for the version behind it"
        );
        let stored = batched
            .store
            .version("target")
            .expect("read")
            .expect("the admissible version was lost to a row that never existed");
        assert_eq!(stored.created_at, 8_000);
    }

    /// The same versions, one per page. This is the baseline the batched answer
    /// has to match; asserting it here keeps the comparison honest if the
    /// comparator itself ever changes.
    #[test]
    fn one_page_and_two_pages_reach_the_same_history() {
        let batched = fixture_named("same");
        occupy(&batched);
        let together = apply_page(
            &batched,
            &[
                version("target", "collides", 9_000),
                version("target", "admissible", 8_000),
            ],
        );

        let apart = fixture_named("same");
        occupy(&apart);
        let mut separately = apply_page(&apart, &[version("target", "collides", 9_000)]);
        separately.extend(apply_page(
            &apart,
            &[version("target", "admissible", 8_000)],
        ));

        assert_eq!(together, separately);
        assert_eq!(
            batched
                .store
                .version("target")
                .expect("read")
                .map(|row| row.created_at),
            apart
                .store
                .version("target")
                .expect("read")
                .map(|row| row.created_at)
        );
    }

    /// A page with no refusal must not pay for the repair path, and a version
    /// that legitimately lost to one that *did* land must stay lost.
    #[test]
    fn a_page_that_was_not_refused_is_answered_by_the_batch_alone() {
        let f = fixture_named("clean");
        let answers = apply_page(
            &f,
            &[
                version("item", "older", 1_000),
                version("item", "newer", 2_000),
                version("other", "unrelated", 3_000),
            ],
        );
        assert_eq!(answers, vec![true, true, true]);
        assert_eq!(
            f.store.version("item").expect("read").unwrap().created_at,
            2_000
        );
    }
}
