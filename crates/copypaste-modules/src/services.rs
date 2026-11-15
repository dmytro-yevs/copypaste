//! Application services behind schema 6 and 7 `ModuleHostRequest`s. The runtime
//! supplies the implementation; every method defaults to `unsupported`, so a
//! host serves exactly what it implements. Capability checks, request limits,
//! state, prompts and endpoints are handled before these methods are called.
use copypaste_module_sdk::{
    native::to_exact_vec, ClipboardSource, ClipboardText, ClipboardWritten, DeviceName,
    HistoryContentKind, HistoryFacets, HistoryItem, HistoryPage, HistoryScope, HostError,
    HostErrorCode, LegacySnippetPage, ModuleHostRequest, ModuleProvenance, ModuleRecord,
    ModuleStorage, RecordPage, RenderedSnippet, ReplacedRecord, SavedRecord, Snippet,
    SnippetSummary, MAX_RECORDS, MAX_RECORD_VALUE_BYTES,
};
use std::collections::BTreeMap;

/// The fields of `ModuleHostRequest::HistoryQuery`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryQuery {
    pub scope: HistoryScope,
    pub search: Option<String>,
    pub kind: Option<HistoryContentKind>,
    pub source_app: Option<String>,
    pub device: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub pinned_only: bool,
    pub limit: u32,
    pub cursor: Option<String>,
}

/// The record bounds of one module: the defaults, or what it declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordLimits {
    pub max_records: usize,
    pub max_value_bytes: usize,
}

impl RecordLimits {
    #[must_use]
    pub fn of(storage: Option<&ModuleStorage>) -> Self {
        storage.map_or(
            Self {
                max_records: MAX_RECORDS,
                max_value_bytes: MAX_RECORD_VALUE_BYTES,
            },
            |storage| Self {
                max_records: storage.max_records as usize,
                max_value_bytes: storage.max_value_bytes as usize,
            },
        )
    }
}

fn unsupported<T>() -> Result<T, HostError> {
    Err(HostError::new(
        HostErrorCode::Unsupported,
        "This host service is not available.",
    ))
}

/// `module_id` is always the calling module, set by the host.
pub trait ModuleHostServices: Send + Sync {
    fn history_query(
        &self,
        _module_id: &str,
        _query: HistoryQuery,
    ) -> Result<HistoryPage, HostError> {
        unsupported()
    }
    fn history_get(
        &self,
        _module_id: &str,
        _scope: HistoryScope,
        _id: &str,
        _offset: u64,
    ) -> Result<HistoryItem, HostError> {
        unsupported()
    }
    fn history_facets(
        &self,
        _module_id: &str,
        _scope: HistoryScope,
    ) -> Result<HistoryFacets, HostError> {
        unsupported()
    }
    fn device_names(&self, _module_id: &str) -> Result<Vec<DeviceName>, HostError> {
        unsupported()
    }
    fn snippet_search(
        &self,
        _module_id: &str,
        _query: &str,
        _limit: u32,
    ) -> Result<Vec<SnippetSummary>, HostError> {
        unsupported()
    }
    fn snippet_get(&self, _module_id: &str, _id: &str) -> Result<Snippet, HostError> {
        unsupported()
    }
    fn snippet_render(
        &self,
        _module_id: &str,
        _id: &str,
        _values: BTreeMap<String, String>,
    ) -> Result<RenderedSnippet, HostError> {
        unsupported()
    }
    fn snippet_save(
        &self,
        _module_id: &str,
        _title: String,
        _body: String,
        _abbreviation: Option<String>,
    ) -> Result<SavedRecord, HostError> {
        unsupported()
    }
    fn set_pinned(
        &self,
        _module_id: &str,
        _scope: HistoryScope,
        _id: &str,
        _pinned: bool,
    ) -> Result<(), HostError> {
        unsupported()
    }
    fn clipboard_write(
        &self,
        _source: ClipboardSource,
        _provenance: ModuleProvenance,
    ) -> Result<ClipboardWritten, HostError> {
        unsupported()
    }
    fn record_list(
        &self,
        _module_id: &str,
        _prefix: Option<String>,
        _after: Option<String>,
        _limit: u32,
    ) -> Result<RecordPage, HostError> {
        unsupported()
    }
    fn record_get(&self, _module_id: &str, _key: &str) -> Result<Option<ModuleRecord>, HostError> {
        unsupported()
    }
    fn record_put(
        &self,
        _module_id: &str,
        _key: String,
        _value_json: String,
        _limits: RecordLimits,
    ) -> Result<(), HostError> {
        unsupported()
    }
    fn record_delete(&self, _module_id: &str, _key: &str) -> Result<(), HostError> {
        unsupported()
    }
    fn record_search(
        &self,
        _module_id: &str,
        _query: &str,
        _prefix: Option<String>,
        _limit: u32,
    ) -> Result<RecordPage, HostError> {
        unsupported()
    }
    fn record_conflicts(
        &self,
        _module_id: &str,
        _limit: u32,
    ) -> Result<Vec<ReplacedRecord>, HostError> {
        unsupported()
    }
    fn clear_record_conflicts(&self, _module_id: &str, _ids: &[u64]) -> Result<(), HostError> {
        unsupported()
    }
    fn clipboard_read(&self, _module_id: &str) -> Result<ClipboardText, HostError> {
        unsupported()
    }
    fn legacy_snippets(
        &self,
        _module_id: &str,
        _after: Option<String>,
        _limit: u32,
    ) -> Result<LegacySnippetPage, HostError> {
        unsupported()
    }
    /// Registers a loaded module's storage declaration.
    fn configure_storage(&self, _module_id: &str, _storage: &ModuleStorage) {}
}

/// Serves one of the requests above. Requests the bridge handles itself are
/// refused as unsupported here.
pub(crate) fn serve(
    services: &dyn ModuleHostServices,
    module_id: &str,
    limits: RecordLimits,
    request: ModuleHostRequest,
) -> Result<Vec<u8>, HostError> {
    use ModuleHostRequest as R;
    fn reply<T: serde::Serialize>(value: Result<T, HostError>) -> Result<Vec<u8>, HostError> {
        to_exact_vec(&value?, copypaste_module_sdk::native::MAX_HOST_BYTES).map_err(|_| {
            HostError::new(
                HostErrorCode::Unavailable,
                "The host reply exceeds its limit.",
            )
        })
    }
    match request {
        R::HistoryQuery {
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
        } => reply(services.history_query(
            module_id,
            HistoryQuery {
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
            },
        )),
        R::HistoryGet { scope, id, offset } => {
            reply(services.history_get(module_id, scope, &id, offset))
        }
        R::HistoryFacets { scope } => reply(services.history_facets(module_id, scope)),
        R::DeviceNames => reply(services.device_names(module_id)),
        R::SnippetSearch { query, limit } => {
            reply(services.snippet_search(module_id, &query, limit))
        }
        R::SnippetGet { id } => reply(services.snippet_get(module_id, &id)),
        R::SnippetRender { id, values } => reply(services.snippet_render(module_id, &id, values)),
        R::SnippetSave {
            title,
            body,
            abbreviation,
        } => reply(services.snippet_save(module_id, title, body, abbreviation)),
        R::SetPinned { scope, id, pinned } => {
            reply(services.set_pinned(module_id, scope, &id, pinned))
        }
        R::ClipboardWrite { source, label } => reply(services.clipboard_write(
            source,
            ModuleProvenance {
                module_id: module_id.into(),
                label,
            },
        )),
        R::RecordList {
            prefix,
            after,
            limit,
        } => reply(services.record_list(module_id, prefix, after, limit)),
        R::RecordGet { key } => reply(services.record_get(module_id, &key)),
        R::RecordPut { key, value_json } => {
            reply(services.record_put(module_id, key, value_json, limits))
        }
        R::RecordDelete { key } => reply(services.record_delete(module_id, &key)),
        R::RecordSearch {
            query,
            prefix,
            limit,
        } => reply(services.record_search(module_id, &query, prefix, limit)),
        R::RecordConflicts { limit } => reply(services.record_conflicts(module_id, limit)),
        R::ClearRecordConflicts { ids } => reply(services.clear_record_conflicts(module_id, &ids)),
        R::ClipboardRead => reply(services.clipboard_read(module_id)),
        R::LegacySnippets { after, limit } => {
            reply(services.legacy_snippets(module_id, after, limit))
        }
        R::Context
        | R::ReadState { .. }
        | R::WriteState { .. }
        | R::ClearState { .. }
        | R::Prompt(_)
        | R::EndpointSend { .. }
        | R::EndpointClose { .. } => unsupported(),
    }
}
