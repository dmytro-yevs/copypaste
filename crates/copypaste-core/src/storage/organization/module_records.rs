//! Synced per-module records: one whole-record last-write-wins record per
//! module and key. Deletion is a version with `deleted` set, so it syncs like
//! any other write. The projection is kept whether or not the module is
//! installed here; removing a module forgets its rows on this device only.
//!
//! A module that declares storage raises its own limits, and while it is
//! configured here the store indexes its declared fields and keeps the local
//! values sync replaced. Values larger than the default bound use a record
//! class that peers predating it never receive.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::super::connection::write_tx;
use super::{write, Record, Store, StoreError};

pub const MAX_MODULE_RECORD_KEY_BYTES: usize = 128;
/// The bound of a module without a storage declaration.
pub const MAX_MODULE_RECORD_VALUE_BYTES: usize = 64 * 1024;
/// The bound any declaration may raise a value to.
pub const MAX_LIBRARY_RECORD_VALUE_BYTES: usize = 512 * 1024;
/// Live records one module may hold without a declaration; arriving versions
/// are stored regardless.
pub const MAX_MODULE_RECORDS: usize = 1024;
/// Most records one page returns.
pub const MAX_MODULE_RECORD_PAGE: u32 = 256;
/// Retained replaced values per module; the oldest go first.
pub const MAX_MODULE_RECORD_CONFLICTS: usize = 256;
/// The value may double when escaped inside the record.
pub(super) const MODULE_RECORD_BYTES: usize = 2 * MAX_LIBRARY_RECORD_VALUE_BYTES + 1024;

/// How many live records a module may hold and how large each may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleRecordLimits {
    pub max_records: usize,
    pub max_value_bytes: usize,
}

impl Default for ModuleRecordLimits {
    fn default() -> Self {
        Self {
            max_records: MAX_MODULE_RECORDS,
            max_value_bytes: MAX_MODULE_RECORD_VALUE_BYTES,
        }
    }
}

/// What a loaded module declared for its records on this device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleStorageConfig {
    /// Top-level string fields of values that `search_module_records` finds.
    pub search_fields: Vec<String>,
    pub keep_conflicts: bool,
}

/// A local value that sync replaced with a newer remote one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacedModuleRecord {
    pub id: u64,
    pub key: String,
    pub value_json: String,
}

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
                self.value_json.len() <= MAX_LIBRARY_RECORD_VALUE_BYTES
                    && serde_json::from_str::<serde::de::IgnoredAny>(&self.value_json).is_ok()
            }
    }

    /// Values beyond the default bound would be declined by every peer that
    /// predates declared storage.
    pub(super) fn large(&self) -> bool {
        self.value_json.len() > MAX_MODULE_RECORD_VALUE_BYTES
    }

    /// Keeps the live local value an arriving one replaces, when the module
    /// asked for it here.
    pub(super) fn retain_replaced(&self, tx: &Connection) -> Result<(), StoreError> {
        let keeps: bool = tx
            .query_row(
                "SELECT keep_conflicts FROM module_storage WHERE module_id=?1",
                [&self.module_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false);
        if !keeps {
            return Ok(());
        }
        let Some(local) = module_record(tx, &self.module_id, &self.key)? else {
            return Ok(());
        };
        if local.deleted || local.value_json == self.value_json {
            return Ok(());
        }
        tx.execute(
            "INSERT INTO module_record_conflicts(module_id,key,value_json) VALUES (?1,?2,?3)",
            params![self.module_id, self.key, local.value_json],
        )?;
        tx.execute(
            "DELETE FROM module_record_conflicts WHERE module_id=?1 AND id NOT IN \
             (SELECT id FROM module_record_conflicts WHERE module_id=?1 ORDER BY id DESC LIMIT ?2)",
            params![self.module_id, MAX_MODULE_RECORD_CONFLICTS as i64],
        )?;
        Ok(())
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
        let fields = search_fields(tx, &self.module_id)?;
        index(
            tx,
            &self.module_id,
            &self.key,
            &self.value_json,
            self.deleted,
            &fields,
        )
    }
}

fn search_fields(conn: &Connection, module_id: &str) -> Result<Vec<String>, StoreError> {
    let fields: Option<String> = conn
        .query_row(
            "SELECT search_fields FROM module_storage WHERE module_id=?1",
            [module_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(fields
        .and_then(|fields| serde_json::from_str(&fields).ok())
        .unwrap_or_default())
}

/// The indexed text of a value: its declared top-level string fields.
fn search_body(value_json: &str, fields: &[String]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(value_json).ok()?;
    let parts: Vec<&str> = fields
        .iter()
        .filter_map(|field| value.get(field).and_then(serde_json::Value::as_str))
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn index(
    tx: &Connection,
    module_id: &str,
    key: &str,
    value_json: &str,
    deleted: bool,
    fields: &[String],
) -> Result<(), StoreError> {
    let previous: Option<i64> = tx
        .query_row(
            "SELECT fts_rowid FROM module_record_search WHERE module_id=?1 AND key=?2",
            [module_id, key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(rowid) = previous {
        tx.execute("DELETE FROM module_record_fts WHERE rowid=?1", [rowid])?;
        tx.execute(
            "DELETE FROM module_record_search WHERE module_id=?1 AND key=?2",
            [module_id, key],
        )?;
    }
    if deleted || fields.is_empty() {
        return Ok(());
    }
    let Some(body) = search_body(value_json, fields) else {
        return Ok(());
    };
    tx.execute("INSERT INTO module_record_fts(body) VALUES (?1)", [body])?;
    tx.execute(
        "INSERT INTO module_record_search(module_id,key,fts_rowid) VALUES (?1,?2,?3)",
        params![module_id, key, tx.last_insert_rowid()],
    )?;
    Ok(())
}

fn clear_index(tx: &Connection, module_id: &str) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM module_record_fts WHERE rowid IN \
         (SELECT fts_rowid FROM module_record_search WHERE module_id=?1)",
        [module_id],
    )?;
    tx.execute(
        "DELETE FROM module_record_search WHERE module_id=?1",
        [module_id],
    )?;
    Ok(())
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

    /// Writes a live record. Answers `false`, writing nothing, when the
    /// value exceeds `limits` or the key is new and the module already holds
    /// as many live records as `limits` allow.
    pub fn put_module_record(
        &self,
        key: &crate::ItemKey,
        module_id: &str,
        record_key: &str,
        value_json: &str,
        limits: ModuleRecordLimits,
    ) -> Result<bool, StoreError> {
        if value_json.len() > limits.max_value_bytes.min(MAX_LIBRARY_RECORD_VALUE_BYTES) {
            return Ok(false);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let live: usize = tx.query_row(
            "SELECT COUNT(*) FROM module_records WHERE module_id=?1 AND deleted=0 AND key<>?2",
            [module_id, record_key],
            |r| r.get(0),
        )?;
        if live >= limits.max_records {
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
        clear_index(&tx, module_id)?;
        tx.execute("DELETE FROM module_storage WHERE module_id=?1", [module_id])?;
        tx.execute(
            "DELETE FROM module_record_conflicts WHERE module_id=?1",
            [module_id],
        )?;
        tx.commit()?;
        Ok(forgotten)
    }

    /// Registers what a loaded module declared. Indexes its live records
    /// again when the searched fields change; a module without fields is not
    /// indexed. Values replaced before `keep_conflicts` was set are not kept.
    pub fn configure_module_storage(
        &self,
        module_id: &str,
        config: &ModuleStorageConfig,
    ) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let fields =
            serde_json::to_string(&config.search_fields).map_err(|_| StoreError::InvalidSchema)?;
        let current: Option<(String, bool)> = tx
            .query_row(
                "SELECT search_fields, keep_conflicts FROM module_storage WHERE module_id=?1",
                [module_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if current.as_ref() == Some(&(fields.clone(), config.keep_conflicts)) {
            return Ok(());
        }
        tx.execute(
            "INSERT INTO module_storage(module_id,search_fields,keep_conflicts) VALUES (?1,?2,?3) \
             ON CONFLICT(module_id) DO UPDATE SET search_fields=excluded.search_fields,\
             keep_conflicts=excluded.keep_conflicts",
            params![module_id, fields, config.keep_conflicts],
        )?;
        if current.is_none_or(|(indexed, _)| indexed != fields) {
            clear_index(&tx, module_id)?;
            let records: Vec<(String, String)> = tx
                .prepare(
                    "SELECT key, value_json FROM module_records WHERE module_id=?1 AND deleted=0",
                )?
                .query_map([module_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (key, value_json) in records {
                index(
                    &tx,
                    module_id,
                    &key,
                    &value_json,
                    false,
                    &config.search_fields,
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Live records of `module_id` matching `query` in their indexed fields,
    /// most relevant first.
    pub fn search_module_records(
        &self,
        module_id: &str,
        query: &str,
        prefix: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ModuleRecord>, StoreError> {
        let Some(query) = super::super::search::sanitize_fts5_query(query) else {
            return Ok(Vec::new());
        };
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(
            "SELECT r.key, r.value_json FROM module_record_fts f \
             JOIN module_record_search s ON s.fts_rowid = f.rowid \
             JOIN module_records r ON r.module_id = s.module_id AND r.key = s.key \
             WHERE module_record_fts MATCH ?1 AND s.module_id = ?2 AND r.deleted = 0 \
             AND substr(r.key, 1, length(?3)) = ?3 ORDER BY bm25(module_record_fts) LIMIT ?4",
        )?;
        let records = statement
            .query_map(
                params![query, module_id, prefix.unwrap_or(""), i64::from(limit)],
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
        Ok(records)
    }

    /// Retained replaced values of `module_id`, oldest first.
    pub fn module_record_conflicts(
        &self,
        module_id: &str,
        limit: u32,
    ) -> Result<Vec<ReplacedModuleRecord>, StoreError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(
            "SELECT id, key, value_json FROM module_record_conflicts WHERE module_id=?1 \
             ORDER BY id LIMIT ?2",
        )?;
        let conflicts = statement
            .query_map(params![module_id, i64::from(limit)], |r| {
                Ok(ReplacedModuleRecord {
                    id: r.get(0)?,
                    key: r.get(1)?,
                    value_json: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(conflicts)
    }

    pub fn clear_module_record_conflicts(
        &self,
        module_id: &str,
        ids: &[u64],
    ) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        for id in ids {
            tx.execute(
                "DELETE FROM module_record_conflicts WHERE module_id=?1 AND id=?2",
                params![module_id, id],
            )?;
        }
        tx.commit()?;
        Ok(())
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
                .put_module_record(
                    &key,
                    "example.tools",
                    &format!("grant/{index}"),
                    "{}",
                    ModuleRecordLimits::default()
                )
                .unwrap());
        }
        store
            .put_module_record(
                &key,
                "example.tools",
                "other",
                "1",
                ModuleRecordLimits::default(),
            )
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
            .put_module_record(
                &key,
                "example.tools",
                "bad",
                "{",
                ModuleRecordLimits::default()
            )
            .is_err());
        assert!(store
            .put_module_record(&key, "Bad", "k", "1", ModuleRecordLimits::default())
            .is_err());

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
            .put_module_record(
                &key,
                "full.module",
                "new",
                "1",
                ModuleRecordLimits::default()
            )
            .unwrap());
        assert!(store
            .put_module_record(
                &key,
                "full.module",
                "k0",
                "2",
                ModuleRecordLimits::default()
            )
            .unwrap());

        assert_eq!(store.forget_module_records("example.tools").unwrap(), 4);
        assert_eq!(store.module_record("example.tools", "other").unwrap(), None);
    }
}
