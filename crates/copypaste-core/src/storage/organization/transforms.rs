//! Saved transform pipeline records. A pipeline's steps are atomic.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{identifier, line, required_line, StoreError};

pub(super) const PIPELINE_BYTES: usize = 80 * 1024;
const MAX_STEPS: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// A JSON array of step objects, interpreted by the transform catalogue.
    pub steps_json: String,
    pub deleted_at: Option<i64>,
}

pub(super) fn pipeline_key(id: &str) -> String {
    format!("pipeline:{id}")
}

impl PipelineRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && required_line(&self.name, 128)
            && self
                .description
                .as_deref()
                .is_none_or(|description| line(description, 512))
            && serde_json::from_str::<Vec<serde_json::Map<String, serde_json::Value>>>(
                &self.steps_json,
            )
            .is_ok_and(|steps| steps.len() <= MAX_STEPS)
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO transform_pipelines(id,record_id,name,description,steps_json,deleted_at,\
             stamp) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET \
             record_id=excluded.record_id,name=excluded.name,description=excluded.description,\
             steps_json=excluded.steps_json,deleted_at=excluded.deleted_at,stamp=excluded.stamp",
            params![
                self.id,
                record_id,
                self.name,
                self.description,
                self.steps_json,
                self.deleted_at,
                stamp
            ],
        )?;
        Ok(())
    }
}

pub(super) fn pipeline(conn: &Connection, id: &str) -> Result<Option<PipelineRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT name,description,steps_json,deleted_at FROM transform_pipelines WHERE id=?1",
            [id],
            |r| {
                Ok(PipelineRecord {
                    id: id.to_owned(),
                    name: r.get(0)?,
                    description: r.get(1)?,
                    steps_json: r.get(2)?,
                    deleted_at: r.get(3)?,
                })
            },
        )
        .optional()?)
}
