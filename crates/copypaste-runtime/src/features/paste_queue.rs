//! The device-local paste queue.

use copypaste_ipc::paste_queue::PasteQueueOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn paste_queue(&self, id: u64, _op: PasteQueueOp) -> Response {
        super::unsupported(id)
    }

    /// A local capture was committed; collecting appends it.
    pub(crate) fn paste_queue_captured(&self, _item_id: &str) {}

    /// History was cleared; the queue is cleared with it.
    pub(crate) fn paste_queue_history_cleared(&self) {}
}
