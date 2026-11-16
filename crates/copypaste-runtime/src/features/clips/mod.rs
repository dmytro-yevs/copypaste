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
    pub(crate) fn clips(&self, id: u64, op: ClipOp) -> Response {
        match op {
            ClipOp::CreateManual { .. } | ClipOp::SaveEdit { .. } => edit::handle(self, id, op),
            ClipOp::Annotations { .. } | ClipOp::SetHiddenFromIntegrations { .. } => {
                privacy::handle(self, id, op)
            }
            ClipOp::Keep { .. } => expiry::handle(self, id, op),
            ClipOp::Trace { .. } => trace::handle(self, id, op),
            ClipOp::Preview { id: clip_id } => self.clip_preview(id, &clip_id),
            ClipOp::LatestFromOtherDevice { .. } => latest::handle(self, id, op),
            ClipOp::RestoreClipboard { .. } => restore::handle(self, id, op),
            ClipOp::DeliverText { .. } => super::delivery::handle(self, id, op),
        }
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use copypaste_ipc::limits::LIST_PREVIEW_BYTES;
    use copypaste_ipc::{Item, Method, ResponseData};

    use super::*;
    use crate::tests::fixture;

    fn preview(runtime: &Runtime, id: &str) -> Item {
        let op = ClipOp::Preview { id: id.into() };
        match runtime.request_blocking(1, Method::Clips { op }).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_preview_is_the_bounded_row_and_an_image_keeps_its_size() {
        let (runtime, _dir) = fixture();
        let body = "z".repeat(LIST_PREVIEW_BYTES * 64);
        runtime.capture_text(&body).unwrap();
        let text = runtime.store.list(1, 0).unwrap().pop().unwrap().id;
        let item = preview(&runtime, &text);
        assert!(item.truncated);
        assert!(item.content.capacity() <= LIST_PREVIEW_BYTES);
        match runtime.get(2, &text).data {
            Some(ResponseData::Item(item)) => assert_eq!(item.content, body),
            other => panic!("unexpected {other:?}"),
        }

        let png = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        runtime
            .capture_binary(&png, "image/png", None, None)
            .unwrap();
        let image = runtime.store.list(1, 0).unwrap().pop().unwrap().id;
        let details = preview(&runtime, &image).image_details.unwrap();
        assert_eq!((details.width, details.height), (1, 1));
    }
}
