//! Tool calls as the MCP surface parses them and the results they produce.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ToolCall {
    ListRecent {
        limit: Option<u32>,
        kind: Option<String>,
        source_app: Option<String>,
        source_device: Option<String>,
        #[serde(default)]
        pinned_only: bool,
        cursor: Option<String>,
    },
    SearchHistory {
        query: String,
        kind: Option<String>,
        source_app: Option<String>,
        source_device: Option<String>,
        since: Option<String>,
        until: Option<String>,
        #[serde(default)]
        pinned_only: bool,
        limit: Option<u32>,
        cursor: Option<String>,
    },
    GetItem {
        id: Option<String>,
        offset: Option<u64>,
    },
    CopyToClipboard {
        id: Option<String>,
        text: Option<String>,
    },
    SearchSnippets {
        query: String,
        limit: Option<u32>,
    },
    GetSnippet {
        id: String,
    },
    RenderSnippet {
        id: String,
        #[serde(default)]
        values: BTreeMap<String, String>,
    },
    SetPinned {
        id: String,
        pinned: bool,
    },
    SaveSnippet {
        title: String,
        body: String,
        abbreviation: Option<String>,
    },
}

impl ToolCall {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::ListRecent { .. } => "list_recent",
            Self::SearchHistory { .. } => "search_history",
            Self::GetItem { .. } => "get_item",
            Self::CopyToClipboard { .. } => "copy_to_clipboard",
            Self::SearchSnippets { .. } => "search_snippets",
            Self::GetSnippet { .. } => "get_snippet",
            Self::RenderSnippet { .. } => "render_snippet",
            Self::SetPinned { .. } => "set_pinned",
            Self::SaveSnippet { .. } => "save_snippet",
        }
    }

    pub(crate) fn returns_content(&self) -> bool {
        !matches!(
            self,
            Self::CopyToClipboard { .. } | Self::SetPinned { .. } | Self::SaveSnippet { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) mime_type: String,
    pub(crate) data_base64: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolResult {
    pub(crate) is_error: bool,
    pub(crate) text: String,
    pub(crate) image: Option<Image>,
    pub(crate) structured: serde_json::Value,
    pub(crate) retry_after_secs: Option<u32>,
}
