//! Snippets, collections, templates, abbreviation search and snippet
//! import/export.

use std::collections::BTreeMap;

use copypaste_ipc::snippets::{self as ipc, SnippetOp};
use copypaste_ipc::{Method, ResponseData};

use super::clips::{DeliveredText, TextDelivery};
use super::{empty_response, expect_data, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetScopeKind {
    All,
    Unfiled,
    Collection,
    Pinned,
    RecentlyDeleted,
}

#[derive(Debug, Clone)]
pub struct SnippetSummaryView {
    pub id: String,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub concealed: bool,
    pub preview: String,
    pub created_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
    pub duplicate_abbreviation: bool,
    pub conflict_copy: bool,
    pub last_used_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SnippetPageView {
    pub snippets: Vec<SnippetSummaryView>,
    pub next_cursor: Option<String>,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateFieldKind {
    Text,
    Multiline,
    Choice,
}

#[derive(Debug, Clone)]
pub struct TemplateFieldView {
    pub name: String,
    pub label: String,
    pub kind: TemplateFieldKind,
    pub default_value: Option<String>,
    pub options: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateLintSeverity {
    Error,
    Warning,
    Hint,
}

/// Offsets are UTF-8 byte offsets into the body.
#[derive(Debug, Clone)]
pub struct TemplateLintView {
    pub severity: TemplateLintSeverity,
    pub code: String,
    pub start: u32,
    pub end: u32,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnippetDetailView {
    pub snippet: SnippetSummaryView,
    pub body_text: String,
    pub body_rich: Option<String>,
    pub revision: String,
    pub fields: Vec<TemplateFieldView>,
    pub lints: Vec<TemplateLintView>,
}

#[derive(Debug, Clone)]
pub struct SnippetDraftInput {
    pub id: Option<String>,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub body_text: String,
    pub body_rich: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub concealed: bool,
    pub base_revision: Option<String>,
}

/// Either `saved` or the live snippet that owns the abbreviation.
#[derive(Debug, Clone)]
pub struct SnippetSaveResult {
    pub saved: Option<SnippetDetailView>,
    pub abbreviation_taken_by_id: Option<String>,
    pub abbreviation_taken_by_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnippetCollectionView {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub accent: Option<String>,
    pub order_key: String,
    pub deleted_at_ms: Option<i64>,
    pub snippet_count: u32,
}

#[derive(Debug, Clone)]
pub struct SnippetCollectionDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: Option<String>,
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetMatchTier {
    ExactAbbreviation,
    AbbreviationPrefix,
    Title,
    FullText,
}

#[derive(Debug, Clone)]
pub struct SnippetHitView {
    pub snippet: SnippetSummaryView,
    pub tier: SnippetMatchTier,
    pub duplicate: bool,
}

/// One template field value.
#[derive(Debug, Clone)]
pub struct TemplateValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateClipboardKind {
    /// Read the current clipboard once, at the start of expansion.
    Host,
    Provided,
    None,
}

/// `text` is used only with [`TemplateClipboardKind::Provided`].
#[derive(Debug, Clone)]
pub struct TemplateClipboard {
    pub kind: TemplateClipboardKind,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RenderedSnippet {
    pub text: String,
    pub cursor_graphemes_from_end: Option<u32>,
    pub warnings: Vec<TemplateLintView>,
    pub missing_required: Vec<String>,
    pub unfilled: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetImportFormat {
    Bundle,
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetImportRowOutcome {
    New,
    Identical,
    Updated,
    AbbreviationConflict,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct SnippetImportRow {
    pub index: u32,
    pub outcome: SnippetImportRowOutcome,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnippetImportCounts {
    pub new_rows: u32,
    pub identical_rows: u32,
    pub updated: u32,
    pub abbreviation_conflicts: u32,
    pub invalid: u32,
    pub collections: u32,
}

#[derive(Debug, Clone)]
pub struct SnippetImportPlanView {
    pub token: String,
    pub counts: SnippetImportCounts,
    pub rows: Vec<SnippetImportRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetSameIdPolicy {
    KeepBoth,
    Replace,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetAbbreviationPolicy {
    ImportWithout,
    RenameWithSuffix,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetCollectionNamePolicy {
    Merge,
    CreateNew,
}

#[derive(Debug, Clone)]
pub struct SnippetImportPolicies {
    pub same_id_different: SnippetSameIdPolicy,
    pub abbreviation_conflict: SnippetAbbreviationPolicy,
    pub collection_name: SnippetCollectionNamePolicy,
}

#[derive(Debug, Clone)]
pub struct SnippetImportReportView {
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub renamed_abbreviations: u32,
    pub without_abbreviation: u32,
    pub collections_created: u32,
    pub unrecognized_placeholders: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetExportScopeKind {
    All,
    Collections,
    Snippets,
}

fn snippet_op(op: SnippetOp) -> Method {
    Method::Snippets { op }
}

fn summary(value: ipc::SnippetSummary) -> SnippetSummaryView {
    SnippetSummaryView {
        id: value.id,
        collection_id: value.collection_id,
        title: value.title,
        abbreviation: value.abbreviation,
        template: value.template,
        pinned: value.pinned,
        concealed: value.concealed,
        preview: value.preview,
        created_at_ms: value.created_at,
        deleted_at_ms: value.deleted_at,
        duplicate_abbreviation: value.duplicate_abbreviation,
        conflict_copy: value.conflict_copy,
        last_used_at_ms: value.last_used_at,
    }
}

fn lint(value: ipc::TemplateLint) -> TemplateLintView {
    TemplateLintView {
        severity: match value.severity {
            ipc::LintSeverity::Error => TemplateLintSeverity::Error,
            ipc::LintSeverity::Warning => TemplateLintSeverity::Warning,
            ipc::LintSeverity::Hint => TemplateLintSeverity::Hint,
        },
        code: value.code,
        start: value.start,
        end: value.end,
        detail: value.detail,
    }
}

fn detail(value: ipc::SnippetDetail) -> SnippetDetailView {
    SnippetDetailView {
        snippet: summary(value.snippet),
        body_text: value.body_text,
        body_rich: value.body_rich,
        revision: value.revision,
        fields: value
            .fields
            .into_iter()
            .map(|field| TemplateFieldView {
                name: field.name,
                label: field.label,
                kind: match field.kind {
                    ipc::TemplateFieldKind::Text => TemplateFieldKind::Text,
                    ipc::TemplateFieldKind::Multiline => TemplateFieldKind::Multiline,
                    ipc::TemplateFieldKind::Choice => TemplateFieldKind::Choice,
                },
                default_value: field.default,
                options: field.options,
                required: field.required,
            })
            .collect(),
        lints: value.lints.into_iter().map(lint).collect(),
    }
}

fn collection(value: ipc::SnippetCollection) -> SnippetCollectionView {
    SnippetCollectionView {
        id: value.id,
        name: value.name,
        display_name: value.display_name,
        icon: value.icon,
        accent: value.accent,
        order_key: value.order_key,
        deleted_at_ms: value.deleted_at,
        snippet_count: value.snippet_count,
    }
}

pub(crate) fn template_values(values: Vec<TemplateValue>) -> BTreeMap<String, String> {
    values
        .into_iter()
        .map(|value| (value.name, value.value))
        .collect()
}

fn clipboard_input(value: TemplateClipboard) -> Result<ipc::ClipboardInput, RuntimeError> {
    Ok(match (value.kind, value.text) {
        (TemplateClipboardKind::Host, _) => ipc::ClipboardInput::Host,
        (TemplateClipboardKind::None, _) => ipc::ClipboardInput::None,
        (TemplateClipboardKind::Provided, Some(text)) => ipc::ClipboardInput::Provided { text },
        (TemplateClipboardKind::Provided, None) => return Err(RuntimeError::internal()),
    })
}

fn ids_or_internal<T>(value: Option<T>) -> Result<T, RuntimeError> {
    value.ok_or_else(RuntimeError::internal)
}

/// List one scope; `collection_id` is required for [`SnippetScopeKind::Collection`].
pub async fn list_snippets(
    scope: SnippetScopeKind,
    collection_id: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> Result<SnippetPageView, RuntimeError> {
    let scope = match scope {
        SnippetScopeKind::All => ipc::SnippetScope::All,
        SnippetScopeKind::Unfiled => ipc::SnippetScope::Unfiled,
        SnippetScopeKind::Collection => ipc::SnippetScope::Collection {
            id: ids_or_internal(collection_id)?,
        },
        SnippetScopeKind::Pinned => ipc::SnippetScope::Pinned,
        SnippetScopeKind::RecentlyDeleted => ipc::SnippetScope::RecentlyDeleted,
    };
    let page = expect_data!(
        snippet_op(SnippetOp::List {
            scope,
            cursor,
            limit,
        }),
        ResponseData::SnippetPage
    );
    Ok(SnippetPageView {
        snippets: page.snippets.into_iter().map(summary).collect(),
        next_cursor: page.next_cursor,
        total: page.total,
    })
}

pub async fn get_snippet(id: String) -> Result<SnippetDetailView, RuntimeError> {
    let value = expect_data!(snippet_op(SnippetOp::Get { id }), ResponseData::Snippet);
    Ok(detail(value))
}

pub async fn save_snippet(draft: SnippetDraftInput) -> Result<SnippetSaveResult, RuntimeError> {
    let outcome = expect_data!(
        snippet_op(SnippetOp::Save {
            draft: ipc::SnippetDraft {
                id: draft.id,
                collection_id: draft.collection_id,
                title: draft.title,
                abbreviation: draft.abbreviation,
                body_text: draft.body_text,
                body_rich: draft.body_rich,
                template: draft.template,
                pinned: draft.pinned,
                concealed: draft.concealed,
                base_revision: draft.base_revision,
            },
        }),
        ResponseData::SnippetSave
    );
    Ok(match outcome {
        ipc::SnippetSaveOutcome::Saved(value) => SnippetSaveResult {
            saved: Some(detail(*value)),
            abbreviation_taken_by_id: None,
            abbreviation_taken_by_title: None,
        },
        ipc::SnippetSaveOutcome::AbbreviationTaken { snippet_id, title } => SnippetSaveResult {
            saved: None,
            abbreviation_taken_by_id: Some(snippet_id),
            abbreviation_taken_by_title: Some(title),
        },
    })
}

pub async fn save_snippet_from_clip(
    clip_id: String,
    collection_id: Option<String>,
) -> Result<SnippetDetailView, RuntimeError> {
    let value = expect_data!(
        snippet_op(SnippetOp::SaveFromClip {
            clip_id,
            collection_id,
        }),
        ResponseData::Snippet
    );
    Ok(detail(value))
}

pub async fn delete_snippets(ids: Vec<String>) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Delete { ids })).await
}

pub async fn restore_snippets(ids: Vec<String>) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Restore { ids })).await
}

pub async fn purge_snippets(ids: Vec<String>) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Purge { ids })).await
}

pub async fn move_snippets(
    ids: Vec<String>,
    collection_id: Option<String>,
) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Move { ids, collection_id })).await
}

pub async fn reorder_snippet(id: String, after_id: Option<String>) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Reorder { id, after_id })).await
}

pub async fn set_snippet_pinned(
    id: String,
    pinned: bool,
    after_id: Option<String>,
) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::SetPinned {
        id,
        pinned,
        after_id,
    }))
    .await
}

/// Returns how many references were rewritten.
pub async fn rename_snippet_abbreviation(
    id: String,
    abbreviation: String,
    update_references: bool,
) -> Result<u64, RuntimeError> {
    Ok(expect_data!(
        snippet_op(SnippetOp::RenameAbbreviation {
            id,
            abbreviation,
            update_references,
        }),
        ResponseData::Count
    ))
}

pub async fn snippet_collections() -> Result<Vec<SnippetCollectionView>, RuntimeError> {
    let values = expect_data!(
        snippet_op(SnippetOp::Collections),
        ResponseData::SnippetCollections
    );
    Ok(values.into_iter().map(collection).collect())
}

pub async fn save_snippet_collection(
    draft: SnippetCollectionDraft,
) -> Result<SnippetCollectionView, RuntimeError> {
    let value = expect_data!(
        snippet_op(SnippetOp::SaveCollection {
            draft: ipc::CollectionDraft {
                id: draft.id,
                name: draft.name,
                icon: draft.icon,
                accent: draft.accent,
            },
        }),
        ResponseData::SnippetCollection
    );
    Ok(collection(value))
}

pub async fn delete_snippet_collection(
    id: String,
    delete_snippets: bool,
) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::DeleteCollection {
        id,
        delete_snippets,
    }))
    .await
}

pub async fn merge_snippet_collections(from: String, into: String) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::MergeCollections { from, into })).await
}

pub async fn lint_snippet_body(
    body: String,
    template: bool,
) -> Result<Vec<TemplateLintView>, RuntimeError> {
    let lints = expect_data!(
        snippet_op(SnippetOp::Lint { body, template }),
        ResponseData::TemplateLints
    );
    Ok(lints.into_iter().map(lint).collect())
}

pub async fn quick_search_snippets(
    query: String,
    include_snippets: bool,
    limit: u32,
) -> Result<Vec<SnippetHitView>, RuntimeError> {
    let hits = expect_data!(
        snippet_op(SnippetOp::QuickSearch {
            query,
            include_snippets,
            limit,
        }),
        ResponseData::SnippetHits
    );
    Ok(hits
        .into_iter()
        .map(|hit| SnippetHitView {
            snippet: summary(hit.snippet),
            tier: match hit.tier {
                ipc::SnippetMatchTier::ExactAbbreviation => SnippetMatchTier::ExactAbbreviation,
                ipc::SnippetMatchTier::AbbreviationPrefix => SnippetMatchTier::AbbreviationPrefix,
                ipc::SnippetMatchTier::Title => SnippetMatchTier::Title,
                ipc::SnippetMatchTier::FullText => SnippetMatchTier::FullText,
            },
            duplicate: hit.duplicate,
        })
        .collect())
}

/// Render a saved snippet (`snippet_id`) or an unsaved body (`body`); exactly
/// one must be set. `locale` is the app's BCP 47 locale.
pub async fn render_snippet(
    snippet_id: Option<String>,
    body: Option<String>,
    values: Vec<TemplateValue>,
    clipboard: TemplateClipboard,
    final_render: bool,
    locale: Option<String>,
) -> Result<RenderedSnippet, RuntimeError> {
    let source = match (snippet_id, body) {
        (Some(id), None) => ipc::RenderSource::Snippet { id },
        (None, Some(text)) => ipc::RenderSource::Body { text },
        _ => return Err(RuntimeError::internal()),
    };
    let rendered = expect_data!(
        snippet_op(SnippetOp::Render {
            source,
            values: template_values(values),
            clipboard: clipboard_input(clipboard)?,
            purpose: if final_render {
                ipc::RenderPurpose::Final
            } else {
                ipc::RenderPurpose::Preview
            },
            locale,
        }),
        ResponseData::Rendered
    );
    Ok(RenderedSnippet {
        text: rendered.text,
        cursor_graphemes_from_end: rendered.cursor_graphemes_from_end,
        warnings: rendered.warnings.into_iter().map(lint).collect(),
        missing_required: rendered.missing_required,
        unfilled: rendered.unfilled,
    })
}

pub async fn use_snippet(
    id: String,
    values: Vec<TemplateValue>,
    clipboard: TemplateClipboard,
    delivery: TextDelivery,
    locale: Option<String>,
) -> Result<DeliveredText, RuntimeError> {
    let delivered = expect_data!(
        snippet_op(SnippetOp::Use {
            id,
            values: template_values(values),
            clipboard: clipboard_input(clipboard)?,
            delivery: delivery.into(),
            locale,
        }),
        ResponseData::Delivered
    );
    Ok(delivered.into())
}

/// Release the template cache and any import plan when a surface closes.
pub async fn release_snippet_resources() -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::Release)).await
}

pub async fn preview_snippet_import(
    path: String,
    passphrase: Option<String>,
    format: SnippetImportFormat,
    convert_placeholders: bool,
) -> Result<SnippetImportPlanView, RuntimeError> {
    let plan = expect_data!(
        snippet_op(SnippetOp::ImportPreview {
            path,
            passphrase,
            format: match format {
                SnippetImportFormat::Bundle => ipc::ImportFormat::Bundle,
                SnippetImportFormat::Json => ipc::ImportFormat::Json,
                SnippetImportFormat::Csv => ipc::ImportFormat::Csv,
            },
            convert_placeholders,
        }),
        ResponseData::SnippetImportPlan
    );
    Ok(SnippetImportPlanView {
        token: plan.token,
        counts: SnippetImportCounts {
            new_rows: plan.counts.new,
            identical_rows: plan.counts.identical,
            updated: plan.counts.updated,
            abbreviation_conflicts: plan.counts.abbreviation_conflicts,
            invalid: plan.counts.invalid,
            collections: plan.counts.collections,
        },
        rows: plan
            .rows
            .into_iter()
            .map(|row| SnippetImportRow {
                index: row.index,
                outcome: match row.outcome {
                    ipc::ImportRowOutcome::New => SnippetImportRowOutcome::New,
                    ipc::ImportRowOutcome::Identical => SnippetImportRowOutcome::Identical,
                    ipc::ImportRowOutcome::Updated => SnippetImportRowOutcome::Updated,
                    ipc::ImportRowOutcome::AbbreviationConflict => {
                        SnippetImportRowOutcome::AbbreviationConflict
                    }
                    ipc::ImportRowOutcome::Invalid => SnippetImportRowOutcome::Invalid,
                },
                reason: row.reason,
            })
            .collect(),
    })
}

pub async fn commit_snippet_import(
    token: String,
    policies: SnippetImportPolicies,
) -> Result<SnippetImportReportView, RuntimeError> {
    let policies = ipc::ImportPolicies {
        same_id_different: match policies.same_id_different {
            SnippetSameIdPolicy::KeepBoth => ipc::SameIdPolicy::KeepBoth,
            SnippetSameIdPolicy::Replace => ipc::SameIdPolicy::Replace,
            SnippetSameIdPolicy::Skip => ipc::SameIdPolicy::Skip,
        },
        abbreviation_conflict: match policies.abbreviation_conflict {
            SnippetAbbreviationPolicy::ImportWithout => {
                ipc::AbbreviationConflictPolicy::ImportWithout
            }
            SnippetAbbreviationPolicy::RenameWithSuffix => {
                ipc::AbbreviationConflictPolicy::RenameWithSuffix
            }
            SnippetAbbreviationPolicy::Skip => ipc::AbbreviationConflictPolicy::Skip,
        },
        collection_name: match policies.collection_name {
            SnippetCollectionNamePolicy::Merge => ipc::CollectionNamePolicy::Merge,
            SnippetCollectionNamePolicy::CreateNew => ipc::CollectionNamePolicy::CreateNew,
        },
    };
    let report = expect_data!(
        snippet_op(SnippetOp::ImportCommit { token, policies }),
        ResponseData::SnippetImportReport
    );
    Ok(SnippetImportReportView {
        inserted: report.inserted,
        updated: report.updated,
        skipped: report.skipped,
        renamed_abbreviations: report.renamed_abbreviations,
        without_abbreviation: report.without_abbreviation,
        collections_created: report.collections_created,
        unrecognized_placeholders: report.unrecognized_placeholders,
    })
}

pub async fn cancel_snippet_import(token: String) -> Result<(), RuntimeError> {
    empty_response(snippet_op(SnippetOp::ImportCancel { token })).await
}

/// Export snippets; `ids` names collections or snippets for those scopes.
/// Returns how many snippets were written.
pub async fn export_snippets(
    path: String,
    scope: SnippetExportScopeKind,
    ids: Vec<String>,
    passphrase: Option<String>,
    include_concealed: bool,
) -> Result<u64, RuntimeError> {
    let scope = match scope {
        SnippetExportScopeKind::All => ipc::ExportScope::All,
        SnippetExportScopeKind::Collections => ipc::ExportScope::Collections { ids },
        SnippetExportScopeKind::Snippets => ipc::ExportScope::Snippets { ids },
    };
    Ok(expect_data!(
        snippet_op(SnippetOp::Export {
            path,
            scope,
            passphrase,
            include_concealed,
        }),
        ResponseData::Count
    ))
}
