//! Validation and application of item streams received from a peer.

use std::collections::{HashMap, HashSet};

use super::summary::summary_key;
use super::{SyncChannel, SyncError, SyncSource, SyncStats};
use crate::protocol::{ItemSummary, SyncMessage};

pub(super) struct Applied {
    pub(super) attempted: HashSet<String>,
    pub(super) floor: Option<i64>,
}

/// Reads `Items` messages until `Done`, applying only versions promised by the
/// peer's summary.
pub(super) async fn receive_items<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    wanted: &HashMap<String, ItemSummary>,
    stats: &mut SyncStats,
) -> Result<Applied, SyncError> {
    // The byte cap can force every requested item into its own `Items` message.
    // One extra frame lets an invalid item be dropped before `Done`; anything
    // beyond one frame per requested id plus that allowance cannot be useful.
    let mut budget = wanted.len().saturating_add(2);
    let mut applied: HashSet<String> = HashSet::new();
    let mut floor: Option<i64> = None;

    loop {
        if budget == 0 {
            return Err(SyncError::PeerOverran);
        }
        budget -= 1;

        let msg = chan.recv().await?;
        msg.validate()?;
        match msg {
            SyncMessage::Items { items } => {
                let mut batch = Vec::with_capacity(items.len());
                let mut stamps = Vec::with_capacity(items.len());
                for mut item in items {
                    let Some(promised) = wanted.get(&item.item_id) else {
                        tracing::warn!("peer sent an item that was not requested; dropping it");
                        stats.skipped += 1;
                        continue;
                    };
                    if item.summary() != *promised {
                        tracing::warn!(
                            "peer sent an item that does not match its summary; dropping it"
                        );
                        stats.skipped += 1;
                        continue;
                    }
                    let payload = if item.binary_content.is_empty() {
                        item.content.as_bytes()
                    } else {
                        &item.binary_content
                    };
                    if !item.deleted
                        && crate::protocol::plaintext_content_hash(payload) != item.content_hash
                    {
                        // `content_hash` is comparator key 2. Recompute it here
                        // so a peer cannot choose the key or target a dedup
                        // collision. Tombstones retain the deleted item's hash
                        // while carrying no payload (manifest 05 rule T-4).
                        tracing::warn!(
                            "peer sent an item whose content does not match its hash; dropping it"
                        );
                        stats.skipped += 1;
                        continue;
                    }
                    if !applied.insert(item.item_id.clone()) {
                        // Applying is idempotent, but a replay inside one
                        // session should not pay for a second store write.
                        stats.skipped += 1;
                        continue;
                    }
                    if item.deleted {
                        // The tombstone must land without its payload or the
                        // delete is lost (manifest 05 rule T-4).
                        item.content.clear();
                    }
                    stamps.push(summary_key(&item.summary()));
                    batch.push(item);
                }
                if batch.is_empty() {
                    continue;
                }
                let outcomes = source.apply_batch(batch)?;
                for (stored, stamp) in outcomes.into_iter().zip(stamps) {
                    if stored {
                        stats.received += 1;
                        floor = Some(floor.map_or(stamp, |low: i64| low.min(stamp)));
                    } else {
                        stats.skipped += 1;
                    }
                }
            }
            SyncMessage::Done => {
                return Ok(Applied {
                    attempted: applied,
                    floor,
                })
            }
            other => {
                return Err(SyncError::Unexpected {
                    expected: "items",
                    got: other.kind(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SyncItem;
    use crate::sync::testutil::{item, ScriptChannel, TestSource};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn wire_frame_is_applied_in_one_source_batch() {
        struct CountingSource {
            inner: TestSource,
            apply_calls: AtomicUsize,
            batch_calls: AtomicUsize,
            last_batch_len: AtomicUsize,
        }

        impl SyncSource for CountingSource {
            fn device_id(&self) -> String {
                self.inner.device_id()
            }
            fn device_name(&self) -> String {
                self.inner.device_name()
            }
            fn summaries(&self, since_ms: i64) -> Result<Vec<ItemSummary>, SyncError> {
                self.inner.summaries(since_ms)
            }
            fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
                self.inner.fetch(ids)
            }
            fn apply(&self, item: SyncItem) -> Result<bool, SyncError> {
                self.apply_calls.fetch_add(1, Ordering::SeqCst);
                self.inner.apply(item)
            }
            fn apply_batch(&self, items: Vec<SyncItem>) -> Result<Vec<bool>, SyncError> {
                self.batch_calls.fetch_add(1, Ordering::SeqCst);
                self.last_batch_len.store(items.len(), Ordering::SeqCst);
                items
                    .into_iter()
                    .map(|item| self.inner.apply(item))
                    .collect()
            }
        }

        let source = CountingSource {
            inner: TestSource::new("dev-a", vec![]),
            apply_calls: AtomicUsize::new(0),
            batch_calls: AtomicUsize::new(0),
            last_batch_len: AtomicUsize::new(0),
        };
        let batch = vec![
            item("one", 100, "a", "dev-b"),
            item("two", 200, "b", "dev-b"),
            item("three", 300, "c", "dev-b"),
        ];
        let wanted = batch
            .iter()
            .map(|item| (item.item_id.clone(), item.summary()))
            .collect();
        let mut channel =
            ScriptChannel::new(vec![SyncMessage::Items { items: batch }, SyncMessage::Done]);
        let mut stats = SyncStats::default();

        let applied = receive_items(&mut channel, &source, &wanted, &mut stats)
            .await
            .unwrap();

        assert_eq!(stats.received, 3);
        assert_eq!(applied.attempted.len(), 3);
        assert_eq!(source.batch_calls.load(Ordering::SeqCst), 1);
        assert_eq!(source.last_batch_len.load(Ordering::SeqCst), 3);
        assert_eq!(source.apply_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unrequested_item_is_dropped_before_apply() {
        let source = TestSource::new("dev-a", vec![]);
        let mut channel = ScriptChannel::new(vec![
            SyncMessage::Items {
                items: vec![item("pushed", 100, "unasked for", "dev-b")],
            },
            SyncMessage::Done,
        ]);
        let mut stats = SyncStats::default();

        receive_items(&mut channel, &source, &HashMap::new(), &mut stats)
            .await
            .unwrap();

        assert!(source.get("pushed").is_none());
        assert_eq!((stats.received, stats.skipped), (0, 1));
    }
}
