//! Manual clips and edited copies.

use copypaste_core::storage::clip_edit::{ClipEditError, ClipWriter};
use copypaste_core::{now_ms, StoredItem};
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use crate::messages::{
    decrypt_error, storage_error, MSG_EMPTY_CONTENT, MSG_ENCRYPT, MSG_NOT_FOUND, MSG_TOO_BIG,
    MSG_UNSUPPORTED_CONTENT,
};
use crate::Runtime;

const MSG_TOO_LARGE_TO_EDIT: &str = "This clip is too large to edit.";
const MSG_INVALID_RICH_TEXT: &str = "This rich-text clip could not be converted to plain text.";

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    let started = now_ms();
    let settings = runtime.settings.config();
    let writer = ClipWriter {
        store: &runtime.store,
        keyring: &runtime.keyring,
        settings: &settings,
        current_settings: || runtime.settings.config(),
    };
    let stored = match op {
        ClipOp::CreateManual { text } => writer.create_manual(&text),
        ClipOp::SaveEdit {
            id: clip_id,
            text,
            replace_original,
        } => writer
            .save_edit(&clip_id, &text, replace_original)
            .map(|edit| edit.item),
        _ => return crate::features::unsupported(id),
    };
    match stored {
        Ok(item) => {
            runtime.note_local_change_since(started);
            stored_item(runtime, id, item)
        }
        Err(error) => refusal(id, &error),
    }
}

/// The stored clip with its annotations, so the caller sees its marker.
fn stored_item(runtime: &Runtime, id: u64, row: StoredItem) -> Response {
    let annotations = runtime
        .store
        .item_annotations(std::slice::from_ref(&row.id));
    match runtime.to_wire(row) {
        Ok((mut item, _)) => {
            item.annotations = annotations
                .ok()
                .and_then(|mut found| found.remove(&item.id));
            Response::ok(id, ResponseData::Item(item))
        }
        Err(error) => decrypt_error(id, &error),
    }
}

fn refusal(id: u64, error: &ClipEditError) -> Response {
    let (code, message) = match error {
        ClipEditError::NotFound => (ErrorCode::NotFound, MSG_NOT_FOUND),
        ClipEditError::NotText => (ErrorCode::UnsupportedContent, MSG_UNSUPPORTED_CONTENT),
        ClipEditError::TooLarge => (ErrorCode::ContentTooLarge, MSG_TOO_LARGE_TO_EDIT),
        ClipEditError::Empty => (ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT),
        ClipEditError::InvalidRichText => (ErrorCode::InvalidRequest, MSG_INVALID_RICH_TEXT),
        ClipEditError::Ingest(copypaste_core::IngestError::TooLarge) => {
            (ErrorCode::InvalidRequest, MSG_TOO_BIG)
        }
        ClipEditError::Ingest(_) => {
            tracing::error!(?error, "a clip edit could not be encrypted");
            (ErrorCode::Internal, MSG_ENCRYPT)
        }
        ClipEditError::Decrypt(error) => return decrypt_error(id, error),
        ClipEditError::Store(error) => return storage_error(id, "clip_edit", error),
    };
    Response::err(id, code, message)
}

#[cfg(test)]
mod tests {
    use copypaste_ipc::payload::ProvenanceKind;
    use copypaste_ipc::Method;

    use super::*;

    fn data(response: Response) -> ResponseData {
        response.data.expect("a successful response")
    }

    #[test]
    fn manual_clips_answer_through_the_grouped_methods() {
        let (runtime, _dir) = crate::tests::fixture();
        let manual = runtime.request_blocking(
            1,
            Method::Clips {
                op: ClipOp::CreateManual {
                    text: "note to self".into(),
                },
            },
        );
        let ResponseData::Item(item) = data(manual) else {
            panic!("an item");
        };
        assert_eq!(item.source_app_name.as_deref(), Some("CopyPaste"));
        assert_eq!(
            item.annotations
                .and_then(|annotations| annotations.provenance)
                .map(|provenance| provenance.kind),
            Some(ProvenanceKind::Manual)
        );
    }
}
