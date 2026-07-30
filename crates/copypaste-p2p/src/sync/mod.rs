//! The merge decision, and the session that drives it.
//!
//! Four responsibilities, four files:
//!
//! * [`merge`] — last-write-wins over four metadata keys, and the argument for
//!   why a structural CRDT crate cannot do this job. The comparator is the one
//!   piece both devices must agree on exactly, so it lives alone and is tested
//!   exhaustively over its whole decision space.
//! * [`plan`] — what to ask a peer for, decided from the two summaries and an
//!   injected clock. Pure, which is what makes the delete-before-create case and
//!   the clock-skew ceiling testable without a session.
//! * [`session`] — the ten messages, the batching, and the bounds that stop a
//!   peer holding the session open.
//! * [`source`] — the two traits the session is generic over, plus what it
//!   reports. Separate so the daemon can implement them without reading the
//!   session, and so the tests can drive the real session over an in-memory
//!   store and an in-memory duplex.
//!
//! The seam between `merge` and `plan` matters beyond file size: `plan` sees
//! only [`ItemSummary`](crate::protocol::ItemSummary)s, so it can evaluate three
//! of the comparator's four keys. Keeping the comparator in one place, with
//! [`merge_decision_by_summary`] defined *in terms of* [`merge_decision`], is
//! what stops the planning half and the applying half reaching different answers
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
