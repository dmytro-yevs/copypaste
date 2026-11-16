//! Serving item requests in bounded plaintext batches.

use std::collections::{HashMap, HashSet};

use super::{SyncChannel, SyncError, SyncSource, SyncStats};
use crate::protocol::{
    ItemSummary, SyncItem, SyncMessage, MAX_CONTENT_BYTES, MAX_ITEMS_PER_MESSAGE,
    MAX_ITEM_BYTES_PER_MESSAGE,
};

pub(super) async fn receive_request<C: SyncChannel>(
    chan: &mut C,
) -> Result<Vec<String>, SyncError> {
    let msg = chan.recv().await?;
    msg.validate()?;
    match msg {
        SyncMessage::Request { item_ids } => Ok(item_ids),
        other => Err(SyncError::Unexpected {
            expected: "request",
            got: other.kind(),
        }),
    }
}

/// Serves only ids present in this session's authenticated summary and closes
/// the stream with `Done`.
pub(super) async fn serve_items<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    advertised: &HashMap<String, ItemSummary>,
    requested: Vec<String>,
    stats: &mut SyncStats,
) -> Result<(), SyncError> {
    let mut seen: HashSet<String> = HashSet::new();
    let allowed: Vec<String> = requested
        .into_iter()
        .filter(|id| {
            if !advertised.contains_key(id) {
                tracing::warn!("peer requested an item that was never advertised; refusing it");
                return false;
            }
            seen.insert(id.clone())
        })
        .collect();

    let mut batch: Vec<SyncItem> = Vec::new();
    let mut batch_bytes = 0usize;
    let mut unavailable: HashSet<&str> = allowed.iter().map(String::as_str).collect();

    // Each fetch holds at most one message's worth of stored bytes, so the
    // pending message plus one fetch is all that is ever resident.
    let sizes = source.stored_sizes(&allowed)?;
    let mut rest = allowed.as_slice();
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(fetch_len(rest, &sizes));
        rest = tail;
        for mut item in source.fetch(chunk)? {
            if !chunk.contains(&item.item_id) || !unavailable.contains(item.item_id.as_str()) {
                // The source returned something it was not asked for.
                tracing::warn!("source returned an item outside the advertised set; dropping it");
                continue;
            }
            if item.deleted {
                item.content.clear();
                item.binary_content.clear();
            }
            let content_bytes = item.content.len().saturating_add(item.binary_content.len());
            if content_bytes > MAX_CONTENT_BYTES {
                tracing::warn!(
                    bytes = content_bytes,
                    max = MAX_CONTENT_BYTES,
                    "item is too large to send; skipping it"
                );
                stats.skipped += 1;
                stats.skipped_too_large += 1;
                unavailable.remove(item.item_id.as_str());
                continue;
            }
            if item.validate().is_err() {
                // Refuse only this version before the channel sees it. An
                // invalid member must never poison an otherwise valid frame.
                tracing::warn!("local item violates the sync wire contract; withholding it");
                stats.skipped_invalid += 1;
                continue;
            }
            unavailable.remove(item.item_id.as_str());
            let message_bytes =
                content_bytes.saturating_add(item.payload_metadata.as_ref().map_or(0, String::len));

            if !batch.is_empty()
                && (batch.len() == MAX_ITEMS_PER_MESSAGE
                    || batch_bytes.saturating_add(message_bytes) > MAX_ITEM_BYTES_PER_MESSAGE)
            {
                stats.sent += batch.len();
                chan.send(SyncMessage::Items {
                    items: std::mem::take(&mut batch),
                })
                .await?;
                batch_bytes = 0;
            }

            batch_bytes += message_bytes;
            batch.push(item);
        }
    }

    if !batch.is_empty() {
        stats.sent += batch.len();
        chan.send(SyncMessage::Items { items: batch }).await?;
    }

    stats.skipped += unavailable.len();
    stats.skipped_unavailable += unavailable.len();
    if !unavailable.is_empty() {
        tracing::warn!("some requested local items were unavailable; other items were served");
    }

    chan.send(SyncMessage::Done).await
}

/// How many leading `ids` one fetch takes: as many as one message can carry
/// by stored size, and a single id when its size is unknown or too large.
fn fetch_len(ids: &[String], sizes: &HashMap<String, u64>) -> usize {
    let mut bytes = 0u64;
    let mut len = 0;
    for id in ids.iter().take(MAX_ITEMS_PER_MESSAGE) {
        let Some(size) = sizes.get(id) else { break };
        bytes = bytes.saturating_add(*size);
        if bytes > MAX_ITEM_BYTES_PER_MESSAGE as u64 {
            break;
        }
        len += 1;
    }
    len.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testutil::{item, ScriptChannel, TestSource};

    #[tokio::test]
    async fn request_cannot_name_an_unadvertised_item() {
        let source = TestSource::new("dev-a", vec![item("secret", 200, "hunter2", "dev-a")]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(
            &mut channel,
            &source,
            &HashMap::new(),
            vec!["secret".into()],
            &mut stats,
        )
        .await
        .unwrap();

        assert_eq!(stats.sent, 0);
        assert_eq!(channel.sent, vec![SyncMessage::Done]);
    }

    #[tokio::test]
    async fn a_missing_source_version_is_counted_without_blocking_a_valid_neighbor() {
        let valid = item("valid", 200, "still available", "dev-a");
        let missing = item("missing", 100, "no longer readable", "dev-a");
        let advertised = HashMap::from([
            (valid.item_id.clone(), valid.summary()),
            (missing.item_id.clone(), missing.summary()),
        ]);
        let source = TestSource::new("dev-a", vec![valid]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();
        serve_items(
            &mut channel,
            &source,
            &advertised,
            vec!["missing".into(), "valid".into()],
            &mut stats,
        )
        .await
        .unwrap();
        assert_eq!(
            (stats.sent, stats.skipped, stats.skipped_unavailable),
            (1, 1, 1)
        );
        assert!(matches!(channel.sent.last(), Some(SyncMessage::Done)));
    }

    #[tokio::test]
    async fn source_cannot_smuggle_an_unadvertised_item() {
        let public = item("public", 100, "fine", "dev-a");
        let source = TestSource::new("dev-a", vec![public.clone()]);
        source.smuggle(item("secret", 200, "hunter2", "dev-a"));
        let advertised = HashMap::from([("public".into(), public.summary())]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(
            &mut channel,
            &source,
            &advertised,
            vec!["public".into()],
            &mut stats,
        )
        .await
        .unwrap();

        assert_eq!(stats.sent, 1);
        assert!(matches!(
            &channel.sent[..],
            [SyncMessage::Items { items }, SyncMessage::Done]
                if items.len() == 1 && items[0].item_id == "public"
        ));
    }

    #[tokio::test]
    async fn an_oversized_item_is_skipped_without_blocking_the_next_item() {
        let mut oversized = item("oversized", 100, "", "dev-a");
        oversized.content = "x".repeat(MAX_CONTENT_BYTES + 1);
        let valid = item("valid", 200, "fits", "dev-a");
        let advertised = HashMap::from([
            (oversized.item_id.clone(), oversized.summary()),
            (valid.item_id.clone(), valid.summary()),
        ]);
        let source = TestSource::new("dev-a", vec![oversized, valid]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(
            &mut channel,
            &source,
            &advertised,
            vec!["oversized".into(), "valid".into()],
            &mut stats,
        )
        .await
        .unwrap();

        assert_eq!(stats.sent, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.skipped_too_large, 1);
        assert!(
            source.get("oversized").is_some(),
            "the source row was deleted"
        );
        assert!(matches!(
            &channel.sent[..],
            [SyncMessage::Items { items }, SyncMessage::Done]
                if items.len() == 1 && items[0].item_id == "valid"
        ));
    }

    #[tokio::test]
    async fn metadata_does_not_reduce_the_raw_content_cap() {
        let mut at_content_cap = item("at-content-cap", 100, "", "dev-a");
        at_content_cap.content = "x".repeat(MAX_CONTENT_BYTES);
        at_content_cap.payload_metadata = Some(crate::sync::testutil::source_icon_metadata());
        let advertised =
            HashMap::from([(at_content_cap.item_id.clone(), at_content_cap.summary())]);
        let source = TestSource::new("dev-a", vec![at_content_cap]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(
            &mut channel,
            &source,
            &advertised,
            vec!["at-content-cap".into()],
            &mut stats,
        )
        .await
        .unwrap();

        assert_eq!((stats.sent, stats.skipped_too_large), (1, 0));
    }

    #[tokio::test]
    async fn every_content_shape_over_the_cap_is_withheld_without_losing_it() {
        // These are boundary test inputs; content-type ownership remains in IPC.
        for (name, content_type, binary, metadata) in [
            ("text", "text", false, None),
            ("rtf", "text/rtf", false, None),
            ("html", "text/html", false, None),
            ("image", "image/png", true, None),
            (
                "file",
                "file",
                true,
                Some(r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#),
            ),
            ("future", "application/x-future", true, None),
        ] {
            let mut oversized = item("oversized", 100, "", "dev-a");
            oversized.content_type = content_type.into();
            oversized.payload_metadata = metadata.map(str::to_owned);
            if binary {
                oversized.binary_content = vec![0, 0xff, 0x80];
                oversized.binary_content.resize(MAX_CONTENT_BYTES + 1, 0);
            } else {
                oversized.content = "x".repeat(MAX_CONTENT_BYTES + 1);
            }
            let expected = oversized.clone();
            let valid = item("valid", 200, "fits", "dev-a");
            let advertised = HashMap::from([
                (oversized.item_id.clone(), oversized.summary()),
                (valid.item_id.clone(), valid.summary()),
            ]);
            let source = TestSource::new("dev-a", vec![oversized, valid]);
            let mut channel = ScriptChannel::new(vec![]);
            let mut stats = SyncStats::default();

            serve_items(
                &mut channel,
                &source,
                &advertised,
                vec!["oversized".into(), "valid".into()],
                &mut stats,
            )
            .await
            .unwrap();

            assert_eq!(
                (stats.sent, stats.skipped, stats.skipped_too_large),
                (1, 1, 1),
                "{name}"
            );
            assert_eq!(
                source.get("oversized"),
                Some(expected),
                "{name} source payload changed"
            );
            assert!(
                matches!(
                    &channel.sent[..],
                    [SyncMessage::Items { items }, SyncMessage::Done]
                        if items.len() == 1 && items[0].item_id == "valid"
                ),
                "{name} blocked the following valid item"
            );
        }
    }

    #[tokio::test]
    async fn serving_large_items_never_fetches_more_than_one_message_of_content() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Measured {
            inner: TestSource,
            largest_fetch: AtomicUsize,
            fetches: AtomicUsize,
        }
        impl SyncSource for Measured {
            fn device_id(&self) -> String {
                self.inner.device_id()
            }
            fn device_name(&self) -> String {
                self.inner.device_name()
            }
            fn summaries(&self, since_ms: i64) -> Result<Vec<ItemSummary>, SyncError> {
                self.inner.summaries(since_ms)
            }
            fn local_summaries(&self, ids: &[String]) -> Result<Vec<ItemSummary>, SyncError> {
                self.inner.local_summaries(ids)
            }
            fn stored_sizes(&self, ids: &[String]) -> Result<HashMap<String, u64>, SyncError> {
                Ok(self
                    .inner
                    .fetch(ids)?
                    .into_iter()
                    .map(|item| (item.item_id, item.content.len() as u64))
                    .collect())
            }
            fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
                self.fetches.fetch_add(1, Ordering::SeqCst);
                let items = self.inner.fetch(ids)?;
                let bytes = items.iter().map(|item| item.content.len()).sum();
                self.largest_fetch.fetch_max(bytes, Ordering::SeqCst);
                Ok(items)
            }
            fn apply(&self, item: SyncItem) -> Result<bool, SyncError> {
                self.inner.apply(item)
            }
        }

        let big = "x".repeat(MAX_ITEM_BYTES_PER_MESSAGE / 2 + 1);
        let mut items: Vec<_> = (0..4)
            .map(|n| item(&format!("big{n}"), 100 + n, &big, "dev-a"))
            .collect();
        items.extend((0..9).map(|n| item(&format!("small{n}"), 200 + n, "short", "dev-a")));
        let advertised = items
            .iter()
            .map(|item| (item.item_id.clone(), item.summary()))
            .collect();
        let requested = items.iter().map(|item| item.item_id.clone()).collect();
        let source = Measured {
            inner: TestSource::new("dev-a", items),
            largest_fetch: AtomicUsize::new(0),
            fetches: AtomicUsize::new(0),
        };
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(&mut channel, &source, &advertised, requested, &mut stats)
            .await
            .unwrap();

        assert_eq!(stats.sent, 13);
        assert!(source.largest_fetch.load(Ordering::SeqCst) <= MAX_ITEM_BYTES_PER_MESSAGE);
        // big0, big1, big2, then big3 with seven small items, then two more.
        assert_eq!(source.fetches.load(Ordering::SeqCst), 5);
        let sent: Vec<_> = channel
            .sent
            .iter()
            .filter_map(|message| match message {
                SyncMessage::Items { items } => {
                    Some(items.iter().map(|item| item.item_id.as_str()).collect())
                }
                _ => None,
            })
            .collect::<Vec<Vec<_>>>();
        assert_eq!(sent[..3], [["big0"], ["big1"], ["big2"]]);
        assert_eq!(sent[3][0], "big3");
    }

    #[tokio::test]
    async fn a_tombstone_drops_stale_binary_payload_before_the_size_gate() {
        let mut tombstone = item("gone", 100, "stale text", "dev-a");
        tombstone.deleted = true;
        tombstone.binary_content = vec![0; MAX_CONTENT_BYTES + 1];
        let valid = item("valid", 200, "fits", "dev-a");
        let advertised = HashMap::from([
            (tombstone.item_id.clone(), tombstone.summary()),
            (valid.item_id.clone(), valid.summary()),
        ]);
        let source = TestSource::new("dev-a", vec![tombstone, valid]);
        let mut channel = ScriptChannel::new(vec![]);
        let mut stats = SyncStats::default();

        serve_items(
            &mut channel,
            &source,
            &advertised,
            vec!["gone".into(), "valid".into()],
            &mut stats,
        )
        .await
        .unwrap();

        assert_eq!((stats.sent, stats.skipped), (2, 0));
        assert!(matches!(
            &channel.sent[..],
            [SyncMessage::Items { items }, SyncMessage::Done]
                if items.len() == 2
                    && items[0].item_id == "gone"
                    && items[0].content.is_empty()
                    && items[0].binary_content.is_empty()
                    && items[1].item_id == "valid"
        ));
    }
}
