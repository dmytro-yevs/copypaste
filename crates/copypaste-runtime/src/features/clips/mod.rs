//! Clip edits, annotations, keep, traces and transient deliveries.

mod edit;
mod expiry;
mod latest;
mod privacy;
mod restore;
mod trace;

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn clips(&self, id: u64, op: ClipOp) -> Response {
        match op {
            ClipOp::CreateManual { .. } | ClipOp::SaveEdit { .. } => edit::handle(self, id, op),
            ClipOp::Annotations { .. } | ClipOp::SetHiddenFromAi { .. } => {
                privacy::handle(self, id, op)
            }
            ClipOp::Keep { .. } => expiry::handle(self, id, op),
            ClipOp::Trace { .. } => trace::handle(self, id, op),
            ClipOp::LatestFromOtherDevice { .. } => latest::handle(self, id, op),
            ClipOp::RestoreClipboard { .. } => restore::handle(self, id, op),
            ClipOp::DeliverText { .. } => super::delivery::handle(self, id, op),
        }
    }
}
