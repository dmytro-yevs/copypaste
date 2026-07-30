//! What a session needs from the world around it: the local store, the channel
//! underneath, and what it reports afterwards.
//!
//! Two traits, both small on purpose. Keeping them here rather than inside
//! [`session`](super::session) is what lets the daemon implement them without
//! reading the session logic, and what lets the session tests drive the real
//! session functions over an in-memory store and an in-memory duplex.

use std::future::Future;

use super::SyncError;
use crate::protocol::{ItemSummary, SyncItem, SyncMessage};

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

/// What the session needs from the local store.
///
/// The daemon implements this over its `Store`. Kept to four methods so the
/// engine can be exercised without a database — every session test drives the
/// real session functions over an in-memory implementation.
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
/// but it MUST re-check with [`merge_decision`](super::merge_decision) against
/// whatever it holds now, using the stored and incoming `origin_device_id`. It
/// is the only place the true origins are known, it is the layer that sees
/// concurrent local writes, and re-checking is what makes replay a no-op rather
/// than a resurrection. Return `false` when the local copy wins.
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
