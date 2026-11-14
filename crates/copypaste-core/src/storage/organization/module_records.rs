//! Synced per-module records: one whole-record last-write-wins record per
//! module and key. Deletion is a version with `deleted` set, so it syncs like
//! any other write. The projection is kept whether or not the module is
//! installed here; removing a module forgets its rows on this device only.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::super::connection::write_tx;
use super::{write, Record, Store, StoreError};

pub const MAX_MODULE_RECORD_KEY_BYTES: usize = 128;
pub const MAX_MODULE_RECORD_VALUE_BYTES: usize = 64 * 1024;
/// Live records one module may hold; arriving versions are stored regardless.
pub const MAX_MODULE_RECORDS: usize = 1024;
/// Most records one page returns.
pub const MAX_MODULE_RECORD_PAGE: u32 = 256;
/// The value may double when escaped inside the record.
pub(super) const MODULE_RECORD_BYTES: usize = 2 * MAX_MODULE_RECORD_VALUE_BYTES + 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRecord {
    pub module_id: String,
    pub key: String,
    /// Any JSON value; empty when `deleted`.
    pub value_json: String,
    pub deleted: bool,
}

pub(super) fn module_record_key(module_id: &str, key: &str) -> String {
    format!("module:{module_id}:{key}")
}

/// The module id rule of the module manifest.
fn module_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-._".contains(&b))
        && !id.contains("..")
        && !id.ends_with('.')
}

/// Record keys as modules choose them, plus the host's reserved `host.` keys.
fn record_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_MODULE_RECORD_KEY_BYTES
        && key.as_bytes()[0].is_ascii_alphanumeric()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
}

impl ModuleRecord {
    pub(super) fn valid(&self) -> bool {
        module_id(&self.module_id)
            && record_key(&self.key)
            && if self.deleted {
                self.value_json.is_empty()
            } else {
                self.value_json.len() <= MAX_MODULE_RECORD_VALUE_BYTES
                    && serde_json::from_str::<serde::de::IgnoredAny>(&self.value_json).is_ok()
            }
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO module_records(module_id,key,record_id,value_json,deleted,stamp) \
             VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(module_id,key) DO UPDATE SET \
             record_id=excluded.record_id,value_json=excluded.value_json,\
             deleted=excluded.deleted,stamp=excluded.stamp",
            params![
                self.module_id,
                self.key,
                record_id,
                self.value_json,
                self.deleted,
                stamp
            ],
        )?;
        Ok(())
    }
}

pub(super) fn module_record(
    conn: &Connection,
    module_id: &str,
    key: &str,
) -> Result<Option<ModuleRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT value_json, deleted FROM module_records WHERE module_id=?1 AND key=?2",
            [module_id, key],
            |r| {
                Ok(ModuleRecord {
                    module_id: module_id.to_owned(),
                    key: key.to_owned(),
                    value_json: r.get(0)?,
                    deleted: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// One page of a module's live records, ordered by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleRecordPage {
    pub records: Vec<ModuleRecord>,
    /// The last key returned when more may follow.
    pub next: Option<String>,
}

impl Store {
    /// A live record, or `None` for a missing or deleted one.
    pub fn module_record(
        &self,
        module_id: &str,
        key: &str,
    ) -> Result<Option<ModuleRecord>, StoreError> {
        let conn = self.conn()?;
        Ok(module_record(&conn, module_id, key)?.filter(|record| !record.deleted))
    }

    /// Live records of `module_id` whose key starts with `prefix`, after the
    /// key `after`, at most `limit` (bounded by [`MAX_MODULE_RECORD_PAGE`]).
    pub fn module_records(
        &self,
        module_id: &str,
        prefix: Option<&str>,
        after: Option<&str>,
        limit: u32,
    ) -> Result<ModuleRecordPage, StoreError> {
        let limit = limit.clamp(1, MAX_MODULE_RECORD_PAGE);
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(
            "SELECT key, value_json FROM module_records WHERE module_id=?1 AND deleted=0 \
             AND substr(key, 1, length(?2)) = ?2 AND key > ?3 ORDER BY key LIMIT ?4",
        )?;
        let mut records = statement
            .query_map(
                params![
                    module_id,
                    prefix.unwrap_or(""),
                    after.unwrap_or(""),
                    i64::from(limit) + 1
                ],
                |r| {
                    Ok(ModuleRecord {
                        module_id: module_id.to_owned(),
                        key: r.get(0)?,
                        value_json: r.get(1)?,
                        deleted: false,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = (records.len() > limit as usize).then(|| {
            records.truncate(limit as usize);
            records.last().map(|record| record.key.clone())
        });
        Ok(ModuleRecordPage {
            records,
            next: next.flatten(),
        })
    }

    /// Writes a live record. Answers `false`, writing nothing, when the key
    /// is new and the module already holds [`MAX_MODULE_RECORDS`] live ones.
    pub fn put_module_record(
        &self,
        key: &crate::ItemKey,
        module_id: &str,
        record_key: &str,
        value_json: &str,
    ) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let live: usize = tx.query_row(
            "SELECT COUNT(*) FROM module_records WHERE module_id=?1 AND deleted=0 AND key<>?2",
            [module_id, record_key],
            |r| r.get(0),
        )?;
        if live >= MAX_MODULE_RECORDS {
            return Ok(false);
        }
        write(
            &tx,
            key,
            Record::ModuleRecord(ModuleRecord {
                module_id: module_id.to_owned(),
                key: record_key.to_owned(),
                value_json: value_json.to_owned(),
                deleted: false,
            }),
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Replaces a live record with a deleted version; `false` when there was
    /// no live record.
    pub fn delete_module_record(
        &self,
        key: &crate::ItemKey,
        module_id: &str,
        record_key: &str,
    ) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        if module_record(&tx, module_id, record_key)?.is_none_or(|record| record.deleted) {
            return Ok(false);
        }
        write(
            &tx,
            key,
            Record::ModuleRecord(ModuleRecord {
                module_id: module_id.to_owned(),
                key: record_key.to_owned(),
                value_json: String::new(),
                deleted: true,
            }),
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Drops every record of a removed module from this device without
    /// writing versions; other devices keep theirs.
    pub fn forget_module_records(&self, module_id: &str) -> Result<usize, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute(
            "DELETE FROM organization_records WHERE id IN \
             (SELECT record_id FROM module_records WHERE module_id=?1)",
            [module_id],
        )?;
        let forgotten = tx.execute("DELETE FROM module_records WHERE module_id=?1", [module_id])?;
        tx.commit()?;
        Ok(forgotten)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::store;
    use super::*;

    #[test]
    fn records_page_by_key_count_only_live_ones_and_reject_bad_values() {
        let store = store();
        let key = crate::Keyring::from_secret(&[9; 32]).item_key();
        for index in 0..3 {
            assert!(store
                .put_module_record(&key, "example.tools", &format!("grant/{index}"), "{}")
                .unwrap());
        }
        store
            .put_module_record(&key, "example.tools", "other", "1")
            .unwrap();
        assert!(store
            .delete_module_record(&key, "example.tools", "grant/1")
            .unwrap());
        assert!(!store
            .delete_module_record(&key, "example.tools", "grant/1")
            .unwrap());
        let first = store
            .module_records("example.tools", Some("grant/"), None, 1)
            .unwrap();
        assert_eq!(first.records[0].key, "grant/0");
        let rest = store
            .module_records("example.tools", Some("grant/"), first.next.as_deref(), 10)
            .unwrap();
        assert_eq!(
            rest.records
                .iter()
                .map(|r| r.key.as_str())
                .collect::<Vec<_>>(),
            ["grant/2"]
        );
        assert_eq!(rest.next, None);
        assert_eq!(
            store.module_record("example.tools", "grant/1").unwrap(),
            None
        );
        assert!(store
            .module_records("other.module", None, None, 10)
            .unwrap()
            .records
            .is_empty());
        assert!(store
            .put_module_record(&key, "example.tools", "bad", "{")
            .is_err());
        assert!(store.put_module_record(&key, "Bad", "k", "1").is_err());

        store
            .conn()
            .unwrap()
            .execute_batch(&format!(
                "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < {}) \
                 INSERT INTO module_records SELECT 'full.module', 'k' || i, 'r' || i, '1', 0, 1 FROM n",
                MAX_MODULE_RECORDS - 1
            ))
            .unwrap();
        assert!(!store
            .put_module_record(&key, "full.module", "new", "1")
            .unwrap());
        assert!(store
            .put_module_record(&key, "full.module", "k0", "2")
            .unwrap());

        assert_eq!(store.forget_module_records("example.tools").unwrap(), 4);
        assert_eq!(store.module_record("example.tools", "other").unwrap(), None);
    }
}
