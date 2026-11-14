//! The data services behind module host requests: history, snippets, the
//! clipboard, synced records and local state.
//!
//! Hosts check the module's confirmed capabilities and own the context,
//! prompt and endpoint requests; everything here serves one already
//! admitted request for one module. History is filtered in SQL by the
//! module's scope after the host's own exclusions, and every text that
//! leaves through these services has its secrets redacted first.

mod history;
mod library;
mod redact;

pub use history::ModuleHistoryFilters;

use copypaste_core::StoreError;
use copypaste_module_sdk::{HostError, HostErrorCode, ModuleHostRequest, ModuleRecord, RecordPage};
use serde_json::Value;

use crate::Runtime;

pub(super) const MSG_NOT_FOUND: &str = "The item does not exist or is not available to modules.";
const MSG_RECORD_LIMIT: &str = "The module holds the most records allowed.";
const MSG_HOST_ONLY: &str = "This request is served by the module host.";
const MSG_STORAGE: &str = "The module data could not be read or stored.";

pub(super) fn not_found() -> HostError {
    HostError::new(HostErrorCode::NotFound, MSG_NOT_FOUND)
}

pub(super) fn invalid(message: &str) -> HostError {
    HostError::new(HostErrorCode::Invalid, message)
}

pub(super) fn unavailable(error: &dyn std::fmt::Debug) -> HostError {
    tracing::warn!(?error, "a module host service failed");
    HostError::new(HostErrorCode::Unavailable, MSG_STORAGE)
}

fn storage(error: StoreError) -> HostError {
    match error {
        StoreError::InvalidCursor | StoreError::InvalidHistoryQuery => {
            invalid("The request is not valid.")
        }
        error => unavailable(&error),
    }
}

fn reply<T: serde::Serialize>(value: &T) -> Result<Value, HostError> {
    serde_json::to_value(value).map_err(|error| unavailable(&error))
}

impl Runtime {
    /// Serves one data request of `module_id` and answers its JSON reply.
    /// `Context`, `Prompt` and the endpoint requests belong to the host and
    /// answer `unsupported` here; capabilities are checked by the caller.
    pub fn serve_module_request(
        &self,
        module_id: &str,
        request: ModuleHostRequest,
    ) -> Result<Value, HostError> {
        request.validate()?;
        match request {
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
            } => reply(&self.module_history_query(
                &scope,
                &ModuleHistoryFilters {
                    search,
                    kind,
                    source_app,
                    device,
                    since_ms,
                    until_ms,
                    pinned_only,
                },
                limit,
                cursor.as_deref(),
            )?),
            ModuleHostRequest::HistoryGet { scope, id, offset } => {
                reply(&self.module_history_get(&scope, &id, offset)?)
            }
            ModuleHostRequest::HistoryFacets { scope } => {
                reply(&self.module_history_facets(&scope)?)
            }
            ModuleHostRequest::DeviceNames => reply(&self.module_device_names()?),
            ModuleHostRequest::SnippetSearch { query, limit } => {
                reply(&self.module_snippet_search(&query, limit)?)
            }
            ModuleHostRequest::SnippetGet { id } => reply(&self.module_snippet_get(&id)?),
            ModuleHostRequest::SnippetRender { id, values } => {
                reply(&self.module_snippet_render(&id, &values)?)
            }
            ModuleHostRequest::SnippetSave {
                title,
                body,
                abbreviation,
            } => reply(&self.module_snippet_save(title, body, abbreviation)?),
            ModuleHostRequest::SetPinned { scope, id, pinned } => {
                self.module_set_pinned(&scope, &id, pinned)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::ClipboardWrite { source, label } => {
                self.module_clipboard_write(module_id, &source, &label)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::RecordList {
                prefix,
                after,
                limit,
            } => reply(&self.module_record_list(
                module_id,
                prefix.as_deref(),
                after.as_deref(),
                limit,
            )?),
            ModuleHostRequest::RecordGet { key } => {
                reply(&self.module_record_get(module_id, &key)?)
            }
            ModuleHostRequest::RecordPut { key, value_json } => {
                self.module_record_put(module_id, &key, &value_json)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::RecordDelete { key } => {
                self.module_record_delete(module_id, &key)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::ReadState { keys } => {
                reply(&self.store.module_state(module_id, &keys).map_err(storage)?)
            }
            ModuleHostRequest::WriteState { values } => {
                self.store
                    .set_module_state(module_id, &values)
                    .map_err(storage)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::ClearState { keys } => {
                self.store
                    .clear_module_state_keys(module_id, &keys)
                    .map_err(storage)?;
                Ok(Value::Null)
            }
            ModuleHostRequest::Context
            | ModuleHostRequest::Prompt(_)
            | ModuleHostRequest::EndpointSend { .. }
            | ModuleHostRequest::EndpointClose { .. } => {
                Err(HostError::new(HostErrorCode::Unsupported, MSG_HOST_ONLY))
            }
        }
    }

    /// Live records of `module_id`, ordered by key.
    pub fn module_record_list(
        &self,
        module_id: &str,
        prefix: Option<&str>,
        after: Option<&str>,
        limit: u32,
    ) -> Result<RecordPage, HostError> {
        let page = self
            .store
            .module_records(module_id, prefix, after, limit)
            .map_err(storage)?;
        Ok(RecordPage {
            records: page.records.into_iter().map(wire_record).collect(),
            next: page.next,
        })
    }

    pub fn module_record_get(
        &self,
        module_id: &str,
        key: &str,
    ) -> Result<Option<ModuleRecord>, HostError> {
        Ok(self
            .store
            .module_record(module_id, key)
            .map_err(storage)?
            .map(wire_record))
    }

    /// Writes a record, including the host's reserved `host.` keys; callers
    /// serving a module refuse those first (`ModuleHostRequest::validate`).
    pub fn module_record_put(
        &self,
        module_id: &str,
        key: &str,
        value_json: &str,
    ) -> Result<(), HostError> {
        let started = copypaste_core::now_ms();
        if !self
            .store
            .put_module_record(&self.keyring.item_key(), module_id, key, value_json)
            .map_err(storage)?
        {
            return Err(invalid(MSG_RECORD_LIMIT));
        }
        self.module_records_written(started);
        Ok(())
    }

    /// Leaves a synced deletion; deleting a missing record succeeds.
    pub fn module_record_delete(&self, module_id: &str, key: &str) -> Result<(), HostError> {
        let started = copypaste_core::now_ms();
        if self
            .store
            .delete_module_record(&self.keyring.item_key(), module_id, key)
            .map_err(storage)?
        {
            self.module_records_written(started);
        }
        Ok(())
    }

    /// Records travel like library records, without a library event.
    fn module_records_written(&self, since: i64) {
        self.p2p.node().note_local_version(since);
        self.p2p.wake();
        self.modules.note_version(since);
    }
}

fn wire_record(record: copypaste_core::storage::organization::ModuleRecord) -> ModuleRecord {
    ModuleRecord {
        module_id: record.module_id,
        key: record.key,
        value_json: record.value_json,
        deleted: record.deleted,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use copypaste_core::storage::organization::{ProvenanceKind, Record, RecordKey};
    use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring};
    use copypaste_ipc::content_type;
    use copypaste_module_sdk::{ClipboardSource, HistoryContent, HistoryContentKind, HistoryScope};

    use super::*;
    use crate::host::{ClipboardWriter, InProcessHost, WriteOptions};

    #[derive(Default)]
    struct Clipboard(Mutex<Vec<String>>);

    impl ClipboardWriter for Clipboard {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _: &str,
            _: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            if let ClipboardPayload::Text(text) = payload {
                self.0.lock().unwrap().push(text.as_str().to_owned());
            }
            Ok(())
        }
    }

    fn scope() -> HistoryScope {
        HistoryScope {
            include_other_devices: false,
            excluded_devices: Vec::new(),
            excluded_source_apps: Vec::new(),
            content_kinds: vec![HistoryContentKind::Text, HistoryContentKind::Link],
            lookback_days: None,
        }
    }

    fn capture(runtime: &Runtime, text: &str) -> String {
        copypaste_core::ingest::ingest(
            &runtime.store,
            &runtime.keyring,
            text,
            content_type::TEXT,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item()
        .id
    }

    fn call(runtime: &Runtime, request: ModuleHostRequest) -> Result<Value, HostError> {
        runtime.serve_module_request("example.tools", request)
    }

    #[test]
    fn served_history_redacts_secrets_and_module_writes_carry_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(Clipboard::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "modules",
            0,
            Arc::new(InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[4; 32])),
        )
        .unwrap();
        let words = "remember to settle the conference invoice before the release";
        let id = capture(
            &runtime,
            &format!("{words}: pay with 4111 1111 1111 1111 today"),
        );
        let expected = format!("{words}: pay with [redacted: payment card] today");

        let page = runtime
            .module_history_query(&scope(), &ModuleHistoryFilters::default(), 10, None)
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].preview, expected);
        let item = runtime.module_history_get(&scope(), &id, 0).unwrap();
        let HistoryContent::Text { text, .. } = item.content else {
            panic!("text content");
        };
        assert_eq!(text, expected);
        let images_only = HistoryScope {
            content_kinds: vec![HistoryContentKind::Image],
            ..scope()
        };
        assert_eq!(
            runtime
                .module_history_get(&images_only, &id, 0)
                .unwrap_err()
                .code,
            HostErrorCode::NotFound
        );

        call(
            &runtime,
            ModuleHostRequest::ClipboardWrite {
                source: ClipboardSource::Text {
                    text: "a summary from the module".into(),
                },
                label: "Summary".into(),
            },
        )
        .unwrap();
        assert_eq!(
            clipboard.0.lock().unwrap().last().map(String::as_str),
            Some("a summary from the module")
        );
        let written = runtime.clipboard_item().unwrap();
        let Some(Record::ClipNote(note)) = runtime
            .store
            .organization_record(RecordKey::ClipNote(&written))
            .unwrap()
        else {
            panic!("a provenance note");
        };
        let provenance = note.provenance.unwrap();
        assert_eq!(provenance.kind, ProvenanceKind::Module);
        assert_eq!(provenance.module.unwrap().module_id, "example.tools");

        call(
            &runtime,
            ModuleHostRequest::RecordPut {
                key: "grants/1".into(),
                value_json: r#"{"name":"Client"}"#.into(),
            },
        )
        .unwrap();
        let listed = call(
            &runtime,
            ModuleHostRequest::RecordList {
                prefix: Some("grants/".into()),
                after: None,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(listed["records"][0]["key"], "grants/1");
        assert!(runtime
            .serve_module_request(
                "other.module",
                ModuleHostRequest::RecordGet {
                    key: "grants/1".into()
                }
            )
            .unwrap()
            .is_null());
        assert_eq!(
            call(
                &runtime,
                ModuleHostRequest::RecordPut {
                    key: "host.preferences".into(),
                    value_json: "{}".into(),
                },
            )
            .unwrap_err()
            .code,
            HostErrorCode::Invalid
        );
    }
}
