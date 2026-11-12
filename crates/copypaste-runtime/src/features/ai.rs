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
    pub(crate) fn ai(&self, id: u64, _op: AiOp) -> Response {
        super::unsupported(id)
    }

    /// Bind `session_id` to the `watch_events` connection `connection`; the
    /// session ends when [`Runtime::on_watch_closed`] reports that connection.
    pub fn attach_ai_session(&self, id: u64, _connection: u64, _session_id: &str) -> Response {
        super::unsupported(id)
    }

    /// A `watch_events` connection closed; end the sessions it owned.
    pub fn on_watch_closed(&self, _connection: u64) {}
}
