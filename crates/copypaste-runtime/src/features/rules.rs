//! Content rules: management, test, retroactive preview and apply.

mod apply;
mod builtins;
mod preview;

use copypaste_ipc::rules::RuleOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn rules(&self, id: u64, _op: RuleOp) -> Response {
        super::unsupported(id)
    }
}
