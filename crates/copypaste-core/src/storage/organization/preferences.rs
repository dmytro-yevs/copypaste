//! Synced library preferences (one record per key) and set members (one
//! record per element), which give per-field and per-element merges.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{required_line, StoreError};

pub(super) const PREFERENCE_BYTES: usize = 8 * 1024;
pub(super) const MEMBER_BYTES: usize = 2 * 1024;
const NAMESPACES: &[&str] = &["snippets", "transforms", "ai_policy", "quick_access"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceRecord {
    pub namespace: String,
    pub key: String,
    /// Any JSON value.
    pub value_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetMemberRecord {
    pub set: String,
    pub member: String,
    pub present: bool,
}

pub(super) fn preference_key(namespace: &str, key: &str) -> String {
    format!("pref:{namespace}:{key}")
}

pub(super) fn member_key(set: &str, member: &str) -> String {
    format!(
        "member:{set}:{}",
        hex::encode(Sha256::digest(member.as_bytes()))
    )
}

/// A feature namespace such as `snippets` or `ai_policy.devices`. It never
/// contains `:`, which keeps the record keys unambiguous.
fn namespace(name: &str) -> bool {
    name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._".contains(&byte))
        && NAMESPACES.iter().any(|root| {
            name.strip_prefix(root)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        })
}

impl PreferenceRecord {
    pub(super) fn valid(&self) -> bool {
        namespace(&self.namespace)
            && required_line(&self.key, 128)
            && serde_json::from_str::<serde_json::Value>(&self.value_json).is_ok()
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO library_preferences(namespace,key,record_id,value_json,stamp) \
             VALUES (?1,?2,?3,?4,?5) ON CONFLICT(namespace,key) DO UPDATE SET \
             record_id=excluded.record_id,value_json=excluded.value_json,stamp=excluded.stamp",
            params![self.namespace, self.key, record_id, self.value_json, stamp],
        )?;
        Ok(())
    }
}

impl SetMemberRecord {
    pub(super) fn valid(&self) -> bool {
        namespace(&self.set) && required_line(&self.member, 1024)
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO library_set_members(set_name,member,record_id,present,stamp) \
             VALUES (?1,?2,?3,?4,?5) ON CONFLICT(set_name,member) DO UPDATE SET \
             record_id=excluded.record_id,present=excluded.present,stamp=excluded.stamp",
            params![self.set, self.member, record_id, self.present, stamp],
        )?;
        Ok(())
    }
}

pub(super) fn preference(
    conn: &Connection,
    namespace: &str,
    key: &str,
) -> Result<Option<PreferenceRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT value_json FROM library_preferences WHERE namespace=?1 AND key=?2",
            [namespace, key],
            |r| {
                Ok(PreferenceRecord {
                    namespace: namespace.to_owned(),
                    key: key.to_owned(),
                    value_json: r.get(0)?,
                })
            },
        )
        .optional()?)
}

pub(super) fn member(
    conn: &Connection,
    set: &str,
    member: &str,
) -> Result<Option<SetMemberRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT present FROM library_set_members WHERE set_name=?1 AND member=?2",
            [set, member],
            |r| {
                Ok(SetMemberRecord {
                    set: set.to_owned(),
                    member: member.to_owned(),
                    present: r.get(0)?,
                })
            },
        )
        .optional()?)
}
