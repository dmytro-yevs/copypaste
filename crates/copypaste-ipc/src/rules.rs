//! Content rules: list, edit, test, preview and retroactive apply.
//!
//! A rule definition crosses the wire as JSON (`definition_json`) so a newer
//! peer's unknown condition or action survives a round trip unchanged; the
//! runtime owns its schema and validation.

use serde::{Deserialize, Serialize};

/// The existing Privacy settings, shown read-only beside the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyDefaults {
    pub skip_secret: bool,
    pub skip_transient: bool,
    pub excluded_app_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    Builtin,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleDeviceScope {
    AllDevices,
    Devices { ids: Vec<String> },
}

/// Local-only statistics; never synced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleStats {
    pub match_count: u64,
    pub last_matched_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleRow {
    pub id: String,
    pub origin: RuleOrigin,
    pub builtin_version: Option<u32>,
    pub name: String,
    pub enabled: bool,
    pub order_key: String,
    pub definition_json: String,
    pub device_scope: RuleDeviceScope,
    /// Error code shown as a badge, for example an uncompilable pattern.
    pub error_badge: Option<String>,
    /// Name of the device that recently added or changed this rule.
    pub new_from_device: Option<String>,
    /// The definition uses a type this build cannot evaluate.
    pub unsupported: bool,
    pub deleted_at: Option<i64>,
    pub stats: RuleStats,
}

/// A losing concurrent definition kept for restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleConflict {
    pub id: String,
    pub rule_id: String,
    pub from_device: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleList {
    pub privacy_defaults: PrivacyDefaults,
    pub rules: Vec<RuleRow>,
    pub conflicts: Vec<RuleConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDraft {
    pub id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub definition_json: String,
    pub device_scope: RuleDeviceScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleError {
    /// JSON pointer into the definition.
    pub path: String,
    pub offset: Option<u32>,
    pub message_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleSample {
    /// Evaluated in memory and never stored.
    Text {
        text: String,
    },
    Clip {
        id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceResult {
    Matched,
    NotMatched,
    Unknown,
    Skipped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCondition {
    pub index: u32,
    pub result: TraceResult,
    pub unknown_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceStep {
    pub rule_id: String,
    pub rule_name: String,
    pub result: TraceResult,
    pub conditions: Vec<TraceCondition>,
    /// Action codes this rule contributed.
    pub actions: Vec<String>,
    pub stopped: bool,
}

/// How a rule set treated one capture, and the combined outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTrace {
    pub steps: Vec<TraceStep>,
    pub skipped: bool,
    pub secret: bool,
    pub hidden_from_integrations: bool,
    pub expires_after_ms: Option<u64>,
    pub tag_ids: Vec<String>,
    pub transform_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePreviewCounts {
    pub matched: u32,
    pub deleted: u32,
    pub secret: u32,
    pub hidden_from_integrations: u32,
    pub expiring: u32,
    pub tagged: u32,
}

/// One matching item. `preview` is empty when `masked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePreviewItem {
    pub id: String,
    pub preview: String,
    pub masked: bool,
    pub created_at: i64,
    pub source_app_name: Option<String>,
}

/// A retroactive plan. `token` is valid until apply, cancel or its deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePreview {
    pub token: String,
    pub counts: RulePreviewCounts,
    pub items: Vec<RulePreviewItem>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleApplyReport {
    pub changed: u32,
    pub deleted: u32,
    pub failed: u32,
    pub cancelled: bool,
}

/// One skipped capture, kept in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipEvent {
    pub at: i64,
    pub source_app_name: Option<String>,
    pub rule_name: String,
}

/// Retroactive preview and apply accept at most this many items.
pub const MAX_RULE_PREVIEW_ITEMS: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RuleOp {
    List,
    Save {
        draft: RuleDraft,
    },
    Delete {
        id: String,
    },
    Restore {
        id: String,
    },
    Reorder {
        id: String,
        after_id: Option<String>,
    },
    SetEnabled {
        id: String,
        enabled: bool,
    },
    ResetBuiltin {
        id: String,
    },
    RestoreConflict {
        conflict_id: String,
    },
    Validate {
        definition_json: String,
    },
    Test {
        definition_json: Option<String>,
        rule_id: Option<String>,
        sample: RuleSample,
        full_set: bool,
    },
    Preview {
        request_id: String,
        definition_json: Option<String>,
        rule_id: Option<String>,
        full_set: bool,
        limit: u32,
    },
    Apply {
        preview_token: String,
        request_id: String,
    },
    /// For callers that cannot subscribe to `Progress` events.
    ApplyProgress {
        request_id: String,
    },
    Cancel {
        request_id: String,
    },
    SkipLog,
}

impl crate::OpGates for RuleOp {
    fn requires_ready(&self) -> bool {
        !matches!(
            self,
            Self::Validate { .. } | Self::ApplyProgress { .. } | Self::Cancel { .. }
        )
    }

    fn interactive(&self) -> bool {
        matches!(self, Self::ApplyProgress { .. } | Self::Cancel { .. })
    }

    fn blocking_weight(&self) -> u32 {
        match self {
            Self::Test { .. } | Self::Preview { .. } | Self::Apply { .. } => {
                crate::limits::content_weight_units(4)
            }
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        matches!(self, Self::Preview { .. } | Self::Apply { .. })
    }

    fn privileged(&self) -> bool {
        false
    }
}
