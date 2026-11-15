//! Declared storage for modules that keep a whole library in synced records.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Host ceiling for `ModuleStorage::max_records`.
pub const MAX_STORAGE_RECORDS: u32 = 50_000;
/// Host ceiling for `ModuleStorage::max_value_bytes`; also the bound
/// `ModuleHostRequest::validate` applies to every `record_put`.
pub const MAX_STORAGE_VALUE_BYTES: u32 = 512 * 1024;
pub const MAX_SEARCH_FIELDS: usize = 8;
pub const MAX_SEARCH_PAGE: u32 = 200;
/// Replaced values the host retains per module until the module clears them.
pub const MAX_RETAINED_CONFLICTS: usize = 256;

/// Raises the synced record limits of one module and asks the host to index
/// and retain what a library needs (schema 7, requires `synced_records`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleStorage {
    pub max_records: u32,
    pub max_value_bytes: u32,
    /// Top-level string fields of record values indexed for `record_search`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_fields: Vec<String>,
    /// Keep local values that sync replaced, for `record_conflicts`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_conflicts: bool,
}

impl ModuleStorage {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut fields = BTreeSet::new();
        if !(1..=MAX_STORAGE_RECORDS).contains(&self.max_records)
            || !(1..=MAX_STORAGE_VALUE_BYTES).contains(&self.max_value_bytes)
            || self.search_fields.len() > MAX_SEARCH_FIELDS
            || self
                .search_fields
                .iter()
                .any(|field| !crate::valid_id(field) || !fields.insert(field))
        {
            return Err("The module storage declaration is invalid.".into());
        }
        Ok(())
    }
}
