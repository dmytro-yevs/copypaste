//! Manual clips and edited copies.

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: ClipOp) -> Response {
    crate::features::unsupported(id)
}
