//! Content rule and rule placement records. A rule's definition is atomic;
//! its enabled flag and position change independently in the placement.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{identifier, required_line, StoreError};

pub(super) const RULE_BYTES: usize = 96 * 1024;
pub(super) const PLACEMENT_BYTES: usize = 1024;
const MAX_CUSTOMIZED_FIELDS: usize = 32;
const MAX_SCOPE_DEVICES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    Builtin,
    User,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentRuleRecord {
    pub id: String,
    pub origin: RuleOrigin,
    pub builtin_version: Option<u32>,
    pub customized_fields: Vec<String>,
    pub name: String,
    /// The rule definition as a JSON object, interpreted by the rules model.
    pub definition_json: String,
    /// Device ids the rule runs on; empty means every device.
    pub device_scope: Vec<String>,
    pub schema_version: u32,
    /// Wall-clock ms of the rule's creation or last semantic edit.
    pub effective_from: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulePlacementRecord {
    pub id: String,
    pub enabled: bool,
    pub order_key: String,
}

pub(super) fn rule_key(id: &str) -> String {
    format!("rule:{id}")
}

pub(super) fn placement_key(id: &str) -> String {
    format!("rule_placement:{id}")
}

fn json_text<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|_| StoreError::InvalidHistoryQuery)
}

impl ContentRuleRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && (self.origin == RuleOrigin::Builtin) == self.builtin_version.is_some()
            && self.customized_fields.len() <= MAX_CUSTOMIZED_FIELDS
            && self
                .customized_fields
                .iter()
                .all(|field| required_line(field, 64))
            && required_line(&self.name, 128)
            && serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                &self.definition_json,
            )
            .is_ok()
            && self.device_scope.len() <= MAX_SCOPE_DEVICES
            && self.device_scope.iter().all(|device| identifier(device))
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        let origin = match self.origin {
            RuleOrigin::Builtin => "builtin",
            RuleOrigin::User => "user",
        };
        tx.execute(
            "INSERT INTO content_rules(id,record_id,origin,builtin_version,customized_json,name,\
             definition_json,device_scope_json,schema_version,effective_from,deleted_at,stamp) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) DO UPDATE SET \
             record_id=excluded.record_id,origin=excluded.origin,\
             builtin_version=excluded.builtin_version,customized_json=excluded.customized_json,\
             name=excluded.name,definition_json=excluded.definition_json,\
             device_scope_json=excluded.device_scope_json,schema_version=excluded.schema_version,\
             effective_from=excluded.effective_from,deleted_at=excluded.deleted_at,\
             stamp=excluded.stamp",
            params![
                self.id,
                record_id,
                origin,
                self.builtin_version,
                json_text(&self.customized_fields)?,
                self.name,
                self.definition_json,
                json_text(&self.device_scope)?,
                self.schema_version,
                self.effective_from,
                self.deleted_at,
                stamp,
            ],
        )?;
        Ok(())
    }
}

impl RulePlacementRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id) && required_line(&self.order_key, 128)
    }

    pub(super) fn project(&self, tx: &Connection, record_id: &str) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO content_rule_placement(id,record_id,enabled,order_key) \
             VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET record_id=excluded.record_id,\
             enabled=excluded.enabled,order_key=excluded.order_key",
            params![self.id, record_id, self.enabled, self.order_key],
        )?;
        Ok(())
    }
}

pub(super) fn rule(conn: &Connection, id: &str) -> Result<Option<ContentRuleRecord>, StoreError> {
    type Row = (
        String,
        Option<u32>,
        String,
        String,
        String,
        String,
        u32,
        i64,
        Option<i64>,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT origin,builtin_version,customized_json,name,definition_json,\
             device_scope_json,schema_version,effective_from,deleted_at \
             FROM content_rules WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        origin,
        builtin_version,
        customized,
        name,
        definition_json,
        scope,
        schema,
        from,
        deleted_at,
    )) = row
    else {
        return Ok(None);
    };
    let invalid = |_| StoreError::InvalidHistoryQuery;
    Ok(Some(ContentRuleRecord {
        id: id.to_owned(),
        origin: if origin == "builtin" {
            RuleOrigin::Builtin
        } else {
            RuleOrigin::User
        },
        builtin_version,
        customized_fields: serde_json::from_str(&customized).map_err(invalid)?,
        name,
        definition_json,
        device_scope: serde_json::from_str(&scope).map_err(invalid)?,
        schema_version: schema,
        effective_from: from,
        deleted_at,
    }))
}

pub(super) fn placement(
    conn: &Connection,
    id: &str,
) -> Result<Option<RulePlacementRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT enabled,order_key FROM content_rule_placement WHERE id=?1",
            [id],
            |r| {
                Ok(RulePlacementRecord {
                    id: id.to_owned(),
                    enabled: r.get(0)?,
                    order_key: r.get(1)?,
                })
            },
        )
        .optional()?)
}
