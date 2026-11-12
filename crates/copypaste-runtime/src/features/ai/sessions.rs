//! MCP sessions, each bound to the `watch_events` connection that attached
//! it, so a client that exits or crashes frees its session.

use copypaste_ipc::ai::AiOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: AiOp) -> Response {
    crate::features::unsupported(id)
}

pub(super) fn attach(_runtime: &Runtime, id: u64, _connection: u64, _session_id: &str) -> Response {
    crate::features::unsupported(id)
}

pub(super) fn connection_closed(_runtime: &Runtime, _connection: u64) {}
