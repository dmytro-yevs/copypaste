//! Snippets, collections, templates and snippet import/export.
//!
//! The library lives in synced organization records (copypaste-core); this
//! service maps it onto the wire, renders templates and delivers their
//! output. Writes are sent once, on save. Logs carry ids, counts and error
//! codes, never titles, abbreviations, bodies or field values.

mod render;
mod transfer;
mod usage;

use std::collections::HashMap;

use copypaste_core::now_ms;
use copypaste_core::snippets::abbreviation;
use copypaste_core::snippets::ranking::SearchTier;
use copypaste_core::storage::snippets::{self as store, SnippetError};
use copypaste_ipc::snippets::{self as wire, SnippetOp};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};

use crate::messages::{storage_error, MSG_NOT_FOUND};
use crate::Runtime;

/// Conflict copies take an id with this prefix (D2a).
const CONFLICT_PREFIX: &str = "conflict-";
/// Shown when a new collection names no icon or accent.
const DEFAULT_COLLECTION_ICON: &str = "folder";
const DEFAULT_COLLECTION_ACCENT: &str = "neutral";

const MSG_ABBREVIATION_TAKEN: &str = "This abbreviation is already used by another snippet.";
const MSG_ABBREVIATION: &str = "An abbreviation is 1 to 32 characters with no spaces.";
const MSG_EMPTY_BODY: &str = "A snippet needs some text.";
const MSG_TOO_LARGE: &str = "The snippet is larger than the limit.";
const MSG_TITLE: &str = "The title is too long or not valid.";
const MSG_COLLECTION: &str = "That collection is not valid.";
const MSG_COLLECTION_TAKEN: &str = "A collection with this name already exists.";
const MSG_LIMIT: &str = "The snippet library is full.";

impl Runtime {
    pub(crate) fn snippets(&self, id: u64, op: SnippetOp) -> Response {
        match op {
            SnippetOp::Render { .. } | SnippetOp::Lint { .. } => render::handle(self, id, op),
            SnippetOp::Release => {
                render::release();
                transfer::release(self);
                Response::ok(id, ResponseData::Empty {})
            }
            SnippetOp::Use { .. } => usage::handle(self, id, op),
            SnippetOp::SaveFromClip {
                clip_id,
                collection_id,
            } => self.save_clip_as_snippet(id, &clip_id, collection_id.as_deref()),
            SnippetOp::ImportPreview { .. }
            | SnippetOp::ImportCommit { .. }
            | SnippetOp::ImportCancel { .. }
            | SnippetOp::Export { .. } => transfer::handle(self, id, op),
            SnippetOp::List {
                scope,
                cursor,
                limit,
            } => self.snippet_list(id, &scope, cursor.as_deref(), limit),
            SnippetOp::Get { id: snippet_id } => match self.store.snippet(&snippet_id) {
                Ok(Some(detail)) => {
                    Response::ok(id, ResponseData::Snippet(render::wire_detail(self, detail)))
                }
                Ok(None) => not_found(id),
                Err(error) => storage_error(id, "snippet", &error),
            },
            SnippetOp::Save { draft } => self.snippet_save(id, draft),
            SnippetOp::QuickSearch {
                query,
                include_snippets,
                limit,
            } => self.snippet_search(id, &query, include_snippets, limit),
            SnippetOp::Collections => match self.store.snippet_collections() {
                Ok(collections) => Response::ok(
                    id,
                    ResponseData::SnippetCollections(wire_collections(collections)),
                ),
                Err(error) => storage_error(id, "snippet_collections", &error),
            },
            op => self.snippet_write(id, op),
        }
    }

    /// Purges snippets whose Recently Deleted window has passed; run by the
    /// long-horizon deadline sweep, never by a timer of its own.
    pub(crate) fn purge_expired_snippets(&self, now: i64) {
        match self
            .store
            .purge_expired_snippets(&self.keyring.item_key(), now)
        {
            Ok(0) => {}
            Ok(purged) => {
                tracing::debug!(purged, "expired snippets purged");
                self.library_written(now, LibraryKinds::SNIPPETS);
            }
            Err(error) => tracing::warn!(?error, "expired snippets could not be purged"),
        }
    }

    fn snippet_list(
        &self,
        id: u64,
        scope: &wire::SnippetScope,
        cursor: Option<&str>,
        limit: u32,
    ) -> Response {
        let scope = match scope {
            wire::SnippetScope::All => store::SnippetScope::All,
            wire::SnippetScope::Unfiled => store::SnippetScope::Unfiled,
            wire::SnippetScope::Collection { id } => store::SnippetScope::Collection(id.clone()),
            wire::SnippetScope::Pinned => store::SnippetScope::Pinned,
            wire::SnippetScope::RecentlyDeleted => store::SnippetScope::RecentlyDeleted,
        };
        let page = self
            .store
            .snippet_page(&scope, cursor, limit)
            .and_then(|page| Ok((page, self.store.snippet_count()?)));
        match page {
            Ok((page, total)) => Response::ok(
                id,
                ResponseData::SnippetPage(wire::SnippetPage {
                    snippets: page.snippets.into_iter().map(wire_summary).collect(),
                    next_cursor: page.next,
                    total: u32::try_from(total).unwrap_or(u32::MAX),
                }),
            ),
            Err(copypaste_core::StoreError::InvalidCursor) => Response::err(
                id,
                ErrorCode::InvalidRequest,
                crate::messages::MSG_BAD_CURSOR,
            ),
            Err(error) => storage_error(id, "snippet_page", &error),
        }
    }

    fn snippet_save(&self, id: u64, draft: wire::SnippetDraft) -> Response {
        let started = now_ms();
        let draft = store::SnippetDraft {
            id: draft.id,
            collection_id: draft.collection_id,
            title: draft.title,
            abbreviation: draft.abbreviation,
            body_text: draft.body_text,
            body_rich: draft.body_rich,
            template: draft.template,
            pinned: draft.pinned,
            concealed: draft.concealed,
            base_revision: draft.base_revision,
            origin_clip_id: None,
        };
        match self.store.save_snippet(&self.keyring.item_key(), &draft) {
            Ok(detail) => {
                self.library_written(started, LibraryKinds::SNIPPETS);
                tracing::debug!(snippet = %detail.snippet.id, "snippet saved");
                Response::ok(
                    id,
                    ResponseData::SnippetSave(wire::SnippetSaveOutcome::Saved(Box::new(
                        render::wire_detail(self, detail),
                    ))),
                )
            }
            Err(SnippetError::AbbreviationTaken { snippet_id, title }) => Response::ok(
                id,
                ResponseData::SnippetSave(wire::SnippetSaveOutcome::AbbreviationTaken {
                    snippet_id,
                    title,
                }),
            ),
            Err(error) => refusal(id, &error),
        }
    }

    fn snippet_search(&self, id: u64, query: &str, include: bool, limit: u32) -> Response {
        if !include {
            return Response::ok(id, ResponseData::SnippetHits(Vec::new()));
        }
        let hits = match self.store.quick_search_snippets(query, limit) {
            Ok(hits) => hits,
            Err(error) => return storage_error(id, "snippet_search", &error),
        };
        let exact = hits
            .iter()
            .filter(|hit| hit.tier == SearchTier::ExactAbbreviation)
            .count();
        let hits = hits
            .into_iter()
            .map(|hit| wire::SnippetHit {
                duplicate: hit.tier == SearchTier::ExactAbbreviation && exact > 1,
                tier: match hit.tier {
                    SearchTier::ExactAbbreviation => wire::SnippetMatchTier::ExactAbbreviation,
                    SearchTier::AbbreviationPrefix => wire::SnippetMatchTier::AbbreviationPrefix,
                    SearchTier::Title => wire::SnippetMatchTier::Title,
                    SearchTier::FullText => wire::SnippetMatchTier::FullText,
                },
                snippet: wire_summary(hit.snippet),
            })
            .collect();
        Response::ok(id, ResponseData::SnippetHits(hits))
    }

    /// The writes that answer `Empty`, a count or a collection.
    fn snippet_write(&self, id: u64, op: SnippetOp) -> Response {
        let started = now_ms();
        let key = self.keyring.item_key();
        let snippets = LibraryKinds::SNIPPETS;
        let both = LibraryKinds::SNIPPETS.union(LibraryKinds::COLLECTIONS);
        let written: Result<(ResponseData, LibraryKinds), SnippetError> = match op {
            SnippetOp::Delete { ids } => self
                .store
                .delete_snippets(&key, &ids)
                .map(|_| (ResponseData::Empty {}, snippets)),
            SnippetOp::Restore { ids } => self
                .store
                .restore_snippets(&key, &ids)
                .map(|_| (ResponseData::Empty {}, snippets)),
            SnippetOp::Purge { ids } => self
                .store
                .purge_snippets(&key, &ids)
                .map(|_| (ResponseData::Empty {}, snippets)),
            SnippetOp::Move { ids, collection_id } => self
                .store
                .move_snippets(&key, &ids, collection_id.as_deref())
                .map(|_| (ResponseData::Empty {}, snippets)),
            SnippetOp::Reorder {
                id: snippet_id,
                after_id,
            } => self
                .store
                .reorder_snippet(&key, &snippet_id, after_id.as_deref())
                .map(|()| (ResponseData::Empty {}, snippets)),
            SnippetOp::SetPinned {
                id: snippet_id,
                pinned,
                after_id,
            } => self
                .store
                .set_snippet_pinned(&key, &snippet_id, pinned, after_id.as_deref())
                .map(|()| (ResponseData::Empty {}, snippets)),
            SnippetOp::RenameAbbreviation {
                id: snippet_id,
                abbreviation,
                update_references,
            } => self
                .rename_abbreviation(&snippet_id, &abbreviation, update_references)
                .map(|count| (ResponseData::Count(count as u64), snippets)),
            SnippetOp::SaveCollection { draft } => self
                .store
                .save_snippet_collection(
                    &key,
                    &store::CollectionDraft {
                        id: draft.id,
                        name: draft.name,
                        icon: draft
                            .icon
                            .unwrap_or_else(|| DEFAULT_COLLECTION_ICON.to_owned()),
                        accent: draft
                            .accent
                            .unwrap_or_else(|| DEFAULT_COLLECTION_ACCENT.to_owned()),
                    },
                )
                .and_then(|saved| {
                    let collections = self.store.snippet_collections()?;
                    wire_collections(collections)
                        .into_iter()
                        .find(|collection| collection.id == saved.id)
                        .ok_or(SnippetError::NotFound)
                })
                .map(|collection| {
                    (
                        ResponseData::SnippetCollection(collection),
                        LibraryKinds::COLLECTIONS,
                    )
                }),
            SnippetOp::DeleteCollection {
                id: collection_id,
                delete_snippets,
            } => self
                .store
                .delete_snippet_collection(&key, &collection_id, delete_snippets)
                .map(|()| (ResponseData::Empty {}, both)),
            SnippetOp::MergeCollections { from, into } => self
                .store
                .merge_snippet_collections(&key, &from, &into)
                .map(|_| (ResponseData::Empty {}, both)),
            _ => return crate::features::unsupported(id),
        };
        match written {
            Ok((data, kinds)) => {
                self.library_written(started, kinds);
                Response::ok(id, data)
            }
            Err(error) => refusal(id, &error),
        }
    }

    /// Renames (or clears) an abbreviation. With `update_references`, every
    /// template that names the old abbreviation is rewritten in the same
    /// transaction.
    fn rename_abbreviation(
        &self,
        snippet_id: &str,
        wanted: &str,
        update_references: bool,
    ) -> Result<usize, SnippetError> {
        let previous = self
            .store
            .snippet(snippet_id)?
            .and_then(|detail| detail.snippet.abbreviation)
            .map(|previous| abbreviation::normalize(&previous));
        let wanted = wanted.trim();
        let rewrite = |body: &str| -> Option<String> {
            let previous = previous.as_deref()?;
            copypaste_template::parse(body).rename_snippet_references(
                &|reference| abbreviation::normalize(reference) == previous,
                wanted,
            )
        };
        let rewrite =
            (update_references && !wanted.is_empty()).then_some(&rewrite as store::BodyRewrite<'_>);
        let rewritten = self.store.rename_snippet_abbreviation(
            &self.keyring.item_key(),
            snippet_id,
            Some(wanted).filter(|wanted| !wanted.is_empty()),
            rewrite,
        )?;
        render::release();
        Ok(rewritten)
    }
}

fn not_found(id: u64) -> Response {
    Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND)
}

fn refusal(id: u64, error: &SnippetError) -> Response {
    let (code, message) = match error {
        SnippetError::AbbreviationTaken { .. } => {
            (ErrorCode::InvalidRequest, MSG_ABBREVIATION_TAKEN)
        }
        SnippetError::InvalidAbbreviation(_) => (ErrorCode::InvalidRequest, MSG_ABBREVIATION),
        SnippetError::EmptyBody => (ErrorCode::InvalidRequest, MSG_EMPTY_BODY),
        SnippetError::TooLarge => (ErrorCode::ContentTooLarge, MSG_TOO_LARGE),
        SnippetError::InvalidTitle => (ErrorCode::InvalidRequest, MSG_TITLE),
        SnippetError::InvalidCollection => (ErrorCode::InvalidRequest, MSG_COLLECTION),
        SnippetError::CollectionNameTaken { .. } => {
            (ErrorCode::InvalidRequest, MSG_COLLECTION_TAKEN)
        }
        SnippetError::LimitReached => (ErrorCode::InvalidRequest, MSG_LIMIT),
        SnippetError::NotFound => (ErrorCode::NotFound, MSG_NOT_FOUND),
        SnippetError::Store(error) => return storage_error(id, "snippets", error),
    };
    Response::err(id, code, message)
}

pub(super) fn wire_summary(summary: store::SnippetSummary) -> wire::SnippetSummary {
    wire::SnippetSummary {
        conflict_copy: summary.id.starts_with(CONFLICT_PREFIX),
        id: summary.id,
        collection_id: summary.collection_id,
        title: summary.title,
        abbreviation: summary.abbreviation,
        template: summary.template,
        pinned: summary.pinned,
        concealed: summary.concealed,
        preview: summary.preview,
        created_at: summary.created_at,
        deleted_at: summary.deleted_at,
        duplicate_abbreviation: summary.duplicate,
        last_used_at: summary.last_used_at,
    }
}

/// Live collections; a name shared after concurrent creation on two devices
/// is told apart as `name (2)`, `name (3)` in display order.
fn wire_collections(collections: Vec<store::SnippetCollection>) -> Vec<wire::SnippetCollection> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    collections
        .into_iter()
        .map(|collection| {
            let count = seen
                .entry(abbreviation::normalize(&collection.name))
                .or_default();
            *count += 1;
            let display_name = if *count == 1 {
                collection.name.clone()
            } else {
                format!("{} ({count})", collection.name)
            };
            wire::SnippetCollection {
                id: collection.id,
                display_name,
                name: collection.name,
                icon: Some(collection.icon).filter(|icon| !icon.is_empty()),
                accent: Some(collection.accent).filter(|accent| !accent.is_empty()),
                order_key: collection.order_key,
                deleted_at: None,
                snippet_count: u32::try_from(collection.snippet_count).unwrap_or(u32::MAX),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

    use copypaste_core::storage::organization::{PreferenceRecord, Record};
    use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring};
    use copypaste_ipc::clips::{Delivered, Delivery};
    use copypaste_ipc::snippets::{
        ClipboardInput, RenderPurpose, RenderSource, SnippetDraft, SnippetMatchTier,
        SnippetSaveOutcome,
    };
    use copypaste_ipc::{content_type, Method};

    use super::*;
    use crate::host::{ClipboardWriter, InProcessHost, WriteOptions};

    #[derive(Default)]
    struct Clipboard(Mutex<Vec<(String, WriteOptions)>>);

    impl Clipboard {
        fn last(&self) -> (String, WriteOptions) {
            self.0.lock().unwrap().last().cloned().expect("a write")
        }
    }

    impl ClipboardWriter for Clipboard {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _: &str,
            options: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            let ClipboardPayload::Text(text) = payload else {
                panic!("snippets write text");
            };
            self.0
                .lock()
                .unwrap()
                .push((text.as_str().to_owned(), options));
            Ok(())
        }
    }

    /// Held by each test: a tracing callsite first hit on a thread with no
    /// subscriber can stay disabled for the test that captures logs.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn runtime() -> (
        Arc<Runtime>,
        Arc<Clipboard>,
        tempfile::TempDir,
        MutexGuard<'static, ()>,
    ) {
        let serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(Clipboard::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "snippets",
            0,
            Arc::new(InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[3; 32])),
        )
        .unwrap();
        (Arc::new(runtime), clipboard, dir, serial)
    }

    fn request(runtime: &Runtime, op: SnippetOp) -> Response {
        runtime.request_blocking(1, Method::Snippets { op })
    }

    fn save(runtime: &Runtime, title: &str, abbreviation: Option<&str>, body: &str) -> String {
        let draft = SnippetDraft {
            id: None,
            collection_id: None,
            title: title.into(),
            abbreviation: abbreviation.map(str::to_owned),
            body_text: body.into(),
            body_rich: None,
            template: body.contains("{{"),
            pinned: false,
            concealed: title.starts_with("Secret"),
            base_revision: None,
        };
        match request(runtime, SnippetOp::Save { draft }).data {
            Some(ResponseData::SnippetSave(SnippetSaveOutcome::Saved(detail))) => detail.snippet.id,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn capture(runtime: &Runtime, text: &str) -> String {
        let item = copypaste_core::ingest::ingest(
            &runtime.store,
            &runtime.keyring,
            text,
            content_type::TEXT,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();
        runtime.note_capture(item.created_at, &item.id);
        item.id
    }

    fn use_snippet(
        runtime: &Runtime,
        id: &str,
        values: &[(&str, &str)],
        delivery: Delivery,
    ) -> Delivered {
        let op = SnippetOp::Use {
            id: id.into(),
            values: values
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            clipboard: ClipboardInput::Host,
            delivery,
            locale: None,
        };
        match request(runtime, op).data {
            Some(ResponseData::Delivered(delivered)) => delivered,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[derive(Clone, Default)]
    struct Logs(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Logs {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Logs {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn logs_from(work: impl FnOnce()) -> String {
        let logs = Logs::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(logs.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, work);
        let bytes = logs.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn using_a_snippet_records_history_only_when_asked_and_never_when_concealed() {
        let (runtime, clipboard, _dir, _serial) = runtime();
        capture(&runtime, "user copy");
        let rows = runtime.store.count().unwrap();
        let logs = logs_from(|| {
            let greeting = save(
                &runtime,
                "Greeting title",
                Some("greetabbr"),
                r#"Hello {{field name="who"}}{{cursor}}!"#,
            );
            let returned = use_snippet(
                &runtime,
                &greeting,
                &[("who", "FieldValueXyz")],
                Delivery::Return,
            );
            assert_eq!(returned.text.as_deref(), Some("Hello FieldValueXyz!"));
            assert_eq!(returned.cursor_graphemes_from_end, Some(1));
            let pasted = use_snippet(
                &runtime,
                &greeting,
                &[("who", "FieldValueXyz")],
                Delivery::Paste,
            );
            assert!(pasted.token.is_some() && pasted.text.is_none());
        });
        assert_eq!(runtime.store.count().unwrap(), rows, "no history row");
        let (written, options) = clipboard.last();
        assert_eq!(written, "Hello FieldValueXyz!");
        assert!(options.transient && !options.secret);
        for private in [
            "Greeting",
            "greetabbr",
            "Hello",
            "FieldValueXyz",
            "user copy",
        ] {
            assert!(!logs.contains(private), "the log names {private}: {logs}");
        }
        assert!(logs.contains("snippet used"), "{logs}");

        runtime
            .store
            .write_organization_record(
                &runtime.keyring.item_key(),
                Record::Preference(PreferenceRecord {
                    namespace: "snippets".into(),
                    key: "record_pasted".into(),
                    value_json: "true".into(),
                }),
            )
            .unwrap();
        let literal = save(&runtime, "Plain", None, "kept in history");
        use_snippet(&runtime, &literal, &[], Delivery::Paste);
        assert_eq!(runtime.store.count().unwrap(), rows + 1);

        let concealed = save(&runtime, "Secret token", None, "s3cr3t value");
        use_snippet(&runtime, &concealed, &[], Delivery::Paste);
        assert_eq!(runtime.store.count().unwrap(), rows + 1, "concealed output");
        let (written, options) = clipboard.last();
        assert_eq!(written, "s3cr3t value");
        assert!(options.secret && options.transient);
    }

    #[test]
    fn the_clipboard_placeholder_reads_what_the_user_copied_before_each_paste() {
        let (runtime, clipboard, _dir, _serial) = runtime();
        let copied = capture(&runtime, "user text");
        let wrapped = save(&runtime, "Wrap", None, "[{{clipboard}}]");
        use_snippet(&runtime, &wrapped, &[], Delivery::Paste);
        assert_eq!(clipboard.last().0, "[user text]");
        let token = use_snippet(&runtime, &wrapped, &[], Delivery::Paste)
            .token
            .unwrap();
        assert_eq!(
            clipboard.last().0,
            "[user text]",
            "the output is not re-read"
        );

        assert!(runtime.restore_clipboard(&token));
        assert_eq!(clipboard.last().0, "user text");
        assert_eq!(runtime.clipboard_item().as_deref(), Some(copied.as_str()));
    }

    #[test]
    fn quick_search_ranks_exact_then_prefix_then_title_then_full_text() {
        let (runtime, _clipboard, _dir, _serial) = runtime();
        let body = save(&runtime, "Gamma", None, "ends with sig");
        let title = save(&runtime, "sig notes", None, "first");
        let prefix = save(&runtime, "Beta", Some("sig2"), "second");
        let exact = save(&runtime, "Alpha", Some("Sig"), "third");
        let op = SnippetOp::QuickSearch {
            query: "sig".into(),
            include_snippets: true,
            limit: 10,
        };
        let Some(ResponseData::SnippetHits(hits)) = request(&runtime, op).data else {
            panic!("hits expected");
        };
        let ranked: Vec<_> = hits
            .iter()
            .map(|hit| (hit.snippet.id.clone(), hit.tier, hit.duplicate))
            .collect();
        assert_eq!(
            ranked,
            vec![
                (exact, SnippetMatchTier::ExactAbbreviation, false),
                (prefix, SnippetMatchTier::AbbreviationPrefix, false),
                (title, SnippetMatchTier::Title, false),
                (body, SnippetMatchTier::FullText, false),
            ]
        );
    }

    #[test]
    fn a_required_clipboard_beyond_history_blocks_use_but_still_previews() {
        let (runtime, clipboard, _dir, _serial) = runtime();
        let needs = save(
            &runtime,
            "Needs",
            None,
            "x{{clipboard index=5 required=true}}y",
        );
        let op = SnippetOp::Render {
            source: RenderSource::Snippet { id: needs.clone() },
            values: BTreeMap::new(),
            clipboard: ClipboardInput::Host,
            purpose: RenderPurpose::Preview,
            locale: Some("uk".into()),
        };
        let Some(ResponseData::Rendered(rendered)) = request(&runtime, op).data else {
            panic!("a preview expected");
        };
        assert_eq!(rendered.text, "xy");
        assert_eq!(rendered.missing_required, vec!["clipboard:5".to_owned()]);
        assert_eq!(rendered.warnings[0].code, "empty_clipboard");

        let op = SnippetOp::Use {
            id: needs,
            values: BTreeMap::new(),
            clipboard: ClipboardInput::Host,
            delivery: Delivery::Paste,
            locale: None,
        };
        let refused = request(&runtime, op);
        assert_eq!(refused.error_code, Some(ErrorCode::InvalidRequest));
        assert!(clipboard.0.lock().unwrap().is_empty());
    }

    #[test]
    fn renaming_an_abbreviation_rewrites_the_templates_that_use_it() {
        let (runtime, _clipboard, _dir, _serial) = runtime();
        let address = save(&runtime, "Address", Some("addr"), "street");
        let letter = save(
            &runtime,
            "Letter",
            None,
            "{{snippet abbr=addr}} / {{snippet abbr=ADDR | upper}}",
        );
        let op = SnippetOp::RenameAbbreviation {
            id: address,
            abbreviation: "home".into(),
            update_references: true,
        };
        assert!(matches!(
            request(&runtime, op).data,
            Some(ResponseData::Count(1))
        ));
        let Some(ResponseData::Snippet(detail)) =
            request(&runtime, SnippetOp::Get { id: letter.clone() }).data
        else {
            panic!("the letter expected");
        };
        assert_eq!(
            detail.body_text,
            r#"{{snippet abbr="home"}} / {{snippet abbr="home" | upper}}"#
        );
        assert!(detail.lints.is_empty(), "{:?}", detail.lints);
        let rendered = use_snippet(&runtime, &letter, &[], Delivery::Return);
        assert_eq!(rendered.text.as_deref(), Some("street / STREET"));
    }
}
