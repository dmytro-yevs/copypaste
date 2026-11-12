//! Bounded, cancellable transform previews.

use copypaste_ipc::transforms::TransformOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: TransformOp) -> Response {
    crate::features::unsupported(id)
}
