//! Clip annotations and hiding clips from integrations.
//!
//! Hiding is a synced clip note. Each change records the note version it
//! was based on, so an unhide made concurrently with a hide elsewhere loses
//! and the clip stays hidden on every device. Showing a clip again also
//! clears a hide that a local content rule set.

use copypaste_core::now_ms;
use copypaste_core::storage::organization::{ClipNoteRecord, Record, RecordKey};
use copypaste_core::StoreError;
use copypaste_ipc::clips::{ClipAnnotations, ClipOp};
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use crate::messages::storage_error;
use crate::Runtime;

/// Most clips one annotation read or hide may name.
const MAX_IDS: usize = 500;
const MSG_TOO_MANY: &str = "Too many clips in one request.";

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    match op {
        ClipOp::Annotations { ids } => {
            if ids.len() > MAX_IDS {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_MANY);
            }
            match runtime.store.item_annotations(&ids) {
                Ok(mut found) => Response::ok(
                    id,
                    ResponseData::ClipAnnotations(
                        ids.into_iter()
                            .filter_map(|clip_id| {
                                let annotations = found.remove(&clip_id)?;
                                Some(ClipAnnotations {
                                    id: clip_id,
                                    annotations,
                                })
                            })
                            .collect(),
                    ),
                ),
                Err(error) => storage_error(id, "clip_annotations", &error),
            }
        }
        ClipOp::SetHiddenFromIntegrations { ids, hidden } => {
            if ids.len() > MAX_IDS {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_MANY);
            }
            let started = now_ms();
            let mut changed = false;
            for clip_id in &ids {
                match set_hidden(runtime, clip_id, hidden) {
                    Ok(written) => changed |= written,
                    Err(error) => return storage_error(id, "set_hidden_from_integrations", &error),
                }
            }
            if changed {
                runtime.note_local_change_since(started);
            }
            Response::ok(id, ResponseData::Empty {})
        }
        _ => crate::features::unsupported(id),
    }
}

/// Writes the clip's note with the new flag; `false` when the clip is gone
/// or already in that state.
fn set_hidden(runtime: &Runtime, clip_id: &str, hidden: bool) -> Result<bool, StoreError> {
    if runtime.store.get(clip_id)?.is_none() {
        return Ok(false);
    }
    let note = match runtime
        .store
        .organization_record(RecordKey::ClipNote(clip_id))?
    {
        Some(Record::ClipNote(note)) => Some(note),
        _ => None,
    };
    let mut changed = false;
    if !hidden {
        if let Some(mut state) = runtime.store.clip_local_state(clip_id)? {
            if state.rule_hidden_from_integrations {
                state.rule_hidden_from_integrations = false;
                changed |= runtime.store.write_clip_local_state(&state)?;
            }
        }
    }
    if note
        .as_ref()
        .is_some_and(|note| note.hidden_from_integrations)
        == hidden
    {
        return Ok(changed);
    }
    let base_stamp = runtime
        .store
        .organization_record_stamp(RecordKey::ClipNote(clip_id))?
        .unwrap_or(0);
    let note = ClipNoteRecord {
        hidden_from_integrations: hidden,
        hidden_changed_at: now_ms(),
        base_stamp,
        ..note.unwrap_or(ClipNoteRecord {
            clip_id: clip_id.to_owned(),
            hidden_from_integrations: false,
            hidden_changed_at: 0,
            base_stamp: 0,
            keep: false,
            expires_at: None,
            expiry_duration_ms: None,
            provenance: None,
        })
    };
    runtime
        .store
        .write_organization_record(&runtime.keyring.item_key(), Record::ClipNote(note))?;
    Ok(true)
}
