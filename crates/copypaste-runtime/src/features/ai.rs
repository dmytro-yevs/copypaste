//! AI tool access (MCP): policy, grants, approvals, sessions and tools.
//!
//! The exposure policy is synced `ai_policy` preferences and sets; grant
//! metadata is synced, its verifier stays on the owning desktop. Sessions,
//! approvals, rate buckets and recent denials live in memory only.

mod approvals;
mod limits;
mod redact;
mod sessions;
mod tools;

use std::sync::{Mutex, MutexGuard};

use copypaste_core::storage::ai_view::{new_ai_credential, new_ai_id};
use copypaste_core::storage::organization::{
    AiGrantRecord, PreferenceRecord, Record, RecordKey, SetMemberRecord,
};
use copypaste_core::{now_ms, StoreError};
use copypaste_ipc::ai::{
    AiContentKind, AiGrant, AiGrantCreated, AiLookback, AiOp, AiPolicy, AiScope,
    ClientConfigSnippet,
};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};

use crate::messages::storage_error;
use crate::Runtime;

const POLICY: &str = "ai_policy";
const EXCLUDED_DEVICES: &str = "excluded_devices";
const EXCLUDED_APPS: &str = "excluded_apps";
const KEY_OTHER_DEVICES: &str = "include_other_devices";
const KEY_KINDS: &str = "content_kinds";
const KEY_LOOKBACK: &str = "default_lookback";
const MAX_NAME_CHARS: usize = 128;
const MAX_MEMBER_CHARS: usize = 1024;
/// The environment variable `copypaste mcp` reads its credential from.
const CREDENTIAL_ENV: &str = "COPYPASTE_MCP_GRANT";

const MSG_GRANT_NOT_FOUND: &str = "That AI tool connection no longer exists.";
const MSG_BAD_POLICY: &str = "That AI tools setting is not valid.";
const MSG_BAD_GRANT: &str =
    "Give the connection a name of up to 128 characters and choose a known client.";

/// The in-memory half of AI access, one per runtime.
#[derive(Default)]
pub(crate) struct AiState(Mutex<sessions::State>);

impl AiState {
    fn lock(&self) -> MutexGuard<'_, sessions::State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Runtime {
    pub(crate) fn ai(&self, id: u64, op: AiOp) -> Response {
        match op {
            AiOp::PendingApprovals
            | AiOp::Approve { .. }
            | AiOp::Deny { .. }
            | AiOp::VolumeDecision { .. } => approvals::handle(self, id, op),
            AiOp::Status { .. }
            | AiOp::SessionOpen { .. }
            | AiOp::SessionAttach { .. }
            | AiOp::SessionClose { .. } => sessions::handle(self, id, op),
            AiOp::Tool { .. } => tools::handle(self, id, op),
            AiOp::Policy => policy_response(self, id),
            AiOp::SetPolicy { key, value_json } => set_policy(self, id, &key, &value_json),
            AiOp::SetPolicyMember {
                set,
                member,
                present,
            } => set_policy_member(self, id, &set, &member, present),
            AiOp::Grants => match grants(self) {
                Ok(grants) => Response::ok(id, ResponseData::AiGrants(grants)),
                Err(error) => storage_error(id, "ai_grants", &error),
            },
            AiOp::CreateGrant {
                name,
                client,
                scopes,
                lookback,
            } => create_grant(self, id, &name, &client, scopes, lookback),
            AiOp::UpdateGrant {
                id: grant_id,
                name,
                scopes,
                lookback,
            } => edit_grant(self, id, &grant_id, |grant| {
                if let Some(name) = name.as_deref() {
                    grant.name = valid_name(name).ok_or(())?;
                }
                if let Some(scopes) = &scopes {
                    grant.scopes = scope_names(scopes);
                }
                if let Some(lookback) = lookback {
                    grant.lookback = lookback_name(lookback).to_owned();
                }
                Ok(())
            }),
            AiOp::RevokeGrant { id: grant_id } => edit_grant(self, id, &grant_id, |grant| {
                revoke(grant);
                Ok(())
            }),
            AiOp::DeleteGrant { id: grant_id } => delete_grant(self, id, &grant_id),
            AiOp::AccessLog { grant_id } => match self.store.ai_access_log(&grant_id) {
                Ok(log) => Response::ok(id, ResponseData::AiAccessLog(log)),
                Err(error) => storage_error(id, "ai_access_log", &error),
            },
        }
    }

    /// Bind `session_id` to the `watch_events` connection `connection`; the
    /// session ends when [`Runtime::on_watch_closed`] reports that connection.
    pub fn attach_ai_session(&self, id: u64, connection: u64, session_id: &str) -> Response {
        sessions::attach(self, id, connection, session_id)
    }

    /// A `watch_events` connection closed; end the sessions it owned.
    pub fn on_watch_closed(&self, connection: u64) {
        sessions::connection_closed(self, connection);
    }
}

pub(super) fn scope_name(scope: AiScope) -> &'static str {
    match scope {
        AiScope::ReadHistory => "read_history",
        AiScope::ReadSnippets => "read_snippets",
        AiScope::WriteClipboard => "write_clipboard",
        AiScope::Organize => "organize",
    }
}

fn scope_names(scopes: &[AiScope]) -> Vec<String> {
    let mut scopes = scopes.to_vec();
    scopes.sort();
    scopes.dedup();
    scopes
        .into_iter()
        .map(|scope| scope_name(scope).to_owned())
        .collect()
}

/// The scopes a stored grant names; an unknown name grants nothing.
fn scopes_of(grant: &AiGrantRecord) -> Vec<AiScope> {
    let mut scopes: Vec<AiScope> = grant
        .scopes
        .iter()
        .filter_map(|name| serde_json::from_value(serde_json::Value::String(name.clone())).ok())
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

fn lookback_name(lookback: AiLookback) -> &'static str {
    match lookback {
        AiLookback::Day => "day",
        AiLookback::Week => "week",
        AiLookback::Month => "month",
        AiLookback::All => "all",
    }
}

/// An unknown stored lookback is read as the shortest window.
fn lookback_of(grant: &AiGrantRecord) -> AiLookback {
    serde_json::from_value(serde_json::Value::String(grant.lookback.clone()))
        .unwrap_or(AiLookback::Day)
}

fn lookback_ms(lookback: AiLookback) -> Option<i64> {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    match lookback {
        AiLookback::Day => Some(DAY),
        AiLookback::Week => Some(7 * DAY),
        AiLookback::Month => Some(30 * DAY),
        AiLookback::All => None,
    }
}

fn valid_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.chars().any(char::is_control))
    .then(|| name.to_owned())
}

/// The policy as stored, with the defaults for anything never set.
fn policy(runtime: &Runtime) -> Result<AiPolicy, StoreError> {
    let preference = |key: &str| -> Result<Option<serde_json::Value>, StoreError> {
        Ok(
            match runtime.store.organization_record(RecordKey::Preference {
                namespace: POLICY,
                key,
            })? {
                Some(Record::Preference(record)) => serde_json::from_str(&record.value_json).ok(),
                _ => None,
            },
        )
    };
    let member_set = |name: &str| {
        runtime
            .store
            .library_set_members(&format!("{POLICY}.{name}"))
    };
    Ok(AiPolicy {
        include_other_devices: preference(KEY_OTHER_DEVICES)?
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        excluded_device_ids: member_set(EXCLUDED_DEVICES)?,
        content_kinds: preference(KEY_KINDS)?
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_else(|| {
                vec![
                    AiContentKind::Text,
                    AiContentKind::Link,
                    AiContentKind::RichText,
                    AiContentKind::FileMetadata,
                ]
            }),
        excluded_source_apps: member_set(EXCLUDED_APPS)?,
        default_lookback: preference(KEY_LOOKBACK)?
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or(AiLookback::Week),
    })
}

fn policy_response(runtime: &Runtime, id: u64) -> Response {
    match policy(runtime) {
        Ok(policy) => Response::ok(id, ResponseData::AiPolicy(policy)),
        Err(error) => storage_error(id, "ai_policy", &error),
    }
}

/// The canonical JSON of a policy value, or `None` when `key` or the value
/// is not one the policy knows.
fn policy_value(key: &str, value_json: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(value_json).ok()?;
    let canonical = match key {
        KEY_OTHER_DEVICES => serde_json::Value::Bool(value.as_bool()?),
        KEY_KINDS => {
            let kinds: Vec<AiContentKind> = serde_json::from_value(value).ok()?;
            let mut unique = Vec::new();
            for kind in kinds {
                if !unique.contains(&kind) {
                    unique.push(kind);
                }
            }
            serde_json::to_value(unique).ok()?
        }
        KEY_LOOKBACK => {
            serde_json::to_value(serde_json::from_value::<AiLookback>(value).ok()?).ok()?
        }
        _ => return None,
    };
    serde_json::to_string(&canonical).ok()
}

fn set_policy(runtime: &Runtime, id: u64, key: &str, value_json: &str) -> Response {
    let Some(value_json) = policy_value(key, value_json) else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_POLICY);
    };
    let record = Record::Preference(PreferenceRecord {
        namespace: POLICY.into(),
        key: key.into(),
        value_json,
    });
    write_policy(runtime, id, record, LibraryKinds::PREFERENCES)
}

fn set_policy_member(
    runtime: &Runtime,
    id: u64,
    set: &str,
    member: &str,
    present: bool,
) -> Response {
    let member = member.trim();
    if ![EXCLUDED_DEVICES, EXCLUDED_APPS].contains(&set)
        || member.is_empty()
        || member.chars().count() > MAX_MEMBER_CHARS
    {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_POLICY);
    }
    let record = Record::SetMember(SetMemberRecord {
        set: format!("{POLICY}.{set}"),
        member: member.into(),
        present,
    });
    write_policy(runtime, id, record, LibraryKinds::SET_MEMBERS)
}

fn write_policy(runtime: &Runtime, id: u64, record: Record, kinds: LibraryKinds) -> Response {
    let started = now_ms();
    if let Err(error) = runtime
        .store
        .write_organization_record(&runtime.keyring.item_key(), record)
    {
        return storage_error(id, "ai_policy", &error);
    }
    runtime.note_local_change_since(started);
    runtime.on_org_changed(kinds);
    policy_response(runtime, id)
}

fn grants(runtime: &Runtime) -> Result<Vec<AiGrant>, StoreError> {
    let local = runtime.meta.device_id();
    let stored = runtime.store.ai_grants()?;
    let remote: Vec<String> = stored
        .iter()
        .filter(|(grant, _)| grant.owner_device != local)
        .map(|(grant, _)| grant.owner_device.clone())
        .collect();
    let names = runtime.store.device_names(&remote)?;
    let local_name = runtime.meta.device_name();
    let mut grants: Vec<AiGrant> = stored
        .into_iter()
        .map(|(grant, verified)| {
            let owned = grant.owner_device == local;
            let owner_name = if owned {
                Some(local_name.clone())
            } else {
                names.get(&grant.owner_device).cloned()
            };
            wire_grant(grant, owner_name, owned && !verified)
        })
        .collect();
    grants.sort_by_key(|grant| grant.owner_device_id != local);
    Ok(grants)
}

fn wire_grant(
    grant: AiGrantRecord,
    owner_device_name: Option<String>,
    reissue_required: bool,
) -> AiGrant {
    AiGrant {
        scopes: scopes_of(&grant),
        lookback: lookback_of(&grant),
        id: grant.id,
        owner_device_id: grant.owner_device,
        owner_device_name,
        name: grant.name,
        client_name: grant.client_name,
        client_executable: grant.client_executable,
        created_at: grant.created_at,
        last_used_day: grant.last_used_day,
        revoked: grant.revoked,
        revoked_at: grant.revoked_at,
        reissue_required,
    }
}

fn revoke(grant: &mut AiGrantRecord) {
    if !grant.revoked {
        grant.revoked = true;
        grant.revoked_at = Some(now_ms());
    }
}

/// A live grant record.
pub(super) fn grant(
    runtime: &Runtime,
    grant_id: &str,
) -> Result<Option<AiGrantRecord>, StoreError> {
    Ok(
        match runtime
            .store
            .organization_record(RecordKey::AiGrant(grant_id))?
        {
            Some(Record::AiGrant(grant)) if !grant.deleted => Some(grant),
            _ => None,
        },
    )
}

/// Writes a grant record and announces it. Revocations also reach live
/// sessions through the approval event.
pub(super) fn write_grant(runtime: &Runtime, grant: AiGrantRecord) -> Result<(), StoreError> {
    let started = now_ms();
    let revoked = grant.revoked;
    runtime
        .store
        .write_organization_record(&runtime.keyring.item_key(), Record::AiGrant(grant))?;
    runtime.note_local_change_since(started);
    runtime.on_org_changed(LibraryKinds::GRANTS);
    if revoked {
        runtime.note_ai_approval();
    }
    Ok(())
}

/// A new grant owned by this device, with a fresh verifier. Answers the
/// record and the credential, which is never stored.
pub(super) fn issue_grant(
    runtime: &Runtime,
    name: String,
    client_name: String,
    client_executable: Option<String>,
    scopes: &[AiScope],
    lookback: AiLookback,
) -> Result<(AiGrantRecord, impl std::ops::Deref<Target = String>), StoreError> {
    let grant = AiGrantRecord {
        id: new_ai_id(),
        owner_device: runtime.meta.device_id().to_owned(),
        name,
        client_name,
        client_executable,
        scopes: scope_names(scopes),
        lookback: lookback_name(lookback).to_owned(),
        created_at: now_ms(),
        last_used_day: None,
        revoked: false,
        revoked_at: None,
        deleted: false,
    };
    let (credential, verifier) = new_ai_credential();
    runtime.store.set_ai_grant_verifier(&grant.id, &verifier)?;
    write_grant(runtime, grant.clone())?;
    Ok((grant, credential))
}

fn create_grant(
    runtime: &Runtime,
    id: u64,
    name: &str,
    client: &str,
    scopes: Vec<AiScope>,
    lookback: AiLookback,
) -> Response {
    let (Some(name), Some(label)) = (valid_name(name), client_label(client)) else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_GRANT);
    };
    match issue_grant(runtime, name, label.into(), None, &scopes, lookback) {
        Ok((grant, credential)) => {
            let snippet = client_snippet(client, &cli_path(), &credential);
            let grant = wire_grant(grant, Some(runtime.meta.device_name()), false);
            Response::ok(
                id,
                ResponseData::AiGrantCreated(AiGrantCreated {
                    grant,
                    credential: credential.to_string(),
                    snippets: snippet.into_iter().collect(),
                }),
            )
        }
        Err(error) => storage_error(id, "ai_grant_create", &error),
    }
}

fn edit_grant(
    runtime: &Runtime,
    id: u64,
    grant_id: &str,
    edit: impl FnOnce(&mut AiGrantRecord) -> Result<(), ()>,
) -> Response {
    let mut record = match grant(runtime, grant_id) {
        Ok(Some(record)) => record,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_GRANT_NOT_FOUND),
        Err(error) => return storage_error(id, "ai_grant", &error),
    };
    if edit(&mut record).is_err() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_GRANT);
    }
    if let Err(error) = write_grant(runtime, record) {
        return storage_error(id, "ai_grant_write", &error);
    }
    match grants(runtime) {
        Ok(grants) => match grants.into_iter().find(|grant| grant.id == grant_id) {
            Some(grant) => Response::ok(id, ResponseData::AiGrant(grant)),
            None => Response::err(id, ErrorCode::NotFound, MSG_GRANT_NOT_FOUND),
        },
        Err(error) => storage_error(id, "ai_grants", &error),
    }
}

/// Deleting revokes too, so a grant removed on one device stops working
/// even where the deletion has not been seen as such.
fn delete_grant(runtime: &Runtime, id: u64, grant_id: &str) -> Response {
    let mut record = match grant(runtime, grant_id) {
        Ok(Some(record)) => record,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_GRANT_NOT_FOUND),
        Err(error) => return storage_error(id, "ai_grant", &error),
    };
    revoke(&mut record);
    record.deleted = true;
    if let Err(error) = write_grant(runtime, record) {
        return storage_error(id, "ai_grant_delete", &error);
    }
    if let Err(error) = runtime.store.forget_ai_grant(grant_id) {
        return storage_error(id, "ai_grant_forget", &error);
    }
    Response::ok(id, ResponseData::Empty {})
}

fn client_label(client: &str) -> Option<&'static str> {
    Some(match client {
        "claude_desktop" => "Claude Desktop",
        "claude_code" => "Claude Code",
        "codex" => "Codex",
        "cursor" => "Cursor",
        "vscode" => "VS Code",
        "other" => "Other",
        _ => return None,
    })
}

/// The installed command-line tool beside this process, or its bare name
/// when it is not there (a development build).
fn cli_path() -> String {
    let name = if cfg!(windows) {
        "copypaste-cli.exe"
    } else {
        "copypaste-cli"
    };
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(name)))
        .filter(|path| path.is_file())
        .map_or_else(|| name.to_owned(), |path| path.display().to_string())
}

/// A JSON string literal; TOML basic strings accept the same escapes.
fn quoted(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// The configuration for one client, credential included.
fn client_snippet(client: &str, cli: &str, credential: &str) -> Option<ClientConfigSnippet> {
    let label = client_label(client)?;
    let server = |extra: Option<(&str, &str)>| {
        let mut entry = serde_json::json!({
            "command": cli,
            "args": ["mcp"],
            "env": { CREDENTIAL_ENV: credential },
        });
        if let Some((key, value)) = extra {
            entry[key] = serde_json::Value::String(value.to_owned());
        }
        entry
    };
    let json = |root: &str, extra| {
        serde_json::to_string_pretty(&serde_json::json!({ root: { "copypaste": server(extra) } }))
            .unwrap_or_default()
    };
    let (language, text) = match client {
        "claude_code" => (
            "shell",
            format!(
                "claude mcp add --scope user copypaste --env {CREDENTIAL_ENV}={credential} -- {} mcp",
                quoted(cli)
            ),
        ),
        "codex" => (
            "toml",
            format!(
                "[mcp_servers.copypaste]\ncommand = {}\nargs = [\"mcp\"]\nenv = {{ {CREDENTIAL_ENV} = {} }}\n",
                quoted(cli),
                quoted(credential)
            ),
        ),
        "vscode" => ("json", json("servers", Some(("type", "stdio")))),
        _ => ("json", json("mcpServers", None)),
    };
    Some(ClientConfigSnippet {
        client: client.to_owned(),
        label: label.to_owned(),
        language: language.to_owned(),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_snippets_quote_paths_and_carry_the_credential() {
        let path = r#"C:\Program Files\CopyPaste\copypaste-cli.exe"#;
        for client in [
            "claude_desktop",
            "claude_code",
            "codex",
            "cursor",
            "vscode",
            "other",
        ] {
            let snippet = client_snippet(client, path, "cpmcp_abc").unwrap();
            assert!(snippet.text.contains("cpmcp_abc"), "{client}");
            assert!(
                snippet
                    .text
                    .contains(r#"C:\\Program Files\\CopyPaste\\copypaste-cli.exe"#),
                "{client}: {}",
                snippet.text
            );
            if snippet.language == "json" {
                let value: serde_json::Value = serde_json::from_str(&snippet.text).unwrap();
                let root = value.as_object().unwrap().values().next().unwrap();
                assert_eq!(root["copypaste"]["args"][0], "mcp");
            }
        }
        assert!(client_snippet("unknown", path, "x").is_none());
    }
}
