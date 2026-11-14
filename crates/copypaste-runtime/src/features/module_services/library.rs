//! Snippets, pins and clipboard writes for modules. Each goes through the
//! same runtime path the app uses; snippets a module reads are live, never
//! concealed, and redacted.

use std::collections::{BTreeMap, HashSet};

use copypaste_core::now_ms;
use copypaste_core::storage::organization::{
    ClipNoteRecord, ModuleOrigin, Provenance, ProvenanceKind, Record, RecordKey,
};
use copypaste_ipc::snippets::{self as wire, SnippetOp, SnippetSaveOutcome};
use copypaste_ipc::{ErrorCode, Method, ResponseData};
use copypaste_module_sdk::{
    ClipboardSource, ClipboardWritten, HistoryScope, HostError, RenderedSnippet, SavedRecord,
    Snippet, SnippetSummary,
};

use super::super::delivery::DeliveryOptions;
use super::redact::{bounded, redact};
use super::{invalid, not_found, unavailable};
use crate::Runtime;

/// Characters of one snippet search preview.
const SNIPPET_PREVIEW_CHARS: usize = 200;
const MSG_SNIPPET: &str = "The snippet does not exist or is not available to modules.";
const MSG_CLIPBOARD: &str =
    "This snippet inserts clipboard content, which CopyPaste does not share with modules. \
     Use a snippet without clipboard placeholders.";
const MSG_ABBREVIATION_TAKEN: &str = "That abbreviation is already used by another snippet.";
const MSG_EMPTY: &str = "The text must not be empty.";

fn snippet_not_found() -> HostError {
    HostError::new(copypaste_module_sdk::HostErrorCode::NotFound, MSG_SNIPPET)
}

impl Runtime {
    /// A live snippet a module may read: never a concealed or deleted one.
    fn readable_snippet(&self, id: &str) -> Result<wire::SnippetDetail, HostError> {
        match self.snippets(0, SnippetOp::Get { id: id.to_owned() }).data {
            Some(ResponseData::Snippet(detail))
                if !detail.snippet.concealed && detail.snippet.deleted_at.is_none() =>
            {
                Ok(detail)
            }
            _ => Err(snippet_not_found()),
        }
    }

    /// Matching snippets, or the most recent for an empty query.
    pub fn module_snippet_search(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SnippetSummary>, HostError> {
        let response = if query.trim().is_empty() {
            self.snippets(
                0,
                SnippetOp::List {
                    scope: wire::SnippetScope::All,
                    cursor: None,
                    limit,
                },
            )
        } else {
            self.snippets(
                0,
                SnippetOp::QuickSearch {
                    query: query.to_owned(),
                    include_snippets: true,
                    limit,
                },
            )
        };
        let summaries = match response.data {
            Some(ResponseData::SnippetPage(page)) => page.snippets,
            Some(ResponseData::SnippetHits(hits)) => {
                hits.into_iter().map(|hit| hit.snippet).collect()
            }
            _ => return Err(unavailable(&response.error)),
        };
        Ok(summaries
            .into_iter()
            .filter(|summary| !summary.concealed && summary.deleted_at.is_none())
            .filter_map(|summary| {
                let redacted = redact(&summary.preview);
                (!redacted.mostly_secret()).then(|| SnippetSummary {
                    preview: bounded(&redacted.text, SNIPPET_PREVIEW_CHARS, false).0,
                    id: summary.id,
                    title: summary.title,
                    abbreviation: summary.abbreviation,
                })
            })
            .collect())
    }

    pub fn module_snippet_get(&self, id: &str) -> Result<Snippet, HostError> {
        let detail = self.readable_snippet(id)?;
        let redacted = redact(&detail.body_text);
        if redacted.mostly_secret() {
            return Err(snippet_not_found());
        }
        Ok(Snippet {
            id: detail.snippet.id,
            title: detail.snippet.title,
            body: redacted.text,
            abbreviation: detail.snippet.abbreviation,
            placeholders: detail.fields.into_iter().map(|field| field.name).collect(),
        })
    }

    /// Expands a snippet with `values`. No capability lets a module read the
    /// clipboard, so a snippet with clipboard placeholders is refused rather
    /// than rendered with them empty.
    pub fn module_snippet_render(
        &self,
        id: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<RenderedSnippet, HostError> {
        let wire = self.readable_snippet(id)?;
        let known: HashSet<&str> = wire
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        let unknown: Vec<&str> = values
            .keys()
            .map(String::as_str)
            .filter(|name| !known.contains(name))
            .collect();
        if !unknown.is_empty() {
            return Err(invalid(&format!("Unknown values: {}", unknown.join(", "))));
        }
        let detail = self
            .store
            .snippet(id)
            .map_err(|error| unavailable(&error))?
            .ok_or_else(snippet_not_found)?;
        let rendered = self
            .render_snippet_isolated(&detail, values)
            .map_err(|error| unavailable(&error))?;
        if rendered.secret {
            return Err(snippet_not_found());
        }
        if rendered.reads_clipboard {
            return Err(invalid(MSG_CLIPBOARD));
        }
        if !rendered.missing_fields.is_empty() {
            return Err(invalid(&format!(
                "Fill in these values: {}.",
                rendered.missing_fields.join(", ")
            )));
        }
        let redacted = redact(&rendered.text);
        if redacted.mostly_secret() {
            return Err(snippet_not_found());
        }
        Ok(RenderedSnippet {
            text: redacted.text,
        })
    }

    pub fn module_snippet_save(
        &self,
        title: String,
        body: String,
        abbreviation: Option<String>,
    ) -> Result<SavedRecord, HostError> {
        if body.is_empty() {
            return Err(invalid(MSG_EMPTY));
        }
        let response = self.snippets(
            0,
            SnippetOp::Save {
                draft: wire::SnippetDraft {
                    id: None,
                    collection_id: None,
                    title,
                    abbreviation: abbreviation.filter(|value| !value.trim().is_empty()),
                    body_text: body,
                    body_rich: None,
                    template: false,
                    pinned: false,
                    concealed: false,
                    base_revision: None,
                },
            },
        );
        match response.data {
            Some(ResponseData::SnippetSave(SnippetSaveOutcome::Saved(detail))) => Ok(SavedRecord {
                id: detail.snippet.id,
            }),
            Some(ResponseData::SnippetSave(SnippetSaveOutcome::AbbreviationTaken { .. })) => {
                Err(invalid(MSG_ABBREVIATION_TAKEN))
            }
            _ if response.error_code == Some(ErrorCode::Internal) => {
                Err(unavailable(&response.error))
            }
            _ => Err(invalid(
                response
                    .error
                    .as_deref()
                    .unwrap_or("The snippet could not be saved."),
            )),
        }
    }

    /// Pins or unpins a visible item through the app's own pin path.
    pub fn module_set_pinned(
        &self,
        scope: &HistoryScope,
        id: &str,
        pinned: bool,
    ) -> Result<(), HostError> {
        self.visible(scope, id)?;
        let response = self.request_blocking(
            0,
            Method::Pin {
                id: id.to_owned(),
                pinned,
            },
        );
        match response.error_code {
            _ if response.ok => Ok(()),
            Some(ErrorCode::NotFound) => Err(not_found()),
            _ => Err(unavailable(&response.error)),
        }
    }

    /// Puts text or a visible item on the clipboard. Text becomes a history
    /// item noted with the module's provenance. Answers the item now on the
    /// clipboard; the delivery's restore token stays with the app's undo.
    pub fn module_clipboard_write(
        &self,
        module_id: &str,
        source: &ClipboardSource,
        label: &str,
    ) -> Result<ClipboardWritten, HostError> {
        match source {
            ClipboardSource::Text { text } => {
                if text.is_empty() {
                    return Err(invalid(MSG_EMPTY));
                }
                let delivered = self
                    .deliver_text(
                        text,
                        DeliveryOptions {
                            record_in_history: true,
                            ..DeliveryOptions::default()
                        },
                    )
                    .map_err(|error| unavailable(&error))?;
                if let Some(item) = &delivered.item {
                    self.note_module_provenance(&item.id, module_id, label);
                }
                Ok(ClipboardWritten {
                    item_id: delivered.item.map(|item| item.id),
                })
            }
            ClipboardSource::Item { id, scope } => {
                self.visible(scope, id)?;
                let delivered = self.deliver_item(id, false, true).map_err(|error| {
                    if matches!(error, copypaste_core::history_error::HistoryError::NotFound) {
                        not_found()
                    } else {
                        unavailable(&error)
                    }
                })?;
                Ok(ClipboardWritten {
                    item_id: delivered.item.map(|item| item.id),
                })
            }
        }
    }

    /// Notes the module on a clip it wrote, unless the clip already carries
    /// a note (the same text was in history before).
    fn note_module_provenance(&self, item_id: &str, module_id: &str, label: &str) {
        match self.store.organization_record(RecordKey::ClipNote(item_id)) {
            Ok(None) => {}
            Ok(Some(_)) => return,
            Err(error) => {
                tracing::warn!(?error, "a module write was saved without its provenance");
                return;
            }
        }
        let note = ClipNoteRecord {
            clip_id: item_id.to_owned(),
            hidden_from_integrations: false,
            hidden_changed_at: 0,
            base_stamp: 0,
            keep: false,
            expires_at: None,
            expiry_duration_ms: None,
            provenance: Some(Provenance {
                kind: ProvenanceKind::Module,
                from_clip_id: None,
                transform_snapshot: None,
                pipeline_name: None,
                module: Some(ModuleOrigin {
                    module_id: module_id.to_owned(),
                    label: label.trim().to_owned(),
                }),
            }),
        };
        let started = now_ms();
        match self
            .store
            .write_organization_record(&self.keyring.item_key(), Record::ClipNote(note))
        {
            Ok(()) => self.note_local_change_since(started),
            Err(error) => tracing::warn!(?error, "a module write was saved without its provenance"),
        }
    }
}
