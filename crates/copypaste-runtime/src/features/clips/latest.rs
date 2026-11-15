//! The newest clip from another device.
//!
//! Without `copy` the answer is a bounded preview, cheap enough for a tray or
//! tile to ask each time it opens. With `copy` the clip is put on the
//! clipboard as an own write of a stored item: the row is promoted (K33) and
//! a secret keeps its concealed markers. Whether a secret may be copied at all
//! is the caller's decision, after device authentication.

use copypaste_core::HistoryPreviewRow;
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::{HistoryQuery, Item, Response, ResponseData, MAX_PAGE_CONTENT_BYTES};

use crate::messages::storage_error;
use crate::Runtime;

/// Rows captured in the same millisecond on one device are rare; a few more
/// than one keep the exact row reachable among them.
const SAME_INSTANT_ROWS: u32 = 8;

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    let ClipOp::LatestFromOtherDevice { copy } = op else {
        return crate::features::unsupported(id);
    };
    let latest = match runtime
        .store
        .latest_from_other_device(runtime.meta.device_id())
    {
        Ok(latest) => latest,
        Err(error) => return storage_error(id, "latest_from_other_device", &error),
    };
    let Some((item_id, origin, created_at)) = latest else {
        return Response::ok(id, ResponseData::MaybeItem(None));
    };
    if copy {
        return match runtime.deliver_item(&item_id, false, true) {
            Ok(delivered) => Response::ok(id, ResponseData::MaybeItem(delivered.item)),
            Err(error) => error.response(id),
        };
    }
    match preview(runtime, &item_id, origin, created_at) {
        Ok(item) => Response::ok(id, ResponseData::MaybeItem(item)),
        Err(error) => storage_error(id, "latest_from_other_device", &error),
    }
}

/// The history preview of one row, found by its origin and capture instant.
fn preview(
    runtime: &Runtime,
    item_id: &str,
    origin: String,
    created_at: i64,
) -> Result<Option<Item>, copypaste_core::StoreError> {
    let query = HistoryQuery {
        origin_device_id: Some(origin),
        captured_from_ms: Some(created_at),
        captured_before_ms: Some(created_at.saturating_add(1)),
        ..HistoryQuery::default()
    };
    let page = runtime.store.query_history_previews_with_semantic(
        &query,
        None,
        SAME_INSTANT_ROWS,
        MAX_PAGE_CONTENT_BYTES,
        Some(runtime.meta.device_id()),
        None,
        (&runtime.keyring.item_key(), &runtime.keyring.preview_key()),
    )?;
    Ok(page
        .items
        .into_iter()
        .find(|entry| entry.row.id == item_id)
        .and_then(|entry| into_item(runtime, entry)))
}

/// An unreadable row has no preview and is reported as absent.
fn into_item(runtime: &Runtime, entry: HistoryPreviewRow) -> Option<Item> {
    let HistoryPreviewRow {
        row,
        preview,
        annotations,
    } = entry;
    let origin = runtime.meta.origin_of(&row);
    let mut item = preview?.into_item(&row, &origin, runtime.meta.device_id());
    item.annotations = annotations;
    Some(item)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring, RemoteVersion};
    use copypaste_ipc::{content_type, Method};

    use super::*;
    use crate::host::{ClipboardWriter, InProcessHost, WriteOptions};

    struct AcceptingClipboard;

    impl ClipboardWriter for AcceptingClipboard {
        fn write(
            &self,
            _: &ClipboardPayload,
            _: &str,
            _: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            Ok(())
        }
    }

    fn latest(runtime: &Runtime, copy: bool) -> Option<Item> {
        let response = runtime.request_blocking(
            1,
            Method::Clips {
                op: ClipOp::LatestFromOtherDevice { copy },
            },
        );
        match response.data {
            Some(ResponseData::MaybeItem(item)) => item,
            other => panic!("expected an optional item, got {other:?}"),
        }
    }

    fn receive(runtime: &Runtime, item_id: &str, origin: &str, created_at: i64) {
        runtime
            .source
            .apply_version(&RemoteVersion {
                item_id,
                content: "from another device",
                binary_content: None,
                payload_metadata: None,
                content_type: content_type::TEXT,
                created_at,
                deleted: false,
                content_hash: None,
                origin_device_id: origin,
                app_bundle_id: None,
                app_name: None,
            })
            .expect("the received row is stored");
    }

    #[test]
    fn only_clips_from_other_devices_are_offered() {
        let (runtime, _dir) = crate::tests::fixture();
        assert!(latest(&runtime, false).is_none());

        let local = runtime.meta.device_id().to_owned();
        receive(&runtime, "older-remote", "laptop", 10);
        receive(&runtime, "newer-remote", "phone", 20);
        receive(&runtime, "own-version", &local, 30);

        let item = latest(&runtime, false).expect("a remote clip");
        assert_eq!(item.id, "newer-remote");
        assert_eq!(item.origin_device_id, "phone");
    }

    #[test]
    fn copying_promotes_the_remote_clip_without_changing_its_origin() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "latest",
            0,
            Arc::new(InProcessHost(Arc::new(AcceptingClipboard))),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        receive(&runtime, "remote", "phone", 10);

        let copied = latest(&runtime, true).expect("the copied clip");
        assert_eq!(copied.id, "remote");
        assert!(copied.created_at > 10, "the copy promotes the row");
        assert_eq!(
            latest(&runtime, false).map(|item| item.origin_device_id),
            Some("phone".to_owned())
        );
    }
}
