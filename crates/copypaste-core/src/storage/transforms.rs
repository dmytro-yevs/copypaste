//! Transform pipeline and preference queries and writes.
//!
//! Pipelines, preferences and set members are synced organization records;
//! these reads go through their projections. A pipeline's steps are kept as
//! the JSON the record carries, so steps this build does not know survive
//! every read and re-sync unchanged.

use rusqlite::params;

use super::organization::{PipelineRecord, PreferenceRecord, Record, RecordKey};
use super::{Store, StoreError};
use crate::ItemKey;

/// Most live pipelines one listing returns.
pub const MAX_PIPELINES: usize = 500;
/// Most preference rows, or members of one set, one read returns.
pub const MAX_LIBRARY_VALUES: usize = 1024;

/// What a pipeline save writes; `id: None` creates a pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineWrite<'a> {
    pub id: Option<&'a str>,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub steps_json: &'a str,
}

impl Store {
    /// Live pipelines by name, ignoring ASCII case.
    pub fn transform_pipelines(&self) -> Result<Vec<PipelineRecord>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT id,name,description,steps_json FROM transform_pipelines \
             WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE, id LIMIT ?1",
        )?;
        let rows = stmt.query_map([MAX_PIPELINES as i64], |r| {
            Ok(PipelineRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                steps_json: r.get(3)?,
                deleted_at: None,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// One pipeline unless it is deleted.
    pub fn live_transform_pipeline(&self, id: &str) -> Result<Option<PipelineRecord>, StoreError> {
        Ok(match self.organization_record(RecordKey::Pipeline(id))? {
            Some(Record::Pipeline(record)) if record.deleted_at.is_none() => Some(record),
            _ => None,
        })
    }

    /// Creates or replaces a pipeline. Saving a deleted pipeline restores it.
    pub fn save_transform_pipeline(
        &self,
        key: &ItemKey,
        write: PipelineWrite<'_>,
    ) -> Result<PipelineRecord, StoreError> {
        let record = PipelineRecord {
            id: write
                .id
                .map_or_else(|| uuid::Uuid::new_v4().to_string(), str::to_owned),
            name: write.name.to_owned(),
            description: write.description.map(str::to_owned),
            steps_json: write.steps_json.to_owned(),
            deleted_at: None,
        };
        self.write_organization_record(key, Record::Pipeline(record.clone()))?;
        Ok(record)
    }

    /// Marks a live pipeline deleted at `at_ms`; `false` when there is none.
    /// The record keeps its fields, so a later edit elsewhere restores it.
    pub fn delete_transform_pipeline(
        &self,
        key: &ItemKey,
        id: &str,
        at_ms: i64,
    ) -> Result<bool, StoreError> {
        let Some(mut record) = self.live_transform_pipeline(id)? else {
            return Ok(false);
        };
        record.deleted_at = Some(at_ms);
        self.write_organization_record(key, Record::Pipeline(record))?;
        Ok(true)
    }

    /// The preferences of namespace `root` and of every namespace under it,
    /// such as `transforms` and `transforms.param_defaults`.
    pub fn library_preferences_under(
        &self,
        root: &str,
    ) -> Result<Vec<PreferenceRecord>, StoreError> {
        let conn = self.conn()?;
        // Namespaces hold only `[a-z0-9._]`, so `/` (after `.`) bounds the
        // range of `root` and `root.*` on the primary key.
        let mut stmt = conn.prepare_cached(
            "SELECT namespace,key,value_json FROM library_preferences \
             WHERE namespace >= ?1 AND namespace < ?2 ORDER BY namespace,key LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![root, format!("{root}/"), MAX_LIBRARY_VALUES as i64],
            |r| {
                Ok(PreferenceRecord {
                    namespace: r.get(0)?,
                    key: r.get(1)?,
                    value_json: r.get(2)?,
                })
            },
        )?;
        let mut found = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        found.retain(|record| {
            record.namespace == root
                || record
                    .namespace
                    .strip_prefix(root)
                    .is_some_and(|rest| rest.starts_with('.'))
        });
        Ok(found)
    }

    /// The present members of one synced set, in byte order.
    pub fn library_set_members(&self, set: &str) -> Result<Vec<String>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT member FROM library_set_members WHERE set_name=?1 AND present=1 \
             ORDER BY member LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![set, MAX_LIBRARY_VALUES as i64], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::organization::SetMemberRecord;
    use super::super::test_support::store;
    use super::*;

    #[test]
    fn pipelines_and_preferences_read_only_what_is_live_and_in_scope() {
        let store = store();
        let key = crate::Keyring::from_secret(&[5; 32]).item_key();
        let steps = r#"[{"transform_id":"future.step","version":9,"params":{"x":1}}]"#;
        let kept = store
            .save_transform_pipeline(
                &key,
                PipelineWrite {
                    id: None,
                    name: "beta",
                    description: None,
                    steps_json: steps,
                },
            )
            .unwrap();
        let gone = store
            .save_transform_pipeline(
                &key,
                PipelineWrite {
                    id: Some("gone"),
                    name: "Alpha",
                    description: Some("old"),
                    steps_json: "[]",
                },
            )
            .unwrap();
        assert!(store.delete_transform_pipeline(&key, &gone.id, 5).unwrap());
        assert!(!store.delete_transform_pipeline(&key, &gone.id, 6).unwrap());
        assert_eq!(store.transform_pipelines().unwrap(), vec![kept.clone()]);
        assert_eq!(
            store
                .live_transform_pipeline(&kept.id)
                .unwrap()
                .unwrap()
                .steps_json,
            steps
        );
        assert!(store.live_transform_pipeline(&gone.id).unwrap().is_none());

        let preference = |namespace: &str, key: &str| {
            Record::Preference(PreferenceRecord {
                namespace: namespace.into(),
                key: key.into(),
                value_json: "true".into(),
            })
        };
        let member = |set: &str, member: &str, present: bool| {
            Record::SetMember(SetMemberRecord {
                set: set.into(),
                member: member.into(),
                present,
            })
        };
        store
            .write_organization_records(
                &key,
                [
                    preference("transforms", "a"),
                    preference("transforms.param_defaults", "case.upper"),
                    preference("snippets", "record_pasted"),
                    member("transforms.hidden", "case.swap", true),
                    member("transforms.hidden", "case.upper", false),
                    member("snippets.other", "x", true),
                ],
            )
            .unwrap();
        let namespaces: Vec<_> = store
            .library_preferences_under("transforms")
            .unwrap()
            .into_iter()
            .map(|record| record.namespace)
            .collect();
        assert_eq!(namespaces, ["transforms", "transforms.param_defaults"]);
        assert_eq!(
            store.library_set_members("transforms.hidden").unwrap(),
            ["case.swap"]
        );
    }
}
