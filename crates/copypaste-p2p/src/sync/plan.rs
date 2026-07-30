//! Deciding what to ask a peer for, from the two summaries alone. Pure and
//! clock-injected, so the delete-before-create case and the clock-skew ceiling
//! are testable without a session.

use std::collections::{HashMap, HashSet};

use super::{merge_decision_by_summary, MergeDecision, SyncStats};
use crate::protocol::{ItemSummary, MAX_REQUEST_IDS_PER_MESSAGE};

/// How far into the future a peer's `created_at` may be before we refuse that
/// version. The lower bound is clamped at the decode boundary; this upper bound
/// needs the local clock. Without it a peer — hostile or merely wrong-clocked —
/// stamps `i64::MAX` on one `item_id` and wins every future comparison for it,
/// censoring that item on every device. A day is far more than any real skew,
/// and the response is to skip that one version, never to fail the session or
/// delete anything (manifest 05 §3.4, R-CLK-2: refusal, not correction).
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

/// Decides what to ask the peer for, from the two summaries alone.
///
/// Three cases, and the third is the subtle one:
///
/// * we have never seen the id — want it, **including when it is a tombstone**.
///   A delete for an unknown item must still be taken, or a create that arrives
///   afterwards has nothing to lose against and resurrects the item (manifest 05
///   rule T-3, `CopyPaste-bfiu`).
/// * the remote version wins the order — want it.
/// * the local version wins or ties — skip. A tie is the replay case, and
///   skipping it is what makes a repeated session free.
pub(super) fn plan(
    local: &HashMap<String, ItemSummary>,
    remote: &[ItemSummary],
    now: i64,
    stats: &mut SyncStats,
) -> HashMap<String, ItemSummary> {
    let ceiling = now.saturating_add(MAX_FUTURE_SKEW_MS);
    let mut wanted: HashMap<String, ItemSummary> = HashMap::new();

    for r in remote {
        if r.created_at > ceiling {
            tracing::warn!(
                skew_ms = r.created_at.saturating_sub(now),
                "peer offered a version stamped beyond the clock-skew ceiling; skipping it"
            );
            stats.skipped += 1;
            continue;
        }

        let take = match local.get(&r.item_id) {
            None => true,
            Some(l) => merge_decision_by_summary(l, r) == MergeDecision::TakeRemote,
        };

        if !take {
            stats.skipped += 1;
            continue;
        }

        // A peer may repeat an id in its summary. Keep the version that wins, so
        // a duplicate cannot smuggle a loser past the comparison above.
        match wanted.get(&r.item_id) {
            Some(seen) if merge_decision_by_summary(seen, r) != MergeDecision::TakeRemote => {
                stats.skipped += 1;
            }
            _ => {
                wanted.insert(r.item_id.clone(), r.clone());
            }
        }
    }

    if wanted.len() > MAX_REQUEST_IDS_PER_MESSAGE {
        let mut by_age: Vec<_> = wanted.values().cloned().collect();
        by_age.sort_unstable_by_key(|s| std::cmp::Reverse(s.created_at));
        by_age.truncate(MAX_REQUEST_IDS_PER_MESSAGE);
        let keep: HashSet<&str> = by_age.iter().map(|s| s.item_id.as_str()).collect();
        let dropped = wanted.len() - keep.len();
        tracing::warn!(
            dropped,
            max = MAX_REQUEST_IDS_PER_MESSAGE,
            "more wanted items than one session transfers; the rest follow next session"
        );
        wanted.retain(|id, _| keep.contains(id.as_str()));
        stats.skipped += dropped;
    }

    wanted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testutil::summary;

    #[test]
    fn an_unknown_tombstone_is_wanted() {
        // Delete-before-create: the tombstone must be taken even though the item
        // was never seen, or a later create resurrects it (T-3).
        let local = HashMap::new();
        let remote = vec![summary("gone", 50, "h", true)];
        let mut stats = SyncStats::default();
        let wanted = plan(&local, &remote, 1_000, &mut stats);
        assert!(wanted.contains_key("gone"));
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn a_version_beyond_the_skew_ceiling_is_skipped() {
        let local = HashMap::new();
        let now = 1_000_000;
        let remote = vec![
            summary("sane", now, "h", false),
            summary("forged", i64::MAX, "h", false),
        ];
        let mut stats = SyncStats::default();
        let wanted = plan(&local, &remote, now, &mut stats);
        assert!(wanted.contains_key("sane"));
        assert!(!wanted.contains_key("forged"));
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn a_duplicated_id_in_a_summary_cannot_smuggle_a_loser() {
        let local = HashMap::new();
        let remote = vec![
            summary("i", 300, "h", false),
            summary("i", 100, "h", false),
            summary("i", 200, "h", false),
        ];
        let mut stats = SyncStats::default();
        let wanted = plan(&local, &remote, 10_000, &mut stats);
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted["i"].created_at, 300);
        assert_eq!(stats.skipped, 2);
    }

    #[test]
    fn wants_beyond_one_session_are_deferred_not_dropped() {
        let local = HashMap::new();
        let remote: Vec<_> = (0..MAX_REQUEST_IDS_PER_MESSAGE + 10)
            .map(|n| summary(&format!("i{n}"), n as i64, "h", false))
            .collect();
        let mut stats = SyncStats::default();
        let wanted = plan(&local, &remote, i64::MAX / 2, &mut stats);
        assert_eq!(wanted.len(), MAX_REQUEST_IDS_PER_MESSAGE);
        assert_eq!(stats.skipped, 10);
        assert!(wanted.contains_key(&format!("i{}", MAX_REQUEST_IDS_PER_MESSAGE + 9)));
        assert!(!wanted.contains_key("i0"));
    }
}
