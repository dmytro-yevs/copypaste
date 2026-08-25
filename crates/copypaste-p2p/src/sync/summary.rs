//! Keyset-paged summary exchange and session watermark calculation.

use std::collections::{HashMap, HashSet};

use super::plan::Plan;
use super::{SyncChannel, SyncError, SyncSource};
use crate::protocol::{
    ItemSummary, SyncMessage, MAX_SUMMARIES_PER_MESSAGE, MAX_SUMMARY_PAGES_PER_SESSION,
};

pub(super) fn summary_key(summary: &ItemSummary) -> i64 {
    summary.created_at.max(summary.pin_updated_at)
}

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

fn remember_page(advertised: &mut HashMap<String, ItemSummary>, items: &[ItemSummary]) {
    for summary in items {
        advertised.insert(summary.item_id.clone(), summary.clone());
    }
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

/// Exchanges every summary page in lock-step so neither side materializes the
/// full local history or fills a bounded channel before reading its peer.
pub(super) async fn exchange_initiator<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    since_ms: i64,
) -> Result<(HashMap<String, ItemSummary>, Vec<ItemSummary>), SyncError> {
    let mut advertised = HashMap::new();
    let mut remote = Vec::new();
    let mut after_id: Option<String> = None;
    let mut cursor_ms = since_ms;
    let mut page = 0;
    loop {
        if page >= MAX_SUMMARY_PAGES_PER_SESSION {
            return Err(SyncError::TooManySummaryPages);
        }
        let (items, more) = next_page(source, cursor_ms, after_id.as_deref())?;
        if let Some(last) = items.last() {
            cursor_ms = summary_key(last);
            after_id = Some(last.item_id.clone());
        }
        remember_page(&mut advertised, &items);
        chan.send(SyncMessage::Summary { items, more }).await?;
        let (peer_items, peer_more) = receive_page(chan).await?;
        remote.extend(peer_items);
        if !more && !peer_more {
            return Ok((advertised, remote));
        }
        page += 1;
    }
}

pub(super) async fn exchange_responder<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    since_ms: i64,
) -> Result<(HashMap<String, ItemSummary>, Vec<ItemSummary>), SyncError> {
    let mut advertised = HashMap::new();
    let mut remote = Vec::new();
    let mut after_id: Option<String> = None;
    let mut cursor_ms = since_ms;
    let mut page = 0;
    loop {
        if page >= MAX_SUMMARY_PAGES_PER_SESSION {
            return Err(SyncError::TooManySummaryPages);
        }
        let (peer_items, peer_more) = receive_page(chan).await?;
        remote.extend(peer_items);
        let (items, more) = next_page(source, cursor_ms, after_id.as_deref())?;
        if let Some(last) = items.last() {
            cursor_ms = summary_key(last);
            after_id = Some(last.item_id.clone());
        }
        remember_page(&mut advertised, &items);
        chan.send(SyncMessage::Summary { items, more }).await?;
        if !more && !peer_more {
            return Ok((advertised, remote));
        }
        page += 1;
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
        };
        let mut channel = ScriptChannel::new(vec![SyncMessage::Summary {
            items: vec![],
            more: false,
        }]);

        let (advertised, remote) = exchange_initiator(&mut channel, &source, 0).await.unwrap();

        assert_eq!(advertised.len(), 1);
        assert!(remote.is_empty());
        assert_eq!(source.full_loads.load(Ordering::SeqCst), 0);
        assert_eq!(source.page_loads.load(Ordering::SeqCst), 1);
    }
}
