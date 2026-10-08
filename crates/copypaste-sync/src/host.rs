//! Serialized services provided by the application to installed sync code.
use crate::LocalItem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const UPLOAD_FLOOR: &str = "cloud_upload_floor_ms";
pub const UPLOAD_FLOOR_ITEM: &str = "cloud_upload_floor_item_id";
pub const UNREADABLE_UPLOADS: &str = "cloud_unreadable_uploads";

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncHostRequest {
    Active,
    DeviceId,
    SyncEnabled,
    Revision,
    ReadState {
        keys: Vec<String>,
    },
    WriteState {
        values: BTreeMap<String, String>,
    },
    ClearState {
        keys: Vec<String>,
    },
    Scan {
        since_ms: i64,
        after_item_id: Option<String>,
    },
    Apply {
        items: Vec<LocalItem>,
    },
    Requeue {
        incoming: LocalItem,
    },
    CommitUpload {
        started_ms: i64,
    },
}
