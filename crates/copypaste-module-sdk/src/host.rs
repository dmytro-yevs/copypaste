//! Host services a module calls through `HostClient::call`, each gated by a
//! manifest capability the user confirmed, and the host-to-module deliveries
//! of endpoint connections. See `CONTRACT-v6.md`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Host services a module may request. Declared in the manifest and confirmed
/// by the user when the module is enabled; anything undeclared is refused.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModuleCapability {
    HistoryRead,
    ClipboardWrite,
    LibraryWrite,
    Snippets,
    SyncedRecords,
    LocalState,
    Prompts,
    Endpoints,
}

/// A long-lived connection a local client may open with `ModuleOperation::Connect`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleEndpoint {
    pub id: String,
    pub platforms: Vec<crate::ModulePlatform>,
}

pub const MAX_ENDPOINTS: usize = 8;
/// Open connections per module; further `Connect` requests are refused as busy.
pub const MAX_ENDPOINT_CONNECTIONS: usize = 8;
/// UTF-8 bytes of one frame's `data`, in either direction.
pub const MAX_ENDPOINT_FRAME_BYTES: usize = 1024 * 1024;
/// Encoded `ModuleDelivery` bound; covers worst-case JSON escaping of a frame.
pub const MAX_DELIVERY_BYTES: usize = 8 * MAX_ENDPOINT_FRAME_BYTES;
pub const MAX_HISTORY_PAGE: u32 = 100;
pub const HISTORY_PREVIEW_CHARS: usize = 500;
pub const HISTORY_TEXT_CHUNK_CHARS: usize = 16 * 1024;
pub const MAX_SNIPPET_PAGE: u32 = 50;
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_LABEL_CHARS: usize = 80;
pub const MAX_STATE_KEYS: usize = 64;
pub const MAX_STATE_VALUE_BYTES: usize = 128 * 1024;
pub const MAX_RECORD_KEY_BYTES: usize = 128;
pub const MAX_RECORD_VALUE_BYTES: usize = 64 * 1024;
/// Live (not deleted) records per module.
pub const MAX_RECORDS: usize = 1024;
pub const MAX_RECORD_PAGE: u32 = 256;
/// Record keys with this prefix belong to the host (synced preferences).
pub const RESERVED_RECORD_PREFIX: &str = "host.";
pub const MAX_PROMPT_CHOICES: usize = 4;
pub const MAX_PROMPT_TIMEOUT_SECS: u32 = 300;
/// Pending prompts per module; more are refused as busy.
pub const MAX_PENDING_PROMPTS: usize = 4;
/// `EventKind` wire name the host emits when the pending prompt set changes.
pub const MODULE_PROMPT_EVENT: &str = "module_prompt";

/// Module-side history filters. The host additionally and always hides
/// deleted, expired, secret, transient, credential and hidden-from-integrations
/// items before applying these.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryScope {
    pub include_other_devices: bool,
    pub excluded_devices: Vec<String>,
    pub excluded_source_apps: Vec<String>,
    /// Visible kinds; an empty list matches nothing.
    pub content_kinds: Vec<HistoryContentKind>,
    /// Unpinned items older than this are invisible; `None` is all time.
    pub lookback_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum HistoryContentKind {
    Text,
    Link,
    RichText,
    Image,
    File,
}

/// Content a module asks the host to place on the clipboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipboardSource {
    Text {
        text: String,
    },
    /// A history item visible in `scope`; also needs `history_read`.
    Item {
        id: String,
        scope: HistoryScope,
    },
}

/// The provenance the host records for module clipboard writes. `module_id`
/// is filled by the host, never taken from the module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleProvenance {
    pub module_id: String,
    pub label: String,
}

/// A host request. The doc of each variant names its success reply.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleHostRequest {
    /// → `ModuleHostContext`. No capability.
    Context,
    /// → `HistoryPage`.
    HistoryQuery {
        scope: HistoryScope,
        search: Option<String>,
        kind: Option<HistoryContentKind>,
        source_app: Option<String>,
        device: Option<String>,
        since_ms: Option<i64>,
        until_ms: Option<i64>,
        #[serde(default)]
        pinned_only: bool,
        limit: u32,
        cursor: Option<String>,
    },
    /// → `HistoryItem`; `NotFound` for invisible and unknown ids alike.
    HistoryGet {
        scope: HistoryScope,
        id: String,
        #[serde(default)]
        offset: u64,
    },
    /// → `HistoryFacets` of items visible in `scope`.
    HistoryFacets { scope: HistoryScope },
    /// → `Vec<DeviceName>` of every known device.
    DeviceNames,
    /// → `Vec<SnippetSummary>`; an empty query lists the most recent.
    SnippetSearch { query: String, limit: u32 },
    /// → `Snippet`.
    SnippetGet { id: String },
    /// → `RenderedSnippet`.
    SnippetRender {
        id: String,
        #[serde(default)]
        values: BTreeMap<String, String>,
    },
    /// → `SavedRecord`.
    SnippetSave {
        title: String,
        body: String,
        abbreviation: Option<String>,
    },
    /// → `null`.
    SetPinned {
        scope: HistoryScope,
        id: String,
        pinned: bool,
    },
    /// → `null`. Recorded with `ModuleProvenance { module_id, label }`.
    ClipboardWrite {
        source: ClipboardSource,
        label: String,
    },
    /// → `RecordPage` of live records, ordered by key.
    RecordList {
        prefix: Option<String>,
        after: Option<String>,
        limit: u32,
    },
    /// → `Option<ModuleRecord>`.
    RecordGet { key: String },
    /// → `null`.
    RecordPut { key: String, value_json: String },
    /// → `null`; leaves a synced tombstone.
    RecordDelete { key: String },
    /// → `BTreeMap<String, Option<String>>`. Same wire shape, limits and
    /// storage as the sync provider state requests.
    ReadState { keys: Vec<String> },
    /// → `null`.
    WriteState { values: BTreeMap<String, String> },
    /// → `null`.
    ClearState { keys: Vec<String> },
    /// → `PromptAnswer`. Blocks the calling thread until answered or expired.
    Prompt(PromptRequest),
    /// → `null`. Never blocks on the module instance; `Busy` when the
    /// connection's outbound queue is full.
    EndpointSend { connection: String, data: String },
    /// → `null`. The host also delivers `EndpointClose`.
    EndpointClose { connection: String },
}

impl ModuleHostRequest {
    /// Capabilities the module must hold. Sync providers additionally keep
    /// implicit access to the three state requests.
    #[must_use]
    pub fn required_capabilities(&self) -> &'static [ModuleCapability] {
        use ModuleCapability as C;
        match self {
            Self::Context => &[],
            Self::HistoryQuery { .. }
            | Self::HistoryGet { .. }
            | Self::HistoryFacets { .. }
            | Self::DeviceNames => &[C::HistoryRead],
            Self::SnippetSearch { .. } | Self::SnippetGet { .. } | Self::SnippetRender { .. } => {
                &[C::Snippets]
            }
            Self::SnippetSave { .. } => &[C::Snippets, C::LibraryWrite],
            Self::SetPinned { .. } => &[C::HistoryRead, C::LibraryWrite],
            Self::ClipboardWrite {
                source: ClipboardSource::Text { .. },
                ..
            } => &[C::ClipboardWrite],
            Self::ClipboardWrite { .. } => &[C::ClipboardWrite, C::HistoryRead],
            Self::RecordList { .. }
            | Self::RecordGet { .. }
            | Self::RecordPut { .. }
            | Self::RecordDelete { .. } => &[C::SyncedRecords],
            Self::ReadState { .. } | Self::WriteState { .. } | Self::ClearState { .. } => {
                &[C::LocalState]
            }
            Self::Prompt(_) => &[C::Prompts],
            Self::EndpointSend { .. } | Self::EndpointClose { .. } => &[C::Endpoints],
        }
    }

    /// Bounds checks shared by every host implementation.
    pub fn validate(&self) -> Result<(), HostError> {
        let valid = match self {
            Self::Context | Self::DeviceNames => true,
            Self::HistoryQuery {
                scope,
                search,
                limit,
                ..
            } => {
                scope.valid()
                    && (1..=MAX_HISTORY_PAGE).contains(limit)
                    && search.as_ref().is_none_or(|search| search.len() <= 1024)
            }
            Self::HistoryGet { scope, .. }
            | Self::HistoryFacets { scope }
            | Self::SetPinned { scope, .. } => scope.valid(),
            Self::SnippetSearch { query, limit } => {
                query.len() <= 1024 && (1..=MAX_SNIPPET_PAGE).contains(limit)
            }
            Self::SnippetGet { .. } => true,
            Self::SnippetRender { values, .. } => {
                values.len() <= 64 && values.values().all(|value| value.len() <= 64 * 1024)
            }
            Self::SnippetSave {
                title,
                body,
                abbreviation,
            } => {
                !title.trim().is_empty()
                    && title.chars().count() <= 160
                    && body.len() <= MAX_CLIPBOARD_TEXT_BYTES
                    && abbreviation.as_ref().is_none_or(|value| value.len() <= 64)
            }
            Self::ClipboardWrite { source, label } => {
                !label.trim().is_empty()
                    && label.chars().count() <= MAX_LABEL_CHARS
                    && match source {
                        ClipboardSource::Text { text } => text.len() <= MAX_CLIPBOARD_TEXT_BYTES,
                        ClipboardSource::Item { scope, .. } => scope.valid(),
                    }
            }
            Self::RecordList { prefix, limit, .. } => {
                (1..=MAX_RECORD_PAGE).contains(limit)
                    && prefix
                        .as_ref()
                        .is_none_or(|prefix| prefix.len() <= MAX_RECORD_KEY_BYTES)
            }
            Self::RecordGet { key } | Self::RecordDelete { key } => valid_record_key(key),
            Self::RecordPut { key, value_json } => {
                valid_record_key(key)
                    && value_json.len() <= MAX_RECORD_VALUE_BYTES
                    && serde_json::from_str::<serde::de::IgnoredAny>(value_json).is_ok()
            }
            Self::ReadState { keys } | Self::ClearState { keys } => {
                keys.len() <= MAX_STATE_KEYS && keys.iter().all(|key| valid_state_key(key))
            }
            Self::WriteState { values } => {
                values.len() <= MAX_STATE_KEYS
                    && values.iter().all(|(key, value)| {
                        valid_state_key(key) && value.len() <= MAX_STATE_VALUE_BYTES
                    })
            }
            Self::Prompt(prompt) => prompt.valid(),
            Self::EndpointSend { data, .. } => data.len() <= MAX_ENDPOINT_FRAME_BYTES,
            Self::EndpointClose { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(HostError::new(
                HostErrorCode::Invalid,
                "The host request exceeds its limits.",
            ))
        }
    }
}

impl HistoryScope {
    fn valid(&self) -> bool {
        self.excluded_devices.len() <= crate::MAX_SET_MEMBERS
            && self.excluded_source_apps.len() <= crate::MAX_SET_MEMBERS
            && self.content_kinds.len() <= 5
            && self.lookback_days.is_none_or(|days| days <= 36_500)
    }
}

/// Same rule as sync provider state keys.
#[must_use]
pub fn valid_state_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 96
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Module-chosen record keys; the host prefix is reserved.
#[must_use]
pub fn valid_record_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_RECORD_KEY_BYTES
        && key.as_bytes()[0].is_ascii_alphanumeric()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
        && !key.starts_with(RESERVED_RECORD_PREFIX)
}

/// The typed failure of a host request, carried as the error reply body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostError {
    pub code: HostErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostErrorCode {
    /// This host does not implement the request.
    Unsupported,
    /// The capability is not declared or not confirmed by the user.
    Denied,
    Invalid,
    NotFound,
    /// A bounded queue, prompt or connection slot is full; retry later.
    Busy,
    /// The service is stopped, disabled or failed.
    Unavailable,
}

impl HostError {
    pub fn new(code: HostErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleHostContext {
    /// The application UI locale, e.g. `uk` or `en-US`.
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryPage {
    pub items: Vec<HistoryEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    pub kind: HistoryContentKind,
    /// At most `HISTORY_PREVIEW_CHARS`; empty for images and files.
    pub preview: String,
    pub truncated: bool,
    pub pinned: bool,
    pub captured_at_ms: i64,
    pub source_app_id: Option<String>,
    pub source_app_name: Option<String>,
    pub device_id: String,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryItem {
    pub entry: HistoryEntry,
    pub content: HistoryContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HistoryContent {
    /// Up to `HISTORY_TEXT_CHUNK_CHARS` characters starting at `offset`.
    Text {
        text: String,
        offset: u64,
        total_chars: u64,
        next_offset: Option<u64>,
    },
    /// A bounded PNG preview, absent when it cannot be decoded.
    Image {
        width: Option<u32>,
        height: Option<u32>,
        png_base64: Option<String>,
    },
    /// `path` only for an available file captured on this device;
    /// otherwise `available_on` names the device holding it.
    File {
        name: Option<String>,
        mime_type: Option<String>,
        size_bytes: Option<u64>,
        path: Option<String>,
        available_on: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryFacets {
    pub source_apps: Vec<HistoryFacet>,
    pub devices: Vec<HistoryFacet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryFacet {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceName {
    pub id: String,
    pub name: String,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetSummary {
    pub id: String,
    pub title: String,
    pub abbreviation: Option<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub body: String,
    pub abbreviation: Option<String>,
    /// Placeholder names `SnippetRender` accepts values for.
    pub placeholders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderedSnippet {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedRecord {
    pub id: String,
}

/// A synced per-module record. `module_id` is set by the host; a module only
/// ever sees its own records. Values are opaque JSON to the host and are
/// end-to-end encrypted by the sync transports like other library records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleRecord {
    pub module_id: String,
    pub key: String,
    pub value_json: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordPage {
    pub records: Vec<ModuleRecord>,
    /// Pass as `after` to continue.
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PromptRequest {
    pub title: String,
    pub body: String,
    pub choices: Vec<PromptChoice>,
    pub timeout_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PromptChoice {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
}

impl PromptRequest {
    fn valid(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        !self.title.trim().is_empty()
            && self.title.chars().count() <= 160
            && self.body.chars().count() <= 4096
            && (1..=MAX_PROMPT_TIMEOUT_SECS).contains(&self.timeout_secs)
            && (1..=MAX_PROMPT_CHOICES).contains(&self.choices.len())
            && self.choices.iter().all(|choice| {
                crate::valid_id(&choice.id)
                    && ids.insert(&choice.id)
                    && !choice.title.trim().is_empty()
                    && choice.title.chars().count() <= 64
            })
    }
}

/// `choice` is `None` when the prompt was dismissed, expired, or the module
/// was disabled while it waited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptAnswer {
    pub choice: Option<String>,
}

/// A pending prompt as the application UI lists it (`ModuleOperation::Prompts`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModulePrompt {
    pub prompt_id: String,
    pub module_id: String,
    pub title: String,
    pub body: String,
    pub choices: Vec<PromptChoice>,
    pub expires_at_ms: i64,
}

/// Host-to-module messages through the `copypaste_module_deliver_v1` export.
/// Calls are serialized with `invoke` on the same instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleDelivery {
    /// `privileged` is true only when the peer is the verified CopyPaste app.
    EndpointOpen {
        endpoint: String,
        connection: String,
        privileged: bool,
    },
    EndpointFrame {
        connection: String,
        data: String,
    },
    /// Final for the connection, whoever closed it.
    EndpointClose {
        connection: String,
    },
}

/// One frame on an IPC connection after the `Connect` acknowledgement, in
/// either direction, carried in the transport's existing message framing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum EndpointFrame {
    Data {
        data: String,
    },
    /// Sent once by the host before it hangs up; a client simply hangs up.
    Close {
        reason: Option<String>,
    },
}

/// The `Connect` acknowledgement body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EndpointConnected {
    pub connection: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModuleOperation, ModuleOutput};
    use serde_json::json;

    fn scope() -> HistoryScope {
        HistoryScope {
            include_other_devices: false,
            excluded_devices: vec!["device".into()],
            excluded_source_apps: Vec::new(),
            content_kinds: vec![HistoryContentKind::Text, HistoryContentKind::Link],
            lookback_days: Some(7),
        }
    }

    #[test]
    fn host_requests_keep_stable_wire_names_gates_and_limits() {
        let state = ModuleHostRequest::WriteState {
            values: BTreeMap::from([("grant_1".into(), "v".into())]),
        };
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            json!({"operation": "write_state", "values": {"grant_1": "v"}}),
            "same wire shape as the sync provider state request"
        );
        let requests = [
            ModuleHostRequest::HistoryQuery {
                scope: scope(),
                search: Some("needle".into()),
                kind: None,
                source_app: None,
                device: None,
                since_ms: None,
                until_ms: None,
                pinned_only: false,
                limit: 20,
                cursor: None,
            },
            ModuleHostRequest::ClipboardWrite {
                source: ClipboardSource::Item {
                    id: "item".into(),
                    scope: scope(),
                },
                label: "Claude".into(),
            },
            ModuleHostRequest::Prompt(PromptRequest {
                title: "Allow access?".into(),
                body: "A client asks for history access.".into(),
                choices: vec![PromptChoice {
                    id: "deny".into(),
                    title: "Deny".into(),
                    destructive: true,
                }],
                timeout_secs: 120,
            }),
            ModuleHostRequest::EndpointSend {
                connection: "c1".into(),
                data: "{}".into(),
            },
        ];
        for request in requests {
            request.validate().unwrap();
            let encoded = serde_json::to_vec(&request).unwrap();
            assert_eq!(
                serde_json::from_slice::<ModuleHostRequest>(&encoded).unwrap(),
                request
            );
        }
        assert_eq!(
            serde_json::to_value(ModuleHostRequest::Context).unwrap(),
            json!({"operation": "context"})
        );
        assert_eq!(
            ModuleHostRequest::ClipboardWrite {
                source: ClipboardSource::Text { text: "x".into() },
                label: "x".into()
            }
            .required_capabilities(),
            [ModuleCapability::ClipboardWrite]
        );
        for invalid in [
            ModuleHostRequest::RecordPut {
                key: "host.preferences".into(),
                value_json: "{}".into(),
            },
            ModuleHostRequest::RecordPut {
                key: "grants/1".into(),
                value_json: "{".into(),
            },
            ModuleHostRequest::ReadState {
                keys: vec!["bad key".into()],
            },
            ModuleHostRequest::SnippetSearch {
                query: String::new(),
                limit: 0,
            },
        ] {
            assert_eq!(invalid.validate().unwrap_err().code, HostErrorCode::Invalid);
        }
    }

    #[test]
    fn operations_deliveries_and_views_round_trip_and_stay_compatible() {
        let legacy: ModuleOperation =
            serde_json::from_value(json!({"operation": "set_enabled", "id": "a", "enabled": true}))
                .unwrap();
        assert_eq!(
            serde_json::to_value(&legacy).unwrap(),
            json!({"operation": "set_enabled", "id": "a", "enabled": true})
        );
        assert!(legacy.privileged());
        let connect = ModuleOperation::Connect {
            id: "copypaste.ai-tools".into(),
            endpoint: "mcp".into(),
        };
        assert!(!connect.privileged());
        assert_eq!(
            serde_json::to_value(&connect).unwrap(),
            json!({"operation": "connect", "id": "copypaste.ai-tools", "endpoint": "mcp"})
        );
        let delivery = ModuleDelivery::EndpointOpen {
            endpoint: "mcp".into(),
            connection: "c1".into(),
            privileged: false,
        };
        let encoded = serde_json::to_string(&delivery).unwrap();
        assert_eq!(
            serde_json::from_str::<ModuleDelivery>(&encoded).unwrap(),
            delivery
        );
        assert_eq!(
            serde_json::to_value(EndpointFrame::Close { reason: None }).unwrap(),
            json!({"frame": "close", "reason": null})
        );

        let output: ModuleOutput = serde_json::from_value(json!({"kind": "view", "view": {
            "sections": [{"title": "Clients", "items": [
                {"kind": "row", "title": "Claude", "subtitle": null, "actions": [
                    {"title": "Revoke", "command": "revoke", "arguments": {"id": "g1"},
                     "destructive": true}]},
                {"kind": "block", "label": "Config", "text": "{\"mcpServers\":{}}"}]}]}}))
        .unwrap();
        let ModuleOutput::View { view } = &output else {
            panic!("view");
        };
        view.validate().unwrap();
        let encoded = serde_json::to_value(&output).unwrap();
        assert_eq!(
            serde_json::from_value::<ModuleOutput>(encoded).unwrap(),
            output
        );
    }
}
