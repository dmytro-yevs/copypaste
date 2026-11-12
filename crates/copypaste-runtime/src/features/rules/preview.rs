//! Retroactive rule previews and their plan tokens.

use copypaste_ipc::rules::RuleOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: RuleOp) -> Response {
    crate::features::unsupported(id)
}
