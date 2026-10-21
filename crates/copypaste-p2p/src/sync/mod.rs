//! The merge decision, and the session that drives it.
//!
//! The comparator is the one piece both devices must agree on exactly, so it
//! lives alone in [`merge`] and is tested over its whole decision space.
//! [`plan`] sees [`ItemSummary`](crate::protocol::ItemSummary)s containing every
//! comparator key; defining
//! [`merge_decision_by_summary`] *in terms of* [`merge_decision`] is what stops
//! the planning half and the applying half reaching different answers
//! (manifest 05 INV-C2).

mod application;
mod error;
mod merge;
mod plan;
mod session;
mod source;
mod summary;
mod transfer;
mod workspace;

#[cfg(test)]
pub(crate) mod testutil;

pub use error::SyncError;
pub use merge::{merge_decision, merge_decision_by_summary, pin_state_wins, MergeDecision};
pub use plan::MAX_FUTURE_SKEW_MS;
pub use session::{run_initiator, run_responder, run_responder_from_first};
pub use source::{
    DeferredFileSource, FileTransferObserver, SyncChannel, SyncCursor, SyncOutcome, SyncSource,
    SyncStats,
};
pub use workspace::{MemorySummaryWorkspace, SummaryWorkspace};

// A cancelled session leaves the blocking task owning the workspace until it
// completes and drops it; no SQLite operation remains on the async reactor.
async fn workspace_call<T: Send + 'static>(
    mut workspace: Box<dyn SummaryWorkspace>,
    blocking: bool,
    f: impl FnOnce(&mut dyn SummaryWorkspace) -> T + Send + 'static,
) -> Result<(Box<dyn SummaryWorkspace>, T), SyncError> {
    if !blocking {
        let result = f(&mut *workspace);
        return Ok((workspace, result));
    }
    tokio::task::spawn_blocking(move || {
        let result = f(&mut *workspace);
        (workspace, result)
    })
    .await
    .map_err(|_| SyncError::Source("the sync session workspace is unavailable".into()))
}
