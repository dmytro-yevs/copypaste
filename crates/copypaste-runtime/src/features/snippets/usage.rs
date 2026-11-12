//! Snippet use: render, deliver and bump the local usage counters.
//!
//! The output leaves as a transient clipboard write, which creates no history
//! row, or as returned text for the Android keyboard. It becomes a history
//! clip only while the synced `snippets.record_pasted` preference is on, and
//! never for concealed output. Usage counters are local and never sync.

use copypaste_core::now_ms;
use copypaste_core::storage::organization::{Record, RecordKey};
use copypaste_ipc::clips::{Delivered, Delivery};
use copypaste_ipc::snippets::SnippetOp;
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use super::render::{render_error, ClipboardText, Expansion};
use crate::features::delivery::DeliveryOptions;
use crate::messages::{storage_error, MSG_NOT_FOUND};
use crate::Runtime;

const PREFERENCES: &str = "snippets";
const KEY_RECORD_PASTED: &str = "record_pasted";
const MSG_REQUIRED: &str = "Fill in the required fields before using this snippet.";

pub(super) fn handle(runtime: &Runtime, id: u64, op: SnippetOp) -> Response {
    let SnippetOp::Use {
        id: snippet_id,
        values,
        clipboard,
        delivery,
        locale,
    } = op
    else {
        return crate::features::unsupported(id);
    };
    let detail = match runtime.store.snippet(&snippet_id) {
        Ok(Some(detail)) if detail.snippet.deleted_at.is_none() => detail,
        Ok(_) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(error) => return storage_error(id, "snippet_use", &error),
    };
    // Read before anything is written, so the restore after a paste and a
    // later `clipboard` placeholder both see what the user copied.
    let user_item = runtime.user_clipboard_item();
    let clipboard = match clipboard {
        copypaste_ipc::snippets::ClipboardInput::Host => ClipboardText::Item(user_item.clone()),
        input => ClipboardText::of(runtime, input),
    };
    let rendering =
        match Expansion::of(&detail, &values, &clipboard, locale.as_deref()).render(runtime) {
            Ok(rendering) => rendering,
            Err(error) => return render_error(id, &error),
        };
    if !rendering.missing_required.is_empty() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_REQUIRED);
    }
    let used = || {
        if let Err(error) = runtime.store.record_snippet_use(&snippet_id, now_ms()) {
            tracing::warn!(?error, "snippet usage could not be recorded");
        }
        tracing::debug!(snippet = %snippet_id, ?delivery, "snippet used");
    };
    let cursor = rendering.cursor_graphemes_from_end;
    if delivery == Delivery::Return {
        used();
        return Response::ok(
            id,
            ResponseData::Delivered(Delivered {
                token: None,
                text: Some(rendering.text),
                cursor_graphemes_from_end: cursor,
            }),
        );
    }
    let record = !rendering.secret && record_pasted(runtime);
    let delivered = runtime.deliver_text(
        &clipboard_line_endings(&rendering.text),
        DeliveryOptions {
            secret: rendering.secret,
            transient: !record,
            record_in_history: record,
            html: false,
        },
    );
    match delivered {
        Ok(delivered) => {
            used();
            let paste = delivery == Delivery::Paste;
            let token = delivered.token.filter(|_| paste);
            if let Some(token) = &token {
                runtime.retarget_restore(token, user_item);
            }
            Response::ok(
                id,
                ResponseData::Delivered(Delivered {
                    token,
                    text: None,
                    // Only a paste can place the caret; a copy leaves it be.
                    cursor_graphemes_from_end: cursor.filter(|_| paste),
                }),
            )
        }
        Err(error) => error.response(id),
    }
}

/// Whether pasted snippet output is kept in history (synced, off by default).
fn record_pasted(runtime: &Runtime) -> bool {
    let record = runtime.store.organization_record(RecordKey::Preference {
        namespace: PREFERENCES,
        key: KEY_RECORD_PASTED,
    });
    match record {
        Ok(Some(Record::Preference(preference))) => {
            serde_json::from_str::<serde_json::Value>(&preference.value_json)
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Clipboard text uses the platform's line endings: CRLF on Windows. A CRLF
/// pair is one caret step, so the cursor offset is unchanged.
fn clipboard_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(windows) && text.contains('\n') {
        text.replace("\r\n", "\n").replace('\n', "\r\n").into()
    } else {
        text.into()
    }
}
