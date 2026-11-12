//! Built-in text transforms, pipelines and transform preferences.

mod defaults;
mod pipelines;
mod preview;

use copypaste_ipc::transforms::TransformOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn transforms(&self, id: u64, _op: TransformOp) -> Response {
        super::unsupported(id)
    }
}
