//! An in-memory host standing in for CopyPaste behind the `Host` boundary.
use crate::host::Host;
use copypaste_module_sdk::{
    ClipboardText, HostError, HostErrorCode, LegacySnippetPage, ModuleHostRequest, ModuleRecord,
    RecordPage, ReplacedRecord,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

const SEARCH_FIELDS: [&str; 3] = ["title", "abbreviation", "text"];

#[derive(Default)]
pub(crate) struct State {
    pub(crate) locale: String,
    pub(crate) records: BTreeMap<String, String>,
    pub(crate) local: BTreeMap<String, String>,
    pub(crate) conflicts: Vec<ReplacedRecord>,
    pub(crate) clipboard: Option<ClipboardText>,
    pub(crate) legacy: Option<LegacySnippetPage>,
    pub(crate) legacy_calls: usize,
}

#[derive(Default)]
pub(crate) struct Fake(Mutex<State>);

impl Fake {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    /// Sync replacing the local value of `key` with `remote`.
    pub(crate) fn sync_replace(&self, key: &str, remote: Value) {
        let mut state = self.state();
        let previous = state
            .records
            .insert(key.to_owned(), remote.to_string())
            .unwrap();
        let id = state.conflicts.len() as u64 + 1;
        state.conflicts.push(ReplacedRecord {
            id,
            key: key.to_owned(),
            value_json: previous,
        });
    }

    pub(crate) fn record(&self, key: &str) -> Option<Value> {
        self.state()
            .records
            .get(key)
            .map(|json| serde_json::from_str(json).unwrap())
    }
}

fn reply<T: serde::Serialize>(value: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(value).unwrap())
}

fn record(key: &str, value_json: &str) -> ModuleRecord {
    ModuleRecord {
        module_id: "copypaste.snippets".into(),
        key: key.to_owned(),
        value_json: value_json.to_owned(),
        deleted: false,
    }
}

impl Host for Fake {
    fn call(&self, request: &ModuleHostRequest) -> Result<Value, HostError> {
        request.validate()?;
        let mut state = self.state();
        match request {
            ModuleHostRequest::Context => reply(json!({ "locale": state.locale })),
            ModuleHostRequest::RecordList {
                prefix,
                after,
                limit,
            } => {
                let mut records: Vec<ModuleRecord> = state
                    .records
                    .iter()
                    .filter(|(key, _)| prefix.as_ref().is_none_or(|prefix| key.starts_with(prefix)))
                    .filter(|(key, _)| after.as_ref().is_none_or(|after| *key > after))
                    .take(*limit as usize + 1)
                    .map(|(key, value)| record(key, value))
                    .collect();
                let more = records.len() > *limit as usize;
                records.truncate(*limit as usize);
                let next = more.then(|| records.last().unwrap().key.clone());
                reply(RecordPage { records, next })
            }
            ModuleHostRequest::RecordGet { key } => {
                reply(state.records.get(key).map(|value| record(key, value)))
            }
            ModuleHostRequest::RecordPut { key, value_json } => {
                state.records.insert(key.clone(), value_json.clone());
                reply(())
            }
            ModuleHostRequest::RecordDelete { key } => {
                state.records.remove(key);
                reply(())
            }
            ModuleHostRequest::RecordSearch {
                query,
                prefix,
                limit,
            } => {
                let needle = query.to_lowercase();
                let records: Vec<ModuleRecord> = state
                    .records
                    .iter()
                    .filter(|(key, _)| prefix.as_ref().is_none_or(|prefix| key.starts_with(prefix)))
                    .filter(|(_, value)| {
                        let value: Value = serde_json::from_str(value).unwrap();
                        SEARCH_FIELDS.iter().any(|field| {
                            value[field]
                                .as_str()
                                .is_some_and(|text| text.to_lowercase().contains(&needle))
                        })
                    })
                    .take(*limit as usize)
                    .map(|(key, value)| record(key, value))
                    .collect();
                reply(RecordPage {
                    records,
                    next: None,
                })
            }
            ModuleHostRequest::RecordConflicts { limit } => reply(
                state
                    .conflicts
                    .iter()
                    .take(*limit as usize)
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            ModuleHostRequest::ClearRecordConflicts { ids } => {
                state
                    .conflicts
                    .retain(|conflict| !ids.contains(&conflict.id));
                reply(())
            }
            ModuleHostRequest::ClipboardRead => {
                reply(state.clipboard.clone().unwrap_or(ClipboardText {
                    text: None,
                    secret: false,
                }))
            }
            ModuleHostRequest::LegacySnippets { .. } => {
                state.legacy_calls += 1;
                match state.legacy.clone() {
                    Some(page) => reply(page),
                    None => Err(HostError::new(HostErrorCode::Unsupported, "Unsupported.")),
                }
            }
            ModuleHostRequest::ReadState { keys } => reply(
                keys.iter()
                    .map(|key| (key.clone(), state.local.get(key).cloned()))
                    .collect::<BTreeMap<_, _>>(),
            ),
            ModuleHostRequest::WriteState { values } => {
                state.local.extend(values.clone());
                reply(())
            }
            _ => Err(HostError::new(HostErrorCode::Unsupported, "Unsupported.")),
        }
    }
}
