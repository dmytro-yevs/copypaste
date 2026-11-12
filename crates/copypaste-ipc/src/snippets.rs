//! Snippets, collections, templates, abbreviation search and snippet
//! import/export.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::clips::Delivery;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SnippetScope {
    All,
    Unfiled,
    Collection { id: String },
    Pinned,
    RecentlyDeleted,
}

/// A snippet as listed. `preview` is empty for a concealed snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetSummary {
    pub id: String,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub concealed: bool,
    pub preview: String,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    /// Another live snippet shares the normalized abbreviation.
    #[serde(default)]
    pub duplicate_abbreviation: bool,
    /// Created from a concurrent body edit on another device.
    #[serde(default)]
    pub conflict_copy: bool,
    /// Local-only ranking signal.
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetPage {
    pub snippets: Vec<SnippetSummary>,
    pub next_cursor: Option<String>,
    /// Live snippets in the whole set, for the soft-limit warning.
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateFieldKind {
    Text,
    Multiline,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateField {
    pub name: String,
    pub label: String,
    pub kind: TemplateFieldKind,
    pub default: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    Error,
    Warning,
    Hint,
}

/// One template diagnostic. Offsets are UTF-8 byte offsets into the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateLint {
    pub severity: LintSeverity,
    pub code: String,
    pub start: u32,
    pub end: u32,
    /// Display argument for the message, for example a cycle or a field name.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetDetail {
    pub snippet: SnippetSummary,
    pub body_text: String,
    pub body_rich: Option<String>,
    /// Pass back as `base_revision` when saving an edit of this body.
    pub revision: String,
    pub fields: Vec<TemplateField>,
    pub lints: Vec<TemplateLint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetDraft {
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

/// A save either succeeds or names the live snippet that owns the
/// abbreviation, so the editor can offer "Go to snippet".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SnippetSaveOutcome {
    Saved(Box<SnippetDetail>),
    AbbreviationTaken { snippet_id: String, title: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetCollection {
    pub id: String,
    pub name: String,
    /// `name`, or `name (2)` after a concurrent-creation collision.
    pub display_name: String,
    pub icon: Option<String>,
    pub accent: Option<String>,
    pub order_key: String,
    pub deleted_at: Option<i64>,
    pub snippet_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: Option<String>,
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnippetMatchTier {
    ExactAbbreviation,
    AbbreviationPrefix,
    Title,
    FullText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetHit {
    pub snippet: SnippetSummary,
    pub tier: SnippetMatchTier,
    /// Several live snippets share this exact abbreviation; none is preselected.
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenderSource {
    Snippet { id: String },
    Body { text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClipboardInput {
    /// Read the current clipboard text once, at the start of expansion.
    Host,
    Provided {
        text: String,
    },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderPurpose {
    Preview,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub text: String,
    pub cursor_graphemes_from_end: Option<u32>,
    pub warnings: Vec<TemplateLint>,
    /// Empty required inputs: a field name, or `clipboard:<index>`.
    pub missing_required: Vec<String>,
    /// Placeholders left verbatim because they could not be filled.
    pub unfilled: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Bundle,
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRowOutcome {
    New,
    Identical,
    Updated,
    AbbreviationConflict,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRow {
    pub index: u32,
    pub outcome: ImportRowOutcome,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCounts {
    pub new: u32,
    pub identical: u32,
    pub updated: u32,
    pub abbreviation_conflicts: u32,
    pub invalid: u32,
    pub collections: u32,
}

/// A parsed, not yet written import. `token` is valid until commit, cancel,
/// `Release` or its deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPlan {
    pub token: String,
    pub counts: ImportCounts,
    pub rows: Vec<ImportRow>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SameIdPolicy {
    #[default]
    KeepBoth,
    Replace,
    Skip,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbbreviationConflictPolicy {
    #[default]
    ImportWithout,
    RenameWithSuffix,
    Skip,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionNamePolicy {
    #[default]
    Merge,
    CreateNew,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportPolicies {
    pub same_id_different: SameIdPolicy,
    pub abbreviation_conflict: AbbreviationConflictPolicy,
    pub collection_name: CollectionNamePolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub renamed_abbreviations: u32,
    pub without_abbreviation: u32,
    pub collections_created: u32,
    pub unrecognized_placeholders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportScope {
    All,
    Collections { ids: Vec<String> },
    Snippets { ids: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SnippetOp {
    List {
        scope: SnippetScope,
        cursor: Option<String>,
        limit: u32,
    },
    Get {
        id: String,
    },
    Save {
        draft: SnippetDraft,
    },
    SaveFromClip {
        clip_id: String,
        collection_id: Option<String>,
    },
    Delete {
        ids: Vec<String>,
    },
    Restore {
        ids: Vec<String>,
    },
    Purge {
        ids: Vec<String>,
    },
    Move {
        ids: Vec<String>,
        collection_id: Option<String>,
    },
    Reorder {
        id: String,
        after_id: Option<String>,
    },
    SetPinned {
        id: String,
        pinned: bool,
        after_id: Option<String>,
    },
    RenameAbbreviation {
        id: String,
        abbreviation: String,
        update_references: bool,
    },
    Collections,
    SaveCollection {
        draft: CollectionDraft,
    },
    DeleteCollection {
        id: String,
        delete_snippets: bool,
    },
    MergeCollections {
        from: String,
        into: String,
    },
    Lint {
        body: String,
        template: bool,
    },
    QuickSearch {
        query: String,
        include_snippets: bool,
        limit: u32,
    },
    Render {
        source: RenderSource,
        #[serde(default)]
        values: BTreeMap<String, String>,
        clipboard: ClipboardInput,
        purpose: RenderPurpose,
        /// BCP 47 app locale, the default for date placeholders.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
    },
    Use {
        id: String,
        #[serde(default)]
        values: BTreeMap<String, String>,
        clipboard: ClipboardInput,
        delivery: Delivery,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
    },
    /// Drop the template parse cache and any outstanding import plan.
    Release,
    ImportPreview {
        path: String,
        passphrase: Option<String>,
        format: ImportFormat,
        convert_placeholders: bool,
    },
    ImportCommit {
        token: String,
        #[serde(default)]
        policies: ImportPolicies,
    },
    ImportCancel {
        token: String,
    },
    Export {
        path: String,
        scope: ExportScope,
        passphrase: Option<String>,
        include_concealed: bool,
    },
}

impl crate::OpGates for SnippetOp {
    fn requires_ready(&self) -> bool {
        true
    }

    fn interactive(&self) -> bool {
        matches!(
            self,
            Self::QuickSearch { .. } | Self::Render { .. } | Self::Use { .. } | Self::Release
        )
    }

    fn blocking_weight(&self) -> u32 {
        match self {
            Self::ImportPreview { .. } | Self::ImportCommit { .. } | Self::Export { .. } => {
                crate::limits::frame_weight_units(2)
            }
            Self::List { .. }
            | Self::Get { .. }
            | Self::Save { .. }
            | Self::SaveFromClip { .. }
            | Self::QuickSearch { .. }
            | Self::Render { .. }
            | Self::Use { .. }
            | Self::Lint { .. } => crate::limits::content_weight_units(4),
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        matches!(
            self,
            Self::ImportPreview { .. } | Self::ImportCommit { .. } | Self::Export { .. }
        )
    }

    fn privileged(&self) -> bool {
        false
    }
}
