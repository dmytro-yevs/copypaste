//! Snippets, collections, templates and snippet import/export.

mod render;
mod transfer;
mod usage;

use copypaste_ipc::snippets::SnippetOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn snippets(&self, id: u64, op: SnippetOp) -> Response {
        match op {
            SnippetOp::Render { .. } | SnippetOp::Lint { .. } | SnippetOp::Release => {
                render::handle(self, id, op)
            }
            SnippetOp::Use { .. } => usage::handle(self, id, op),
            SnippetOp::ImportPreview { .. }
            | SnippetOp::ImportCommit { .. }
            | SnippetOp::ImportCancel { .. }
            | SnippetOp::Export { .. } => transfer::handle(self, id, op),
            _ => super::unsupported(id),
        }
    }
}
