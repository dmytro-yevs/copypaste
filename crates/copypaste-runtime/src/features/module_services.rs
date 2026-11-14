//! The data services behind module host requests: history, snippets, the
//! clipboard and synced records.
//!
//! The module host checks the module's confirmed capabilities and request
//! limits and serves context, local state, prompts and endpoints itself;
//! everything here serves one already admitted request for one module.
//! History is filtered in SQL by the module's scope after the host's own
//! exclusions, and every text that leaves through these services has its
//! secrets redacted first.

mod history;
mod library;
mod redact;

pub use history::ModuleHistoryFilters;

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use copypaste_core::StoreError;
use copypaste_module_sdk::{
    ClipboardSource, ClipboardWritten, DeviceName, HistoryFacets, HistoryItem, HistoryPage,
    HistoryScope, HostError, HostErrorCode, ModuleProvenance, ModuleRecord, RecordPage,
    RenderedSnippet, SavedRecord, Snippet, SnippetSummary, RESERVED_RECORD_PREFIX,
};
use copypaste_modules::{HistoryQuery, ModuleHostServices};

use crate::{Runtime, RuntimeError};

pub(super) const MSG_NOT_FOUND: &str = "The item does not exist or is not available to modules.";
const MSG_RECORD_LIMIT: &str = "The module holds the most records allowed.";
const MSG_STORAGE: &str = "The module data could not be read or stored.";
const MSG_STOPPED: &str = "CopyPaste is stopping.";

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

/// The runtime as the module host's application services. It holds the
/// runtime weakly: the runtime owns the module host.
struct RuntimeServices(Weak<Runtime>);

impl RuntimeServices {
    fn runtime(&self) -> Result<Arc<Runtime>, HostError> {
        self.0
            .upgrade()
            .ok_or_else(|| HostError::new(HostErrorCode::Unavailable, MSG_STOPPED))
    }
}

impl ModuleHostServices for RuntimeServices {
    fn history_query(&self, _: &str, query: HistoryQuery) -> Result<HistoryPage, HostError> {
        self.runtime()?.module_history_query(
            &query.scope,
            &ModuleHistoryFilters {
                search: query.search,
                kind: query.kind,
                source_app: query.source_app,
                device: query.device,
                since_ms: query.since_ms,
                until_ms: query.until_ms,
                pinned_only: query.pinned_only,
            },
            query.limit,
            query.cursor.as_deref(),
        )
    }
    fn history_get(
        &self,
        _: &str,
        scope: HistoryScope,
        id: &str,
        offset: u64,
    ) -> Result<HistoryItem, HostError> {
        self.runtime()?.module_history_get(&scope, id, offset)
    }
    fn history_facets(&self, _: &str, scope: HistoryScope) -> Result<HistoryFacets, HostError> {
        self.runtime()?.module_history_facets(&scope)
    }
    fn device_names(&self, _: &str) -> Result<Vec<DeviceName>, HostError> {
        self.runtime()?.module_device_names()
    }
    fn snippet_search(
        &self,
        _: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SnippetSummary>, HostError> {
        self.runtime()?.module_snippet_search(query, limit)
    }
    fn snippet_get(&self, _: &str, id: &str) -> Result<Snippet, HostError> {
        self.runtime()?.module_snippet_get(id)
    }
    fn snippet_render(
        &self,
        _: &str,
        id: &str,
        values: BTreeMap<String, String>,
    ) -> Result<RenderedSnippet, HostError> {
        self.runtime()?.module_snippet_render(id, &values)
    }
    fn snippet_save(
        &self,
        _: &str,
        title: String,
        body: String,
        abbreviation: Option<String>,
    ) -> Result<SavedRecord, HostError> {
        self.runtime()?
            .module_snippet_save(title, body, abbreviation)
    }
    fn set_pinned(
        &self,
        _: &str,
        scope: HistoryScope,
        id: &str,
        pinned: bool,
    ) -> Result<(), HostError> {
        self.runtime()?.module_set_pinned(&scope, id, pinned)
    }
    fn clipboard_write(
        &self,
        source: ClipboardSource,
        provenance: ModuleProvenance,
    ) -> Result<ClipboardWritten, HostError> {
        self.runtime()?
            .module_clipboard_write(&provenance.module_id, &source, &provenance.label)
    }
    fn record_list(
        &self,
        module_id: &str,
        prefix: Option<String>,
        after: Option<String>,
        limit: u32,
    ) -> Result<RecordPage, HostError> {
        self.runtime()?
            .module_record_list(module_id, prefix.as_deref(), after.as_deref(), limit)
    }
    fn record_get(&self, module_id: &str, key: &str) -> Result<Option<ModuleRecord>, HostError> {
        self.runtime()?.module_record_get(module_id, key)
    }
    fn record_put(
        &self,
        module_id: &str,
        key: String,
        value_json: String,
    ) -> Result<(), HostError> {
        self.runtime()?
            .module_record_put(module_id, &key, &value_json)
    }
    fn record_delete(&self, module_id: &str, key: &str) -> Result<(), HostError> {
        self.runtime()?.module_record_delete(module_id, key)
    }
}

impl Runtime {
    /// Serves modules' history, snippet, clipboard and record requests from
    /// this runtime. Every host calls this once the runtime is shared.
    pub fn attach_module_services(self: &Arc<Self>) -> Result<(), RuntimeError> {
        self.modules
            .set_host_services(Arc::new(RuntimeServices(Arc::downgrade(self))))
            .map_err(|_| RuntimeError::Storage)
    }

    /// Live records of `module_id`, ordered by key, without the host's
    /// reserved ones.
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
            records: page
                .records
                .into_iter()
                .filter(|record| !record.key.starts_with(RESERVED_RECORD_PREFIX))
                .map(wire_record)
                .collect(),
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
    use std::sync::Mutex;

    use copypaste_core::storage::organization::{ProvenanceKind, Record, RecordKey};
    use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring};
    use copypaste_ipc::snippets::{SnippetDraft, SnippetOp, SnippetSaveOutcome};
    use copypaste_ipc::{content_type, ResponseData};
    use copypaste_module_sdk::{HistoryContent, HistoryContentKind, HOST_PREFERENCES_RECORD};

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

    const MODULE: &str = "example.tools";

    fn open(clipboard: Arc<Clipboard>) -> (Arc<Runtime>, RuntimeServices, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Arc::new(
            Runtime::open_with_keyring(
                dir.path(),
                "modules",
                0,
                Arc::new(InProcessHost(clipboard)),
                Arc::new(Keyring::from_secret(&[4; 32])),
            )
            .unwrap(),
        );
        let services = RuntimeServices(Arc::downgrade(&runtime));
        (runtime, services, dir)
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

    fn query() -> HistoryQuery {
        HistoryQuery {
            scope: scope(),
            search: None,
            kind: None,
            source_app: None,
            device: None,
            since_ms: None,
            until_ms: None,
            pinned_only: false,
            limit: 10,
            cursor: None,
        }
    }

    #[test]
    fn served_history_redacts_secrets_and_module_writes_carry_provenance() {
        let clipboard = Arc::new(Clipboard::default());
        let (runtime, services, _dir) = open(clipboard.clone());
        let words = "remember to settle the conference invoice before the release";
        let id = capture(
            &runtime,
            &format!("{words}: pay with 4111 1111 1111 1111 today"),
        );
        let expected = format!("{words}: pay with [redacted: payment card] today");

        let page = services.history_query(MODULE, query()).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].preview, expected);
        let item = services.history_get(MODULE, scope(), &id, 0).unwrap();
        let HistoryContent::Text { text, .. } = item.content else {
            panic!("text content");
        };
        assert_eq!(text, expected);
        let images_only = HistoryScope {
            content_kinds: vec![HistoryContentKind::Image],
            ..scope()
        };
        assert_eq!(
            services
                .history_get(MODULE, images_only, &id, 0)
                .unwrap_err()
                .code,
            HostErrorCode::NotFound
        );

        let written = services
            .clipboard_write(
                ClipboardSource::Text {
                    text: "a summary from the module".into(),
                },
                ModuleProvenance {
                    module_id: MODULE.into(),
                    label: "Summary".into(),
                },
            )
            .unwrap()
            .item_id
            .expect("the written text is a history item");
        assert_eq!(
            clipboard.0.lock().unwrap().last().map(String::as_str),
            Some("a summary from the module")
        );
        assert_eq!(runtime.clipboard_item().as_deref(), Some(written.as_str()));
        let Some(Record::ClipNote(note)) = runtime
            .store
            .organization_record(RecordKey::ClipNote(&written))
            .unwrap()
        else {
            panic!("a provenance note");
        };
        let provenance = note.provenance.unwrap();
        assert_eq!(provenance.kind, ProvenanceKind::Module);
        assert_eq!(provenance.module.unwrap().module_id, MODULE);
        let wire = runtime
            .store
            .item_annotations(std::slice::from_ref(&written))
            .unwrap()[&written]
            .provenance
            .clone()
            .expect("module provenance reaches clients");
        assert_eq!(wire.kind, copypaste_ipc::payload::ProvenanceKind::Module);
        assert_eq!(
            (wire.module_id.as_deref(), wire.label.as_deref()),
            (Some(MODULE), Some("Summary"))
        );

        services
            .record_put(MODULE, "grants/1".into(), r#"{"name":"Client"}"#.into())
            .unwrap();
        services
            .record_put(MODULE, HOST_PREFERENCES_RECORD.into(), "{}".into())
            .unwrap();
        let listed = services.record_list(MODULE, None, None, 10).unwrap();
        let keys: Vec<_> = listed.records.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["grants/1"], "reserved records stay the host's");
        assert!(services
            .record_get("other.module", "grants/1")
            .unwrap()
            .is_none());
    }

    #[test]
    fn modules_get_live_snippets_and_readable_refusals() {
        let (runtime, services, _dir) = open(Arc::default());
        let save = |title: &str, abbreviation: Option<&str>, body: &str, concealed: bool| {
            let draft = SnippetDraft {
                id: None,
                collection_id: None,
                title: title.into(),
                abbreviation: abbreviation.map(str::to_owned),
                body_text: body.into(),
                body_rich: None,
                template: body.contains("{{"),
                pinned: false,
                concealed,
                base_revision: None,
            };
            match runtime.snippets(0, SnippetOp::Save { draft }).data {
                Some(ResponseData::SnippetSave(SnippetSaveOutcome::Saved(detail))) => {
                    detail.snippet.id
                }
                other => panic!("unexpected {other:?}"),
            }
        };
        let greeting = save(
            "Greeting",
            Some("hi"),
            r#"Hello {{field name="who" required=true}}"#,
            false,
        );
        let quote = save("Quote", None, "> {{clipboard}}", false);
        let hidden = save("Greeting key", None, "a concealed greeting", true);

        let found = services.snippet_search(MODULE, "greeting", 10).unwrap();
        assert_eq!(
            found.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            [greeting.as_str()]
        );
        assert_eq!(
            services.snippet_get(MODULE, &hidden).unwrap_err().code,
            HostErrorCode::NotFound
        );
        let rendered = services
            .snippet_render(
                MODULE,
                &greeting,
                BTreeMap::from([("who".into(), "Ada".into())]),
            )
            .unwrap();
        assert_eq!(rendered.text, "Hello Ada");

        let refusal = |id: &str| {
            let error = services
                .snippet_render(MODULE, id, BTreeMap::new())
                .unwrap_err();
            assert_eq!(error.code, HostErrorCode::Invalid);
            error.message
        };
        assert_eq!(refusal(&greeting), "Fill in these values: who.");
        assert!(refusal(&quote).contains("clipboard"));
        let taken = services
            .snippet_save(MODULE, "Other".into(), "body".into(), Some("hi".into()))
            .unwrap_err();
        assert_eq!(taken.code, HostErrorCode::Invalid);
        assert!(taken.message.contains("abbreviation"));
    }
}
