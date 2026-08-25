use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::{ErrorCode, Response, ResponseData};
use tracing::error;

use super::wire::to_wire_and_payload;
use crate::server::messages::{
    decrypt_error, storage_error, MSG_CLIPBOARD, MSG_NOT_FOUND, MSG_UNSUPPORTED_CONTENT,
};
use crate::AppState;

pub(crate) fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    let (item, payload) = match fetch(state, id, item_id) {
        Ok(opened) => opened,
        Err(response) => return response,
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
        Err(response) => return response,
    };
    if !matches!(payload, ClipboardPayload::Text(_)) {
        return unsupported(id);
    }
    if let Err(error) = state.clipboard().write_payload(&item.id, &payload) {
        return write_error(id, error);
    }
    Response::ok(id, ResponseData::Item(item))
}

fn fetch(
    state: &AppState,
    id: u64,
    item_id: &str,
) -> Result<(copypaste_ipc::Item, ClipboardPayload), Response> {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Err(Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND)),
        Err(error) => return Err(storage_error(id, "get", &error)),
    };
    to_wire_and_payload(state, row).map_err(|error| decrypt_error(id, &error))
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
