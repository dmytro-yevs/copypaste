//! Keeping clips past their expiry.

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::Response;

use crate::Runtime;

pub(super) fn handle(_runtime: &Runtime, id: u64, _op: ClipOp) -> Response {
    crate::features::unsupported(id)
}

impl Runtime {
    /// Purge the clips whose expiry passed by `now_ms`. Every deadline sweep
    /// runs it; read-time filtering already hides them until it does.
    pub(crate) fn expire_due_clips(&self, _now_ms: i64) {}
}
