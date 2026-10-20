//! Keyset-paged summary exchange and session watermark calculation.

use std::collections::{HashMap, HashSet};

#[cfg(test)]
use super::plan::Plan;
use super::{SummaryWorkspace, SyncChannel, SyncError, SyncSource};
use crate::protocol::{
    ItemSummary, SyncMessage, MAX_SUMMARIES_PER_MESSAGE, MAX_SUMMARY_PAGES_PER_SESSION,
};

pub(super) fn summary_key(summary: &ItemSummary) -> i64 {
    summary.created_at.max(summary.pin_updated_at)
}

#[cfg(test)]
pub(super) fn watermark(
    previous: i64,
    remote: &[ItemSummary],
    planned: &Plan,
    attempted: &HashSet<String>,
) -> i64 {
    let mut reached = previous;
    let mut blocked = i64::MAX;
    for summary in remote {
        let key = summary_key(summary);
        let unresolved = planned.deferred.contains(&summary.item_id)
            || (planned.wanted.contains_key(&summary.item_id)
                && !attempted.contains(&summary.item_id));
        if unresolved {
            blocked = blocked.min(key);
        } else {
            reached = reached.max(key);
        }
    }
    reached.min(blocked)
}

fn next_page<S: SyncSource>(
    source: &S,
    since_ms: i64,
    after_id: Option<&str>,
) -> Result<(Vec<ItemSummary>, bool), SyncError> {
    let mut items = source.summary_page(since_ms, after_id, MAX_SUMMARIES_PER_MESSAGE + 1)?;
    let more = items.len() > MAX_SUMMARIES_PER_MESSAGE;
    if more {
        items.truncate(MAX_SUMMARIES_PER_MESSAGE);
    }
    Ok((items, more))
}

/// Exchange directly into a session workspace while preserving wire order.
pub(super) async fn exchange_workspace<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    since_ms: i64,
    mut workspace: Box<dyn SummaryWorkspace>,
    initiator: bool,
) -> Result<Box<dyn SummaryWorkspace>, SyncError> {
    let mut after_id = None;
    let mut cursor_ms = since_ms;
    for _ in 0..MAX_SUMMARY_PAGES_PER_SESSION {
        let peer_first = if initiator {
            None
        } else {
            Some(receive_page(chan).await?)
        };
        let (items, more) = next_page(source, cursor_ms, after_id.as_deref())?;
        if let Some(last) = items.last() {
            cursor_ms = summary_key(last);
            after_id = Some(last.item_id.clone());
        }
        let blocking = workspace.may_block(&items);
        let (next, (result, items)) =
            super::workspace_call(workspace, blocking, move |workspace| {
                let result = workspace.add_advertised(&items);
                (result, items)
            })
            .await?;
        workspace = next;
        result?;
        chan.send(SyncMessage::Summary { items, more }).await?;
        let (peer_items, peer_more) = match peer_first {
            Some(page) => page,
            None => receive_page(chan).await?,
        };
        // Capture local metadata before retaining this remote page. The
        // advertised table authorizes requests only; it can omit a local row
        // below the peer cursor even though that row must still win planning.
        let ids: Vec<_> = peer_items.iter().map(|item| item.item_id.clone()).collect();
        let comparison = source.local_summaries(&ids)?;
        let blocking = workspace.may_block(&comparison);
        let (next, result) = super::workspace_call(workspace, blocking, move |workspace| {
            workspace.add_comparison(&comparison)
        })
        .await?;
        workspace = next;
        result?;
        let blocking = workspace.may_block(&peer_items);
        let (next, result) = super::workspace_call(workspace, blocking, move |workspace| {
            workspace.add_remote(&peer_items)
        })
        .await?;
        workspace = next;
        result?;
        if !more && !peer_more {
            return Ok(workspace);
        }
    }
    Err(SyncError::TooManySummaryPages)
}

pub(super) fn workspace_watermark(
    previous: i64,
    workspace: &mut dyn SummaryWorkspace,
    wanted: &HashMap<String, ItemSummary>,
    attempted: &HashSet<String>,
) -> Result<i64, SyncError> {
    let mut reached = previous;
    let mut blocked = i64::MAX;
    let mut cursor = 0;
    loop {
        let page = workspace.remote_page(cursor, 256)?;
        if page.is_empty() {
            break;
        }
        for (next, summary) in page {
            cursor = next;
            let key = summary_key(&summary);
            let unresolved = workspace.is_deferred(&summary.item_id)?
                || (wanted.contains_key(&summary.item_id) && !attempted.contains(&summary.item_id));
            if unresolved {
                blocked = blocked.min(key);
            } else {
                reached = reached.max(key);
            }
        }
    }
    Ok(reached.min(blocked))
}

async fn receive_page<C: SyncChannel>(chan: &mut C) -> Result<(Vec<ItemSummary>, bool), SyncError> {
    let msg = chan.recv().await?;
    msg.validate()?;
    match msg {
        SyncMessage::Summary { items, more } => Ok((items, more)),
        other => Err(SyncError::Unexpected {
            expected: "summary",
            got: other.kind(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SyncItem;
    use crate::sync::testutil::{item, summary, ScriptChannel, TestSource};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn planned(wanted: &[&ItemSummary], deferred: &[&str]) -> Plan {
        Plan {
            wanted: wanted
                .iter()
                .map(|s| (s.item_id.clone(), (*s).clone()))
                .collect(),
            deferred: deferred.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    #[test]
    fn watermark_stops_below_unfinished_work() {
        let remote = vec![
            summary("a", 100, "h", false),
            summary("b", 200, "h", false),
            summary("c", 300, "h", false),
        ];
        let plan = planned(&[&remote[1]], &[]);

        assert_eq!(watermark(0, &remote, &plan, &HashSet::new()), 200);
        let attempted = HashSet::from(["b".to_string()]);
        assert_eq!(watermark(0, &remote, &plan, &attempted), 300);
    }

    #[test]
    fn watermark_never_moves_without_remote_work() {
        assert_eq!(
            watermark(5_000, &[], &planned(&[], &[]), &HashSet::new()),
            5_000
        );
    }

    #[test]
    fn deferred_work_pulls_the_watermark_back() {
        let remote = vec![
            summary("old", 100, "h", false),
            summary("new", 900, "h", false),
        ];
        assert_eq!(
            watermark(5_000, &remote, &planned(&[], &["old"]), &HashSet::new()),
            100
        );
    }

    #[test]
    fn refused_future_stamp_cannot_advance_the_watermark() {
        let remote = vec![
            summary("sane", 900, "h", false),
            summary("forged", i64::MAX, "h", false),
        ];
        assert_eq!(
            watermark(0, &remote, &planned(&[], &["forged"]), &HashSet::new()),
            900
        );
    }

    #[test]
    fn pin_stamp_is_part_of_the_cursor_key() {
        let mut pinned = summary("ancient", 800, "h", false);
        pinned.pin_updated_at = 6_000;
        assert_eq!(summary_key(&pinned), 6_000);
        assert_eq!(summary_key(&summary("plain", 800, "h", false)), 800);
    }

    #[tokio::test]
    async fn exchange_reads_bounded_source_pages() {
        struct PagedOnly {
            inner: TestSource,
            full_loads: AtomicUsize,
            page_loads: AtomicUsize,
            comparison_loads: AtomicUsize,
        }

        impl SyncSource for PagedOnly {
            fn device_id(&self) -> String {
                self.inner.device_id()
            }
            fn device_name(&self) -> String {
                self.inner.device_name()
            }
            fn summaries(&self, since_ms: i64) -> Result<Vec<ItemSummary>, SyncError> {
                self.full_loads.fetch_add(1, Ordering::SeqCst);
                self.inner.summaries(since_ms)
            }
            fn local_summaries(&self, ids: &[String]) -> Result<Vec<ItemSummary>, SyncError> {
                self.comparison_loads.fetch_add(1, Ordering::SeqCst);
                self.inner.local_summaries(ids)
            }
            fn summary_page(
                &self,
                since_ms: i64,
                after_id: Option<&str>,
                limit: usize,
            ) -> Result<Vec<ItemSummary>, SyncError> {
                self.page_loads.fetch_add(1, Ordering::SeqCst);
                Ok(self.inner.page(since_ms, after_id, limit))
            }
            fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
                self.inner.fetch(ids)
            }
            fn apply(&self, item: SyncItem) -> Result<bool, SyncError> {
                self.inner.apply(item)
            }
        }

        let source = PagedOnly {
            inner: TestSource::new("dev-a", vec![item("one", 100, "body", "dev-a")]),
            full_loads: AtomicUsize::new(0),
            page_loads: AtomicUsize::new(0),
            comparison_loads: AtomicUsize::new(0),
        };
        let mut channel = ScriptChannel::new(vec![SyncMessage::Summary {
            items: vec![],
            more: false,
        }]);

        let mut workspace = exchange_workspace(
            &mut channel,
            &source,
            0,
            source.summary_workspace().unwrap(),
            true,
        )
        .await
        .unwrap();

        assert!(workspace.advertised("one").unwrap().is_some());
        assert!(workspace.remote_page(0, 1).unwrap().is_empty());
        assert_eq!(source.full_loads.load(Ordering::SeqCst), 0);
        assert_eq!(source.page_loads.load(Ordering::SeqCst), 1);
        assert_eq!(source.comparison_loads.load(Ordering::SeqCst), 1);
    }
}
