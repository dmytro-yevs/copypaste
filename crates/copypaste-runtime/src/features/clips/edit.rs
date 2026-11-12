//! Manual clips, edited copies and snippets saved from clips.

use copypaste_core::storage::clip_edit::{ClipEditError, ClipWriter};
use copypaste_core::storage::snippets::{SnippetDetail, SnippetError};
use copypaste_core::{now_ms, StoredItem};
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::snippets as wire;
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};

use crate::messages::{
    decrypt_error, storage_error, MSG_CONTENT_TOO_LARGE, MSG_EMPTY_CONTENT, MSG_ENCRYPT,
    MSG_NOT_FOUND, MSG_TOO_BIG, MSG_UNSUPPORTED_CONTENT,
};
use crate::Runtime;

const MSG_TOO_LARGE_TO_EDIT: &str = "This clip is too large to edit.";
const MSG_INVALID_RICH_TEXT: &str = "This rich-text clip could not be converted to plain text.";
const MSG_SNIPPET_REFUSED: &str = "The snippet could not be saved.";

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

impl Runtime {
    /// `SnippetOp::SaveFromClip`: a literal snippet with a clip's text.
    pub(crate) fn save_clip_as_snippet(
        &self,
        id: u64,
        clip_id: &str,
        collection_id: Option<&str>,
    ) -> Response {
        let started = now_ms();
        match self
            .store
            .save_clip_as_snippet(&self.keyring.item_key(), clip_id, collection_id)
        {
            Ok(detail) => {
                self.note_local_change_since(started);
                self.on_org_changed(LibraryKinds::SNIPPETS);
                Response::ok(id, ResponseData::Snippet(wire_detail(detail)))
            }
            Err(error) => refusal(id, &error),
        }
    }
}

/// Template fields and lints are derived from the body when the editor opens
/// it; a snippet saved from a clip starts with Template off.
fn wire_detail(detail: SnippetDetail) -> wire::SnippetDetail {
    let snippet = detail.snippet;
    wire::SnippetDetail {
        snippet: wire::SnippetSummary {
            id: snippet.id,
            collection_id: snippet.collection_id,
            title: snippet.title,
            abbreviation: snippet.abbreviation,
            template: snippet.template,
            pinned: snippet.pinned,
            concealed: snippet.concealed,
            preview: snippet.preview,
            created_at: snippet.created_at,
            deleted_at: snippet.deleted_at,
            duplicate_abbreviation: snippet.duplicate,
            last_used_at: snippet.last_used_at,
            // A new snippet is never a conflict copy.
            conflict_copy: false,
        },
        body_text: detail.body_text,
        body_rich: detail.body_rich,
        revision: detail.revision.unwrap_or_default(),
        fields: Vec::new(),
        lints: Vec::new(),
    }
}

fn refusal(id: u64, error: &ClipEditError) -> Response {
    let (code, message) = match error {
        ClipEditError::NotFound | ClipEditError::Snippet(SnippetError::NotFound) => {
            (ErrorCode::NotFound, MSG_NOT_FOUND)
        }
        ClipEditError::NotText => (ErrorCode::UnsupportedContent, MSG_UNSUPPORTED_CONTENT),
        ClipEditError::TooLarge => (ErrorCode::ContentTooLarge, MSG_TOO_LARGE_TO_EDIT),
        ClipEditError::Snippet(SnippetError::TooLarge) => {
            (ErrorCode::ContentTooLarge, MSG_CONTENT_TOO_LARGE)
        }
        ClipEditError::Empty => (ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT),
        ClipEditError::InvalidRichText => (ErrorCode::InvalidRequest, MSG_INVALID_RICH_TEXT),
        ClipEditError::Snippet(_) => (ErrorCode::InvalidRequest, MSG_SNIPPET_REFUSED),
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
    use copypaste_ipc::snippets::SnippetOp;
    use copypaste_ipc::Method;

    use super::*;

    fn data(response: Response) -> ResponseData {
        response.data.expect("a successful response")
    }

    #[test]
    fn manual_clips_and_saved_snippets_answer_through_the_grouped_methods() {
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

        let saved = runtime.request_blocking(
            2,
            Method::Snippets {
                op: SnippetOp::SaveFromClip {
                    clip_id: item.id,
                    collection_id: None,
                },
            },
        );
        let ResponseData::Snippet(detail) = data(saved) else {
            panic!("a snippet");
        };
        assert_eq!(detail.body_text, "note to self");
        assert!(!detail.snippet.template);
    }
}
