//! The merge decision, and the session that drives it.
//!
//! # Why last-write-wins, and why it is not a library
//!
//! CLAUDE.md rule 1 says reach for a crate first. The structural CRDT crates
//! (`automerge`, `yrs`) are the obvious candidates and they cannot be used here,
//! under exemption 2: **the package cannot see what it would need to see.** A
//! structural CRDT merges *inside* a value — it needs to read the text to
//! produce a character-level merge. Our items are opaque ciphertext at rest and
//! the merge runs over metadata alone, so there is nothing for a structural CRDT
//! to operate on. Last-write-wins over four metadata keys is not a shortcut
//! here; it is the only thing that can work, and it is about forty lines.
//!
//! The cost of that decision, stated rather than hidden: two devices that edit
//! the *same* item concurrently keep one version and drop the other. For a
//! clipboard that is correct — a clipboard entry is a snapshot, not a document.
//!
//! # The order
//!
//! Compare two versions of the same `item_id`. Larger wins:
//!
//! 1. `created_at` — the version stamp in milliseconds. A tombstone carries the
//!    time of the *deletion*, which is what makes a delete beat the version it
//!    deleted.
//! 2. `content_hash` — lexicographic. Deterministic, and equal hashes mean equal
//!    content, so this key only ever separates versions that genuinely differ.
//! 3. `deleted` — a tombstone beats a live version. See below.
//! 4. `origin_device_id` — lexicographic; exists only to make the order total.
//!
//! Ties on all four keep the local copy. That strict `>` at the end is what
//! makes re-delivery free: a version that has already been applied compares
//! equal and loses, so replaying a whole session changes nothing.
//!
//! ## Why `deleted` is key 3 and not key 5
//!
//! The specification for this module named three keys: `created_at`,
//! `content_hash`, `origin_device_id`. `deleted` is inserted between the second
//! and the third — the three named keys keep their relative priority — for two
//! reasons, and the second one is load-bearing:
//!
//! * **Delete-wins on an exact-timestamp tie.** A tombstone keeps the item's
//!   content hash (the store does this deliberately, so that re-copying deleted
//!   content is not swallowed by dedup). So a delete of an unmodified item ties
//!   its own live version on keys 1 and 2, and without key 3 the winner would be
//!   whichever device id sorts higher. That resurrects deletes — the exact class
//!   of bug the sync manifest spends the most words on (INV-N2, `CopyPaste-ojhe`).
//! * **One comparator on both sides of the session.** An [`ItemSummary`] does not
//!   carry `origin_device_id`, so the *planning* half of a session cannot
//!   evaluate key 4 — it only sees keys 1-3. If `deleted` sat below the origin,
//!   the planner and the applier could reach different answers for the same pair
//!   and the two devices would never converge. With `deleted` above the origin,
//!   key 4 is consulted only when two versions are identical in time, content and
//!   state — a case where there is nothing to transfer and nothing to disagree
//!   about. Manifest 05 INV-C2 is precisely the bug that comes from two
//!   comparators; this keeps there being one.
//!
//! # The session
//!
//! One round trip of summaries, one request in each direction, items streamed
//! back in bounded batches. Ten messages, no state machine, no anti-entropy
//! engine: v1 shipped a HELLO/HAVE/WANT engine of about 5,000 lines that the
//! daemon never once instantiated, and everything here is on the path the daemon
//! calls.
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
use std::future::Future;

use crate::protocol::{
    ItemSummary, ProtocolError, SyncItem, SyncMessage, MAX_CONTENT_BYTES, MAX_ITEMS_PER_MESSAGE,
    MAX_ITEM_BYTES_PER_MESSAGE, MAX_REQUEST_IDS_PER_MESSAGE, MAX_SUMMARIES_PER_MESSAGE,
    PROTOCOL_VERSION,
};

/// How far into the future a peer's `created_at` may be before we refuse to
/// take that version.
///
/// The lower bound on timestamps is clamped at the decode boundary; the upper
/// bound needs the local clock and so lives here. Without it a peer — hostile,
/// or merely holding a wrong clock — can stamp `i64::MAX` on one `item_id` and
/// win every future comparison for it, effectively censoring that item on every
/// device. A day of slack is far more than the skew any real pair of devices
/// shows, and the response is to skip that one version, never to fail the
/// session or to delete anything (manifest 05 §3.4, R-CLK-2: no correction is
/// attempted, only refusal).
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

/// Which of two versions of one item survives.
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
/// copy, which is what makes applying the same item twice a no-op.
///
/// Order: `created_at`, then `content_hash`, then `deleted`, then
/// `origin_device_id` — see the module docs for why `deleted` sits where it
/// does. Content is never an input: at rest it is ciphertext, and the whole
/// design depends on the decision being reachable from metadata alone.
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

/// The same comparator over the keys a summary carries.
///
/// A summary has no `origin_device_id`, so the fourth key is neutralised by
/// passing the same value on both sides. That is not a second comparator — it is
/// [`merge_decision`] with one input held equal, which is why the two can never
/// drift apart (manifest 05 §7.2: if two shapes are needed, define one in terms
/// of the other).
///
/// When this returns `KeepLocal` because all three visible keys tie, the two
/// versions hold the same content at the same instant in the same state: there
/// is nothing worth transferring, whatever the origins say.
pub fn merge_decision_by_summary(local: &ItemSummary, remote: &ItemSummary) -> MergeDecision {
    merge_decision(local, "", remote, "")
}

/// What one session moved.
///
/// * `sent` — items handed to the peer.
/// * `received` — items the local source accepted and stored.
/// * `skipped` — remote versions declined: an LWW loser, a timestamp beyond the
///   skew ceiling, a want that did not fit this session, an item the peer sent
///   that we had not asked for, or an apply the source itself refused.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncStats {
    pub sent: usize,
    pub received: usize,
    pub skipped: usize,
}

/// The result of one completed session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub stats: SyncStats,
    pub peer_device_id: String,
    pub peer_device_name: String,
}

/// Session failures.
///
/// No variant carries a path or any item content: these are logged and shown to
/// users (CLAUDE.md rule 4). Implementations of [`SyncSource`] must hold to the
/// same rule when they build [`SyncError::Source`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// The channel underneath failed — closed, timed out, or refused a frame.
    #[error("sync channel error: {0}")]
    Channel(String),

    #[error("peer sent a {got} message where a {expected} was expected")]
    Unexpected {
        expected: &'static str,
        got: &'static str,
    },

    /// The local store failed. The message must already be safe to display.
    #[error("sync source error: {0}")]
    Source(String),

    /// The peer claims our own device id. Either the pairing is pointed at this
    /// device, or something is reflecting our traffic; neither is a session.
    #[error("peer reports this device's own id")]
    SelfSync,

    /// The peer kept sending after it had answered everything it was asked for.
    /// Bounding this is what stops a peer from holding the session open forever.
    #[error("peer sent more than it was asked for")]
    PeerOverran,
}

/// What the session needs from the local store.
///
/// The daemon implements this over its `Store`. Kept to four methods so the
/// engine can be exercised without a database — every test below drives the real
/// session functions over an in-memory implementation.
///
/// # Sensitive items must never leave the device
///
/// [`summaries`](SyncSource::summaries) MUST exclude any item flagged sensitive,
/// and [`fetch`](SyncSource::fetch) MUST refuse to return one even if asked. The
/// session enforces a third layer on top: it serves only ids it advertised in
/// its own summary, so an item that never appeared in a summary cannot be pulled
/// out of this device by a request. Three layers for one rule, deliberately —
/// data leaving the device is the case that matters, and the storage layer
/// carries the same belt-and-braces for the search index.
///
/// # Apply must guard itself
///
/// [`apply`](SyncSource::apply) is handed a version the session believes wins,
/// but it MUST re-check with [`merge_decision`] against whatever it holds now,
/// using the stored and incoming `origin_device_id`. It is the only place the
/// true origins are known, it is the layer that sees concurrent local writes,
/// and re-checking is what makes replay a no-op rather than a resurrection.
/// Return `false` when the local copy wins.
pub trait SyncSource {
    fn device_id(&self) -> String;
    fn device_name(&self) -> String;

    /// Summaries of everything eligible to sync. Sensitive items excluded.
    fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError>;

    /// Full items for the given ids, plaintext. Unknown ids are omitted rather
    /// than erroring; sensitive ids are omitted too.
    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError>;

    /// Applies a remote item. Returns whether it was stored.
    fn apply(&self, item: SyncItem) -> Result<bool, SyncError>;
}

/// The channel one session runs over.
///
/// Deliberately not `transport::Session`: the session logic has no business
/// knowing about Noise, and a trait this small is what lets both halves be
/// driven over an in-memory duplex in the tests. The daemon implements it over
/// the real Noise session, encoding with [`SyncMessage::encode`] and decoding
/// with [`SyncMessage::decode`] so the bounds are applied on the way in.
///
/// `recv` must return an error rather than blocking forever when the peer goes
/// away; timeouts belong to the implementation, not here. Either half can abort
/// mid-session — a bound was exceeded, the store failed — and it does so without
/// a goodbye, so the other end is left waiting on a message that will never
/// arrive. A channel with no read deadline turns that into a stuck task.
pub trait SyncChannel {
    fn send(&mut self, msg: SyncMessage) -> impl Future<Output = Result<(), SyncError>> + Send;
    fn recv(&mut self) -> impl Future<Output = Result<SyncMessage, SyncError>> + Send;
}

/// Runs the initiating half of a session to completion.
pub async fn run_initiator<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
) -> Result<SyncOutcome, SyncError> {
    let peer = {
        chan.send(local_hello(source)).await?;
        recv_hello(chan, source).await?
    };

    let advertised = advertise(chan, source).await?;
    let remote = recv_summary(chan).await?;

    let mut stats = SyncStats::default();
    let wanted = plan(&advertised, &remote, now_ms(), &mut stats);

    chan.send(SyncMessage::Request {
        item_ids: wanted.keys().cloned().collect(),
    })
    .await?;
    receive_items(chan, source, &wanted, &mut stats).await?;

    let requested = recv_request(chan).await?;
    serve_items(chan, source, &advertised, requested, &mut stats).await?;

    Ok(SyncOutcome {
        stats,
        peer_device_id: peer.0,
        peer_device_name: peer.1,
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
) -> Result<SyncOutcome, SyncError> {
    let peer = {
        let peer = recv_hello(chan, source).await?;
        chan.send(local_hello(source)).await?;
        peer
    };

    let remote = recv_summary(chan).await?;
    let advertised = advertise(chan, source).await?;

    let mut stats = SyncStats::default();

    let requested = recv_request(chan).await?;
    serve_items(chan, source, &advertised, requested, &mut stats).await?;

    let wanted = plan(&advertised, &remote, now_ms(), &mut stats);
    chan.send(SyncMessage::Request {
        item_ids: wanted.keys().cloned().collect(),
    })
    .await?;
    receive_items(chan, source, &wanted, &mut stats).await?;

    Ok(SyncOutcome {
        stats,
        peer_device_id: peer.0,
        peer_device_name: peer.1,
    })
}

// ---------------------------------------------------------------- session parts

fn local_hello<S: SyncSource>(source: &S) -> SyncMessage {
    SyncMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: source.device_id(),
        device_name: source.device_name(),
    }
}

/// Reads the peer's hello and fails closed on anything unexpected.
///
/// The version check itself lives in [`SyncMessage::validate`], so it applies to
/// every ingress path rather than only to this one; calling it again here costs
/// nothing and covers a channel implementation that skipped decode-time
/// validation.
async fn recv_hello<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
) -> Result<(String, String), SyncError> {
    let msg = chan.recv().await?;
    msg.validate()?;
    match msg {
        SyncMessage::Hello {
            device_id,
            device_name,
            ..
        } => {
            if device_id == source.device_id() {
                return Err(SyncError::SelfSync);
            }
            Ok((device_id, device_name))
        }
        other => Err(SyncError::Unexpected {
            expected: "hello",
            got: other.kind(),
        }),
    }
}

/// Sends our summary and returns exactly what was advertised, keyed by id.
///
/// The returned map is the session's authority on two questions: what we may
/// serve (nothing outside it, so a sensitive item can never be requested out of
/// us) and what the peer's summary is compared against.
async fn advertise<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
) -> Result<HashMap<String, ItemSummary>, SyncError> {
    let mut items = source.summaries()?;

    if items.len() > MAX_SUMMARIES_PER_MESSAGE {
        // Newest first, then truncate: a clipboard's newest entries are the ones
        // a user is waiting for, and the remainder is picked up by the next
        // session. Sessions repeat, so this delays convergence rather than
        // preventing it.
        tracing::warn!(
            count = items.len(),
            max = MAX_SUMMARIES_PER_MESSAGE,
            "more items than one summary message holds; advertising the newest"
        );
        items.sort_unstable_by_key(|s| std::cmp::Reverse(s.created_at));
        items.truncate(MAX_SUMMARIES_PER_MESSAGE);
    }

    chan.send(SyncMessage::Summary {
        items: items.clone(),
    })
    .await?;

    Ok(items.into_iter().map(|s| (s.item_id.clone(), s)).collect())
}

async fn recv_summary<C: SyncChannel>(chan: &mut C) -> Result<Vec<ItemSummary>, SyncError> {
    let msg = chan.recv().await?;
    msg.validate()?;
    match msg {
        SyncMessage::Summary { items } => Ok(items),
        other => Err(SyncError::Unexpected {
            expected: "summary",
            got: other.kind(),
        }),
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
fn plan(
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

/// Reads `Items` messages until `Done`, applying what was asked for.
///
/// Everything unasked-for is dropped. That is the guard against a peer pushing
/// an item we never evaluated — the merge decision was made against the summary,
/// so an item that does not match the summary it was promised as is not the item
/// we agreed to take.
async fn receive_items<C: SyncChannel, S: SyncSource>(
    chan: &mut C,
    source: &S,
    wanted: &HashMap<String, ItemSummary>,
    stats: &mut SyncStats,
) -> Result<(), SyncError> {
    // A peer that never sends `Done` must not hold the session open. It needs at
    // most one message per batch; two spare covers an empty tail batch and the
    // `Done` itself.
    let mut budget = wanted.len().div_ceil(MAX_ITEMS_PER_MESSAGE) + 2;
    let mut applied: HashSet<String> = HashSet::new();

    loop {
        if budget == 0 {
            return Err(SyncError::PeerOverran);
        }
        budget -= 1;

        let msg = chan.recv().await?;
        msg.validate()?;
        match msg {
            SyncMessage::Items { items } => {
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
                    if source.apply(item)? {
                        stats.received += 1;
                    } else {
                        stats.skipped += 1;
                    }
                }
            }
            SyncMessage::Done => return Ok(()),
            other => {
                return Err(SyncError::Unexpected {
                    expected: "items",
                    got: other.kind(),
                })
            }
        }
    }
}

/// Answers a request, then closes the stream with `Done`.
///
/// Only ids present in our own advertised summary are served. That is what makes
/// "a sensitive item never leaves the device" a property of the protocol and not
/// only of the store: a sensitive item is absent from `summaries()`, so it is
/// absent from `advertised`, so no request can name it.
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
            if item.content.len() > MAX_CONTENT_BYTES {
                // Skip the one item rather than failing the session: the rest of
                // the history is still worth syncing.
                tracing::warn!(
                    bytes = item.content.len(),
                    max = MAX_CONTENT_BYTES,
                    "item is too large to send; skipping it"
                );
                continue;
            }

            if !batch.is_empty()
                && (batch.len() == MAX_ITEMS_PER_MESSAGE
                    || batch_bytes + item.content.len() > MAX_ITEM_BYTES_PER_MESSAGE)
            {
                stats.sent += batch.len();
                chan.send(SyncMessage::Items {
                    items: std::mem::take(&mut batch),
                })
                .await?;
                batch_bytes = 0;
            }

            batch_bytes += item.content.len();
            batch.push(item);
        }
    }

    if !batch.is_empty() {
        stats.sent += batch.len();
        chan.send(SyncMessage::Items { items: batch }).await?;
    }

    chan.send(SyncMessage::Done).await
}

/// Milliseconds since the Unix epoch.
///
/// `copypaste-core` has the same helper, but this crate does not depend on it
/// and adding a dependency to reach three lines of `SystemTime` is not a trade
/// worth making. A clock before the epoch reads as 0, which loses every
/// comparison — the safe direction.
fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::sync::mpsc;

    // ------------------------------------------------------------- fixtures

    fn summary(id: &str, created_at: i64, hash: &str, deleted: bool) -> ItemSummary {
        ItemSummary {
            item_id: id.into(),
            created_at,
            deleted,
            content_hash: hash.into(),
        }
    }

    fn item(id: &str, created_at: i64, content: &str, origin: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: content.into(),
            content_type: "text".into(),
            created_at,
            deleted: false,
            // Stands in for a real digest: short, and different for different
            // content. A hash the length of the content would be rejected by
            // the field bounds, which is the correct behaviour but a poor
            // fixture.
            content_hash: format!("h-{}-{}", content.len(), &content[..content.len().min(16)]),
            origin_device_id: origin.into(),
        }
    }

    fn tombstone(id: &str, created_at: i64, hash: &str, origin: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: String::new(),
            content_type: "text".into(),
            created_at,
            deleted: true,
            content_hash: hash.into(),
            origin_device_id: origin.into(),
        }
    }

    /// An in-memory store that behaves the way the daemon's `Store` must:
    /// `apply` re-runs the comparator against what it holds, using the true
    /// origins, and reports whether anything changed.
    struct TestSource {
        device_id: String,
        device_name: String,
        items: Mutex<HashMap<String, SyncItem>>,
        sensitive: Mutex<HashSet<String>>,
        /// Items the source hands back even though they were not requested —
        /// used to prove the session refuses them.
        smuggle: Mutex<Vec<SyncItem>>,
    }

    impl TestSource {
        fn new(device_id: &str, items: Vec<SyncItem>) -> Self {
            Self {
                device_id: device_id.into(),
                device_name: format!("{device_id} name"),
                items: Mutex::new(items.into_iter().map(|i| (i.item_id.clone(), i)).collect()),
                sensitive: Mutex::new(HashSet::new()),
                smuggle: Mutex::new(Vec::new()),
            }
        }

        fn mark_sensitive(&self, id: &str) {
            self.sensitive.lock().unwrap().insert(id.into());
        }

        fn snapshot(&self) -> Vec<SyncItem> {
            let mut v: Vec<_> = self.items.lock().unwrap().values().cloned().collect();
            v.sort_by(|a, b| a.item_id.cmp(&b.item_id));
            v
        }

        fn get(&self, id: &str) -> Option<SyncItem> {
            self.items.lock().unwrap().get(id).cloned()
        }
    }

    impl SyncSource for TestSource {
        fn device_id(&self) -> String {
            self.device_id.clone()
        }

        fn device_name(&self) -> String {
            self.device_name.clone()
        }

        fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError> {
            let sensitive = self.sensitive.lock().unwrap();
            let mut v: Vec<_> = self
                .items
                .lock()
                .unwrap()
                .values()
                .filter(|i| !sensitive.contains(&i.item_id))
                .map(|i| i.summary())
                .collect();
            v.sort_by(|a, b| a.item_id.cmp(&b.item_id));
            Ok(v)
        }

        fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
            let sensitive = self.sensitive.lock().unwrap();
            let items = self.items.lock().unwrap();
            let mut out: Vec<_> = ids
                .iter()
                .filter(|id| !sensitive.contains(*id))
                .filter_map(|id| items.get(id).cloned())
                .collect();
            out.extend(self.smuggle.lock().unwrap().iter().cloned());
            Ok(out)
        }

        fn apply(&self, incoming: SyncItem) -> Result<bool, SyncError> {
            let mut items = self.items.lock().unwrap();
            if let Some(local) = items.get(&incoming.item_id) {
                let decision = merge_decision(
                    &local.summary(),
                    &local.origin_device_id,
                    &incoming.summary(),
                    &incoming.origin_device_id,
                );
                if decision == MergeDecision::KeepLocal {
                    return Ok(false);
                }
            }
            items.insert(incoming.item_id.clone(), incoming);
            Ok(true)
        }
    }

    /// One end of an in-memory duplex. Messages go through the real encoder and
    /// decoder, so every bound in `protocol` is exercised by every session test.
    struct MemChannel {
        tx: mpsc::Sender<Vec<u8>>,
        rx: mpsc::Receiver<Vec<u8>>,
    }

    fn duplex() -> (MemChannel, MemChannel) {
        let (a_tx, b_rx) = mpsc::channel(8);
        let (b_tx, a_rx) = mpsc::channel(8);
        (
            MemChannel { tx: a_tx, rx: a_rx },
            MemChannel { tx: b_tx, rx: b_rx },
        )
    }

    impl SyncChannel for MemChannel {
        async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
            let bytes = msg.encode()?;
            self.tx
                .send(bytes)
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

    /// A channel that replays a fixed script and records what it was sent —
    /// for the cases a well-behaved peer cannot produce.
    struct ScriptChannel {
        script: std::collections::VecDeque<SyncMessage>,
        sent: Vec<SyncMessage>,
    }

    impl ScriptChannel {
        fn new(script: Vec<SyncMessage>) -> Self {
            Self {
                script: script.into(),
                sent: Vec::new(),
            }
        }
    }

    impl SyncChannel for ScriptChannel {
        async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
            self.sent.push(msg);
            Ok(())
        }

        async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
            self.script
                .pop_front()
                .ok_or_else(|| SyncError::Channel("script exhausted".into()))
        }
    }

    /// Runs both halves over one duplex.
    ///
    /// Each half drops its end when it finishes, so a half that fails leaves the
    /// other with a closed channel rather than a wait that never ends — a test
    /// that hangs says far less than one that fails.
    async fn try_session(
        a: &TestSource,
        b: &TestSource,
    ) -> (
        Result<SyncOutcome, SyncError>,
        Result<SyncOutcome, SyncError>,
    ) {
        let (ca, cb) = duplex();
        tokio::join!(
            async move {
                let mut ca = ca;
                let out = run_initiator(&mut ca, a).await;
                drop(ca);
                out
            },
            async move {
                let mut cb = cb;
                let out = run_responder(&mut cb, b).await;
                drop(cb);
                out
            }
        )
    }

    async fn session(a: &TestSource, b: &TestSource) -> (SyncOutcome, SyncOutcome) {
        let (ra, rb) = try_session(a, b).await;
        (ra.expect("initiator"), rb.expect("responder"))
    }

    // ------------------------------------------------- the order, exhaustively

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
        for (x, _) in &space {
            for (y, _) in &space {
                assert_eq!(
                    merge_decision_by_summary(x, y),
                    merge_decision(x, "same", y, "same"),
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

    // ------------------------------------------------------------- planning

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
        // Newest kept, so the user sees the most recent entries first.
        assert!(wanted.contains_key(&format!("i{}", MAX_REQUEST_IDS_PER_MESSAGE + 9)));
        assert!(!wanted.contains_key("i0"));
    }

    // -------------------------------------------------------------- sessions

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
        assert_eq!(oa.stats.received, 1);
        assert_eq!(oa.stats.sent, 1);
        assert_eq!(ob.stats.received, 1);
        assert_eq!(ob.stats.sent, 1);
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
        // Same pair, roles swapped.
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

        // No oscillation: another full round moves nothing.
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
            },
            SyncMessage::Summary { items: vec![] },
            SyncMessage::Request {
                item_ids: vec!["secret".into()],
            },
            SyncMessage::Done,
        ]);

        let outcome = run_responder(&mut chan, &a).await.unwrap();
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
        a.smuggle
            .lock()
            .unwrap()
            .push(item("secret", 200, "hunter2", "dev-a"));
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
            },
            SyncMessage::Summary { items: vec![] },
            SyncMessage::Items {
                items: vec![item("pushed", 100, "unasked for", "dev-b")],
            },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a).await.unwrap();
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
            },
            SyncMessage::Summary {
                items: vec![promised],
            },
            SyncMessage::Items { items: vec![lying] },
            SyncMessage::Done,
            SyncMessage::Request { item_ids: vec![] },
        ]);

        let outcome = run_initiator(&mut chan, &a).await.unwrap();
        assert!(a.get("x").is_none());
        assert_eq!(outcome.stats.skipped, 1);
    }

    #[tokio::test]
    async fn a_peer_that_never_says_done_is_cut_off() {
        let a = TestSource::new("dev-a", vec![]);
        let mut script = vec![
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "dev-b".into(),
                device_name: "B".into(),
            },
            SyncMessage::Summary { items: vec![] },
        ];
        // Nothing was wanted, so any stream of items is already an overrun.
        script.extend(std::iter::repeat_with(|| SyncMessage::Items { items: vec![] }).take(64));

        let mut chan = ScriptChannel::new(script);
        assert_eq!(
            run_initiator(&mut chan, &a).await,
            Err(SyncError::PeerOverran)
        );
    }

    #[tokio::test]
    async fn a_version_mismatch_fails_closed_on_both_halves() {
        let a = TestSource::new("dev-a", vec![]);
        let stale = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION + 1,
            device_id: "dev-b".into(),
            device_name: "B".into(),
        };

        let mut chan = ScriptChannel::new(vec![stale.clone()]);
        assert_eq!(
            run_initiator(&mut chan, &a).await,
            Err(SyncError::Protocol(ProtocolError::VersionMismatch {
                ours: PROTOCOL_VERSION,
                theirs: PROTOCOL_VERSION + 1,
            }))
        );

        let mut chan = ScriptChannel::new(vec![stale]);
        assert!(matches!(
            run_responder(&mut chan, &a).await,
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
        }]);
        assert_eq!(run_initiator(&mut chan, &a).await, Err(SyncError::SelfSync));
    }

    #[tokio::test]
    async fn a_message_out_of_turn_is_an_error() {
        let a = TestSource::new("dev-a", vec![]);
        let mut chan = ScriptChannel::new(vec![SyncMessage::Done]);
        assert_eq!(
            run_initiator(&mut chan, &a).await,
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
    async fn a_large_item_gets_a_message_to_itself() {
        // Two items that cannot share a message: proof the byte budget splits
        // batches, not just the item count.
        let big = "x".repeat(MAX_ITEM_BYTES_PER_MESSAGE / 2 + 1);
        let a = TestSource::new(
            "dev-a",
            vec![
                item("big1", 100, &big, "dev-a"),
                item("big2", 200, &big, "dev-a"),
            ],
        );
        let b = TestSource::new("dev-b", vec![]);

        let (oa, ob) = session(&a, &b).await;
        assert_eq!(oa.stats.sent, 2);
        assert_eq!(ob.stats.received, 2);
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

    #[test]
    fn errors_disclose_nothing_sensitive() {
        for err in [
            SyncError::SelfSync,
            SyncError::PeerOverran,
            SyncError::Unexpected {
                expected: "hello",
                got: "done",
            },
            SyncError::Channel("peer went away".into()),
        ] {
            let text = err.to_string();
            assert!(!text.contains('/'), "error names a path: {text}");
        }
    }
}
