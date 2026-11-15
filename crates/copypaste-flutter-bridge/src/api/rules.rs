//! Content rules: list, edit, validate, test, preview and apply.
//!
//! Definitions stay JSON so a rule written by a newer peer round-trips
//! unchanged; the runtime owns their schema.

use copypaste_ipc::rules::{self as ipc, RuleOp};
use copypaste_ipc::{Method, ResponseData};

use super::{empty_response, expect_data, RuntimeError};

#[derive(Debug, Clone)]
pub struct RulePrivacyDefaults {
    pub skip_secret: bool,
    pub skip_transient: bool,
    pub excluded_app_count: u32,
}

#[derive(Debug, Clone)]
pub struct RuleStatsView {
    pub match_count: u64,
    pub last_matched_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

/// One rule. An empty `device_ids` means the rule runs on every device.
#[derive(Debug, Clone)]
pub struct RuleView {
    pub id: String,
    pub builtin: bool,
    pub builtin_version: Option<u32>,
    pub name: String,
    pub enabled: bool,
    pub order_key: String,
    pub definition_json: String,
    pub device_ids: Vec<String>,
    pub error_badge: Option<String>,
    pub new_from_device: Option<String>,
    pub unsupported: bool,
    pub deleted_at_ms: Option<i64>,
    pub stats: RuleStatsView,
}

#[derive(Debug, Clone)]
pub struct RuleConflictView {
    pub id: String,
    pub rule_id: String,
    pub from_device: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RuleListView {
    pub privacy_defaults: RulePrivacyDefaults,
    pub rules: Vec<RuleView>,
    pub conflicts: Vec<RuleConflictView>,
}

/// A rule to save. An empty `device_ids` means every device.
#[derive(Debug, Clone)]
pub struct RuleDraftInput {
    pub id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub definition_json: String,
    pub device_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuleErrorView {
    pub path: String,
    pub offset: Option<u32>,
    pub message_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTraceResult {
    Matched,
    NotMatched,
    Unknown,
    Skipped,
    Error,
}

#[derive(Debug, Clone)]
pub struct RuleTraceConditionView {
    pub index: u32,
    pub result: RuleTraceResult,
    pub unknown_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuleTraceStepView {
    pub rule_id: String,
    pub rule_name: String,
    pub result: RuleTraceResult,
    pub conditions: Vec<RuleTraceConditionView>,
    pub actions: Vec<String>,
    pub stopped: bool,
}

#[derive(Debug, Clone)]
pub struct RuleTraceView {
    pub steps: Vec<RuleTraceStepView>,
    pub skipped: bool,
    pub secret: bool,
    pub expires_after_ms: Option<u64>,
    pub tag_ids: Vec<String>,
    pub transform_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RulePreviewCountsView {
    pub matched: u32,
    pub deleted: u32,
    pub secret: u32,
    pub expiring: u32,
    pub tagged: u32,
}

#[derive(Debug, Clone)]
pub struct RulePreviewItemView {
    pub id: String,
    pub preview: String,
    pub masked: bool,
    pub created_at_ms: i64,
    pub source_app_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RulePreviewView {
    pub token: String,
    pub counts: RulePreviewCountsView,
    pub items: Vec<RulePreviewItemView>,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct RuleApplyReportView {
    pub changed: u32,
    pub deleted: u32,
    pub failed: u32,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct OperationProgressView {
    pub request_id: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone)]
pub struct RuleSkipEvent {
    pub at_ms: i64,
    pub source_app_name: Option<String>,
    pub rule_name: String,
}

pub(crate) fn operation_progress(value: copypaste_ipc::OperationProgress) -> OperationProgressView {
    OperationProgressView {
        request_id: value.request_id,
        done: value.done,
        total: value.total,
    }
}

fn trace_result(value: ipc::TraceResult) -> RuleTraceResult {
    match value {
        ipc::TraceResult::Matched => RuleTraceResult::Matched,
        ipc::TraceResult::NotMatched => RuleTraceResult::NotMatched,
        ipc::TraceResult::Unknown => RuleTraceResult::Unknown,
        ipc::TraceResult::Skipped => RuleTraceResult::Skipped,
        ipc::TraceResult::Error => RuleTraceResult::Error,
    }
}

pub(crate) fn rule_trace(trace: ipc::RuleTrace) -> RuleTraceView {
    RuleTraceView {
        steps: trace
            .steps
            .into_iter()
            .map(|step| RuleTraceStepView {
                rule_id: step.rule_id,
                rule_name: step.rule_name,
                result: trace_result(step.result),
                conditions: step
                    .conditions
                    .into_iter()
                    .map(|condition| RuleTraceConditionView {
                        index: condition.index,
                        result: trace_result(condition.result),
                        unknown_reason: condition.unknown_reason,
                    })
                    .collect(),
                actions: step.actions,
                stopped: step.stopped,
            })
            .collect(),
        skipped: trace.skipped,
        secret: trace.secret,
        expires_after_ms: trace.expires_after_ms,
        tag_ids: trace.tag_ids,
        transform_ids: trace.transform_ids,
    }
}

fn device_scope(device_ids: Vec<String>) -> ipc::RuleDeviceScope {
    if device_ids.is_empty() {
        ipc::RuleDeviceScope::AllDevices
    } else {
        ipc::RuleDeviceScope::Devices { ids: device_ids }
    }
}

fn rule_view(row: ipc::RuleRow) -> RuleView {
    RuleView {
        id: row.id,
        builtin: row.origin == ipc::RuleOrigin::Builtin,
        builtin_version: row.builtin_version,
        name: row.name,
        enabled: row.enabled,
        order_key: row.order_key,
        definition_json: row.definition_json,
        device_ids: match row.device_scope {
            ipc::RuleDeviceScope::AllDevices => Vec::new(),
            ipc::RuleDeviceScope::Devices { ids } => ids,
        },
        error_badge: row.error_badge,
        new_from_device: row.new_from_device,
        unsupported: row.unsupported,
        deleted_at_ms: row.deleted_at,
        stats: RuleStatsView {
            match_count: row.stats.match_count,
            last_matched_at_ms: row.stats.last_matched_at,
            last_error: row.stats.last_error,
        },
    }
}

fn rule_op(op: RuleOp) -> Method {
    Method::Rules { op }
}

async fn rule_response(op: RuleOp) -> Result<RuleView, RuntimeError> {
    Ok(rule_view(expect_data!(rule_op(op), ResponseData::Rule)))
}

pub async fn content_rules() -> Result<RuleListView, RuntimeError> {
    let list = expect_data!(rule_op(RuleOp::List), ResponseData::RuleList);
    Ok(RuleListView {
        privacy_defaults: RulePrivacyDefaults {
            skip_secret: list.privacy_defaults.skip_secret,
            skip_transient: list.privacy_defaults.skip_transient,
            excluded_app_count: list.privacy_defaults.excluded_app_count,
        },
        rules: list.rules.into_iter().map(rule_view).collect(),
        conflicts: list
            .conflicts
            .into_iter()
            .map(|conflict| RuleConflictView {
                id: conflict.id,
                rule_id: conflict.rule_id,
                from_device: conflict.from_device,
                created_at_ms: conflict.created_at,
            })
            .collect(),
    })
}

pub async fn save_content_rule(draft: RuleDraftInput) -> Result<RuleView, RuntimeError> {
    rule_response(RuleOp::Save {
        draft: ipc::RuleDraft {
            id: draft.id,
            name: draft.name,
            enabled: draft.enabled,
            definition_json: draft.definition_json,
            device_scope: device_scope(draft.device_ids),
        },
    })
    .await
}

pub async fn delete_content_rule(id: String) -> Result<(), RuntimeError> {
    empty_response(rule_op(RuleOp::Delete { id })).await
}

pub async fn restore_content_rule(id: String) -> Result<(), RuntimeError> {
    empty_response(rule_op(RuleOp::Restore { id })).await
}

pub async fn reorder_content_rule(
    id: String,
    after_id: Option<String>,
) -> Result<(), RuntimeError> {
    empty_response(rule_op(RuleOp::Reorder { id, after_id })).await
}

pub async fn set_content_rule_enabled(id: String, enabled: bool) -> Result<RuleView, RuntimeError> {
    rule_response(RuleOp::SetEnabled { id, enabled }).await
}

pub async fn reset_builtin_content_rule(id: String) -> Result<RuleView, RuntimeError> {
    rule_response(RuleOp::ResetBuiltin { id }).await
}

pub async fn restore_content_rule_conflict(conflict_id: String) -> Result<RuleView, RuntimeError> {
    rule_response(RuleOp::RestoreConflict { conflict_id }).await
}

pub async fn validate_content_rule(
    definition_json: String,
) -> Result<Vec<RuleErrorView>, RuntimeError> {
    let errors = expect_data!(
        rule_op(RuleOp::Validate { definition_json }),
        ResponseData::RuleErrors
    );
    Ok(errors
        .into_iter()
        .map(|error| RuleErrorView {
            path: error.path,
            offset: error.offset,
            message_code: error.message_code,
        })
        .collect())
}

/// Evaluate a draft or a saved rule against a sample text or a stored clip.
/// Exactly one of `sample_text` and `sample_clip_id` must be set.
pub async fn test_content_rule(
    definition_json: Option<String>,
    rule_id: Option<String>,
    sample_text: Option<String>,
    sample_clip_id: Option<String>,
    full_set: bool,
) -> Result<RuleTraceView, RuntimeError> {
    let sample = match (sample_text, sample_clip_id) {
        (Some(text), None) => ipc::RuleSample::Text { text },
        (None, Some(id)) => ipc::RuleSample::Clip { id },
        _ => return Err(RuntimeError::internal()),
    };
    let trace = expect_data!(
        rule_op(RuleOp::Test {
            definition_json,
            rule_id,
            sample,
            full_set,
        }),
        ResponseData::RuleTrace
    );
    Ok(rule_trace(trace))
}

pub async fn preview_content_rule(
    request_id: String,
    definition_json: Option<String>,
    rule_id: Option<String>,
    full_set: bool,
    limit: u32,
) -> Result<RulePreviewView, RuntimeError> {
    let preview = expect_data!(
        rule_op(RuleOp::Preview {
            request_id,
            definition_json,
            rule_id,
            full_set,
            limit,
        }),
        ResponseData::RulePreview
    );
    Ok(RulePreviewView {
        token: preview.token,
        counts: RulePreviewCountsView {
            matched: preview.counts.matched,
            deleted: preview.counts.deleted,
            secret: preview.counts.secret,
            expiring: preview.counts.expiring,
            tagged: preview.counts.tagged,
        },
        items: preview
            .items
            .into_iter()
            .map(|item| RulePreviewItemView {
                id: item.id,
                preview: item.preview,
                masked: item.masked,
                created_at_ms: item.created_at,
                source_app_name: item.source_app_name,
            })
            .collect(),
        truncated: preview.truncated,
    })
}

pub async fn apply_content_rule(
    preview_token: String,
    request_id: String,
) -> Result<RuleApplyReportView, RuntimeError> {
    let report = expect_data!(
        rule_op(RuleOp::Apply {
            preview_token,
            request_id,
        }),
        ResponseData::RuleApplyReport
    );
    Ok(RuleApplyReportView {
        changed: report.changed,
        deleted: report.deleted,
        failed: report.failed,
        cancelled: report.cancelled,
    })
}

/// For surfaces that cannot subscribe to progress events.
pub async fn content_rule_apply_progress(
    request_id: String,
) -> Result<Option<OperationProgressView>, RuntimeError> {
    let progress = expect_data!(
        rule_op(RuleOp::ApplyProgress { request_id }),
        ResponseData::OperationProgress
    );
    Ok(progress.map(operation_progress))
}

pub async fn cancel_content_rule_work(request_id: String) -> Result<(), RuntimeError> {
    empty_response(rule_op(RuleOp::Cancel { request_id })).await
}

pub async fn content_rule_skip_log() -> Result<Vec<RuleSkipEvent>, RuntimeError> {
    let events = expect_data!(rule_op(RuleOp::SkipLog), ResponseData::SkipLog);
    Ok(events
        .into_iter()
        .map(|event| RuleSkipEvent {
            at_ms: event.at,
            source_app_name: event.source_app_name,
            rule_name: event.rule_name,
        })
        .collect())
}
