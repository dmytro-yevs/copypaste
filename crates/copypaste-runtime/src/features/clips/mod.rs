//! Clip edits, annotations, keep, traces and transient deliveries.

mod edit;
mod expiry;
mod latest;
mod privacy;
mod restore;
mod trace;

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn clips(&self, id: u64, _op: ClipOp) -> Response {
        super::unsupported(id)
    }
}
