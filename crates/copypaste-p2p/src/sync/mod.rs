//! The merge decision, and the session that drives it.
//!
//! The comparator is the one piece both devices must agree on exactly, so it
//! lives alone in [`merge`] and is tested over its whole decision space.
//! [`plan`] sees only [`ItemSummary`](crate::protocol::ItemSummary)s, so it can
//! evaluate three of the comparator's four keys; defining
//! [`merge_decision_by_summary`] *in terms of* [`merge_decision`] is what stops
//! the planning half and the applying half reaching different answers
//! (manifest 05 INV-C2).

mod error;
mod merge;
mod plan;
mod session;
mod source;

#[cfg(test)]
mod testutil;

pub use error::SyncError;
pub use merge::{merge_decision, merge_decision_by_summary, MergeDecision};
pub use plan::MAX_FUTURE_SKEW_MS;
pub use session::{run_initiator, run_responder};
pub use source::{SyncChannel, SyncOutcome, SyncSource, SyncStats};
