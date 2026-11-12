//! The device-local paste queue: collecting, ordering and ordered delivery.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::PasteQueueSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePhase {
    Empty,
    Active,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueEntryStatus {
    Pending,
    Pasted,
    Skipped,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueContentKind {
    Text,
    RichText,
    Image,
    Files,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueAddedVia {
    Collect,
    HistoryAdd,
    SnippetAdd,
    SelectionAction,
    Share,
}

/// One entry as displayed. `preview` is empty when `masked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub entry_id: String,
    pub position_label: String,
    pub status: QueueEntryStatus,
    pub content_kind: QueueContentKind,
    pub preview: String,
    pub masked: bool,
    pub edited: bool,
    pub source_app_bundle_id: Option<String>,
    pub source_app_name: Option<String>,
    pub source_app_icon_id: Option<String>,
    pub added_via: QueueAddedVia,
    /// When a snippet entry's template was rendered.
    pub rendered_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteQueueState {
    pub revision: u64,
    pub collecting: bool,
    pub phase: QueuePhase,
    pub full: bool,
    pub entries: Vec<QueueEntry>,
    /// Set when the newest entry is older than the stale-hint threshold.
    pub stale_since: Option<i64>,
    /// While set, `UndoClear` restores the entries removed by `Clear`.
    pub undo_until: Option<i64>,
    pub settings: PasteQueueSettings,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueRestart {
    #[default]
    StartFresh,
    Append,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueTextVia {
    SelectionAction,
    Share,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueSource {
    Clip {
        id: String,
    },
    /// Rendered when added, with these field values.
    Snippet {
        id: String,
        #[serde(default)]
        values: BTreeMap<String, String>,
    },
    Text {
        text: String,
        via: QueueTextVia,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueTarget {
    Next,
    All,
    Entry { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueDeliveryMode {
    /// Transient clipboard write; the caller pastes, then confirms.
    Clipboard,
    /// Return the text for an editor commit (Android IME).
    Return,
    /// Clipboard write with no paste to confirm ("Copy all").
    CopyOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NothingToPaste {
    Empty,
    Finished,
    /// Paste all needs a text-only or files-only queue.
    Mixed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueNotices {
    pub unavailable: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueDelivery {
    pub token: Option<String>,
    pub entry_ids: Vec<String>,
    pub text: Option<String>,
    pub notices: QueueNotices,
    pub finished: bool,
    pub nothing_to_paste: Option<NothingToPaste>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueDeliveryOutcome {
    Inserted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PasteQueueOp {
    Get,
    SetCollecting {
        on: bool,
        #[serde(default)]
        on_finished: QueueRestart,
    },
    Add {
        sources: Vec<QueueSource>,
    },
    Remove {
        entry_ids: Vec<String>,
    },
    RemovePasted,
    Clear,
    UndoClear,
    Reverse,
    Move {
        entry_id: String,
        to_index: u32,
    },
    /// `None` drops the entry's local override.
    Edit {
        entry_id: String,
        text: Option<String>,
    },
    Deliver {
        target: QueueTarget,
        plain_text: Option<bool>,
        mode: QueueDeliveryMode,
    },
    Confirm {
        token: String,
        outcome: QueueDeliveryOutcome,
    },
    StepBack,
    Skip,
}

impl crate::OpGates for PasteQueueOp {
    fn requires_ready(&self) -> bool {
        true
    }

    fn interactive(&self) -> bool {
        matches!(
            self,
            Self::Deliver { .. } | Self::Confirm { .. } | Self::StepBack | Self::Skip
        )
    }

    fn blocking_weight(&self) -> u32 {
        match self {
            Self::Deliver { .. } | Self::Add { .. } | Self::Edit { .. } => {
                crate::limits::content_weight_units(4)
            }
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        false
    }

    fn privileged(&self) -> bool {
        false
    }
}
