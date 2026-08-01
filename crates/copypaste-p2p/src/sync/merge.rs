//! The merge decision: which of two versions of one item survives.
//!
//! # Why last-write-wins, and why it is not a library
//!
//! The structural CRDT crates (`automerge`, `yrs`) cannot be used here, under
//! `CLAUDE.md` rule 1 exemption 2: a structural CRDT merges *inside* a value,
//! and our items are opaque ciphertext at rest with the merge running over
//! metadata alone, so there is nothing for it to operate on. Last-write-wins
//! over four metadata keys is not a shortcut, it is the only thing that works.
//! Its cost, stated rather than hidden: two devices that edit the *same* item
//! concurrently keep one version and drop the other — correct for a clipboard,
//! where an entry is a snapshot, not a document.
//!
//! # The order
//!
//! Compare two versions of the same `item_id`. Larger wins:
//!
//! 1. `created_at` — the version stamp in milliseconds. A tombstone carries the
//!    time of the *deletion*, which is what makes a delete beat the version it
//!    deleted.
//! 2. `content_hash` — lexicographic; equal hashes mean equal content, so this
//!    key only ever separates versions that genuinely differ.
//! 3. `deleted` — a tombstone beats a live version. See below.
//! 4. `origin_device_id` — lexicographic; exists only to make the order total.
//!
//! Ties on all four keep the local copy. That strict `>` at the end is what
//! makes re-delivery free: an already-applied version compares equal and loses,
//! so replaying a whole session changes nothing.
//!
//! ## Why `deleted` is key 3 and not key 5
//!
//! The specification for this module named three keys: `created_at`,
//! `content_hash`, `origin_device_id`. `deleted` is inserted between the second
//! and the third — the named keys keep their relative priority — for two
//! reasons, the second load-bearing:
//!
//! * **Delete-wins on an exact-timestamp tie.** A tombstone keeps the item's
//!   content hash (deliberately, so re-copying deleted content is not swallowed
//!   by dedup), so a delete of an unmodified item ties its own live version on
//!   keys 1 and 2, and without key 3 the winner would be whichever device id
//!   sorts higher. That resurrects deletes (INV-N2, `CopyPaste-ojhe`).
//! * **One comparator on both sides of the session.** [`ItemSummary`] carries
//!   every merge key, including `origin_device_id`, so planning and applying
//!   cannot choose different winners for an origin-only conflict.

use crate::protocol::ItemSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeDecision {
    KeepLocal,
    TakeRemote,
}

/// Picks the winner between two versions of the same `item_id`.
///
/// Deterministic, symmetric and total: `merge_decision(a, b)` and
/// `merge_decision(b, a)` name the same survivor, so two peers cannot ping-pong
/// a pair of versions between each other. Equal on every key keeps the local
/// copy, which is what makes applying the same item twice a no-op. Content is
/// never an input — at rest it is ciphertext, and the design depends on the
/// decision being reachable from metadata alone.
pub fn merge_decision(
    local: &ItemSummary,
    local_origin: &str,
    remote: &ItemSummary,
    remote_origin: &str,
) -> MergeDecision {
    fn decide<T: Ord>(local: T, remote: T) -> Option<MergeDecision> {
        match remote.cmp(&local) {
            std::cmp::Ordering::Greater => Some(MergeDecision::TakeRemote),
            std::cmp::Ordering::Less => Some(MergeDecision::KeepLocal),
            std::cmp::Ordering::Equal => None,
        }
    }

    decide(local.created_at, remote.created_at)
        .or_else(|| decide(local.content_hash.as_str(), remote.content_hash.as_str()))
        .or_else(|| decide(local.deleted, remote.deleted))
        .or_else(|| decide(local_origin, remote_origin))
        // Equal on all four: the same version. Keep local (INV-I1).
        .unwrap_or(MergeDecision::KeepLocal)
}

/// The same comparator over a summary. Not a second comparator — it delegates
/// to [`merge_decision`] with the complete metadata, which keeps planning and
/// apply decisions identical (manifest 05 INV-C2).
pub fn merge_decision_by_summary(local: &ItemSummary, remote: &ItemSummary) -> MergeDecision {
    merge_decision(
        local,
        &local.origin_device_id,
        remote,
        &remote.origin_device_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testutil::summary;

    /// Every distinguishable point in the decision space: three timestamps,
    /// three hashes, both delete states, three origins.
    fn decision_space() -> Vec<(ItemSummary, String)> {
        let mut out = Vec::new();
        for created_at in [1i64, 2, 3] {
            for hash in ["ha", "hb", "hc"] {
                for deleted in [false, true] {
                    for origin in ["d1", "d2", "d3"] {
                        out.push((
                            summary("same-id", created_at, hash, deleted),
                            origin.to_string(),
                        ));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn merge_is_symmetric_across_the_whole_decision_space() {
        // The proof that two peers cannot ping-pong: whichever side asks, the
        // same version is named the survivor.
        let space = decision_space();
        for (x, xo) in &space {
            for (y, yo) in &space {
                let from_x = match merge_decision(x, xo, y, yo) {
                    MergeDecision::TakeRemote => (y, yo),
                    MergeDecision::KeepLocal => (x, xo),
                };
                let from_y = match merge_decision(y, yo, x, xo) {
                    MergeDecision::TakeRemote => (x, xo),
                    MergeDecision::KeepLocal => (y, yo),
                };
                assert_eq!(
                    from_x, from_y,
                    "peers disagreed on the winner of {x:?}/{xo} vs {y:?}/{yo}"
                );
            }
        }
    }

    #[test]
    fn merge_is_exactly_the_four_key_lexicographic_order() {
        // Pinning the order to a tuple comparison proves it is a strict total
        // order — hence transitive, hence order-independent (INV-C1, INV-C3).
        let key = |s: &ItemSummary, o: &str| {
            (
                s.created_at,
                s.content_hash.clone(),
                s.deleted,
                o.to_string(),
            )
        };
        let space = decision_space();
        for (x, xo) in &space {
            for (y, yo) in &space {
                let expected = if key(y, yo) > key(x, xo) {
                    MergeDecision::TakeRemote
                } else {
                    MergeDecision::KeepLocal
                };
                assert_eq!(merge_decision(x, xo, y, yo), expected);
            }
        }
    }

    #[test]
    fn summary_comparator_agrees_with_the_full_one() {
        // The equivalence that stops the two shapes drifting apart (§7.2).
        let space = decision_space();
        for (x, xo) in &space {
            for (y, yo) in &space {
                let mut x = x.clone();
                x.origin_device_id = xo.clone();
                let mut y = y.clone();
                y.origin_device_id = yo.clone();
                assert_eq!(
                    merge_decision_by_summary(&x, &y),
                    merge_decision(&x, xo, &y, yo),
                    "summary comparator diverged"
                );
            }
        }
    }

    #[test]
    fn a_later_version_wins() {
        let older = summary("i", 100, "h", false);
        let newer = summary("i", 200, "h", false);
        assert_eq!(
            merge_decision(&older, "a", &newer, "b"),
            MergeDecision::TakeRemote
        );
        assert_eq!(
            merge_decision(&newer, "a", &older, "b"),
            MergeDecision::KeepLocal
        );
    }

    #[test]
    fn equal_timestamps_break_on_content_hash() {
        let lo = summary("i", 100, "aaa", false);
        let hi = summary("i", 100, "bbb", false);
        assert_eq!(
            merge_decision(&lo, "z", &hi, "a"),
            MergeDecision::TakeRemote
        );
        assert_eq!(merge_decision(&hi, "a", &lo, "z"), MergeDecision::KeepLocal);
    }

    #[test]
    fn equal_timestamp_and_hash_break_on_delete_then_origin() {
        let live = summary("i", 100, "h", false);
        let dead = summary("i", 100, "h", true);
        // Delete wins regardless of which device ids are involved — the case
        // that would otherwise resurrect a tombstone.
        assert_eq!(
            merge_decision(&live, "zzz", &dead, "aaa"),
            MergeDecision::TakeRemote
        );
        assert_eq!(
            merge_decision(&dead, "aaa", &live, "zzz"),
            MergeDecision::KeepLocal
        );
        // With the delete state equal too, the origin is the last resort.
        assert_eq!(
            merge_decision(&live, "aaa", &live, "zzz"),
            MergeDecision::TakeRemote
        );
    }

    #[test]
    fn an_exact_tie_keeps_local() {
        let s = summary("i", 100, "h", false);
        assert_eq!(merge_decision(&s, "d", &s, "d"), MergeDecision::KeepLocal);
    }

    /// A re-copy restamps `created_at` (`copypaste_core::Store::insert_or_bump`),
    /// and `created_at` is key 1 — so a bump is a version and it travels. The
    /// property that makes that safe is that it settles: the peer takes it once
    /// and the next round is a tie.
    #[test]
    fn a_recopy_bump_travels_once_and_then_settles() {
        let before = summary("x", 100, "h", false);
        let bumped = summary("x", 500, "h", false);
        // Same item, same content, same origin — only the stamp moved.
        assert_eq!(
            merge_decision(&before, "a", &bumped, "a"),
            MergeDecision::TakeRemote
        );
        assert_eq!(
            merge_decision(&bumped, "a", &before, "a"),
            MergeDecision::KeepLocal,
            "the device that bumped must not take its own older copy back"
        );
        assert_eq!(
            merge_decision(&bumped, "a", &bumped, "a"),
            MergeDecision::KeepLocal,
            "a second round must change nothing"
        );
    }

    /// A bump must never outrank a delete of the same item made later. Key 1
    /// decides it, and a tombstone carries the time of the deletion.
    #[test]
    fn a_bump_cannot_resurrect_a_later_delete() {
        let bumped = summary("x", 500, "h", false);
        let deleted = summary("x", 900, "h", true);
        assert_eq!(
            merge_decision(&bumped, "a", &deleted, "b"),
            MergeDecision::TakeRemote
        );
        assert_eq!(
            merge_decision(&deleted, "b", &bumped, "a"),
            MergeDecision::KeepLocal
        );
    }

    #[test]
    fn an_older_live_version_cannot_resurrect_a_newer_tombstone() {
        let dead = summary("i", 200, "h", true);
        let live = summary("i", 100, "h2", false);
        assert_eq!(
            merge_decision(&dead, "a", &live, "b"),
            MergeDecision::KeepLocal
        );
        assert_eq!(
            merge_decision(&live, "b", &dead, "a"),
            MergeDecision::TakeRemote
        );
    }
}
