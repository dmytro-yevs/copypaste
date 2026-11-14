//! Per-client access grants. Grant metadata is a synced module record so any
//! device can list and revoke it; a grant works only on the device that
//! approved it, which keeps that approval in local state. A revocation is a
//! separate record that nothing ever clears, so a stale write of the grant
//! cannot undo it under whole-record last-write-wins.
use crate::host::{self, Host};
use copypaste_module_sdk::{
    HostError, HostErrorCode, ModuleHostRequest, ModuleRecord, RecordPage, MAX_RECORD_PAGE,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const GRANT_PREFIX: &str = "grant/";
const REVOKED_PREFIX: &str = "revoked/";
const APPROVALS_KEY: &str = "approvals";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    ReadHistory,
    ReadSnippets,
    WriteClipboard,
    Organize,
}

pub(crate) const READ_SCOPES: [Scope; 2] = [Scope::ReadHistory, Scope::ReadSnippets];
pub(crate) const ALL_SCOPES: [Scope; 4] = [
    Scope::ReadHistory,
    Scope::ReadSnippets,
    Scope::WriteClipboard,
    Scope::Organize,
];

impl Scope {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ReadHistory => "read_history",
            Self::ReadSnippets => "read_snippets",
            Self::WriteClipboard => "write_clipboard",
            Self::Organize => "organize",
        }
    }
}

/// Unknown fields are kept out of the struct but tolerated, so an older
/// module version can still read grants a newer one wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Grant {
    pub(crate) id: String,
    pub(crate) client_name: String,
    pub(crate) owner_device: String,
    #[serde(default)]
    pub(crate) owner_name: Option<String>,
    /// Scope names; an unknown name grants nothing.
    pub(crate) scopes: Vec<String>,
    /// Later connections of the same client on the owner device are
    /// authorized without asking.
    #[serde(default)]
    pub(crate) remember: bool,
    pub(crate) created_at: i64,
    /// Days since the Unix epoch, so the record changes at most daily.
    #[serde(default)]
    pub(crate) last_used_day: Option<i64>,
    #[serde(default)]
    pub(crate) revoked_at: Option<i64>,
}

impl Grant {
    pub(crate) fn scopes(&self) -> Vec<Scope> {
        let mut scopes: Vec<Scope> = self
            .scopes
            .iter()
            .filter_map(|name| serde_json::from_value(serde_json::Value::String(name.clone())).ok())
            .collect();
        scopes.sort();
        scopes.dedup();
        scopes
    }

    pub(crate) fn set_scopes(&mut self, scopes: &[Scope]) {
        self.scopes = scopes.iter().map(|scope| scope.name().to_owned()).collect();
    }

    pub(crate) fn revoked(&self) -> bool {
        self.revoked_at.is_some()
    }
}

#[derive(Serialize, Deserialize)]
struct Revocation {
    at: i64,
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn list_records(host: &dyn Host, prefix: &str) -> Result<Vec<ModuleRecord>, HostError> {
    let mut records = Vec::new();
    let mut after = None;
    loop {
        let page: RecordPage = host::request(
            host,
            ModuleHostRequest::RecordList {
                prefix: Some(prefix.to_owned()),
                after: after.take(),
                limit: MAX_RECORD_PAGE,
            },
        )?;
        records.extend(page.records.into_iter().filter(|record| !record.deleted));
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(records),
        }
    }
}

fn get_record(host: &dyn Host, key: String) -> Result<Option<ModuleRecord>, HostError> {
    Ok(
        host::request::<Option<ModuleRecord>>(host, ModuleHostRequest::RecordGet { key })?
            .filter(|record| !record.deleted),
    )
}

/// Every grant, oldest first, with revocations applied.
pub(crate) fn list(host: &dyn Host) -> Result<Vec<Grant>, HostError> {
    let revoked: BTreeMap<String, i64> = list_records(host, REVOKED_PREFIX)?
        .into_iter()
        .filter_map(|record| {
            let at = serde_json::from_str::<Revocation>(&record.value_json)
                .ok()?
                .at;
            Some((record.key.strip_prefix(REVOKED_PREFIX)?.to_owned(), at))
        })
        .collect();
    let mut grants: Vec<Grant> = list_records(host, GRANT_PREFIX)?
        .into_iter()
        .filter_map(|record| serde_json::from_str::<Grant>(&record.value_json).ok())
        .map(|mut grant| {
            if let Some(at) = revoked.get(&grant.id) {
                grant.revoked_at.get_or_insert(*at);
            }
            grant
        })
        .collect();
    grants.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    Ok(grants)
}

pub(crate) fn get(host: &dyn Host, id: &str) -> Result<Option<Grant>, HostError> {
    let Some(record) = get_record(host, format!("{GRANT_PREFIX}{id}"))? else {
        return Ok(None);
    };
    let Ok(mut grant) = serde_json::from_str::<Grant>(&record.value_json) else {
        return Ok(None);
    };
    if grant.revoked_at.is_none() {
        if let Some(record) = get_record(host, format!("{REVOKED_PREFIX}{id}"))? {
            grant.revoked_at = serde_json::from_str::<Revocation>(&record.value_json)
                .ok()
                .map(|revocation| revocation.at);
        }
    }
    Ok(Some(grant))
}

pub(crate) fn put(host: &dyn Host, grant: &Grant) -> Result<(), HostError> {
    let value_json = serde_json::to_string(grant)
        .map_err(|_| HostError::new(HostErrorCode::Invalid, "The grant is invalid."))?;
    host::request(
        host,
        ModuleHostRequest::RecordPut {
            key: format!("{GRANT_PREFIX}{}", grant.id),
            value_json,
        },
    )
}

pub(crate) fn revoke(host: &dyn Host, id: &str, now_ms: i64) -> Result<(), HostError> {
    host::request::<()>(
        host,
        ModuleHostRequest::RecordPut {
            key: format!("{REVOKED_PREFIX}{id}"),
            value_json: format!("{{\"at\":{now_ms}}}"),
        },
    )?;
    if let Some(mut grant) = get(host, id)? {
        grant.revoked_at.get_or_insert(now_ms);
        put(host, &grant)?;
    }
    Ok(())
}

/// Deletes the grant. Its revocation stays, so a late copy cannot revive it.
pub(crate) fn remove(host: &dyn Host, id: &str, now_ms: i64) -> Result<(), HostError> {
    revoke(host, id, now_ms)?;
    host::request::<()>(
        host,
        ModuleHostRequest::RecordDelete {
            key: format!("{GRANT_PREFIX}{id}"),
        },
    )?;
    forget_approval(host, id)
}

/// The grants this device approved, with when.
pub(crate) fn approvals(host: &dyn Host) -> Result<BTreeMap<String, i64>, HostError> {
    Ok(host::read_state(host, APPROVALS_KEY)?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

pub(crate) fn approve(host: &dyn Host, id: &str, now_ms: i64) -> Result<(), HostError> {
    let mut approvals = approvals(host)?;
    approvals.insert(id.to_owned(), now_ms);
    save_approvals(host, &approvals)
}

fn forget_approval(host: &dyn Host, id: &str) -> Result<(), HostError> {
    let mut approvals = approvals(host)?;
    if approvals.remove(id).is_some() {
        save_approvals(host, &approvals)?;
    }
    Ok(())
}

fn save_approvals(host: &dyn Host, approvals: &BTreeMap<String, i64>) -> Result<(), HostError> {
    let live: BTreeSet<String> = list(host)?.into_iter().map(|grant| grant.id).collect();
    let kept: BTreeMap<&String, &i64> = approvals
        .iter()
        .filter(|(id, _)| live.contains(*id) || approvals.len() <= crate::limits::MAX_GRANTS)
        .collect();
    let json = serde_json::to_string(&kept)
        .map_err(|_| HostError::new(HostErrorCode::Invalid, "The approvals are invalid."))?;
    host::write_state(host, APPROVALS_KEY, json)
}
