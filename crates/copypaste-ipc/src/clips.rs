//! Clip operations beyond capture: manual clips, edits, annotations, keep,
//! rule traces and transient deliveries.

use serde::{Deserialize, Serialize};

use crate::payload::ItemAnnotations;

/// How produced text leaves the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Put on the clipboard for a native paste the caller performs next.
    Paste,
    /// Put on the clipboard only.
    Copy,
    /// Return the text without touching the clipboard (Android IME).
    Return,
}

/// The result of a delivery. `token` names a restorable clipboard write;
/// `text` is present only for [`Delivery::Return`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivered {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_graphemes_from_end: Option<u32>,
}

/// Annotations of one clip, keyed by its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipAnnotations {
    pub id: String,
    pub annotations: ItemAnnotations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClipOp {
    /// A new text clip attributed to CopyPaste with `manual` provenance.
    CreateManual {
        text: String,
    },
    /// Save an edited copy as a new clip; optionally tombstone the original.
    SaveEdit {
        id: String,
        text: String,
        replace_original: bool,
    },
    Annotations {
        ids: Vec<String>,
    },
    SetHiddenFromIntegrations {
        ids: Vec<String>,
        hidden: bool,
    },
    /// Clear any expiry and keep the clip.
    Keep {
        id: String,
    },
    /// Which rules acted on this clip at capture or receipt.
    Trace {
        id: String,
    },
    /// The newest clip that arrived from another device, optionally copied.
    LatestFromOtherDevice {
        copy: bool,
    },
    /// Restore the clipboard held before `token`'s write, if it is still ours.
    RestoreClipboard {
        token: String,
        after_ms: u32,
    },
    /// Deliver text that is not a stored clip (transient, self-suppressed).
    DeliverText {
        text: String,
        secret: bool,
        delivery: Delivery,
    },
}

impl crate::OpGates for ClipOp {
    fn requires_ready(&self) -> bool {
        true
    }

    fn interactive(&self) -> bool {
        matches!(
            self,
            Self::DeliverText { .. } | Self::RestoreClipboard { .. } | Self::Keep { .. }
        )
    }

    /// Admission units in MiB, as for the history verbs.
    fn blocking_weight(&self) -> u32 {
        match self {
            Self::CreateManual { .. }
            | Self::SaveEdit { .. }
            | Self::DeliverText { .. }
            | Self::LatestFromOtherDevice { .. } => crate::limits::content_weight_units(4),
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        false
    }

    /// Un-hiding a clip from integrations is served only to the CopyPaste app.
    fn privileged(&self) -> bool {
        matches!(self, Self::SetHiddenFromIntegrations { hidden: false, .. })
    }
}
