//! AI tool access (MCP): exposure policy, client grants, approvals, sessions
//! and tool calls.
//!
//! Every privacy, scope and limit check runs in the runtime; the `copypaste
//! mcp` process only translates. Administrative operations are privileged and
//! served by the daemon only to the CopyPaste app.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiScope {
    ReadHistory,
    ReadSnippets,
    WriteClipboard,
    Organize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiLookback {
    Day,
    Week,
    Month,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiContentKind {
    Text,
    Link,
    RichText,
    Image,
    FileMetadata,
}

/// The synced exposure policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPolicy {
    pub include_other_devices: bool,
    pub excluded_device_ids: Vec<String>,
    pub content_kinds: Vec<AiContentKind>,
    pub excluded_source_apps: Vec<String>,
    pub default_lookback: AiLookback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiGrant {
    pub id: String,
    pub owner_device_id: String,
    pub owner_device_name: Option<String>,
    pub name: String,
    /// Reported by the client; informational and untrusted.
    pub client_name: String,
    pub client_executable: Option<String>,
    pub scopes: Vec<AiScope>,
    pub lookback: AiLookback,
    pub created_at: i64,
    pub last_used_day: Option<i64>,
    pub revoked: bool,
    pub revoked_at: Option<i64>,
    /// This device owns the grant but holds no verifier (after a restore).
    pub reissue_required: bool,
}

/// A copyable client configuration that embeds the credential.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientConfigSnippet {
    pub client: String,
    pub label: String,
    pub language: String,
    pub text: String,
}

/// The credential is shown once and never stored in plaintext.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiGrantCreated {
    pub grant: AiGrant,
    pub credential: String,
    pub snippets: Vec<ClientConfigSnippet>,
}

impl std::fmt::Debug for ClientConfigSnippet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConfigSnippet")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AiGrantCreated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiGrantCreated")
            .field("grant", &self.grant)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiApprovalKind {
    /// A client without a valid credential asks for access.
    Access,
    /// A session passed its read-volume allowance.
    Volume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiApprovalRequest {
    pub request_id: String,
    pub kind: AiApprovalKind,
    pub client_name: String,
    pub client_version: Option<String>,
    pub executable: Option<String>,
    pub grant_name: Option<String>,
    pub requested_at: i64,
    pub expires_at: i64,
    pub read_bytes: Option<u64>,
}

/// One served tool call. Ids only; never content or queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAccessEntry {
    pub at: i64,
    pub tool: String,
    pub item_ids: Vec<String>,
    pub result_count: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSessionState {
    Unauthorized,
    PendingApproval,
    Authorized,
    Revoked,
    Disabled,
    Expired,
    AtCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStatus {
    pub mcp_enabled: bool,
    pub session: Option<AiSessionState>,
    pub scopes: Vec<AiScope>,
    pub lookback: Option<AiLookback>,
    pub contributing_devices: u32,
    /// Code of the one next step that unblocks access, if any.
    pub next_step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSession {
    pub session_id: String,
    pub authorized_scopes: Option<Vec<AiScope>>,
    pub pending_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum AiToolCall {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiImage {
    pub mime_type: String,
    pub data_base64: String,
}

/// A tool result in MCP terms; `structured_json` is the typed payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiToolResult {
    pub is_error: bool,
    pub text: String,
    pub image: Option<AiImage>,
    pub structured_json: Option<String>,
    pub retry_after_secs: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AiOp {
    Policy,
    SetPolicy {
        key: String,
        value_json: String,
    },
    SetPolicyMember {
        set: String,
        member: String,
        present: bool,
    },
    Grants,
    CreateGrant {
        name: String,
        client: String,
        scopes: Vec<AiScope>,
        lookback: AiLookback,
    },
    UpdateGrant {
        id: String,
        name: Option<String>,
        scopes: Option<Vec<AiScope>>,
        lookback: Option<AiLookback>,
    },
    RevokeGrant {
        id: String,
    },
    DeleteGrant {
        id: String,
    },
    PendingApprovals,
    Approve {
        request_id: String,
        scopes: Vec<AiScope>,
        lookback: AiLookback,
    },
    Deny {
        request_id: String,
    },
    VolumeDecision {
        request_id: String,
        allow: bool,
    },
    AccessLog {
        grant_id: String,
    },
    Status {
        session_id: Option<String>,
    },
    SessionOpen {
        credential: Option<String>,
        client_name: String,
        client_version: Option<String>,
        executable: Option<String>,
    },
    /// Valid only on a `watch_events` connection, which then owns the session.
    SessionAttach {
        session_id: String,
    },
    SessionClose {
        session_id: String,
    },
    Tool {
        session_id: String,
        call: AiToolCall,
    },
}

impl crate::OpGates for AiOp {
    fn requires_ready(&self) -> bool {
        !matches!(self, Self::Status { .. })
    }

    fn interactive(&self) -> bool {
        false
    }

    fn blocking_weight(&self) -> u32 {
        match self {
            Self::Tool { .. } => crate::limits::content_weight_units(4),
            _ => 0,
        }
    }

    fn is_long_running(&self) -> bool {
        false
    }

    /// Approvals, grant administration and policy changes.
    fn privileged(&self) -> bool {
        matches!(
            self,
            Self::Approve { .. }
                | Self::Deny { .. }
                | Self::VolumeDecision { .. }
                | Self::CreateGrant { .. }
                | Self::UpdateGrant { .. }
                | Self::RevokeGrant { .. }
                | Self::DeleteGrant { .. }
                | Self::SetPolicy { .. }
                | Self::SetPolicyMember { .. }
        )
    }
}
