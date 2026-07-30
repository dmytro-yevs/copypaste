//! What a session needs from the world around it: the local store, the channel
//! underneath, and what it reports afterwards.
//!
//! Two traits, both small on purpose, so the daemon can implement them without
//! reading the session logic and the tests can drive the real session functions
//! over an in-memory store and an in-memory duplex.

use std::future::Future;

use super::SyncError;
use crate::protocol::{ItemSummary, SyncItem, SyncMessage};

/// What one session moved. `skipped` counts remote versions declined: an LWW
/// loser, a timestamp beyond the skew ceiling, a want that did not fit this
/// session, an item the peer sent unasked, or an apply the source refused.
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

/// What the session needs from the local store. The daemon implements it over
/// its `Store`.
///
/// # Sensitive items must never leave the device
///
/// [`summaries`](SyncSource::summaries) MUST exclude any item flagged sensitive,
/// and [`fetch`](SyncSource::fetch) MUST refuse to return one even if asked. The
/// session is the third layer: it serves only ids it advertised in its own
/// summary, so an item that never appeared in a summary cannot be pulled out of
/// this device by a request. Three layers for one rule, deliberately.
///
/// # Apply must guard itself
///
/// [`apply`](SyncSource::apply) is handed a version the session believes wins,
/// but it MUST re-check with [`merge_decision`](super::merge_decision) against
/// whatever it holds now, using the stored and incoming `origin_device_id`. It
/// is the only place the true origins are known and the only layer that sees
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

/// The channel one session runs over. Deliberately not `transport::Session`:
/// the session logic has no business knowing about Noise. The daemon implements
/// it over the real Noise session, encoding with [`SyncMessage::encode`] and
/// decoding with [`SyncMessage::decode`] so the bounds apply on the way in.
///
/// `recv` must return an error rather than block forever when the peer goes
/// away; timeouts belong to the implementation, not here. Either half can abort
/// mid-session without a goodbye, leaving the other end waiting on a message
/// that never arrives, and a channel with no read deadline turns that into a
/// stuck task.
pub trait SyncChannel {
    fn send(&mut self, msg: SyncMessage) -> impl Future<Output = Result<(), SyncError>> + Send;
    fn recv(&mut self) -> impl Future<Output = Result<SyncMessage, SyncError>> + Send;
}
