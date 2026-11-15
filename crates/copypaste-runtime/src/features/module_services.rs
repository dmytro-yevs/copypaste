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
mod provided;
mod redact;

pub use history::ModuleHistoryFilters;

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use copypaste_core::storage::legacy_snippets::{LegacyCollectionRow, LegacySnippetRow};
use copypaste_core::storage::organization::{ModuleRecordLimits, ModuleStorageConfig};
use copypaste_core::StoreError;
use copypaste_module_sdk::{
    ClipboardSource, ClipboardText, ClipboardWritten, DeviceName, HistoryFacets, HistoryItem,
    HistoryPage, HistoryScope, HostError, HostErrorCode, LegacyCollection, LegacySnippet,
    LegacySnippetPage, ModuleHostRequest, ModuleProvenance, ModuleRecord, ModuleStorage,
    RecordPage, RenderedSnippet, ReplacedRecord, SavedRecord, Snippet, SnippetSummary,
    RESERVED_RECORD_PREFIX,
};
use copypaste_modules::{HistoryQuery, ModuleHostServices, RecordLimits};

use crate::{Runtime, RuntimeError};

pub(super) const MSG_NOT_FOUND: &str = "The item does not exist or is not available to modules.";
const MSG_RECORD_LIMIT: &str = "The module holds the most records allowed.";
const MSG_STORAGE: &str = "The module data could not be read or stored.";
const MSG_STOPPED: &str = "CopyPaste is stopping.";
const MSG_NO_SNIPPETS: &str = "Install the Snippets module to use snippets.";

pub(super) fn not_found() -> HostError {
    HostError::new(HostErrorCode::NotFound, MSG_NOT_FOUND)
}

/// Snippet requests are answered only by the module that provides snippets.
fn no_snippets() -> HostError {
    HostError::new(HostErrorCode::Unavailable, MSG_NO_SNIPPETS)
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
        module_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SnippetSummary>, HostError> {
        let runtime = self.runtime()?;
        let request = ModuleHostRequest::SnippetSearch {
            query: query.to_owned(),
            limit,
        };
        match runtime.provided(module_id, &request) {
            Some(found) => found.map(provided::summaries),
            None => Err(no_snippets()),
        }
    }
    fn snippet_get(&self, module_id: &str, id: &str) -> Result<Snippet, HostError> {
        let runtime = self.runtime()?;
        let request = ModuleHostRequest::SnippetGet { id: id.to_owned() };
        match runtime.provided(module_id, &request) {
            Some(found) => found.and_then(provided::snippet),
            None => Err(no_snippets()),
        }
    }
    fn snippet_render(
        &self,
        module_id: &str,
        id: &str,
        values: BTreeMap<String, String>,
    ) -> Result<RenderedSnippet, HostError> {
        let runtime = self.runtime()?;
        let request = ModuleHostRequest::SnippetRender {
            id: id.to_owned(),
            values,
        };
        match runtime.provided(module_id, &request) {
            Some(found) => found.and_then(provided::rendered),
            None => Err(no_snippets()),
        }
    }
    fn snippet_save(
        &self,
        module_id: &str,
        title: String,
        body: String,
        abbreviation: Option<String>,
    ) -> Result<SavedRecord, HostError> {
        let runtime = self.runtime()?;
        let request = ModuleHostRequest::SnippetSave {
            title,
            body,
            abbreviation,
        };
        runtime
            .provided(module_id, &request)
            .unwrap_or_else(|| Err(no_snippets()))
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
        limits: RecordLimits,
    ) -> Result<(), HostError> {
        self.runtime()?
            .module_record_put(module_id, &key, &value_json, limits)
    }
    fn record_delete(&self, module_id: &str, key: &str) -> Result<(), HostError> {
        self.runtime()?.module_record_delete(module_id, key)
    }
    fn record_search(
        &self,
        module_id: &str,
        query: &str,
        prefix: Option<String>,
        limit: u32,
    ) -> Result<RecordPage, HostError> {
        let records = self
            .runtime()?
            .store
            .search_module_records(module_id, query, prefix.as_deref(), limit)
            .map_err(storage)?;
        Ok(RecordPage {
            records: records
                .into_iter()
                .filter(|record| !record.key.starts_with(RESERVED_RECORD_PREFIX))
                .map(wire_record)
                .collect(),
            next: None,
        })
    }
    fn record_conflicts(
        &self,
        module_id: &str,
        limit: u32,
    ) -> Result<Vec<ReplacedRecord>, HostError> {
        let kept = self
            .runtime()?
            .store
            .module_record_conflicts(module_id, limit)
            .map_err(storage)?;
        Ok(kept
            .into_iter()
            .map(|record| ReplacedRecord {
                id: record.id,
                key: record.key,
                value_json: record.value_json,
            })
            .collect())
    }
    fn clear_record_conflicts(&self, module_id: &str, ids: &[u64]) -> Result<(), HostError> {
        self.runtime()?
            .store
            .clear_module_record_conflicts(module_id, ids)
            .map_err(storage)
    }
    fn clipboard_read(&self, _: &str) -> Result<ClipboardText, HostError> {
        let runtime = self.runtime()?;
        let text = runtime
            .user_clipboard_item()
            .and_then(|item| runtime.clip_plain_text(&item));
        Ok(ClipboardText {
            secret: text.as_ref().is_some_and(|(_, secret)| *secret),
            text: text.map(|(text, _)| text),
        })
    }
    fn legacy_snippets(
        &self,
        module_id: &str,
        after: Option<String>,
        limit: u32,
    ) -> Result<LegacySnippetPage, HostError> {
        let runtime = self.runtime()?;
        if after.is_none() {
            runtime.seed_legacy_snippet_preferences(module_id)?;
        }
        let page = runtime
            .store
            .legacy_snippet_page(after.as_deref(), limit)
            .map_err(storage)?;
        Ok(LegacySnippetPage {
            collections: page
                .collections
                .into_iter()
                .map(legacy_collection)
                .collect(),
            snippets: page.snippets.into_iter().map(legacy_snippet).collect(),
            next: page.next,
        })
    }
    fn configure_storage(&self, module_id: &str, declared: &ModuleStorage) {
        let Ok(runtime) = self.runtime() else {
            return;
        };
        let config = ModuleStorageConfig {
            search_fields: declared.search_fields.clone(),
            keep_conflicts: declared.keep_conflicts,
        };
        if let Err(error) = runtime.store.configure_module_storage(module_id, &config) {
            tracing::warn!(?error, module_id, "module storage could not be configured");
        }
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
        limits: RecordLimits,
    ) -> Result<(), HostError> {
        let started = copypaste_core::now_ms();
        let limits = ModuleRecordLimits {
            max_records: limits.max_records,
            max_value_bytes: limits.max_value_bytes,
        };
        if !self
            .store
            .put_module_record(&self.keyring.item_key(), module_id, key, value_json, limits)
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

    /// The module that takes over the library starts from the app's own
    /// synced snippet preferences, unless its preferences already exist.
    fn seed_legacy_snippet_preferences(&self, module_id: &str) -> Result<(), HostError> {
        use copypaste_core::storage::organization::{Record, RecordKey};
        use copypaste_module_sdk::HOST_PREFERENCES_RECORD;
        if self
            .store
            .module_record(module_id, HOST_PREFERENCES_RECORD)
            .map_err(storage)?
            .is_some()
        {
            return Ok(());
        }
        let mut seeded = serde_json::Map::new();
        for (legacy, field) in [
            ("record_pasted", "record-pasted"),
            ("require_auth", "require-auth"),
        ] {
            let record = self
                .store
                .organization_record(RecordKey::Preference {
                    namespace: "snippets",
                    key: legacy,
                })
                .map_err(storage)?;
            if let Some(Record::Preference(preference)) = record {
                if let Ok(serde_json::Value::Bool(value)) =
                    serde_json::from_str(&preference.value_json)
                {
                    seeded.insert(field.to_owned(), value.into());
                }
            }
        }
        if seeded.is_empty() {
            return Ok(());
        }
        self.module_record_put(
            module_id,
            HOST_PREFERENCES_RECORD,
            &serde_json::Value::Object(seeded).to_string(),
            RecordLimits {
                max_records: usize::MAX,
                max_value_bytes: copypaste_module_sdk::MAX_RECORD_VALUE_BYTES,
            },
        )
    }

    /// Records travel like library records, without a library event.
    /// Native surfaces (the Android widget) may list module results.
    fn module_records_written(&self, since: i64) {
        self.p2p.node().note_local_version(since);
        self.p2p.wake();
        self.modules.note_version(since);
        self.host.surfaces_changed();
    }
}

fn legacy_collection(row: LegacyCollectionRow) -> LegacyCollection {
    LegacyCollection {
        id: row.id,
        name: row.name,
        icon: row.icon,
        accent: row.accent,
        order_key: row.order_key,
        deleted_at_ms: row.deleted_at_ms,
    }
}

fn legacy_snippet(row: LegacySnippetRow) -> LegacySnippet {
    LegacySnippet {
        id: row.id,
        collection_id: row.collection_id,
        title: row.title,
        abbreviation: row.abbreviation,
        template: row.template,
        pinned: row.pinned,
        pin_order: row.pin_order,
        order_key: row.order_key,
        concealed: row.concealed,
        deleted_at_ms: row.deleted_at_ms,
        created_at_ms: row.created_at_ms,
        created_device: row.created_device,
        revision: row.revision,
        ancestors: row.ancestors,
        text: row.text,
        rich_html: row.rich_html,
        use_count: row.use_count,
        last_used_at_ms: row.last_used_at_ms,
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

    use copypaste_core::storage::organization::{
        ProvenanceKind, Record, RecordKey, SnippetBodyRecord, SnippetRecord,
    };
    use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring};
    use copypaste_ipc::content_type;
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
            .record_put(
                MODULE,
                "grants/1".into(),
                r#"{"name":"Client"}"#.into(),
                RecordLimits::of(None),
            )
            .unwrap();
        services
            .record_put(
                MODULE,
                HOST_PREFERENCES_RECORD.into(),
                "{}".into(),
                RecordLimits::of(None),
            )
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
    fn snippets_need_a_provider_and_the_legacy_library_is_read_from_synced_records() {
        let (runtime, services, _dir) = open(Arc::default());
        let unavailable = |error: HostError| {
            assert_eq!(error.code, HostErrorCode::Unavailable);
            assert_eq!(error.message, MSG_NO_SNIPPETS);
        };
        unavailable(services.snippet_search(MODULE, "greeting", 10).unwrap_err());
        unavailable(services.snippet_get(MODULE, "s1").unwrap_err());
        unavailable(
            services
                .snippet_render(MODULE, "s1", BTreeMap::new())
                .unwrap_err(),
        );
        unavailable(
            services
                .snippet_save(MODULE, "Title".into(), "body".into(), None)
                .unwrap_err(),
        );

        let key = runtime.keyring.item_key();
        for (id, concealed, text) in [("s1", false, "Kind regards"), ("s2", true, "secret")] {
            let records = [
                Record::Snippet(SnippetRecord {
                    id: id.into(),
                    collection_id: None,
                    title: id.into(),
                    abbreviation: None,
                    template: false,
                    pinned: false,
                    pin_order: None,
                    order_key: "a".into(),
                    concealed,
                    deleted_at: None,
                    purged: false,
                    created_at: 1,
                    created_device: "laptop".into(),
                }),
                Record::SnippetBody(SnippetBodyRecord {
                    id: id.into(),
                    revision: "a".repeat(32),
                    ancestors: Vec::new(),
                    text: text.into(),
                    rich_html: None,
                }),
            ];
            for record in records {
                runtime
                    .store
                    .write_organization_record(&key, record)
                    .unwrap();
            }
        }
        let legacy = services.legacy_snippets(MODULE, None, 10).unwrap();
        let concealed = legacy
            .snippets
            .iter()
            .find(|snippet| snippet.id == "s2")
            .expect("the migration reads concealed snippets too");
        assert!(concealed.concealed);
        assert_eq!(concealed.text, "secret");
        assert_eq!(legacy.snippets.len(), 2);
        assert_eq!(legacy.next, None);
    }

    #[test]
    fn declared_storage_is_searched_and_the_clipboard_is_read() {
        let (runtime, services, _dir) = open(Arc::default());
        let declared = ModuleStorage {
            max_records: 10,
            max_value_bytes: 256 * 1024,
            search_fields: vec!["title".into()],
            keep_conflicts: false,
        };
        services.configure_storage(MODULE, &declared);
        let large = serde_json::json!({"title": "Shipping address", "body": "x".repeat(100_000)});
        services
            .record_put(
                MODULE,
                "s/1".into(),
                large.to_string(),
                RecordLimits::of(Some(&declared)),
            )
            .unwrap();
        assert_eq!(
            services
                .record_put(
                    MODULE,
                    "s/2".into(),
                    large.to_string(),
                    RecordLimits::of(None)
                )
                .unwrap_err()
                .code,
            HostErrorCode::Invalid,
            "undeclared modules keep the default bound"
        );
        let found = services.record_search(MODULE, "ship", None, 10).unwrap();
        assert_eq!(found.records[0].key, "s/1");

        capture(&runtime, "copied text");
        assert_eq!(
            services.clipboard_read(MODULE).unwrap().text.as_deref(),
            Some("copied text")
        );
        runtime
            .store
            .write_organization_record(
                &runtime.keyring.item_key(),
                Record::Preference(copypaste_core::storage::organization::PreferenceRecord {
                    namespace: "snippets".into(),
                    key: "require_auth".into(),
                    value_json: "true".into(),
                }),
            )
            .unwrap();
        services.legacy_snippets(MODULE, None, 10).unwrap();
        let seeded = services
            .record_get(MODULE, copypaste_module_sdk::HOST_PREFERENCES_RECORD)
            .unwrap()
            .unwrap();
        assert_eq!(seeded.value_json, r#"{"require-auth":true}"#);
    }
}
