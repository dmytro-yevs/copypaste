//! Native clipboard transport over the shared History action policy.
use crate::AppState;
use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_ipc::{Response, ResponseData};

pub(crate) fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    copy_with(state, id, item_id, false)
}
pub(crate) fn copy_plain_text(state: &AppState, id: u64, item_id: &str) -> Response {
    copy_with(state, id, item_id, true)
}
pub(crate) fn copy_text(state: &AppState, id: u64, item_id: &str, text: &str) -> Response {
    match history_actions::copy_text(&state.store, item_id, text, |payload| {
        state
            .clipboard()
            .write_payload(item_id, payload, copypaste_ipc::content_type::TEXT)
    }) {
        Ok(stamp) => {
            state.instant_clipboard.note_local(&state.store, stamp);
            state.p2p.node().note_local_version(stamp);
            state.note_local_change();
            Response::ok(id, ResponseData::Empty {})
        }
        Err(error) => error.response(id),
    }
}
fn copy_with(state: &AppState, id: u64, item_id: &str, plain_text: bool) -> Response {
    let key = state.keyring.item_key();
    let result = (|| {
        let opened = history_actions::open(&state.store, &key, item_id)?;
        let origin = super::wire::origin_identity_for(state, opened.row());
        let prepared = history_actions::prepare_copy(
            &state.store,
            &key,
            opened,
            CopyOptions {
                origin: &origin,
                local_device_id: state.meta.device_id(),
                plain_text,
                allow_content_uri: false,
                max_file_size_bytes: state.settings.get().max_file_size_bytes,
            },
        )?;
        state
            .clipboard()
            .write_payload(
                &prepared.item.id,
                prepared.payload(),
                &prepared.content_type,
            )
            .map_err(copypaste_core::history_error::HistoryError::from)?;
        let item = history_actions::finish_copy(&state.store, prepared)?;
        state
            .instant_clipboard
            .note_local(&state.store, item.created_at);
        state.note_local_change();
        Ok::<_, copypaste_core::history_error::HistoryError>(item)
    })();
    match result {
        Ok(item) => Response::ok(id, ResponseData::Item(item)),
        Err(error) => error.response(id),
    }
}
pub(crate) fn save_file(state: &AppState, id: u64, item_id: &str, destination: &str) -> Response {
    match history_actions::save_file(
        &state.store,
        &state.keyring.item_key(),
        item_id,
        destination,
        state.settings.get().max_file_size_bytes,
    ) {
        Ok(()) => Response::ok(id, ResponseData::Empty {}),
        Err(error) => error.response(id),
    }
}
