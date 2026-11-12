//! The device-local paste queue.

use copypaste_ipc::paste_queue::PasteQueueOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn paste_queue(&self, id: u64, _op: PasteQueueOp) -> Response {
        super::unsupported(id)
    }
}
