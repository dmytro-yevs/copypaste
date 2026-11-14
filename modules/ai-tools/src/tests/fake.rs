//! An in-memory host with the contract's visibility rules, standing in for
//! CopyPaste behind the `HostClient` boundary.
use crate::host::Host;
use copypaste_module_sdk::{
    ClipboardSource, ClipboardWritten, DeviceName, HistoryContent, HistoryContentKind,
    HistoryEntry, HistoryFacet, HistoryFacets, HistoryItem, HistoryPage, HistoryScope, HostError,
    HostErrorCode, ModuleHostRequest, ModuleRecord, PromptAnswer, PromptRequest, RecordPage,
    RenderedSnippet, SavedRecord, Snippet, SnippetSummary, HISTORY_TEXT_CHUNK_CHARS,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

pub(crate) const LOCAL: &str = "desktop";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub(crate) struct FakeItem {
    pub(crate) entry: HistoryEntry,
    pub(crate) text: String,
    /// Deleted, secret or otherwise hidden by the host.
    pub(crate) hidden: bool,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) locale: String,
    pub(crate) items: Vec<FakeItem>,
    pub(crate) snippets: Vec<Snippet>,
    pub(crate) records: BTreeMap<String, String>,
    pub(crate) local: BTreeMap<String, String>,
    pub(crate) answers: VecDeque<Option<String>>,
    pub(crate) prompts: Vec<PromptRequest>,
    pub(crate) writes: Vec<(ClipboardSource, String)>,
    pub(crate) sent: Vec<(String, String)>,
    pub(crate) closed: Vec<String>,
    pub(crate) operations: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Fake(Mutex<State>);

impl Fake {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    pub(crate) fn capture(&self, text: &str, device: &str, age_ms: i64) -> String {
        let mut state = self.state();
        let id = format!("item{}", state.items.len() + 1);
        state.items.push(FakeItem {
            entry: HistoryEntry {
                id: id.clone(),
                kind: HistoryContentKind::Text,
                preview: text.chars().take(500).collect(),
                truncated: text.chars().count() > 500,
                pinned: false,
                captured_at_ms: crate::tools::now_ms() - age_ms,
                source_app_id: Some("com.editor".into()),
                source_app_name: Some("Editor".into()),
                device_id: device.into(),
                device_name: Some(device.into()),
            },
            text: text.into(),
            hidden: false,
        });
        id
    }

    pub(crate) fn answer(&self, choice: Option<&str>) {
        self.state().answers.push_back(choice.map(str::to_owned));
    }

    /// The JSON-RPC message sent on `connection` with `id`.
    pub(crate) fn reply(&self, connection: &str, id: u64) -> Option<Value> {
        self.state()
            .sent
            .iter()
            .filter(|(sent, _)| sent == connection)
            .filter_map(|(_, data)| serde_json::from_str::<Value>(data).ok())
            .find(|message| message["id"] == json!(id))
    }

    pub(crate) fn notifications(&self, connection: &str, method: &str) -> usize {
        self.state()
            .sent
            .iter()
            .filter(|(sent, data)| sent == connection && data.contains(method))
            .count()
    }
}

fn visible(item: &FakeItem, scope: &HistoryScope, now: i64) -> bool {
    let entry = &item.entry;
    !item.hidden
        && (scope.include_other_devices || entry.device_id == LOCAL)
        && !scope.excluded_devices.contains(&entry.device_id)
        && entry
            .source_app_id
            .as_ref()
            .is_none_or(|app| !scope.excluded_source_apps.contains(app))
        && scope.content_kinds.contains(&entry.kind)
        && (entry.pinned
            || scope
                .lookback_days
                .is_none_or(|days| entry.captured_at_ms >= now - i64::from(days) * DAY_MS))
}

fn error(code: HostErrorCode, message: &str) -> HostError {
    HostError::new(code, message)
}

fn reply<T: serde::Serialize>(value: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(value).unwrap())
}

impl State {
    fn find(&self, scope: &HistoryScope, id: &str) -> Result<usize, HostError> {
        let now = crate::tools::now_ms();
        self.items
            .iter()
            .position(|item| item.entry.id == id && visible(item, scope, now))
            .ok_or_else(|| error(HostErrorCode::NotFound, "Not found."))
    }

    fn render(&self, id: &str, values: &BTreeMap<String, String>) -> Result<Value, HostError> {
        let snippet = self
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
            .ok_or_else(|| error(HostErrorCode::NotFound, "Not found."))?;
        let missing: Vec<&str> = snippet
            .placeholders
            .iter()
            .filter(|name| !values.contains_key(*name))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            return Err(error(
                HostErrorCode::Invalid,
                &format!("Missing required variables: {}", missing.join(", ")),
            ));
        }
        let text = values
            .iter()
            .fold(snippet.body.clone(), |text, (name, value)| {
                text.replace(&format!("{{{{{name}}}}}"), value)
            });
        reply(RenderedSnippet { text })
    }
}

impl Host for Fake {
    fn call(&self, request: &ModuleHostRequest) -> Result<Value, HostError> {
        request.validate()?;
        let mut state = self.state();
        let operation = serde_json::to_value(request).unwrap()["operation"]
            .as_str()
            .unwrap()
            .to_owned();
        state.operations.push(operation);
        let now = crate::tools::now_ms();
        match request {
            ModuleHostRequest::Context => reply(json!({ "locale": state.locale })),
            ModuleHostRequest::DeviceNames => reply(vec![
                DeviceName {
                    id: LOCAL.into(),
                    name: "Desktop".into(),
                    local: true,
                },
                DeviceName {
                    id: "phone".into(),
                    name: "Phone".into(),
                    local: false,
                },
            ]),
            ModuleHostRequest::HistoryQuery {
                scope,
                search,
                kind,
                source_app,
                device,
                since_ms,
                until_ms,
                pinned_only,
                limit,
                cursor,
            } => {
                let start: usize = match cursor {
                    Some(cursor) => cursor
                        .parse()
                        .map_err(|_| error(HostErrorCode::Invalid, "Bad cursor."))?,
                    None => 0,
                };
                let mut matches: Vec<&FakeItem> = state
                    .items
                    .iter()
                    .filter(|item| {
                        let entry = &item.entry;
                        visible(item, scope, now)
                            && kind.is_none_or(|kind| entry.kind == kind)
                            && source_app
                                .as_ref()
                                .is_none_or(|app| entry.source_app_id.as_ref() == Some(app))
                            && device
                                .as_ref()
                                .is_none_or(|device| &entry.device_id == device)
                            && since_ms.is_none_or(|since| entry.captured_at_ms >= since)
                            && until_ms.is_none_or(|until| entry.captured_at_ms < until)
                            && (!pinned_only || entry.pinned)
                            && search.as_ref().is_none_or(|search| {
                                item.text.to_lowercase().contains(&search.to_lowercase())
                            })
                    })
                    .collect();
                matches.sort_by_key(|item| -item.entry.captured_at_ms);
                let end = (start + *limit as usize).min(matches.len());
                reply(HistoryPage {
                    items: matches[start.min(end)..end]
                        .iter()
                        .map(|item| item.entry.clone())
                        .collect(),
                    next_cursor: (end < matches.len()).then(|| end.to_string()),
                })
            }
            ModuleHostRequest::HistoryGet { scope, id, offset } => {
                let item = &state.items[state.find(scope, id)?];
                let total = item.text.chars().count() as u64;
                if *offset > total {
                    return Err(error(HostErrorCode::Invalid, "Bad offset."));
                }
                let text: String = item
                    .text
                    .chars()
                    .skip(*offset as usize)
                    .take(HISTORY_TEXT_CHUNK_CHARS)
                    .collect();
                let end = offset + text.chars().count() as u64;
                reply(HistoryItem {
                    entry: item.entry.clone(),
                    content: HistoryContent::Text {
                        text,
                        offset: *offset,
                        total_chars: total,
                        next_offset: (end < total).then_some(end),
                    },
                })
            }
            ModuleHostRequest::HistoryFacets { scope } => {
                let mut facets = HistoryFacets {
                    source_apps: Vec::new(),
                    devices: Vec::new(),
                };
                for item in state.items.iter().filter(|item| visible(item, scope, now)) {
                    let entry = &item.entry;
                    if let Some(app) = &entry.source_app_id {
                        if !facets.source_apps.iter().any(|facet| &facet.id == app) {
                            facets.source_apps.push(HistoryFacet {
                                id: app.clone(),
                                label: entry.source_app_name.clone().unwrap_or_default(),
                            });
                        }
                    }
                    if !facets
                        .devices
                        .iter()
                        .any(|facet| facet.id == entry.device_id)
                    {
                        facets.devices.push(HistoryFacet {
                            id: entry.device_id.clone(),
                            label: entry.device_name.clone().unwrap_or_default(),
                        });
                    }
                }
                reply(facets)
            }
            ModuleHostRequest::SetPinned { scope, id, pinned } => {
                let index = state.find(scope, id)?;
                state.items[index].entry.pinned = *pinned;
                reply(())
            }
            ModuleHostRequest::ClipboardWrite { source, label } => {
                let item_id = match source {
                    ClipboardSource::Item { scope, id } => {
                        state.find(scope, id)?;
                        id.clone()
                    }
                    ClipboardSource::Text { .. } => "written".to_owned(),
                };
                state.writes.push((source.clone(), label.clone()));
                reply(ClipboardWritten {
                    item_id: Some(item_id),
                })
            }
            ModuleHostRequest::SnippetSearch { query, limit } => reply(
                state
                    .snippets
                    .iter()
                    .filter(|snippet| {
                        snippet.abbreviation.as_deref() == Some(query.as_str())
                            || snippet.title.contains(query.as_str())
                    })
                    .take(*limit as usize)
                    .map(|snippet| SnippetSummary {
                        id: snippet.id.clone(),
                        title: snippet.title.clone(),
                        abbreviation: snippet.abbreviation.clone(),
                        preview: snippet.body.clone(),
                    })
                    .collect::<Vec<_>>(),
            ),
            ModuleHostRequest::SnippetGet { id } => state
                .snippets
                .iter()
                .find(|snippet| &snippet.id == id)
                .map(|snippet| reply(snippet.clone()))
                .unwrap_or_else(|| Err(error(HostErrorCode::NotFound, "Not found."))),
            ModuleHostRequest::SnippetRender { id, values } => state.render(id, values),
            ModuleHostRequest::SnippetSave {
                title,
                body,
                abbreviation,
            } => {
                if let Some(owner) = state
                    .snippets
                    .iter()
                    .find(|snippet| abbreviation.is_some() && snippet.abbreviation == *abbreviation)
                {
                    return Err(error(
                        HostErrorCode::Invalid,
                        &format!(
                            "That abbreviation is already used by \"{}\". Nothing was saved.",
                            owner.title
                        ),
                    ));
                }
                let id = format!("snippet{}", state.snippets.len() + 1);
                state.snippets.push(Snippet {
                    id: id.clone(),
                    title: title.clone(),
                    body: body.clone(),
                    abbreviation: abbreviation.clone(),
                    placeholders: Vec::new(),
                });
                reply(SavedRecord { id })
            }
            ModuleHostRequest::RecordList {
                prefix,
                after,
                limit,
            } => {
                let records: Vec<ModuleRecord> = state
                    .records
                    .iter()
                    .filter(|(key, _)| prefix.as_ref().is_none_or(|prefix| key.starts_with(prefix)))
                    .filter(|(key, _)| after.as_ref().is_none_or(|after| *key > after))
                    .take(*limit as usize + 1)
                    .map(|(key, value_json)| ModuleRecord {
                        module_id: "copypaste.ai-tools".into(),
                        key: key.clone(),
                        value_json: value_json.clone(),
                        deleted: false,
                    })
                    .collect();
                let more = records.len() > *limit as usize;
                let records: Vec<ModuleRecord> =
                    records.into_iter().take(*limit as usize).collect();
                let next = more.then(|| records.last().unwrap().key.clone());
                reply(RecordPage { records, next })
            }
            ModuleHostRequest::RecordGet { key } => {
                reply(state.records.get(key).map(|value| ModuleRecord {
                    module_id: "copypaste.ai-tools".into(),
                    key: key.clone(),
                    value_json: value.clone(),
                    deleted: false,
                }))
            }
            ModuleHostRequest::RecordPut { key, value_json } => {
                state.records.insert(key.clone(), value_json.clone());
                reply(())
            }
            ModuleHostRequest::RecordDelete { key } => {
                state.records.remove(key);
                reply(())
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
            ModuleHostRequest::ClearState { keys } => {
                for key in keys {
                    state.local.remove(key);
                }
                reply(())
            }
            ModuleHostRequest::Prompt(prompt) => {
                state.prompts.push(prompt.clone());
                let choice = state.answers.pop_front().flatten();
                reply(PromptAnswer { choice })
            }
            ModuleHostRequest::EndpointSend { connection, data } => {
                state.sent.push((connection.clone(), data.clone()));
                reply(())
            }
            ModuleHostRequest::EndpointClose { connection } => {
                state.closed.push(connection.clone());
                reply(())
            }
        }
    }
}
