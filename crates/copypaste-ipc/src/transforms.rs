//! Built-in text transforms, saved pipelines and transform preferences.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformParamKind {
    Choice,
    Bool,
    Integer,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformParamOption {
    pub value: String,
    pub label: String,
}

/// One parameter of a transform. `default_json` is the JSON default value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformParam {
    pub key: String,
    pub label: String,
    pub kind: TransformParamKind,
    pub default_json: String,
    #[serde(default)]
    pub options: Vec<TransformParamOption>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub max_len: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransformFlags {
    pub can_be_default: bool,
    pub needs_free_text: bool,
    pub inherits_sensitivity: bool,
    pub rich_input: bool,
    pub deterministic: bool,
    /// Usable as a template modifier.
    pub modifier: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformInfo {
    pub id: String,
    pub version: u32,
    pub category: String,
    pub name: String,
    pub description: String,
    pub example_in: String,
    pub example_out: String,
    pub keywords: Vec<String>,
    pub params: Vec<TransformParam>,
    pub flags: TransformFlags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformSource {
    Clip { id: String },
    Text { text: String },
    Clipboard,
}

/// One pipeline step. `params_json` is a JSON object of parameter values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformStep {
    pub transform_id: String,
    pub version: u32,
    pub params_json: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextCounts {
    pub chars: u64,
    pub words: u64,
    pub lines: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformWarning {
    pub code: String,
    pub detail: Option<String>,
}

/// A failed step. Positions locate the input error, when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformFailure {
    pub code: String,
    pub offset: Option<u64>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    pub output_prefix: String,
    pub counts: TextCounts,
    pub warnings: Vec<TransformWarning>,
    pub error: Option<TransformFailure>,
    pub not_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformPreview {
    pub request_id: String,
    pub input_counts: TextCounts,
    pub steps: Vec<StepResult>,
    pub truncated: bool,
    pub no_changes: bool,
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformSteps {
    Steps { steps: Vec<TransformStep> },
    Pipeline { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformAction {
    Paste,
    /// A new plain-text clip with `transformed` provenance, also copied.
    Copy,
    SaveNewClip,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<TransformStep>,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineDraft {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<TransformStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FavoriteKind {
    Transform,
    Pipeline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteRef {
    pub kind: FavoriteKind,
    pub id: String,
}

/// A stable application identity on one platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppIdentity {
    pub platform: String,
    pub identifier: String,
}

/// `pipeline_id: None` means "no default transform" for that app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPipelineDefault {
    pub app: AppIdentity,
    pub pipeline_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamDefaults {
    pub transform_id: String,
    pub params_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformPreferences {
    pub favorites: Vec<FavoriteRef>,
    pub hidden_transforms: Vec<String>,
    pub param_defaults: Vec<ParamDefaults>,
    pub default_paste_pipeline_id: Option<String>,
    pub per_app_defaults: Vec<AppPipelineDefault>,
    /// Tracking parameters as `name` or `name@domain`.
    pub tracking_remove: Vec<String>,
    pub tracking_allow: Vec<String>,
    pub restore_clipboard_after_paste: bool,
    pub suggestions_enabled: bool,
    pub disabled_detectors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TransformOp {
    Catalogue {
        locale: String,
    },
    Preview {
        request_id: String,
        source: TransformSource,
        steps: Vec<TransformStep>,
    },
    Cancel {
        request_id: String,
    },
    Run {
        source: TransformSource,
        steps_or_pipeline: TransformSteps,
        action: TransformAction,
        bypass_default: bool,
    },
    /// Up to three suggested transform ids for the source.
    Suggest {
        source: TransformSource,
    },
    Pipelines,
    SavePipeline {
        draft: PipelineDraft,
    },
    DeletePipeline {
        id: String,
    },
    Preferences,
    SetPreference {
        key: String,
        value_json: String,
    },
    SetMember {
        set: String,
        member: String,
        present: bool,
    },
    DefaultFor {
        app_identity: AppIdentity,
    },
}

impl crate::OpGates for TransformOp {
    fn requires_ready(&self) -> bool {
        !matches!(self, Self::Catalogue { .. } | Self::Cancel { .. })
    }

    fn interactive(&self) -> bool {
        matches!(self, Self::Cancel { .. })
    }

    /// Input is bounded at 16 MiB and output at 32 MiB.
    fn blocking_weight(&self) -> u32 {
        match self {
            Self::Preview { .. } | Self::Run { .. } => 48,
            Self::Suggest { .. } => crate::limits::content_weight_units(1),
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        matches!(self, Self::Run { .. })
    }

    fn privileged(&self) -> bool {
        false
    }
}
