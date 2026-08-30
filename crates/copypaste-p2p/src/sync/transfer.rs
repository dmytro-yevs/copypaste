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

    // Bound each fetch so plaintext for an entire history is never resident.
    for chunk in allowed.chunks(MAX_ITEMS_PER_MESSAGE) {
        for mut item in source.fetch(chunk)? {
            if !advertised.contains_key(&item.item_id) {
                // The source returned something it was not asked for. This
                // layer owns the final sensitive-content egress guard.
                tracing::warn!("source returned an item outside the advertised set; dropping it");
                continue;
            }
            if item.deleted {
                item.content.clear();
                item.binary_content.clear();
            }
            let payload_bytes = item.content.len().saturating_add(item.binary_content.len());
            if payload_bytes > MAX_CONTENT_BYTES {
                tracing::warn!(
                    bytes = payload_bytes,
                    max = MAX_CONTENT_BYTES,
                    "item is too large to send; skipping it"
                );
                stats.skipped += 1;
                stats.skipped_too_large += 1;
                continue;
            }

            if !batch.is_empty()
                && (batch.len() == MAX_ITEMS_PER_MESSAGE
                    || batch_bytes.saturating_add(payload_bytes) > MAX_ITEM_BYTES_PER_MESSAGE)
            {
                stats.sent += batch.len();
                chan.send(SyncMessage::Items {
                    items: std::mem::take(&mut batch),
                })
                .await?;
                batch_bytes = 0;
            }

            batch_bytes += payload_bytes;
            batch.push(item);
        }
    }

    if !batch.is_empty() {
        stats.sent += batch.len();
        chan.send(SyncMessage::Items { items: batch }).await?;
    }

    chan.send(SyncMessage::Done).await
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
