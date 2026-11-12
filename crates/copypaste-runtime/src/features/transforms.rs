//! Built-in text transforms, pipelines and transform preferences.

mod defaults;
mod pipelines;
mod preview;

use copypaste_ipc::transforms::TransformOp;
use copypaste_ipc::Response;

use crate::Runtime;

impl Runtime {
    pub(crate) fn transforms(&self, id: u64, op: TransformOp) -> Response {
        match op {
            TransformOp::Preview { .. } | TransformOp::Cancel { .. } => {
                preview::handle(self, id, op)
            }
            TransformOp::Pipelines
            | TransformOp::SavePipeline { .. }
            | TransformOp::DeletePipeline { .. } => pipelines::handle(self, id, op),
            TransformOp::Preferences
            | TransformOp::SetPreference { .. }
            | TransformOp::SetMember { .. }
            | TransformOp::DefaultFor { .. } => defaults::handle(self, id, op),
            _ => super::unsupported(id),
        }
    }
}
