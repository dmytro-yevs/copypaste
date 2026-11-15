//! Pins and clipboard writes for modules. Each goes through the same runtime
//! path the app uses.

use copypaste_core::now_ms;
use copypaste_core::storage::organization::{
    ClipNoteRecord, ModuleOrigin, Provenance, ProvenanceKind, Record, RecordKey,
};
use copypaste_ipc::{ErrorCode, Method};
use copypaste_module_sdk::{ClipboardSource, ClipboardWritten, HistoryScope, HostError};

use super::super::delivery::DeliveryOptions;
use super::{invalid, not_found, unavailable};
use crate::Runtime;

const MSG_EMPTY: &str = "The text must not be empty.";

impl Runtime {
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
    pub(crate) fn note_module_provenance(&self, item_id: &str, module_id: &str, label: &str) {
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
