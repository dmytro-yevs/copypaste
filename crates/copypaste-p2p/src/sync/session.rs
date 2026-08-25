//! The session: ten messages, no state machine.
//!
//! One round trip of summaries, one request in each direction, items streamed
//! back in bounded batches. v1 shipped a 5,000-line HELLO/HAVE/WANT
//! anti-entropy engine the daemon never instantiated; everything here is on the
//! path the daemon calls.
//!
//! ```text
//! initiator                      responder
//!   Hello              ->
//!                      <-        Hello
//!   Summary            ->
//!                      <-        Summary
//!   Request            ->
//!                      <-        Items* Done
//!                      <-        Request
//!   Items* Done        ->
//! ```
//!
//! Both halves are generic over [`SyncChannel`], so the tests below drive a real
//! session over an in-memory duplex and the daemon passes its Noise session.

use std::collections::{HashMap, HashSet};

use super::plan::{plan, Plan};
use super::{SyncChannel, SyncCursor, SyncError, SyncOutcome, SyncSource, SyncStats};
use crate::now_ms;
use crate::protocol::{
    ItemSummary, SyncItem, SyncMessage, MAX_CONTENT_BYTES, MAX_ITEMS_PER_MESSAGE,
    MAX_ITEM_BYTES_PER_MESSAGE, MAX_SUMMARIES_PER_MESSAGE, MAX_SUMMARY_PAGES_PER_SESSION,
    PROTOCOL_VERSION,
};

#[cfg(test)]
use crate::protocol::{content_hash, plaintext_content_hash};

/// Runs the initiating half of a session to completion.
pub async fn run_initiator<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    listen_addr: Option<&str>,
    cursor: SyncCursor,
) -> Result<SyncOutcome, SyncError> {
    let peer = {
        chan.send(local_hello(source, listen_addr, cursor.since_ms))
            .await?;
        recv_hello(chan, source).await?
    };

    let (advertised, remote) =
        exchange_summaries_initiator(chan, source, cursor.advertise_from(peer.4)).await?;

    let mut stats = SyncStats::default();
    let planned = plan(&advertised, &remote, now_ms(), &mut stats);

    chan.send(SyncMessage::Request {
        item_ids: planned.wanted.keys().cloned().collect(),
    })
    .await?;
    let applied = receive_items(chan, source, &planned.wanted, &mut stats).await?;

    let requested = recv_request(chan).await?;
    serve_items(chan, source, &advertised, requested, &mut stats).await?;

    Ok(SyncOutcome {
        stats,
        peer_device_id: peer.0,
        peer_device_name: peer.1,
        peer_profile: peer.2,
        peer_listen_addr: peer.3,
        cursor: SyncCursor {
            since_ms: watermark(cursor.since_ms, &remote, &planned, &applied.attempted),
            relay_floor_ms: None,
        },
        applied_floor: applied.floor,
    })
}

/// Runs the responding half of a session to completion.
///
/// The mirror image of [`run_initiator`]: every send here pairs with a receive
/// there, in the same order, so neither side can wait on a message the other has
/// not sent yet.
pub async fn run_responder<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    listen_addr: Option<&str>,
    cursor: SyncCursor,
) -> Result<SyncOutcome, SyncError> {
    let peer = {
        let peer = recv_hello(chan, source).await?;
        chan.send(local_hello(source, listen_addr, cursor.since_ms))
            .await?;
        peer
    };

    let (advertised, remote) =
        exchange_summaries_responder(chan, source, cursor.advertise_from(peer.4)).await?;

    let mut stats = SyncStats::default();

    let requested = recv_request(chan).await?;
    serve_items(chan, source, &advertised, requested, &mut stats).await?;

    let planned = plan(&advertised, &remote, now_ms(), &mut stats);
    chan.send(SyncMessage::Request {
        item_ids: planned.wanted.keys().cloned().collect(),
    })
    .await?;
    let applied = receive_items(chan, source, &planned.wanted, &mut stats).await?;

    Ok(SyncOutcome {
        stats,
        peer_device_id: peer.0,
        peer_device_name: peer.1,
        peer_profile: peer.2,
        peer_listen_addr: peer.3,
        cursor: SyncCursor {
            since_ms: watermark(cursor.since_ms, &remote, &planned, &applied.attempted),
            relay_floor_ms: None,
        },
        applied_floor: applied.floor,
    })
}

fn local_hello<S: SyncSource>(source: &S, listen_addr: Option<&str>, since_ms: i64) -> SyncMessage {
    SyncMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: source.device_id(),
        device_name: source.device_name(),
        profile: Some(source.device_profile()),
        listen_addr: listen_addr.map(str::to_owned),
        since_ms,
    }
}

/// Reads the peer's hello and fails closed on anything unexpected. The version
/// check lives in [`SyncMessage::validate`] so it covers every ingress path;
/// repeating it here also covers a channel that skipped decode-time
/// validation.
async fn recv_hello<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
) -> Result<
    (
        String,
        String,
        Option<crate::DeviceProfile>,
        Option<std::net::SocketAddr>,
        i64,
    ),
    SyncError,
> {
    let msg = chan.recv().await?;
    msg.validate()?;
    match msg {
        SyncMessage::Hello {
            device_id,
            device_name,
            profile,
            listen_addr,
            since_ms,
            ..
        } => {
            if device_id == source.device_id() {
                return Err(SyncError::SelfSync);
            }
            Ok((
                device_id,
                device_name,
                profile,
                // Validated, not merely parsed: this becomes the address every
                // later round dials, and it replaces one that worked.
                listen_addr
                    .and_then(|addr| addr.parse().ok())
                    .filter(crate::netif::is_dialable),
                since_ms,
            ))
        }
        other => Err(SyncError::Unexpected {
            expected: "hello",
            got: other.kind(),
        }),
    }
}

/// Reads exactly one authenticated summary page.
async fn recv_summary_page<C: SyncChannel>(
    chan: &mut C,
) -> Result<(Vec<ItemSummary>, bool), SyncError> {
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

struct Applied {
    attempted: HashSet<String>,
    floor: Option<i64>,
}

pub(super) fn summary_key(summary: &ItemSummary) -> i64 {
    summary.created_at.max(summary.pin_updated_at)
}

fn watermark(
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

fn next_summary_page<S: SyncSource>(
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

/// Exchanges every summary page in lock-step. Each side reads one SQLite (or
/// in-memory) page at a time so a catch-up round never holds the whole history
/// in a single `Vec` before the first page is sent. A `more` marker makes the
/// page boundary explicit, while the lock-step exchange keeps either side from
/// filling a bounded channel before it reads the other side's history.
async fn exchange_summaries_initiator<C: SyncChannel, S: SyncSource>(
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
        let (items, more) = next_summary_page(source, cursor_ms, after_id.as_deref())?;
        if let Some(last) = items.last() {
            cursor_ms = summary_key(last);
            after_id = Some(last.item_id.clone());
        }
        remember_page(&mut advertised, &items);
        chan.send(SyncMessage::Summary { items, more }).await?;
        let (peer_items, peer_more) = recv_summary_page(chan).await?;
        remote.extend(peer_items);
        if !more && !peer_more {
            return Ok((advertised, remote));
        }
        page += 1;
    }
}

async fn exchange_summaries_responder<C: SyncChannel, S: SyncSource>(
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
        let (peer_items, peer_more) = recv_summary_page(chan).await?;
        remote.extend(peer_items);
        let (items, more) = next_summary_page(source, cursor_ms, after_id.as_deref())?;
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

async fn recv_request<C: SyncChannel>(chan: &mut C) -> Result<Vec<String>, SyncError> {
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

/// Reads `Items` messages until `Done`, applying what was asked for. Everything
/// unasked-for is dropped: the merge decision was made against the summary, so
/// an item that does not match what it was promised as is not the item we
/// agreed to take.
async fn receive_items<C: SyncChannel, S: SyncSource>(
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
                        // The peer's `content_hash` is comparator key 2. Taking
                        // its word for it lets a hostile peer pick that key
                        // freely — and collide with a targeted item's hash so
                        // the version it names is refused. Recomputed here
                        // rather than in `apply`, so every `SyncSource` gets it.
                        // A tombstone is exempt: it carries the hash of the item
                        // it deletes and no content to hash (rule T-4).
                        tracing::warn!(
                            "peer sent an item whose content does not match its hash; dropping it"
                        );
                        stats.skipped += 1;
                        continue;
                    }
                    if !applied.insert(item.item_id.clone()) {
                        // Within-session replay. Harmless — apply is idempotent —
                        // but there is no reason to pay for it twice.
                        stats.skipped += 1;
                        continue;
                    }
                    if item.deleted {
                        // A tombstone carries no content (manifest 05 rule T-4).
                        // Clearing rather than refusing: the tombstone itself
                        // must still land, or the delete is lost.
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

/// Answers a request, then closes the stream with `Done`. Only ids in our own
/// advertised summary are served, which makes "a sensitive item never leaves the
/// device" a property of the protocol and not only of the store: absent from
/// `summaries()`, absent from `advertised`, unnameable by any request.
async fn serve_items<C: SyncChannel, S: SyncSource>(
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

    // Fetched a batch at a time so the plaintext of a thousand items is never
    // resident at once.
    for chunk in allowed.chunks(MAX_ITEMS_PER_MESSAGE) {
        for mut item in source.fetch(chunk)? {
            if !advertised.contains_key(&item.item_id) {
                // The source handed back something it was not asked for. Refuse
                // it here too: this is the layer that promised not to leak.
                tracing::warn!("source returned an item outside the advertised set; dropping it");
                continue;
            }
            if item.deleted {
                item.content.clear();
            }
            let payload_bytes = item.content.len().saturating_add(item.binary_content.len());
            if payload_bytes > MAX_CONTENT_BYTES {
                // Skip the one item rather than failing the session: the rest of
                // the history is still worth syncing.
                tracing::warn!(
                    bytes = payload_bytes,
                    max = MAX_CONTENT_BYTES,
                    "item is too large to send; skipping it"
                );
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
    use crate::protocol::{
        ProtocolError, MAX_REQUEST_IDS_PER_MESSAGE, MAX_SUMMARY_PAGES_PER_SESSION,
    };
    use crate::sync::testutil::{
        item, session, session_with, summary, tombstone, try_session,
        try_session_with_listen_addresses, ScriptChannel, TestSource,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CONVERGED: SyncCursor = SyncCursor {
        since_ms: 5_000,
        relay_floor_ms: None,
    };

    #[tokio::test]
    async fn a_delete_survives_a_cursor_advance() {
        let history = vec![
            item("doomed", 800, "secret note", "dev-a"),
            item("recent", 5_000, "newer", "dev-b"),
        ];
        let a = TestSource::new("dev-a", history.clone());
        let b = TestSource::new("dev-b", history);

        a.apply(tombstone(
            "doomed",
            6_000,
            &content_hash("secret note"),
            "dev-a",
        ))
        .unwrap();

        let (oa, _) = session_with(&a, &b, CONVERGED, CONVERGED).await;

        assert!(
            b.get("doomed").expect("still known").deleted,
            "a delete stamped above the cursor must still cross it"
        );
        assert!(oa.cursor.since_ms >= CONVERGED.since_ms);
    }

    #[tokio::test]
    async fn a_relayed_delete_is_advertised_again_below_the_cursor() {
        let history = vec![
            item("doomed", 800, "secret note", "dev-a"),
            item("recent", 5_000, "newer", "dev-b"),
        ];
        let b = TestSource::new("dev-b", history.clone());
        let c = TestSource::new("dev-c", history);

        b.apply(tombstone(
            "doomed",
            900,
            &content_hash("secret note"),
            "dev-a",
        ))
        .unwrap();

        session_with(&b, &c, CONVERGED, CONVERGED).await;
        assert!(
            !c.get("doomed").expect("still known").deleted,
            "a cursor at 5000 cannot see a tombstone stamped 900 on its own"
        );

        let relaying = SyncCursor {
            since_ms: 5_000,
            relay_floor_ms: Some(900),
        };
        session_with(&b, &c, relaying, CONVERGED).await;
        assert!(
            c.get("doomed").expect("still known").deleted,
            "the relay floor is what stops a delete falling below the cursor (T-3)"
        );
    }

    #[tokio::test]
    async fn pinning_an_old_item_still_crosses_the_cursor() {
        let old = item("ancient", 800, "kept", "dev-a");
        let a = TestSource::new("dev-a", vec![old.clone()]);
        let b = TestSource::new("dev-b", vec![old.clone()]);

        a.apply(SyncItem {
            pinned: true,
            pin_order: Some(1.0),
            pin_updated_at: 6_000,
            ..old
        })
        .unwrap();

        session_with(&a, &b, CONVERGED, CONVERGED).await;

        let pinned = b.get("ancient").expect("still known");
        assert!(
            pinned.pinned,
            "pin state moves on pin_updated_at, not created_at; a created_at cursor loses it"
        );
        assert_eq!(pinned.pin_updated_at, 6_000);
        assert_eq!(
            pinned.created_at, 800,
            "the content version must not be restamped"
        );
    }

    #[tokio::test]
    async fn a_converged_round_advertises_only_the_boundary_entry() {
        let history: Vec<_> = (0..200)
            .map(|n| item(&format!("i{n:04}"), 1_000 + n as i64, "same", "dev-a"))
            .collect();
        let a = TestSource::new("dev-a", history.clone());
        let b = TestSource::new("dev-b", history);

        let (oa, ob) = session(&a, &b).await;

        assert_eq!(oa.cursor.since_ms, 1_199);
        assert_eq!(a.summaries(oa.cursor.since_ms).unwrap().len(), 1);
        assert_eq!(b.summaries(ob.cursor.since_ms).unwrap().len(), 1);
    }

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
    fn the_watermark_stops_below_anything_the_session_did_not_finish() {
        let remote = vec![
            summary("a", 100, "h", false),
            summary("b", 200, "h", false),
            summary("c", 300, "h", false),
        ];
        let plan = planned(&[&remote[1]], &[]);

        assert_eq!(
            watermark(0, &remote, &plan, &HashSet::new()),
            200,
            "an item that was wanted but never arrived must be offered again"
        );
        let attempted: HashSet<String> = ["b".to_string()].into_iter().collect();
        assert_eq!(watermark(0, &remote, &plan, &attempted), 300);
    }

    #[test]
    fn the_watermark_never_moves_on_its_own() {
        let plan = planned(&[], &[]);
        assert_eq!(watermark(5_000, &[], &plan, &HashSet::new()), 5_000);
    }

    #[test]
    fn a_deferred_want_pulls_the_watermark_back_below_it() {
        let remote = vec![
            summary("old", 100, "h", false),
            summary("new", 900, "h", false),
        ];
        let plan = planned(&[], &["old"]);
        assert_eq!(watermark(5_000, &remote, &plan, &HashSet::new()), 100);
    }

    #[test]
    fn a_refused_future_stamp_cannot_drag_the_watermark_forward() {
        let remote = vec![
            summary("sane", 900, "h", false),
            summary("forged", i64::MAX, "h", false),
        ];
        let plan = planned(&[], &["forged"]);
        assert_eq!(watermark(0, &remote, &plan, &HashSet::new()), 900);
    }

    #[test]
    fn the_pin_stamp_is_part_of_the_cursor_key() {
        let mut pinned = summary("ancient", 800, "h", false);
        pinned.pin_updated_at = 6_000;
        assert_eq!(summary_key(&pinned), 6_000);
        assert_eq!(summary_key(&summary("plain", 800, "h", false)), 800);
    }

    #[tokio::test]
    async fn two_divergent_devices_converge() {
        let a = TestSource::new(
            "dev-a",
            vec![
                item("shared", 100, "same", "dev-a"),
                item("only-a", 200, "from a", "dev-a"),
            ],
        );
        let b = TestSource::new(
            "dev-b",
            vec![
                item("shared", 100, "same", "dev-a"),
                item("only-b", 300, "from b", "dev-b"),
            ],
        );

        let (oa, ob) = session(&a, &b).await;

        assert_eq!(a.snapshot(), b.snapshot(), "devices did not converge");
        assert_eq!(a.snapshot().len(), 3);
        assert_eq!(oa.peer_device_id, "dev-b");
        assert_eq!(ob.peer_device_id, "dev-a");
        assert_eq!(oa.peer_device_name, "dev-b name");
        assert_eq!(
            oa.peer_profile
                .as_ref()
                .and_then(|profile| profile.protocol_version),
            Some(PROTOCOL_VERSION)
        );
        assert_eq!(oa.stats.received, 1);
        assert_eq!(oa.stats.sent, 1);
        assert_eq!(ob.stats.received, 1);
        assert_eq!(ob.stats.sent, 1);
    }

    #[tokio::test]
    async fn a_local_floor_recovers_a_later_peer_item_from_a_slower_clock() {
        let initiator = TestSource::new(
            "joiner",
            vec![item("joiner-pre", 400, "joiner before pairing", "joiner")],
        );
        let responder = TestSource::new(
            "creator",
            vec![item(
                "creator-pre",
                100,
                "creator before pairing",
                "creator",
            )],
        );
        let (first_i, first_r) = session(&initiator, &responder).await;
        assert_eq!(first_i.cursor.since_ms, 100);
        assert_eq!(first_r.cursor.since_ms, 400);

        let (second_i, second_r) =
            session_with(&initiator, &responder, first_i.cursor, first_r.cursor).await;
        responder
            .apply(item(
                "creator-live",
                300,
                "creator after pairing",
                "creator",
            ))
            .unwrap();

        let responder_cursor = SyncCursor {
            relay_floor_ms: Some(300),
            ..second_r.cursor
        };

        let (third_i, _) =
            session_with(&initiator, &responder, second_i.cursor, responder_cursor).await;

        assert!(initiator.get("creator-live").is_some());
        assert_eq!(third_i.stats.received, 1);
    }

    #[tokio::test]
    async fn a_history_larger_than_one_summary_page_eventually_converges() {
        let history: Vec<_> = (0..=MAX_SUMMARIES_PER_MESSAGE)
            .map(|n| item(&format!("history-{n:05}"), n as i64, "archived", "dev-a"))
            .collect();
        let a = TestSource::new("dev-a", history);
        let b = TestSource::new("dev-b", vec![]);

        // Requests are deliberately bounded at 1,000 items, so drain enough
        // complete sessions to cover every wanted page. The oldest item is the
        // regression: it used to be absent from every summary forever.
        for _ in 0..=MAX_SUMMARIES_PER_MESSAGE.div_ceil(MAX_REQUEST_IDS_PER_MESSAGE) {
            session(&a, &b).await;
        }

        assert_eq!(a.snapshot(), b.snapshot());
        assert!(
            b.get("history-00000").is_some(),
            "old history was never advertised"
        );
        assert_eq!(b.snapshot().len(), MAX_SUMMARIES_PER_MESSAGE + 1);
    }

    #[tokio::test]
    async fn an_origin_only_conflict_converges_during_planning() {
        let a = TestSource::new("dev-a", vec![item("x", 100, "same", "dev-a")]);
        let b = TestSource::new("dev-b", vec![item("x", 100, "same", "dev-z")]);

        session(&a, &b).await;

        assert_eq!(a.snapshot(), b.snapshot());
        assert_eq!(a.get("x").unwrap().origin_device_id, "dev-z");
    }

    #[tokio::test]
    async fn pin_order_conflicts_converge_as_authenticated_version_state() {
        let mut a_item = item("x", 100, "same", "dev-a");
        a_item.pinned = true;
        a_item.pin_order = Some(2.0);
        let mut b_item = item("x", 100, "same", "dev-z");
        b_item.pinned = true;
        b_item.pin_order = Some(1.0);
        let a = TestSource::new("dev-a", vec![a_item]);
        let b = TestSource::new("dev-b", vec![b_item]);

        session(&a, &b).await;

        assert_eq!(a.snapshot(), b.snapshot());
        let winner = a.get("x").unwrap();
        assert!(winner.pinned);
        assert_eq!(winner.pin_order, Some(1.0));
        assert_eq!(winner.origin_device_id, "dev-z");
    }

    #[tokio::test]
    async fn a_newer_p2p_unpin_clears_the_remote_order() {
        let mut pinned = item("x", 100, "same", "dev-a");
        pinned.pinned = true;
        pinned.pin_order = Some(4.0);
        let unpinned = item("x", 200, "same", "dev-b");
        let a = TestSource::new("dev-a", vec![pinned]);
        let b = TestSource::new("dev-b", vec![unpinned]);

        session(&a, &b).await;

        assert_eq!(a.snapshot(), b.snapshot());
        let winner = a.get("x").unwrap();
        assert!(!winner.pinned);
        assert_eq!(winner.pin_order, None);
    }

    #[tokio::test]
    async fn convergence_does_not_depend_on_who_initiates() {
        let mk = || {
            (
                TestSource::new("dev-a", vec![item("x", 100, "a-version", "dev-a")]),
                TestSource::new("dev-b", vec![item("x", 200, "b-version", "dev-b")]),
            )
        };

        let (a1, b1) = mk();
        session(&a1, &b1).await;

        let (a2, b2) = mk();
        let (rb, ra) = try_session(&b2, &a2).await;
        ra.unwrap();
        rb.unwrap();

        assert_eq!(a1.snapshot(), b1.snapshot());
        assert_eq!(a2.snapshot(), b2.snapshot());
        assert_eq!(
            a1.snapshot(),
            a2.snapshot(),
            "the winner changed with roles"
        );
        assert_eq!(a1.get("x").unwrap().content, "b-version");
    }

    #[tokio::test]
    async fn replaying_a_whole_session_changes_nothing() {
        let a = TestSource::new("dev-a", vec![item("only-a", 100, "a", "dev-a")]);
        let b = TestSource::new("dev-b", vec![item("only-b", 200, "b", "dev-b")]);

        session(&a, &b).await;
        let after_first = a.snapshot();

        let (oa, ob) = session(&a, &b).await;

        assert_eq!(a.snapshot(), after_first);
        assert_eq!(b.snapshot(), after_first);
        for o in [&oa, &ob] {
            assert_eq!(o.stats.sent, 0, "an item was transferred twice");
            assert_eq!(o.stats.received, 0, "an item was applied twice");
            // Every one of the peer's summaries tied and was declined. That is
            // the whole idempotency mechanism: an already-applied version
            // compares equal and loses.
            assert_eq!(o.stats.skipped, 2);
        }
    }

    #[tokio::test]
    async fn a_third_session_after_three_way_sync_is_a_fixed_point() {
        let a = TestSource::new("dev-a", vec![item("a1", 100, "a", "dev-a")]);
        let b = TestSource::new("dev-b", vec![item("b1", 200, "b", "dev-b")]);
        let c = TestSource::new("dev-c", vec![item("c1", 300, "c", "dev-c")]);

        session(&a, &b).await;
        session(&b, &c).await;
        session(&c, &a).await;
        session(&a, &b).await;

        assert_eq!(a.snapshot(), b.snapshot());
        assert_eq!(b.snapshot(), c.snapshot());
        assert_eq!(a.snapshot().len(), 3);

        let (oa, ob) = session(&a, &b).await;
        assert_eq!((oa.stats.sent, oa.stats.received), (0, 0));
        assert_eq!((ob.stats.sent, ob.stats.received), (0, 0));
    }

    #[tokio::test]
    async fn a_delete_propagates_and_is_not_resurrected() {
        let a = TestSource::new("dev-a", vec![tombstone("x", 200, "h", "dev-a")]);
        let b = TestSource::new("dev-b", vec![item("x", 100, "still here", "dev-a")]);

        let (_, ob) = session(&a, &b).await;

        let landed = b.get("x").unwrap();
        assert!(landed.deleted, "tombstone did not land");
        assert!(landed.content.is_empty(), "tombstone carried content");
        assert_eq!(ob.stats.received, 1);

        // And the stale live copy cannot come back on a later session.
        session(&a, &b).await;
        assert!(b.get("x").unwrap().deleted);
        assert!(a.get("x").unwrap().deleted);
    }

    #[tokio::test]
    async fn a_delete_for_an_item_the_peer_never_saw_is_stored() {
        // T-3. Without this, the create below resurrects the item.
        let a = TestSource::new("dev-a", vec![tombstone("x", 200, "h", "dev-a")]);
        let b = TestSource::new("dev-b", vec![]);

        session(&a, &b).await;
        assert!(b.get("x").expect("tombstone was not stored").deleted);

        // A create for that id, stamped earlier, now has something to lose to.
        let c = TestSource::new("dev-c", vec![item("x", 100, "resurrected?", "dev-c")]);
        session(&b, &c).await;
        assert!(b.get("x").unwrap().deleted, "item was resurrected");
        assert!(c.get("x").unwrap().deleted, "delete did not propagate on");
    }

    #[tokio::test]
    async fn a_newer_edit_beats_an_older_delete() {
        // The converse of delete-wins: a tombstone is a version, not a veto.
        let a = TestSource::new("dev-a", vec![tombstone("x", 100, "h", "dev-a")]);
        let b = TestSource::new("dev-b", vec![item("x", 200, "edited later", "dev-b")]);

        session(&a, &b).await;
        assert!(!a.get("x").unwrap().deleted);
        assert_eq!(a.get("x").unwrap().content, "edited later");
    }

    #[tokio::test]
    async fn a_sensitive_item_is_never_advertised_or_served() {
        let a = TestSource::new(
            "dev-a",
            vec![
                item("public", 100, "fine", "dev-a"),
                item("secret", 200, "hunter2", "dev-a"),
            ],
        );
        a.mark_sensitive("secret");
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;

        assert!(b.get("public").is_some());
        assert!(b.get("secret").is_none(), "a sensitive item was synced");
        assert_eq!(oa.stats.sent, 1);
        assert_eq!(ob.stats.received, 1);
    }

    #[tokio::test]
    async fn a_sensitive_tombstone_syncs_empty_while_a_live_secret_is_excluded() {
        let secret = "previous secret";
        let a = TestSource::new(
            "dev-a",
            vec![
                item("live-secret", 300, "hunter2", "dev-a"),
                tombstone("deleted-secret", 200, &content_hash(secret), "dev-a"),
            ],
        );
        a.mark_sensitive("live-secret");
        a.mark_sensitive("deleted-secret");
        let b = TestSource::new("dev-b", vec![item("deleted-secret", 100, secret, "dev-a")]);

        let (oa, ob) = session(&a, &b).await;

        assert!(b.get("live-secret").is_none(), "a live secret was synced");
        let tombstone = b.get("deleted-secret").expect("tombstone was not synced");
        assert!(tombstone.deleted);
        assert!(tombstone.content.is_empty(), "tombstone carried content");
        assert_eq!((oa.stats.sent, ob.stats.received), (1, 1));
    }

    #[tokio::test]
    async fn a_completed_session_returns_the_peer_listen_address() {
        let a = TestSource::new("dev-a", vec![]);
        let b = TestSource::new("dev-b", vec![]);
        let (from_a, from_b) = try_session_with_listen_addresses(
            &a,
            &b,
            Some("192.0.2.1:47654"),
            Some("192.0.2.2:47654"),
        )
        .await;

        assert_eq!(
            from_a.unwrap().peer_listen_addr,
            Some("192.0.2.2:47654".parse().unwrap())
        );
        assert_eq!(
            from_b.unwrap().peer_listen_addr,
            Some("192.0.2.1:47654".parse().unwrap())
        );
    }

    #[tokio::test]
    async fn an_item_that_was_never_advertised_cannot_be_requested_out() {
        // The protocol-level layer of the same rule: even a peer that already
        // knows the id — because it went sensitive after an earlier session —
        // gets nothing back.
        let a = TestSource::new("dev-a", vec![item("secret", 200, "hunter2", "dev-a")]);
        a.mark_sensitive("secret");

        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![],
                more: false,
            },
            SyncMessage::Request {
                item_ids: vec!["secret".into()],
            },
            SyncMessage::Done,
        ]);

        let outcome = run_responder(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert_eq!(outcome.stats.sent, 0);
        for msg in &chan.sent {
            if let SyncMessage::Items { items } = msg {
                assert!(items.is_empty(), "a sensitive item was put on the wire");
            }
        }
    }

    #[tokio::test]
    async fn an_item_the_source_smuggles_in_is_not_sent() {
        let a = TestSource::new("dev-a", vec![item("public", 100, "fine", "dev-a")]);
        a.smuggle(item("secret", 200, "hunter2", "dev-a"));
        a.mark_sensitive("secret");
        let b = TestSource::new("dev-b", vec![]);

        session(&a, &b).await;
        assert!(
            b.get("secret").is_none(),
            "a smuggled item reached the peer"
        );
        assert!(b.get("public").is_some());
    }

    #[tokio::test]
    async fn an_unrequested_item_is_not_applied() {
        let a = TestSource::new("dev-a", vec![]);
        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![],
                more: false,
            },
            SyncMessage::Items {
                items: vec![item("pushed", 100, "unasked for", "dev-b")],
            },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert!(a.get("pushed").is_none(), "an unrequested item was applied");
        assert_eq!(outcome.stats.received, 0);
        assert_eq!(outcome.stats.skipped, 1);
    }

    #[tokio::test]
    async fn an_item_that_does_not_match_its_summary_is_dropped() {
        // The merge decision was made against the summary; the payload has to be
        // the thing that was promised.
        let a = TestSource::new("dev-a", vec![]);
        let mut lying = item("x", 100, "as promised", "dev-b");
        let promised = lying.summary();
        lying.created_at = 999_999;

        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![promised],
                more: false,
            },
            SyncMessage::Items { items: vec![lying] },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert!(a.get("x").is_none());
        assert_eq!(outcome.stats.skipped, 1);
    }

    /// The peer's `content_hash` is comparator key 2, so a peer that is believed
    /// picks that key freely — it can hand over content while claiming the hash
    /// of a *different* item and collide with it in the dedup index, making the
    /// targeted version be refused. The receiver recomputes instead.
    #[tokio::test]
    async fn an_item_whose_content_does_not_hash_to_its_claimed_hash_is_dropped() {
        let a = TestSource::new("dev-a", vec![]);
        let mut forged = item("x", 100, "attacker payload", "dev-b");
        forged.content_hash = content_hash("some other item's content");
        let promised = forged.summary();

        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![promised],
                more: false,
            },
            SyncMessage::Items {
                items: vec![forged],
            },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert!(a.get("x").is_none(), "a forged content_hash was applied");
        assert_eq!(outcome.stats.received, 0);
        assert_eq!(outcome.stats.skipped, 1);
    }

    /// The exemption that must survive the check: a tombstone carries the hash of
    /// the item it deletes and no content to hash (rule T-4), so recomputing
    /// would drop every delete.
    #[tokio::test]
    async fn a_tombstone_is_exempt_from_the_hash_check() {
        let a = TestSource::new("dev-a", vec![item("x", 100, "live version", "dev-a")]);
        let grave = tombstone("x", 200, &content_hash("live version"), "dev-b");

        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![grave.summary()],
                more: false,
            },
            SyncMessage::Items { items: vec![grave] },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert_eq!(outcome.stats.received, 1, "the delete was dropped");
        assert!(a.get("x").is_some_and(|i| i.deleted));
    }

    #[tokio::test]
    async fn a_peer_that_never_says_done_is_cut_off() {
        let a = TestSource::new("dev-a", vec![]);
        let mut script = vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: vec![],
                more: false,
            },
        ];
        // Nothing was wanted, so any stream of items is already an overrun.
        script.extend(std::iter::repeat_with(|| SyncMessage::Items { items: vec![] }).take(64));

        let mut chan = ScriptChannel::new(script);
        assert_eq!(
            run_initiator(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::PeerOverran)
        );
    }

    #[tokio::test]
    async fn a_peer_cannot_keep_the_summary_stream_open_forever() {
        let a = TestSource::new("dev-a", vec![]);
        let mut script = vec![SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "dev-b".into(),
            device_name: "B".into(),
            profile: None,
            listen_addr: None,
            since_ms: 0,
        }];
        script.extend(
            (0..=MAX_SUMMARY_PAGES_PER_SESSION).map(|_| SyncMessage::Summary {
                items: vec![],
                more: true,
            }),
        );
        let mut chan = ScriptChannel::new(script);
        assert_eq!(
            run_initiator(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::TooManySummaryPages)
        );
    }

    #[tokio::test]
    async fn a_version_mismatch_fails_closed_on_both_halves() {
        let a = TestSource::new("dev-a", vec![]);
        let stale = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION + 1,
            device_id: "dev-b".into(),
            device_name: "B".into(),
            profile: None,
            listen_addr: None,
            since_ms: 0,
        };

        let mut chan = ScriptChannel::new(vec![stale.clone()]);
        assert_eq!(
            run_initiator(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::Protocol(ProtocolError::VersionMismatch {
                ours: PROTOCOL_VERSION,
                theirs: PROTOCOL_VERSION + 1,
            }))
        );

        let mut chan = ScriptChannel::new(vec![stale]);
        assert!(matches!(
            run_responder(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::Protocol(ProtocolError::VersionMismatch { .. }))
        ));
    }

    #[tokio::test]
    async fn a_peer_claiming_our_own_device_id_is_refused() {
        let a = TestSource::new("dev-a", vec![]);
        let mut chan = ScriptChannel::new(vec![SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "dev-a".into(),
            device_name: "me".into(),
            profile: None,
            listen_addr: None,
            since_ms: 0,
        }]);
        assert_eq!(
            run_initiator(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::SelfSync)
        );
    }

    #[tokio::test]
    async fn a_message_out_of_turn_is_an_error() {
        let a = TestSource::new("dev-a", vec![]);
        let mut chan = ScriptChannel::new(vec![SyncMessage::Done]);
        assert_eq!(
            run_initiator(&mut chan, &a, None, SyncCursor::default()).await,
            Err(SyncError::Unexpected {
                expected: "hello",
                got: "done"
            })
        );
    }

    #[tokio::test]
    async fn many_items_are_split_into_bounded_batches() {
        let items: Vec<_> = (0..MAX_ITEMS_PER_MESSAGE * 3 + 1)
            .map(|n| {
                item(
                    &format!("i{n}"),
                    100 + n as i64,
                    &format!("body {n}"),
                    "dev-a",
                )
            })
            .collect();
        let count = items.len();
        let a = TestSource::new("dev-a", items);
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;

        assert_eq!(oa.stats.sent, count);
        assert_eq!(ob.stats.received, count);
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[tokio::test]
    async fn large_text_items_split_without_overrunning_the_receiver() {
        let big = "x".repeat(MAX_ITEM_BYTES_PER_MESSAGE / 2 + 1);
        let a = TestSource::new(
            "dev-a",
            vec![
                item("big1", 100, &big, "dev-a"),
                item("big2", 200, &big, "dev-a"),
                item("big3", 300, &big, "dev-a"),
            ],
        );
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;
        assert_eq!(oa.stats.sent, 3);
        assert_eq!(ob.stats.received, 3);
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[tokio::test]
    async fn large_binary_items_split_without_overrunning_the_receiver() {
        let bytes = vec![7; MAX_ITEM_BYTES_PER_MESSAGE / 2 + 1];
        let items: Vec<_> = ["binary-a", "binary-b", "binary-c"]
            .into_iter()
            .map(|id| SyncItem {
                item_id: id.into(),
                content: String::new(),
                binary_content: bytes.clone(),
                payload_metadata: None,
                source_app_bundle_id: None,
                source_app_name: None,
                content_type: "image/png".into(),
                created_at: 1,
                deleted: false,
                content_hash: plaintext_content_hash(&bytes),
                origin_device_id: "dev-a".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            })
            .collect();
        let a = TestSource::new("dev-a", items);
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;
        assert_eq!(oa.stats.sent, 3);
        assert_eq!(ob.stats.received, 3);
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[tokio::test]
    async fn an_oversized_item_is_skipped_without_failing_the_session() {
        let a = TestSource::new(
            "dev-a",
            vec![
                item("huge", 100, &"x".repeat(MAX_CONTENT_BYTES + 1), "dev-a"),
                item("fine", 200, "small", "dev-a"),
            ],
        );
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;
        assert_eq!(oa.stats.sent, 1);
        assert_eq!(ob.stats.received, 1);
        assert!(b.get("fine").is_some());
        assert!(b.get("huge").is_none());
    }

    #[tokio::test]
    async fn arrival_order_does_not_change_the_outcome() {
        // Three versions of one item, applied in every order, all end the same.
        let versions = [
            item("x", 100, "first", "dev-a"),
            item("x", 200, "second", "dev-b"),
            item("x", 150, "third", "dev-c"),
        ];
        let orders = [[0, 1, 2], [2, 1, 0], [1, 2, 0], [0, 2, 1]];
        let mut results = Vec::new();
        for order in orders {
            let store = TestSource::new("dev-z", vec![]);
            for i in order {
                store.apply(versions[i].clone()).unwrap();
            }
            results.push(store.snapshot());
        }
        for r in &results {
            assert_eq!(r, &results[0], "arrival order changed the final state");
            assert_eq!(r[0].content, "second");
        }
    }

    #[tokio::test]
    async fn applying_the_same_item_twice_stores_it_once() {
        let store = TestSource::new("dev-z", vec![]);
        let v = item("x", 100, "body", "dev-a");
        assert!(store.apply(v.clone()).unwrap());
        assert!(
            !store.apply(v.clone()).unwrap(),
            "second apply stored again"
        );
        assert_eq!(store.snapshot().len(), 1);
    }

    #[tokio::test]
    async fn receive_items_applies_a_wire_batch_in_one_source_call() {
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

        let a = CountingSource {
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
        let promised: Vec<_> = batch.iter().map(SyncItem::summary).collect();
        let mut chan = ScriptChannel::new(vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
            SyncMessage::Summary {
                items: promised,
                more: false,
            },
            SyncMessage::Items { items: batch },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a, None, SyncCursor::default())
            .await
            .unwrap();
        assert_eq!(outcome.stats.received, 3);
        assert_eq!(
            a.batch_calls.load(Ordering::SeqCst),
            1,
            "a wire Items frame must land as one apply_batch"
        );
        assert_eq!(a.last_batch_len.load(Ordering::SeqCst), 3);
        assert_eq!(
            a.apply_calls.load(Ordering::SeqCst),
            0,
            "receive_items must not fall back to per-item apply"
        );
        assert!(a.inner.get("one").is_some());
        assert!(a.inner.get("two").is_some());
        assert!(a.inner.get("three").is_some());
    }

    #[tokio::test]
    async fn summary_exchange_does_not_load_the_full_history_at_once() {
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

        let history: Vec<_> = (0..50)
            .map(|n| item(&format!("i{n:02}"), 1_000 + n as i64, "body", "dev-a"))
            .collect();
        let a = PagedOnly {
            inner: TestSource::new("dev-a", history),
            full_loads: AtomicUsize::new(0),
            page_loads: AtomicUsize::new(0),
        };
        let b = PagedOnly {
            inner: TestSource::new("dev-b", vec![]),
            full_loads: AtomicUsize::new(0),
            page_loads: AtomicUsize::new(0),
        };

        let (oa, ob) = {
            use tokio::sync::mpsc;
            let (a_tx, b_rx) = mpsc::channel(8);
            let (b_tx, a_rx) = mpsc::channel(8);
            struct Mem {
                tx: mpsc::Sender<Vec<u8>>,
                rx: mpsc::Receiver<Vec<u8>>,
            }
            impl SyncChannel for Mem {
                async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
                    self.tx
                        .send(msg.encode()?)
                        .await
                        .map_err(|_| SyncError::Channel("peer went away".into()))
                }
                async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
                    let bytes = self
                        .rx
                        .recv()
                        .await
                        .ok_or_else(|| SyncError::Channel("peer went away".into()))?;
                    Ok(SyncMessage::decode(&bytes)?)
                }
            }
            let (oa, ob) = tokio::join!(
                async {
                    let mut ca = Mem { tx: a_tx, rx: a_rx };
                    let out = run_initiator(&mut ca, &a, None, SyncCursor::default()).await;
                    drop(ca);
                    out
                },
                async {
                    let mut cb = Mem { tx: b_tx, rx: b_rx };
                    let out = run_responder(&mut cb, &b, None, SyncCursor::default()).await;
                    drop(cb);
                    out
                }
            );
            (oa.unwrap(), ob.unwrap())
        };
        assert_eq!(oa.stats.sent, 50);
        assert_eq!(ob.stats.received, 50);
        assert_eq!(a.full_loads.load(Ordering::SeqCst), 0);
        assert_eq!(b.full_loads.load(Ordering::SeqCst), 0);
        assert!(a.page_loads.load(Ordering::SeqCst) >= 1);
        assert!(b.page_loads.load(Ordering::SeqCst) >= 1);
    }
}
