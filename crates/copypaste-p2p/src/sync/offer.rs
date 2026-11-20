//! A manual send: one device offers chosen versions and the other takes the
//! ones it lacks. No cursor moves on either side, so a send cannot skip
//! history and runs while automatic sync is paused.
//!
//! ```text
//! sender                         receiver
//!   Offer              ->
//!                      <-        Request
//!   Items* Done        ->
//! ```

use super::application::receive_items;
use super::plan::plan_workspace;
use super::transfer::{receive_request, serve_items};
use super::{
    MemorySummaryWorkspace, SummaryWorkspace, SyncChannel, SyncError, SyncSource, SyncStats,
};
use crate::now_ms;
use crate::protocol::{ItemSummary, SyncMessage, VersionRange, MAX_OFFER_ITEMS};

/// The live versions among `item_ids` this device may send to
/// `peer_device_id`. Empty means the policy or history leaves nothing to send.
///
/// # Errors
///
/// [`SyncError`] when the source cannot be read.
pub fn offer_for<S: SyncSource>(
    source: &S,
    peer_device_id: &str,
    item_ids: &[String],
) -> Result<Vec<ItemSummary>, SyncError> {
    let ids: Vec<String> = item_ids.iter().take(MAX_OFFER_ITEMS).cloned().collect();
    let withheld = source.withheld_from(peer_device_id, &ids)?;
    let mut items = source.local_summaries(&ids)?;
    items.retain(|item| !item.deleted && !withheld.contains(&item.item_id));
    Ok(items)
}

/// Offer `items` and serve the ones the receiver requests.
///
/// # Errors
///
/// [`SyncError`] for a channel or source failure.
pub async fn send_offer<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    items: Vec<ItemSummary>,
) -> Result<SyncStats, SyncError> {
    chan.send(SyncMessage::Offer {
        protocol_version: VersionRange::SUPPORTED.min,
        max_protocol_version: VersionRange::SUPPORTED.wire_max(),
        device_id: source.device_id(),
        items: items.clone(),
    })
    .await?;
    let requested = receive_request(chan).await?;
    let advertised = items
        .into_iter()
        .map(|item| (item.item_id.clone(), item))
        .collect();
    let mut stats = SyncStats::default();
    serve_items(chan, source, &advertised, requested, &mut stats).await?;
    Ok(stats)
}

/// Answer an [`SyncMessage::Offer`] from `peer_device_id`: request what wins
/// locally and apply it under the same checks as a session.
///
/// Returns the stats and the oldest applied stamp, for relaying the versions on.
///
/// # Errors
///
/// [`SyncError`] for a channel or source failure, or an offer that claims
/// this device's own id.
pub async fn receive_offer<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    peer_device_id: &str,
    offered: Vec<ItemSummary>,
) -> Result<(SyncStats, Option<i64>), SyncError> {
    if peer_device_id == source.device_id() {
        return Err(SyncError::SelfSync);
    }
    let ids: Vec<String> = offered.iter().map(|item| item.item_id.clone()).collect();
    let mut workspace = MemorySummaryWorkspace::default();
    workspace.add_comparison(&source.local_summaries(&ids)?)?;
    workspace.add_remote(&offered)?;
    let mut stats = SyncStats::default();
    let wanted = plan_workspace(&mut workspace, now_ms(), &mut stats)?;
    chan.send(SyncMessage::Request {
        item_ids: wanted.keys().cloned().collect(),
    })
    .await?;
    let applied = receive_items(chan, source, peer_device_id, &wanted, &mut stats).await?;
    Ok((stats, applied.floor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testutil::{item, TestSource};

    #[tokio::test]
    async fn an_offer_moves_only_the_chosen_versions_the_receiver_lacks() {
        let sender = TestSource::new(
            "dev-a",
            vec![
                item("chosen", 100, "chosen", "dev-a"),
                item("held", 200, "held", "dev-a"),
                item("other", 300, "other", "dev-a"),
            ],
        );
        let receiver = TestSource::new("dev-b", vec![item("held", 200, "held", "dev-a")]);
        let items = offer_for(&sender, "dev-b", &["chosen".into(), "held".into()]).unwrap();
        assert_eq!(items.len(), 2);

        let (mut left, mut right) = crate::sync::testutil::duplex();
        let (sent, received) = tokio::join!(send_offer(&mut left, &sender, items), async {
            let SyncMessage::Offer {
                device_id, items, ..
            } = right.recv().await.unwrap()
            else {
                panic!("expected an offer");
            };
            receive_offer(&mut right, &receiver, &device_id, items).await
        });

        assert_eq!(sent.unwrap().sent, 1);
        assert_eq!(received.unwrap().0.received, 1);
        let ids: Vec<_> = receiver
            .snapshot()
            .into_iter()
            .map(|item| item.item_id)
            .collect();
        assert_eq!(ids, ["chosen", "held"]);
    }

    #[test]
    fn a_withheld_kind_is_not_offered() {
        let sender = TestSource::new("dev-a", vec![item("text", 100, "text", "dev-a")]);
        sender.block("dev-b", "text");
        assert!(offer_for(&sender, "dev-b", &["text".into()])
            .unwrap()
            .is_empty());
    }
}
