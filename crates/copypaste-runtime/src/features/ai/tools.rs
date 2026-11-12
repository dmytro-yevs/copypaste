//! MCP tool calls.

use copypaste_ipc::ai::AiOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: AiOp) -> Response {
    crate::features::unsupported(id)
}
