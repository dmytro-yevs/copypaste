//! AI tool access (MCP): policy, grants, approvals, sessions and tools.

mod approvals;
mod limits;
mod redact;
mod sessions;
mod tools;

use copypaste_ipc::ai::AiOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn ai(&self, id: u64, op: AiOp) -> Response {
        match op {
            AiOp::PendingApprovals
            | AiOp::Approve { .. }
            | AiOp::Deny { .. }
            | AiOp::VolumeDecision { .. } => approvals::handle(self, id, op),
            AiOp::Status { .. }
            | AiOp::SessionOpen { .. }
            | AiOp::SessionAttach { .. }
            | AiOp::SessionClose { .. } => sessions::handle(self, id, op),
            AiOp::Tool { .. } => tools::handle(self, id, op),
            _ => super::unsupported(id),
        }
    }

    /// Bind `session_id` to the `watch_events` connection `connection`; the
    /// session ends when [`Runtime::on_watch_closed`] reports that connection.
    pub fn attach_ai_session(&self, id: u64, connection: u64, session_id: &str) -> Response {
        sessions::attach(self, id, connection, session_id)
    }

    /// A `watch_events` connection closed; end the sessions it owned.
    pub fn on_watch_closed(&self, connection: u64) {
        sessions::connection_closed(self, connection);
    }
}
