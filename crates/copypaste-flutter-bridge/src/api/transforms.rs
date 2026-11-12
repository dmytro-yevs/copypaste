//! Built-in text transforms, saved pipelines and transform preferences.

use copypaste_ipc::transforms::{self as ipc, TransformOp};
use copypaste_ipc::{Method, ResponseData};

use super::clips::DeliveredText;
use super::{clip, empty_response, expect_data, Clip, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformParamKind {
    Choice,
    Bool,
    Integer,
    Text,
}

#[derive(Debug, Clone)]
pub struct TransformParamOptionView {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct TransformParamView {
    pub key: String,
    pub label: String,
    pub kind: TransformParamKind,
    pub default_json: String,
    pub options: Vec<TransformParamOptionView>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub max_len: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TransformFlagsView {
    pub can_be_default: bool,
    pub needs_free_text: bool,
    pub inherits_sensitivity: bool,
    pub rich_input: bool,
    pub deterministic: bool,
    pub modifier: bool,
}

#[derive(Debug, Clone)]
pub struct TransformInfoView {
    pub id: String,
    pub version: u32,
    pub category: String,
    pub name: String,
    pub description: String,
    pub example_in: String,
    pub example_out: String,
    pub keywords: Vec<String>,
    pub params: Vec<TransformParamView>,
    pub flags: TransformFlagsView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSourceKind {
    Clip,
    Text,
    Clipboard,
}

/// `clip_id` is required for [`TransformSourceKind::Clip`], `text` for
/// [`TransformSourceKind::Text`].
#[derive(Debug, Clone)]
pub struct TransformSourceInput {
    pub kind: TransformSourceKind,
    pub clip_id: Option<String>,
    pub text: Option<String>,
}

/// One pipeline step; `params_json` is a JSON object of parameter values.
#[derive(Debug, Clone)]
pub struct TransformStepView {
    pub transform_id: String,
    pub version: u32,
    pub params_json: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct TextCountsView {
    pub chars: u64,
    pub words: u64,
    pub lines: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct TransformWarningView {
    pub code: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransformFailureView {
    pub code: String,
    pub offset: Option<u64>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TransformStepResultView {
    pub output_prefix: String,
    pub counts: TextCountsView,
    pub warnings: Vec<TransformWarningView>,
    pub error: Option<TransformFailureView>,
    pub not_run: bool,
}

#[derive(Debug, Clone)]
pub struct TransformPreviewView {
    pub request_id: String,
    pub input_counts: TextCountsView,
    pub steps: Vec<TransformStepResultView>,
    pub truncated: bool,
    pub no_changes: bool,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformAction {
    Paste,
    /// A new plain-text clip with transformed provenance, also copied.
    Copy,
    SaveNewClip,
    Return,
}

/// A run either delivered text or created a clip.
#[derive(Debug, Clone)]
pub struct TransformRunResult {
    pub delivered: Option<DeliveredText>,
    pub clip: Option<Clip>,
}

#[derive(Debug, Clone)]
pub struct TransformPipelineView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<TransformStepView>,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TransformPipelineDraft {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<TransformStepView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformFavoriteKind {
    Transform,
    Pipeline,
}

#[derive(Debug, Clone)]
pub struct TransformFavoriteView {
    pub kind: TransformFavoriteKind,
    pub id: String,
}

/// `pipeline_id: None` means "no default transform" for that app.
#[derive(Debug, Clone)]
pub struct TransformAppDefaultView {
    pub platform: String,
    pub identifier: String,
    pub pipeline_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransformParamDefaultsView {
    pub transform_id: String,
    pub params_json: String,
}

#[derive(Debug, Clone)]
pub struct TransformPreferencesView {
    pub favorites: Vec<TransformFavoriteView>,
    pub hidden_transforms: Vec<String>,
    pub param_defaults: Vec<TransformParamDefaultsView>,
    pub default_paste_pipeline_id: Option<String>,
    pub per_app_defaults: Vec<TransformAppDefaultView>,
    pub tracking_remove: Vec<String>,
    pub tracking_allow: Vec<String>,
    pub restore_clipboard_after_paste: bool,
    pub suggestions_enabled: bool,
    pub disabled_detectors: Vec<String>,
}

fn transform_op(op: TransformOp) -> Method {
    Method::Transforms { op }
}

fn source(value: TransformSourceInput) -> Result<ipc::TransformSource, RuntimeError> {
    Ok(match (value.kind, value.clip_id, value.text) {
        (TransformSourceKind::Clip, Some(id), _) => ipc::TransformSource::Clip { id },
        (TransformSourceKind::Text, _, Some(text)) => ipc::TransformSource::Text { text },
        (TransformSourceKind::Clipboard, _, _) => ipc::TransformSource::Clipboard,
        _ => return Err(RuntimeError::internal()),
    })
}

fn step(value: TransformStepView) -> ipc::TransformStep {
    ipc::TransformStep {
        transform_id: value.transform_id,
        version: value.version,
        params_json: value.params_json,
        enabled: value.enabled,
    }
}

fn step_view(value: ipc::TransformStep) -> TransformStepView {
    TransformStepView {
        transform_id: value.transform_id,
        version: value.version,
        params_json: value.params_json,
        enabled: value.enabled,
    }
}

fn counts(value: ipc::TextCounts) -> TextCountsView {
    TextCountsView {
        chars: value.chars,
        words: value.words,
        lines: value.lines,
        bytes: value.bytes,
    }
}

fn info(value: ipc::TransformInfo) -> TransformInfoView {
    TransformInfoView {
        id: value.id,
        version: value.version,
        category: value.category,
        name: value.name,
        description: value.description,
        example_in: value.example_in,
        example_out: value.example_out,
        keywords: value.keywords,
        params: value
            .params
            .into_iter()
            .map(|param| TransformParamView {
                key: param.key,
                label: param.label,
                kind: match param.kind {
                    ipc::TransformParamKind::Choice => TransformParamKind::Choice,
                    ipc::TransformParamKind::Bool => TransformParamKind::Bool,
                    ipc::TransformParamKind::Integer => TransformParamKind::Integer,
                    ipc::TransformParamKind::Text => TransformParamKind::Text,
                },
                default_json: param.default_json,
                options: param
                    .options
                    .into_iter()
                    .map(|option| TransformParamOptionView {
                        value: option.value,
                        label: option.label,
                    })
                    .collect(),
                min: param.min,
                max: param.max,
                max_len: param.max_len,
            })
            .collect(),
        flags: TransformFlagsView {
            can_be_default: value.flags.can_be_default,
            needs_free_text: value.flags.needs_free_text,
            inherits_sensitivity: value.flags.inherits_sensitivity,
            rich_input: value.flags.rich_input,
            deterministic: value.flags.deterministic,
            modifier: value.flags.modifier,
        },
    }
}

fn preview(value: ipc::TransformPreview) -> TransformPreviewView {
    TransformPreviewView {
        request_id: value.request_id,
        input_counts: counts(value.input_counts),
        steps: value
            .steps
            .into_iter()
            .map(|result| TransformStepResultView {
                output_prefix: result.output_prefix,
                counts: counts(result.counts),
                warnings: result
                    .warnings
                    .into_iter()
                    .map(|warning| TransformWarningView {
                        code: warning.code,
                        detail: warning.detail,
                    })
                    .collect(),
                error: result.error.map(|error| TransformFailureView {
                    code: error.code,
                    offset: error.offset,
                    line: error.line,
                    column: error.column,
                }),
                not_run: result.not_run,
            })
            .collect(),
        truncated: value.truncated,
        no_changes: value.no_changes,
        sensitive: value.sensitive,
    }
}

fn pipeline(value: ipc::Pipeline) -> TransformPipelineView {
    TransformPipelineView {
        id: value.id,
        name: value.name,
        description: value.description,
        steps: value.steps.into_iter().map(step_view).collect(),
        deleted_at_ms: value.deleted_at,
    }
}

fn preferences(value: ipc::TransformPreferences) -> TransformPreferencesView {
    TransformPreferencesView {
        favorites: value
            .favorites
            .into_iter()
            .map(|favorite| TransformFavoriteView {
                kind: match favorite.kind {
                    ipc::FavoriteKind::Transform => TransformFavoriteKind::Transform,
                    ipc::FavoriteKind::Pipeline => TransformFavoriteKind::Pipeline,
                },
                id: favorite.id,
            })
            .collect(),
        hidden_transforms: value.hidden_transforms,
        param_defaults: value
            .param_defaults
            .into_iter()
            .map(|defaults| TransformParamDefaultsView {
                transform_id: defaults.transform_id,
                params_json: defaults.params_json,
            })
            .collect(),
        default_paste_pipeline_id: value.default_paste_pipeline_id,
        per_app_defaults: value
            .per_app_defaults
            .into_iter()
            .map(|default| TransformAppDefaultView {
                platform: default.app.platform,
                identifier: default.app.identifier,
                pipeline_id: default.pipeline_id,
            })
            .collect(),
        tracking_remove: value.tracking_remove,
        tracking_allow: value.tracking_allow,
        restore_clipboard_after_paste: value.restore_clipboard_after_paste,
        suggestions_enabled: value.suggestions_enabled,
        disabled_detectors: value.disabled_detectors,
    }
}

async fn preferences_response(op: TransformOp) -> Result<TransformPreferencesView, RuntimeError> {
    Ok(preferences(expect_data!(
        transform_op(op),
        ResponseData::TransformPreferences
    )))
}

/// The catalogue with names and descriptions in `locale`.
pub async fn transform_catalogue(locale: String) -> Result<Vec<TransformInfoView>, RuntimeError> {
    let catalogue = expect_data!(
        transform_op(TransformOp::Catalogue { locale }),
        ResponseData::TransformCatalogue
    );
    Ok(catalogue.into_iter().map(info).collect())
}

pub async fn preview_transform(
    request_id: String,
    source_input: TransformSourceInput,
    steps: Vec<TransformStepView>,
) -> Result<TransformPreviewView, RuntimeError> {
    let result = expect_data!(
        transform_op(TransformOp::Preview {
            request_id,
            source: source(source_input)?,
            steps: steps.into_iter().map(step).collect(),
        }),
        ResponseData::TransformPreview
    );
    Ok(preview(result))
}

pub async fn cancel_transform(request_id: String) -> Result<(), RuntimeError> {
    empty_response(transform_op(TransformOp::Cancel { request_id })).await
}

/// Run `steps`, or the saved pipeline `pipeline_id` when it is set.
pub async fn run_transform(
    source_input: TransformSourceInput,
    steps: Vec<TransformStepView>,
    pipeline_id: Option<String>,
    action: TransformAction,
    bypass_default: bool,
) -> Result<TransformRunResult, RuntimeError> {
    let steps_or_pipeline = match pipeline_id {
        Some(id) => ipc::TransformSteps::Pipeline { id },
        None => ipc::TransformSteps::Steps {
            steps: steps.into_iter().map(step).collect(),
        },
    };
    let action = match action {
        TransformAction::Paste => ipc::TransformAction::Paste,
        TransformAction::Copy => ipc::TransformAction::Copy,
        TransformAction::SaveNewClip => ipc::TransformAction::SaveNewClip,
        TransformAction::Return => ipc::TransformAction::Return,
    };
    let method = transform_op(TransformOp::Run {
        source: source(source_input)?,
        steps_or_pipeline,
        action,
        bypass_default,
    });
    match super::request_data(method).await? {
        ResponseData::Delivered(delivered) => Ok(TransformRunResult {
            delivered: Some(delivered.into()),
            clip: None,
        }),
        ResponseData::Item(item) => Ok(TransformRunResult {
            delivered: None,
            clip: Some(clip(item)),
        }),
        _ => Err(RuntimeError::internal()),
    }
}

/// Up to three suggested transform ids for the source.
pub async fn suggest_transforms(
    source_input: TransformSourceInput,
) -> Result<Vec<String>, RuntimeError> {
    Ok(expect_data!(
        transform_op(TransformOp::Suggest {
            source: source(source_input)?,
        }),
        ResponseData::TransformSuggestions
    ))
}

pub async fn transform_pipelines() -> Result<Vec<TransformPipelineView>, RuntimeError> {
    let pipelines = expect_data!(
        transform_op(TransformOp::Pipelines),
        ResponseData::Pipelines
    );
    Ok(pipelines.into_iter().map(pipeline).collect())
}

pub async fn save_transform_pipeline(
    draft: TransformPipelineDraft,
) -> Result<TransformPipelineView, RuntimeError> {
    let saved = expect_data!(
        transform_op(TransformOp::SavePipeline {
            draft: ipc::PipelineDraft {
                id: draft.id,
                name: draft.name,
                description: draft.description,
                steps: draft.steps.into_iter().map(step).collect(),
            },
        }),
        ResponseData::Pipeline
    );
    Ok(pipeline(saved))
}

pub async fn delete_transform_pipeline(id: String) -> Result<(), RuntimeError> {
    empty_response(transform_op(TransformOp::DeletePipeline { id })).await
}

pub async fn transform_preferences() -> Result<TransformPreferencesView, RuntimeError> {
    preferences_response(TransformOp::Preferences).await
}

/// Set one synced preference; answers the updated preferences.
pub async fn set_transform_preference(
    key: String,
    value_json: String,
) -> Result<TransformPreferencesView, RuntimeError> {
    preferences_response(TransformOp::SetPreference { key, value_json }).await
}

/// Add or remove one member of a synced set; answers the updated preferences.
pub async fn set_transform_set_member(
    set: String,
    member: String,
    present: bool,
) -> Result<TransformPreferencesView, RuntimeError> {
    preferences_response(TransformOp::SetMember {
        set,
        member,
        present,
    })
    .await
}

/// The default paste pipeline for one application, if any.
pub async fn default_transform_pipeline_for(
    platform: String,
    identifier: String,
) -> Result<Option<String>, RuntimeError> {
    Ok(expect_data!(
        transform_op(TransformOp::DefaultFor {
            app_identity: ipc::AppIdentity {
                platform,
                identifier,
            },
        }),
        ResponseData::PipelineDefault
    ))
}
