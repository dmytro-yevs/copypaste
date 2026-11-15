//! Restoring the clipboard after a transient delivery.

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::{Response, ResponseData};

use crate::Runtime;

/// Accepted whether or not a restore will run: a token that is stale, or a
/// clipboard that changed in the meantime, means there is nothing to put back.
pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    match op {
        ClipOp::RestoreClipboard { token, after_ms } => {
            runtime.schedule_clipboard_restore(token, after_ms);
            Response::ok(id, ResponseData::Empty {})
        }
        _ => crate::features::unsupported(id),
    }
}
