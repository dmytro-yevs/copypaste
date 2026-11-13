//! Rule traces of stored clips: "Why is this clip here, secret or expiring?"

use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use crate::messages::{storage_error, MSG_NOT_FOUND};
use crate::Runtime;

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    let ClipOp::Trace { id: clip_id } = op else {
        return crate::features::unsupported(id);
    };
    match runtime.clip_rule_trace(&clip_id) {
        Ok(Some(trace)) => Response::ok(id, ResponseData::RuleTrace(trace)),
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(error) => storage_error(id, "clip_rule_trace", &error),
    }
}
