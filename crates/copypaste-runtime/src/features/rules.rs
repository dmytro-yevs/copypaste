//! Content rules: management, test, retroactive preview and apply.

mod apply;
mod builtins;
mod preview;

use copypaste_ipc::rules::RuleOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn rules(&self, id: u64, op: RuleOp) -> Response {
        match op {
            RuleOp::Preview { .. } => preview::handle(self, id, op),
            RuleOp::Apply { .. } | RuleOp::ApplyProgress { .. } | RuleOp::Cancel { .. } => {
                apply::handle(self, id, op)
            }
            RuleOp::ResetBuiltin { .. } => builtins::handle(self, id, op),
            _ => super::unsupported(id),
        }
    }
}
