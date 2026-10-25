//! Deciding what to ask a peer for, from the two summaries alone. Pure and
//! clock-injected, so the delete-before-create case and the clock-skew ceiling
//! are testable without a session.

use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use super::{merge_decision_by_summary, pin_state_wins, MergeDecision};
use super::{SummaryWorkspace, SyncError, SyncStats};
use crate::protocol::{ItemSummary, MAX_REQUEST_IDS_PER_MESSAGE};

/// How far into the future a peer's `created_at` may be before we refuse that
/// version. The lower bound is clamped at the decode boundary; this upper bound
/// needs the local clock. Without it a peer — hostile or merely wrong-clocked —
/// stamps `i64::MAX` on one `item_id` and wins every future comparison for it,
/// censoring that item on every device. A day is far more than any real skew,
/// and the response is to skip that one version, never to fail the session or
/// delete anything (manifest 05 §3.4, R-CLK-2: refusal, not correction).
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

#[cfg(test)]
pub(super) struct Plan {
    pub(super) wanted: HashMap<String, ItemSummary>,
    pub(super) deferred: HashSet<String>,
}

/// Plan a session by scanning bounded workspace pages rather than retaining
/// every remote version and every selected candidate in process memory.
pub(super) fn plan_workspace(
    workspace: &mut dyn SummaryWorkspace,
    now: i64,
    stats: &mut SyncStats,
) -> Result<HashMap<String, ItemSummary>, SyncError> {
    let ceiling = now.saturating_add(MAX_FUTURE_SKEW_MS);
    let mut cursor = 0;
    loop {
        let page = workspace.remote_page(cursor, 256)?;
        if page.is_empty() {
            break;
        }
        for (next, r) in page {
            cursor = next;
            if r.created_at > ceiling {
                tracing::warn!(
                    skew_ms = r.created_at.saturating_sub(now),
                    "peer offered a version stamped beyond the clock-skew ceiling; skipping it"
                );
                stats.skipped += 1;
                stats.skipped_future += 1;
                workspace.defer(&r.item_id)?;
                continue;
            }
            if !workspace.remote_wins(&r)? {
                stats.skipped += 1;
                continue;
            }
            if !workspace.consider_wanted(&r)? {
                stats.skipped += 1;
            }
        }
    }
    let (selected, dropped) = workspace.finish_plan(MAX_REQUEST_IDS_PER_MESSAGE)?;
    if dropped > 0 {
        tracing::warn!(
            dropped,
            max = MAX_REQUEST_IDS_PER_MESSAGE,
            "more wanted items than one session transfers; the rest follow next session"
        );
        stats.skipped += dropped;
        stats.deferred += dropped;
    }
    Ok(selected
        .into_iter()
        .map(|s| (s.item_id.clone(), s))
        .collect())
}

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
#[cfg(test)]
pub(super) fn plan(
    local: &HashMap<String, ItemSummary>,
    remote: &[ItemSummary],
    now: i64,
    stats: &mut SyncStats,
) -> Plan {
    let ceiling = now.saturating_add(MAX_FUTURE_SKEW_MS);
    let mut wanted: HashMap<String, ItemSummary> = HashMap::new();
    let mut deferred: HashSet<String> = HashSet::new();

    for r in remote {
        if r.created_at > ceiling {
            tracing::warn!(
                skew_ms = r.created_at.saturating_sub(now),
                "peer offered a version stamped beyond the clock-skew ceiling; skipping it"
            );
            stats.skipped += 1;
            stats.skipped_future += 1;
            deferred.insert(r.item_id.clone());
            continue;
        }

        let take = match local.get(&r.item_id) {
            None => true,
            Some(l) => {
                merge_decision_by_summary(l, r) == MergeDecision::TakeRemote || pin_state_wins(l, r)
            }
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
        for id in wanted.keys() {
            if !keep.contains(id.as_str()) {
                deferred.insert(id.clone());
            }
        }
        wanted.retain(|id, _| keep.contains(id.as_str()));
        stats.skipped += dropped;
        stats.deferred += dropped;
    }

    Plan { wanted, deferred }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testutil::summary;

    #[test]
    fn workspace_matches_existing_plan_and_watermark_with_duplicates_and_deferred() {
        use crate::sync::{MemorySummaryWorkspace, SummaryWorkspace};
        let local_items = vec![summary("local", 50, "h", false)];
        let local: HashMap<_, _> = local_items
            .iter()
            .map(|s| (s.item_id.clone(), s.clone()))
            .collect();
        let mut remote: Vec<_> = (0..MAX_REQUEST_IDS_PER_MESSAGE + 10)
            .map(|i| summary(&format!("item-{i}"), i as i64 + 100, "h", false))
            .collect();
        remote.extend([
            summary("local", 20, "h", false),
            summary("item-4", 5, "h", false),
            summary("future", i64::MAX, "h", false),
        ]);
        let mut old_stats = SyncStats::default();
        let old = plan(&local, &remote, 10_000, &mut old_stats);
        let mut workspace = MemorySummaryWorkspace::default();
        workspace.add_advertised(&local_items).unwrap();
        workspace.add_comparison(&local_items).unwrap();
        workspace.add_remote(&remote).unwrap();
        let mut new_stats = SyncStats::default();
        let wanted = plan_workspace(&mut workspace, 10_000, &mut new_stats).unwrap();
        assert_eq!(wanted, old.wanted);
        assert_eq!(new_stats, old_stats);
        let attempted = HashSet::from(["item-900".to_string()]);
        assert_eq!(
            super::super::summary::workspace_watermark(0, &mut workspace, &wanted, &attempted)
                .unwrap(),
            super::super::summary::watermark(0, &remote, &old, &attempted),
        );
    }

    #[test]
    fn an_unknown_tombstone_is_wanted() {
        // Delete-before-create: the tombstone must be taken even though the item
        // was never seen, or a later create resurrects it (T-3).
        let local = HashMap::new();
        let remote = vec![summary("gone", 50, "h", true)];
        let mut stats = SyncStats::default();
        let wanted = plan(&local, &remote, 1_000, &mut stats).wanted;
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
        let wanted = plan(&local, &remote, now, &mut stats).wanted;
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
        let wanted = plan(&local, &remote, 10_000, &mut stats).wanted;
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
        let wanted = plan(&local, &remote, i64::MAX / 2, &mut stats).wanted;
        assert_eq!(wanted.len(), MAX_REQUEST_IDS_PER_MESSAGE);
        assert_eq!(stats.skipped, 10);
        assert!(wanted.contains_key(&format!("i{}", MAX_REQUEST_IDS_PER_MESSAGE + 9)));
        assert!(!wanted.contains_key("i0"));
    }
}
