//! AI tool access (MCP): the exposure policy, client grants, approvals and
//! status. Administrative calls are served by the daemon only to this app.

use copypaste_ipc::ai::{self as ipc, AiOp};
use copypaste_ipc::{ConfigPatch, Method, ResponseData};

use super::{empty_response, expect_data, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiScope {
    ReadHistory,
    ReadSnippets,
    WriteClipboard,
    Organize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiLookback {
    Day,
    Week,
    Month,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiContentKind {
    Text,
    Link,
    RichText,
    Image,
    FileMetadata,
}

#[derive(Debug, Clone)]
pub struct AiPolicyView {
    pub include_other_devices: bool,
    pub excluded_device_ids: Vec<String>,
    pub content_kinds: Vec<AiContentKind>,
    pub excluded_source_apps: Vec<String>,
    pub default_lookback: AiLookback,
}

#[derive(Debug, Clone)]
pub struct AiGrantView {
    pub id: String,
    pub owner_device_id: String,
    pub owner_device_name: Option<String>,
    pub name: String,
    pub client_name: String,
    pub client_executable: Option<String>,
    pub scopes: Vec<AiScope>,
    pub lookback: AiLookback,
    pub created_at_ms: i64,
    pub last_used_day: Option<i64>,
    pub revoked: bool,
    pub revoked_at_ms: Option<i64>,
    pub reissue_required: bool,
}

/// A copyable client configuration that embeds the credential.
#[derive(Clone)]
pub struct AiClientConfigSnippet {
    pub client: String,
    pub label: String,
    pub language: String,
    pub text: String,
}

/// The credential is shown once; it is never stored in plaintext.
#[derive(Clone)]
pub struct AiGrantCreatedView {
    pub grant: AiGrantView,
    pub credential: String,
    pub snippets: Vec<AiClientConfigSnippet>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiApprovalKind {
    Access,
    Volume,
}

#[derive(Debug, Clone)]
pub struct AiApprovalView {
    pub request_id: String,
    pub kind: AiApprovalKind,
    pub client_name: String,
    pub client_version: Option<String>,
    pub executable: Option<String>,
    pub grant_name: Option<String>,
    pub requested_at_ms: i64,
    pub expires_at_ms: i64,
    pub read_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AiAccessEntryView {
    pub at_ms: i64,
    pub tool: String,
    pub item_ids: Vec<String>,
    pub result_count: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiSessionState {
    Unauthorized,
    PendingApproval,
    Authorized,
    Revoked,
    Disabled,
    Expired,
    AtCapacity,
}

#[derive(Debug, Clone)]
pub struct AiStatusView {
    pub mcp_enabled: bool,
    pub session: Option<AiSessionState>,
    pub scopes: Vec<AiScope>,
    pub lookback: Option<AiLookback>,
    pub contributing_devices: u32,
    pub next_step: Option<String>,
}

fn ai_op(op: AiOp) -> Method {
    Method::Ai { op }
}

fn scope(value: ipc::AiScope) -> AiScope {
    match value {
        ipc::AiScope::ReadHistory => AiScope::ReadHistory,
        ipc::AiScope::ReadSnippets => AiScope::ReadSnippets,
        ipc::AiScope::WriteClipboard => AiScope::WriteClipboard,
        ipc::AiScope::Organize => AiScope::Organize,
    }
}

fn scope_input(value: AiScope) -> ipc::AiScope {
    match value {
        AiScope::ReadHistory => ipc::AiScope::ReadHistory,
        AiScope::ReadSnippets => ipc::AiScope::ReadSnippets,
        AiScope::WriteClipboard => ipc::AiScope::WriteClipboard,
        AiScope::Organize => ipc::AiScope::Organize,
    }
}

fn scopes_input(values: Vec<AiScope>) -> Vec<ipc::AiScope> {
    values.into_iter().map(scope_input).collect()
}

fn lookback(value: ipc::AiLookback) -> AiLookback {
    match value {
        ipc::AiLookback::Day => AiLookback::Day,
        ipc::AiLookback::Week => AiLookback::Week,
        ipc::AiLookback::Month => AiLookback::Month,
        ipc::AiLookback::All => AiLookback::All,
    }
}

fn lookback_input(value: AiLookback) -> ipc::AiLookback {
    match value {
        AiLookback::Day => ipc::AiLookback::Day,
        AiLookback::Week => ipc::AiLookback::Week,
        AiLookback::Month => ipc::AiLookback::Month,
        AiLookback::All => ipc::AiLookback::All,
    }
}

fn policy(value: ipc::AiPolicy) -> AiPolicyView {
    AiPolicyView {
        include_other_devices: value.include_other_devices,
        excluded_device_ids: value.excluded_device_ids,
        content_kinds: value
            .content_kinds
            .into_iter()
            .map(|kind| match kind {
                ipc::AiContentKind::Text => AiContentKind::Text,
                ipc::AiContentKind::Link => AiContentKind::Link,
                ipc::AiContentKind::RichText => AiContentKind::RichText,
                ipc::AiContentKind::Image => AiContentKind::Image,
                ipc::AiContentKind::FileMetadata => AiContentKind::FileMetadata,
            })
            .collect(),
        excluded_source_apps: value.excluded_source_apps,
        default_lookback: lookback(value.default_lookback),
    }
}

fn grant(value: ipc::AiGrant) -> AiGrantView {
    AiGrantView {
        id: value.id,
        owner_device_id: value.owner_device_id,
        owner_device_name: value.owner_device_name,
        name: value.name,
        client_name: value.client_name,
        client_executable: value.client_executable,
        scopes: value.scopes.into_iter().map(scope).collect(),
        lookback: lookback(value.lookback),
        created_at_ms: value.created_at,
        last_used_day: value.last_used_day,
        revoked: value.revoked,
        revoked_at_ms: value.revoked_at,
        reissue_required: value.reissue_required,
    }
}

async fn policy_response(op: AiOp) -> Result<AiPolicyView, RuntimeError> {
    Ok(policy(expect_data!(ai_op(op), ResponseData::AiPolicy)))
}

async fn grant_response(op: AiOp) -> Result<AiGrantView, RuntimeError> {
    Ok(grant(expect_data!(ai_op(op), ResponseData::AiGrant)))
}

/// Whether this desktop serves AI tools. A device-local setting.
pub async fn ai_tools_enabled() -> Result<bool, RuntimeError> {
    let applied = expect_data!(Method::GetConfig, ResponseData::Config);
    Ok(applied.config.mcp_enabled)
}

pub async fn set_ai_tools_enabled(enabled: bool) -> Result<bool, RuntimeError> {
    let applied = expect_data!(
        Method::SetConfig {
            patch: ConfigPatch {
                mcp_enabled: Some(enabled),
                ..ConfigPatch::default()
            },
        },
        ResponseData::Config
    );
    Ok(applied.config.mcp_enabled)
}

pub async fn ai_policy() -> Result<AiPolicyView, RuntimeError> {
    policy_response(AiOp::Policy).await
}

/// Set one synced policy key; answers the updated policy.
pub async fn set_ai_policy(key: String, value_json: String) -> Result<AiPolicyView, RuntimeError> {
    policy_response(AiOp::SetPolicy { key, value_json }).await
}

/// Add or remove one member of a synced policy set; answers the policy.
pub async fn set_ai_policy_member(
    set: String,
    member: String,
    present: bool,
) -> Result<AiPolicyView, RuntimeError> {
    policy_response(AiOp::SetPolicyMember {
        set,
        member,
        present,
    })
    .await
}

/// Every grant, including those owned by other desktops.
pub async fn ai_grants() -> Result<Vec<AiGrantView>, RuntimeError> {
    let grants = expect_data!(ai_op(AiOp::Grants), ResponseData::AiGrants);
    Ok(grants.into_iter().map(grant).collect())
}

pub async fn create_ai_grant(
    name: String,
    client: String,
    scopes: Vec<AiScope>,
    lookback: AiLookback,
) -> Result<AiGrantCreatedView, RuntimeError> {
    let created = expect_data!(
        ai_op(AiOp::CreateGrant {
            name,
            client,
            scopes: scopes_input(scopes),
            lookback: lookback_input(lookback),
        }),
        ResponseData::AiGrantCreated
    );
    Ok(AiGrantCreatedView {
        grant: grant(created.grant),
        credential: created.credential,
        snippets: created
            .snippets
            .into_iter()
            .map(|snippet| AiClientConfigSnippet {
                client: snippet.client,
                label: snippet.label,
                language: snippet.language,
                text: snippet.text,
            })
            .collect(),
    })
}

pub async fn update_ai_grant(
    id: String,
    name: Option<String>,
    scopes: Option<Vec<AiScope>>,
    lookback: Option<AiLookback>,
) -> Result<AiGrantView, RuntimeError> {
    grant_response(AiOp::UpdateGrant {
        id,
        name,
        scopes: scopes.map(scopes_input),
        lookback: lookback.map(lookback_input),
    })
    .await
}

pub async fn revoke_ai_grant(id: String) -> Result<AiGrantView, RuntimeError> {
    grant_response(AiOp::RevokeGrant { id }).await
}

pub async fn delete_ai_grant(id: String) -> Result<(), RuntimeError> {
    empty_response(ai_op(AiOp::DeleteGrant { id })).await
}

pub async fn ai_pending_approvals() -> Result<Vec<AiApprovalView>, RuntimeError> {
    let approvals = expect_data!(ai_op(AiOp::PendingApprovals), ResponseData::AiApprovals);
    Ok(approvals
        .into_iter()
        .map(|request| AiApprovalView {
            request_id: request.request_id,
            kind: match request.kind {
                ipc::AiApprovalKind::Access => AiApprovalKind::Access,
                ipc::AiApprovalKind::Volume => AiApprovalKind::Volume,
            },
            client_name: request.client_name,
            client_version: request.client_version,
            executable: request.executable,
            grant_name: request.grant_name,
            requested_at_ms: request.requested_at,
            expires_at_ms: request.expires_at,
            read_bytes: request.read_bytes,
        })
        .collect())
}

pub async fn approve_ai_request(
    request_id: String,
    scopes: Vec<AiScope>,
    lookback: AiLookback,
) -> Result<(), RuntimeError> {
    empty_response(ai_op(AiOp::Approve {
        request_id,
        scopes: scopes_input(scopes),
        lookback: lookback_input(lookback),
    }))
    .await
}

pub async fn deny_ai_request(request_id: String) -> Result<(), RuntimeError> {
    empty_response(ai_op(AiOp::Deny { request_id })).await
}

pub async fn decide_ai_volume(request_id: String, allow: bool) -> Result<(), RuntimeError> {
    empty_response(ai_op(AiOp::VolumeDecision { request_id, allow })).await
}

pub async fn ai_access_log(grant_id: String) -> Result<Vec<AiAccessEntryView>, RuntimeError> {
    let entries = expect_data!(
        ai_op(AiOp::AccessLog { grant_id }),
        ResponseData::AiAccessLog
    );
    Ok(entries
        .into_iter()
        .map(|entry| AiAccessEntryView {
            at_ms: entry.at,
            tool: entry.tool,
            item_ids: entry.item_ids,
            result_count: entry.result_count,
            error: entry.error,
        })
        .collect())
}

pub async fn ai_status() -> Result<AiStatusView, RuntimeError> {
    let status = expect_data!(
        ai_op(AiOp::Status { session_id: None }),
        ResponseData::AiStatus
    );
    Ok(AiStatusView {
        mcp_enabled: status.mcp_enabled,
        session: status.session.map(|state| match state {
            ipc::AiSessionState::Unauthorized => AiSessionState::Unauthorized,
            ipc::AiSessionState::PendingApproval => AiSessionState::PendingApproval,
            ipc::AiSessionState::Authorized => AiSessionState::Authorized,
            ipc::AiSessionState::Revoked => AiSessionState::Revoked,
            ipc::AiSessionState::Disabled => AiSessionState::Disabled,
            ipc::AiSessionState::Expired => AiSessionState::Expired,
            ipc::AiSessionState::AtCapacity => AiSessionState::AtCapacity,
        }),
        scopes: status.scopes.into_iter().map(scope).collect(),
        lookback: status.lookback.map(lookback),
        contributing_devices: status.contributing_devices,
        next_step: status.next_step,
    })
}
