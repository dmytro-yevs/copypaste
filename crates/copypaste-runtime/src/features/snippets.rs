//! Snippets, collections, templates and snippet import/export.

mod render;
mod transfer;
mod usage;

use copypaste_ipc::snippets::SnippetOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn snippets(&self, id: u64, _op: SnippetOp) -> Response {
        super::unsupported(id)
    }
}
