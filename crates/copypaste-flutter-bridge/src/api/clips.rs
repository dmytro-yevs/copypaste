//! Clip edits, annotations, keep, rule traces and transient deliveries.

use copypaste_ipc::clips::{ClipOp, Delivery};
use copypaste_ipc::{Method, ResponseData};

use super::rules::{rule_trace, RuleTraceView};
use super::{clip, empty_response, expect_data, item_response, Clip, RuntimeError};

/// How produced text leaves the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDelivery {
    Paste,
    Copy,
    Return,
}

impl From<TextDelivery> for Delivery {
    fn from(value: TextDelivery) -> Self {
        match value {
            TextDelivery::Paste => Self::Paste,
            TextDelivery::Copy => Self::Copy,
            TextDelivery::Return => Self::Return,
        }
    }
}

/// A delivery result; `text` is present only for [`TextDelivery::Return`].
#[derive(Debug, Clone)]
pub struct DeliveredText {
    pub token: Option<String>,
    pub text: Option<String>,
    pub cursor_graphemes_from_end: Option<u32>,
}

impl From<copypaste_ipc::clips::Delivered> for DeliveredText {
    fn from(value: copypaste_ipc::clips::Delivered) -> Self {
        Self {
            token: value.token,
            text: value.text,
            cursor_graphemes_from_end: value.cursor_graphemes_from_end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipProvenanceKind {
    Manual,
    Edited,
    Transformed,
    AiTool,
}

#[derive(Debug, Clone)]
pub struct ClipTransformStamp {
    pub transform_id: String,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct ClipProvenance {
    pub kind: ClipProvenanceKind,
    pub from_clip_id: Option<String>,
    pub transform_snapshot: Vec<ClipTransformStamp>,
    pub pipeline_name: Option<String>,
    pub grant_name: Option<String>,
}

/// Rule, expiry, AI and provenance state of one clip.
#[derive(Debug, Clone)]
pub struct ClipAnnotationState {
    pub expires_at_ms: Option<i64>,
    pub hidden_from_ai: bool,
    pub rule_secret: bool,
    pub provenance: Option<ClipProvenance>,
    pub rule_tag_ids: Vec<String>,
    pub keep: bool,
}

impl From<copypaste_ipc::ItemAnnotations> for ClipAnnotationState {
    fn from(value: copypaste_ipc::ItemAnnotations) -> Self {
        use copypaste_ipc::payload::ProvenanceKind;
        Self {
            expires_at_ms: value.expires_at,
            hidden_from_ai: value.hidden_from_ai,
            rule_secret: value.rule_secret,
            provenance: value.provenance.map(|provenance| ClipProvenance {
                kind: match provenance.kind {
                    ProvenanceKind::Manual => ClipProvenanceKind::Manual,
                    ProvenanceKind::Edited => ClipProvenanceKind::Edited,
                    ProvenanceKind::Transformed => ClipProvenanceKind::Transformed,
                    ProvenanceKind::AiTool => ClipProvenanceKind::AiTool,
                },
                from_clip_id: provenance.from_clip_id,
                transform_snapshot: provenance
                    .transform_snapshot
                    .into_iter()
                    .map(|stamp| ClipTransformStamp {
                        transform_id: stamp.transform_id,
                        version: stamp.version,
                    })
                    .collect(),
                pipeline_name: provenance.pipeline_name,
                grant_name: provenance.grant_name,
            }),
            rule_tag_ids: value.rule_tag_ids,
            keep: value.keep,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClipAnnotationEntry {
    pub id: String,
    pub annotations: ClipAnnotationState,
}

fn clip_op(op: ClipOp) -> Method {
    Method::Clips { op }
}

/// A new text clip attributed to CopyPaste with a manual marker.
pub async fn create_manual_clip(text: String) -> Result<Clip, RuntimeError> {
    item_response(clip_op(ClipOp::CreateManual { text })).await
}

/// Save an edited copy as a new clip; optionally delete the original.
pub async fn save_clip_edit(
    id: String,
    text: String,
    replace_original: bool,
) -> Result<Clip, RuntimeError> {
    item_response(clip_op(ClipOp::SaveEdit {
        id,
        text,
        replace_original,
    }))
    .await
}

pub async fn clip_annotations(ids: Vec<String>) -> Result<Vec<ClipAnnotationEntry>, RuntimeError> {
    let entries = expect_data!(
        clip_op(ClipOp::Annotations { ids }),
        ResponseData::ClipAnnotations
    );
    Ok(entries
        .into_iter()
        .map(|entry| ClipAnnotationEntry {
            id: entry.id,
            annotations: entry.annotations.into(),
        })
        .collect())
}

/// Un-hiding is accepted only from the CopyPaste app.
pub async fn set_clips_hidden_from_ai(ids: Vec<String>, hidden: bool) -> Result<(), RuntimeError> {
    empty_response(clip_op(ClipOp::SetHiddenFromAi { ids, hidden })).await
}

/// Clear any expiry and keep the clip.
pub async fn keep_clip(id: String) -> Result<Clip, RuntimeError> {
    item_response(clip_op(ClipOp::Keep { id })).await
}

pub async fn clip_rule_trace(id: String) -> Result<RuleTraceView, RuntimeError> {
    let trace = expect_data!(clip_op(ClipOp::Trace { id }), ResponseData::RuleTrace);
    Ok(rule_trace(trace))
}

pub async fn latest_clip_from_other_device(copy: bool) -> Result<Option<Clip>, RuntimeError> {
    let item = expect_data!(
        clip_op(ClipOp::LatestFromOtherDevice { copy }),
        ResponseData::MaybeItem
    );
    Ok(item.map(clip))
}

/// Restore the clipboard held before `token`'s write, if it is still ours.
pub async fn restore_clipboard(token: String, after_ms: u32) -> Result<(), RuntimeError> {
    empty_response(clip_op(ClipOp::RestoreClipboard { token, after_ms })).await
}

/// Deliver text that is not a stored clip, without creating a history row.
pub async fn deliver_text(
    text: String,
    secret: bool,
    delivery: TextDelivery,
) -> Result<DeliveredText, RuntimeError> {
    let delivered = expect_data!(
        clip_op(ClipOp::DeliverText {
            text,
            secret,
            delivery: delivery.into(),
        }),
        ResponseData::Delivered
    );
    Ok(delivered.into())
}
