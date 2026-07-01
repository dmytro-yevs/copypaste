//! Shared test fixtures for the `rest` module's split test suites
//! (`read::tests`, `write::tests`, `reencrypt::tests`).
//!
//! `pub(super)` so the fixtures are visible to sibling test modules under
//! `crate::rest` without being part of the crate's public API.

use crate::models::CloudClipboardRow;

pub(super) fn live_row() -> CloudClipboardRow {
    CloudClipboardRow {
        id: "row-uuid-1".into(),
        item_id: "item-uuid-1".into(),
        content_type: "text".into(),
        payload_ct: Some("\\xdeadbeef".into()),
        lamport_ts: 10,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        device_id: "device-a".into(),
        deleted: false,
        pinned: false,
        pin_order: None,
    }
}

pub(super) fn tombstone_row() -> CloudClipboardRow {
    CloudClipboardRow {
        id: "row-uuid-2".into(),
        item_id: "item-uuid-2".into(),
        content_type: "text".into(),
        payload_ct: None,
        lamport_ts: 20,
        wall_time: 1_700_000_001_000,
        expires_at: None,
        app_bundle_id: None,
        device_id: "device-a".into(),
        deleted: true,
        pinned: false,
        pin_order: None,
    }
}
