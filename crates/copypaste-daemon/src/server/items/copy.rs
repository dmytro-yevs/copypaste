use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::{ErrorCode, Response, ResponseData};
use tracing::error;

use super::wire::{is_oversized_text, to_wire_and_payload};
use crate::server::messages::{
    decrypt_error, storage_error, MSG_BACKUP_EXISTS, MSG_BACKUP_NO_DIR, MSG_BAD_PATH,
    MSG_CLIPBOARD, MSG_CONTENT_TOO_LARGE, MSG_NOT_FOUND, MSG_SAVE_FILE_FAILED,
    MSG_UNSUPPORTED_CONTENT,
};
use crate::AppState;

pub(crate) fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    let (item, payload) = match fetch(state, id, item_id) {
        Ok(opened) => opened,
        Err(response) => return *response,
    };
    if let Err(error) = state.clipboard().write_payload(&item.id, &payload) {
        return write_error(id, error);
    }
    Response::ok(id, ResponseData::Item(item))
}

/// Copy text only. Binary display labels are presentation, never content.
pub(crate) fn copy_plain_text(state: &AppState, id: u64, item_id: &str) -> Response {
    let (item, payload) = match fetch(state, id, item_id) {
        Ok(opened) => opened,
        Err(response) => return *response,
    };
    if !matches!(payload, ClipboardPayload::Text(_)) {
        return unsupported(id);
    }
    if let Err(error) = state.clipboard().write_payload(&item.id, &payload) {
        return write_error(id, error);
    }
    Response::ok(id, ResponseData::Item(item))
}

pub(crate) fn save_file(
    state: &AppState,
    id: u64,
    item_id: &str,
    raw_destination: &str,
) -> Response {
    let destination = raw_destination.trim();
    if destination.is_empty() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_PATH);
    }
    let path = std::path::Path::new(destination);
    if path.exists() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BACKUP_EXISTS);
    }
    if path.parent().is_none_or(|parent| !parent.is_dir()) {
        return Response::err(id, ErrorCode::NotFound, MSG_BACKUP_NO_DIR);
    }
    let (_, payload) = match fetch(state, id, item_id) {
        Ok(opened) => opened,
        Err(response) => return *response,
    };
    if !matches!(
        payload,
        ClipboardPayload::Image { .. } | ClipboardPayload::File { .. }
    ) {
        return unsupported(id);
    }
    match payload.save_file_to(path) {
        Ok(()) => Response::ok(id, ResponseData::Empty {}),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_BACKUP_EXISTS)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Response::err(id, ErrorCode::NotFound, MSG_BACKUP_NO_DIR)
        }
        Err(error) => {
            error!(error = ?error, "file clip save failed");
            Response::err(id, ErrorCode::Internal, MSG_SAVE_FILE_FAILED)
        }
    }
}

fn fetch(
    state: &AppState,
    id: u64,
    item_id: &str,
) -> Result<(copypaste_ipc::Item, ClipboardPayload), Box<Response>> {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err(Box::new(Response::err(
                id,
                ErrorCode::NotFound,
                MSG_NOT_FOUND,
            )))
        }
        Err(error) => return Err(Box::new(storage_error(id, "get", &error))),
    };
    let (item, payload) =
        to_wire_and_payload(state, row).map_err(|error| Box::new(decrypt_error(id, &error)))?;
    if is_oversized_text(&payload) {
        return Err(Box::new(Response::err(
            id,
            ErrorCode::ContentTooLarge,
            MSG_CONTENT_TOO_LARGE,
        )));
    }
    Ok((item, payload))
}

fn write_error(id: u64, error: ClipboardWriteError) -> Response {
    match error {
        ClipboardWriteError::UnsupportedContent => unsupported(id),
        ClipboardWriteError::Failed => {
            error!("pasteboard write failed");
            Response::err(id, ErrorCode::Internal, MSG_CLIPBOARD)
        }
    }
}

fn unsupported(id: u64) -> Response {
    Response::err(id, ErrorCode::UnsupportedContent, MSG_UNSUPPORTED_CONTENT)
}
