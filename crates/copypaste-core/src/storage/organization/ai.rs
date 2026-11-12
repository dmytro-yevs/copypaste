//! AI client grant records. Grant metadata syncs so any device can list and
//! revoke a grant; the credential verifier stays on the owning desktop.
//! A revocation is monotonic: a version that would undo it is answered by
//! writing the revoked version back.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{identifier, line, required_line, StoreError};

pub(super) const GRANT_BYTES: usize = 4 * 1024;
const MAX_SCOPES: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiGrantRecord {
    pub id: String,
    pub owner_device: String,
    pub name: String,
    pub client_name: String,
    pub client_executable: Option<String>,
    pub scopes: Vec<String>,
    pub lookback: String,
    pub created_at: i64,
    /// Day number (days since the Unix epoch) of the last use.
    pub last_used_day: Option<i64>,
    pub revoked: bool,
    pub revoked_at: Option<i64>,
    pub deleted: bool,
}

pub(super) fn grant_key(id: &str) -> String {
    format!("grant:{id}")
}

impl AiGrantRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && identifier(&self.owner_device)
            && required_line(&self.name, 128)
            && required_line(&self.client_name, 128)
            && self
                .client_executable
                .as_deref()
                .is_none_or(|path| line(path, 1024))
            && self.scopes.len() <= MAX_SCOPES
            && self.scopes.iter().all(|scope| required_line(scope, 64))
            && required_line(&self.lookback, 32)
            && self.revoked == self.revoked_at.is_some()
    }

    pub(super) fn project(&self, tx: &Connection, record_id: &str) -> Result<(), StoreError> {
        let scopes =
            serde_json::to_string(&self.scopes).map_err(|_| StoreError::InvalidHistoryQuery)?;
        tx.execute(
            "INSERT INTO ai_grants(id,record_id,owner_device,name,client_name,client_executable,\
             scopes_json,lookback,created_at,last_used_day,revoked,revoked_at,deleted) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET \
             record_id=excluded.record_id,owner_device=excluded.owner_device,name=excluded.name,\
             client_name=excluded.client_name,client_executable=excluded.client_executable,\
             scopes_json=excluded.scopes_json,lookback=excluded.lookback,\
             created_at=excluded.created_at,last_used_day=excluded.last_used_day,\
             revoked=excluded.revoked,revoked_at=excluded.revoked_at,deleted=excluded.deleted",
            params![
                self.id,
                record_id,
                self.owner_device,
                self.name,
                self.client_name,
                self.client_executable,
                scopes,
                self.lookback,
                self.created_at,
                self.last_used_day,
                self.revoked,
                self.revoked_at,
                self.deleted,
            ],
        )?;
        Ok(())
    }
}

pub(super) fn keeps_revocation(
    tx: &Connection,
    incoming: &AiGrantRecord,
) -> Result<bool, StoreError> {
    if incoming.revoked {
        return Ok(false);
    }
    Ok(tx
        .query_row(
            "SELECT revoked FROM ai_grants WHERE id=?1",
            [&incoming.id],
            |r| r.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
}

pub(super) fn grant(conn: &Connection, id: &str) -> Result<Option<AiGrantRecord>, StoreError> {
    type Row = (
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        i64,
        Option<i64>,
        bool,
        Option<i64>,
        bool,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT owner_device,name,client_name,client_executable,scopes_json,lookback,\
             created_at,last_used_day,revoked,revoked_at,deleted FROM ai_grants WHERE id=?1",
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
                    r.get(9)?,
                    r.get(10)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(
            owner_device,
            name,
            client_name,
            client_executable,
            scopes,
            lookback,
            created_at,
            last_used_day,
            revoked,
            revoked_at,
            deleted,
        )| {
            Ok(AiGrantRecord {
                id: id.to_owned(),
                owner_device,
                name,
                client_name,
                client_executable,
                scopes: serde_json::from_str(&scopes)
                    .map_err(|_| StoreError::InvalidHistoryQuery)?,
                lookback,
                created_at,
                last_used_day,
                revoked,
                revoked_at,
                deleted,
            })
        },
    )
    .transpose()
}
