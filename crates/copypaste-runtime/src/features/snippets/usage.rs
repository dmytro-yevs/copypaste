//! Snippet use: render, deliver and bump the local usage counters.

use copypaste_ipc::snippets::SnippetOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: SnippetOp) -> Response {
    crate::features::unsupported(id)
}
