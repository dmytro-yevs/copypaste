//! Bounded tag-management contract shared by all platform hosts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_HISTORY_TAGS: usize = 500;
pub const MAX_TAG_NAME_CHARS: usize = 64;
pub const MAX_TAG_CLIPS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryTag {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryTags {
    pub tags: Vec<HistoryTag>,
    pub assignments: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HistoryTagMutation {
    Save {
        id: Option<String>,
        name: String,
    },
    Delete {
        id: String,
    },
    Assign {
        clip_ids: Vec<String>,
        tag_ids: Vec<String>,
        assigned: bool,
    },
}
