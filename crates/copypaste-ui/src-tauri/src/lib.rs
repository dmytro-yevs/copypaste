//! The Tauri bridge: React on one side, the daemon's Unix socket on the other.
//!
//! This crate is deliberately thin. It owns no state, caches nothing, and
//! retries nothing — React Query does all three on the frontend, and a second
//! policy here would only be able to disagree with it. Every command is one
//! connection, one newline-delimited JSON request, one typed reply.
//!
//! Three things it does not do, on purpose:
//!
//! * **It does not redefine the wire types.** [`copypaste_ipc::Item`] and
//!   friends are serialised straight through to the WebView. v1 had three
//!   models of this contract; there is one.
//! * **It does not frame bytes by hand.** Framing is
//!   [`tokio_util::codec::LinesCodec`], the same codec the daemon and the CLI
//!   use (CLAUDE.md rule 1).
//! * **It never puts a path in an error string.** The socket lives under the
//!   user's home directory, so naming it discloses the local username
//!   (CLAUDE.md rule 4). Command errors are rendered verbatim by the frontend.

#![forbid(unsafe_code)]

use copypaste_ipc::redact::scrub_paths;
use copypaste_ipc::{
    socket_path, ErrorCode, Item, Method, Request, Response, ResponseData, StatusData,
    MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LinesCodec};

/// The one message the frontend turns into its "daemon not running" state.
/// Actionable, and free of anything that would name the socket.
const MSG_UNREACHABLE: &str =
    "CopyPaste isn't running. Start the daemon with `copypaste-daemon`, then try again.";

/// Ids only have to be unique within this process; the daemon echoes them back
/// so a reply can be matched to its request.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// daemon client
// ---------------------------------------------------------------------------

/// Send one request and return its payload, or a message fit to show a user.
///
/// Connects per call: the daemon may not be running when the UI starts, may be
/// restarted under it, and a held-open socket would only add a reconnect state
/// machine this bridge has no reason to own.
async fn call(method: Method) -> Result<Option<ResponseData>, String> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // Every connect failure collapses to one pathless message. The error kind
    // (missing, refused, permission denied) is not actionably different to a
    // user, and reporting it would tempt someone to include the path.
    let stream = UnixStream::connect(socket_path())
        .await
        .map_err(|_| MSG_UNREACHABLE.to_string())?;
    let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

    let line = serde_json::to_string(&Request {
        id,
        protocol_version: PROTOCOL_VERSION,
        method,
    })
    .map_err(|_| "Could not encode the request.".to_string())?;

    framed
        .send(line)
        .await
        .map_err(|_| MSG_UNREACHABLE.to_string())?;

    let line = match framed.next().await {
        Some(Ok(line)) => line,
        // A hang-up with no reply is the daemon's read-timeout behaviour and is
        // indistinguishable from it having gone away.
        Some(Err(_)) | None => return Err(MSG_UNREACHABLE.to_string()),
    };

    let response: Response = serde_json::from_str(&line)
        .map_err(|_| "Could not understand the daemon's reply.".to_string())?;

    if response.id != id {
        return Err("The daemon answered a different request.".to_string());
    }

    into_data(response)
}

/// Split a reply into payload or user-facing failure.
///
/// Branches on `error_code`, never on the `error` string (port manifest 04, I9).
fn into_data(response: Response) -> Result<Option<ResponseData>, String> {
    if response.ok {
        return Ok(response.data);
    }
    let message = match response.error_code {
        Some(ErrorCode::NotReady) => "CopyPaste is still starting up. Try again in a moment.".to_string(),
        Some(ErrorCode::ProtocolMismatch) => format!(
            "The app and the daemon are different versions: this app speaks IPC protocol v{PROTOCOL_VERSION}. \
             Upgrade both and restart the daemon."
        ),
        _ => scrub_paths(
            response
                .error
                .as_deref()
                .unwrap_or("The daemon reported a failure but gave no reason."),
        ),
    };
    Err(message)
}

fn shape_error(expected: &str) -> String {
    format!("The daemon replied with something other than {expected}; the app and the daemon may be different versions.")
}

fn expect_items(data: Option<ResponseData>) -> Result<Vec<Item>, String> {
    match data {
        Some(ResponseData::Items(items)) => Ok(items),
        _ => Err(shape_error("a list of items")),
    }
}

fn expect_item(data: Option<ResponseData>) -> Result<Item, String> {
    match data {
        Some(ResponseData::Item(item)) => Ok(item),
        _ => Err(shape_error("an item")),
    }
}

fn expect_status(data: Option<ResponseData>) -> Result<StatusData, String> {
    match data {
        Some(ResponseData::Status(status)) => Ok(status),
        _ => Err(shape_error("daemon status")),
    }
}

// ---------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------

/// Most recent items, newest first; pinned ahead of unpinned.
#[tauri::command]
async fn list(limit: u32, offset: u32) -> Result<Vec<Item>, String> {
    expect_items(call(Method::List { limit, offset }).await?)
}

/// Full-text search. Sensitive items are never indexed and never returned.
#[tauri::command]
async fn search(query: String, limit: u32) -> Result<Vec<Item>, String> {
    expect_items(call(Method::Search { query, limit }).await?)
}

/// Put an item's content back on the system clipboard.
#[tauri::command]
async fn copy_item(id: String) -> Result<Item, String> {
    expect_item(call(Method::Copy { id }).await?)
}

#[tauri::command]
async fn add_item(content: String) -> Result<Item, String> {
    expect_item(call(Method::Add { content }).await?)
}

/// `true` once the daemon has confirmed the row is gone. An unknown id is a
/// `not_found` failure, not a quiet `false`.
#[tauri::command]
async fn delete_item(id: String) -> Result<bool, String> {
    call(Method::Delete { id }).await?;
    Ok(true)
}

/// Returns the updated item, so the caller does not have to re-list.
#[tauri::command]
async fn set_pinned(id: String, pinned: bool) -> Result<Item, String> {
    expect_item(call(Method::Pin { id, pinned }).await?)
}

#[tauri::command]
async fn status() -> Result<StatusData, String> {
    expect_status(call(Method::Status).await?)
}

// ---------------------------------------------------------------------------
// window + tray
// ---------------------------------------------------------------------------

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window("main")
}

fn toggle_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit CopyPaste", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("CopyPaste")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_window(app),
            // The only way out: closing the window merely hides it
            // (manifest 06, INV-36).
            "quit" => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing hides; quitting is a tray decision (manifest 06, INV-36).
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list,
            search,
            copy_item,
            add_item,
            delete_item,
            set_pinned,
            status
        ])
        .run(tauri::generate_context!())
        .expect("could not start CopyPaste");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_supplied_path_never_reaches_the_frontend() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":false,"error":"could not open /Users/dmitriy/Library/x.db","error_code":"internal"}"#,
        )
        .unwrap();
        let err = into_data(response).unwrap_err();
        assert!(!err.contains("dmitriy"), "{err}");
        assert!(err.contains("<path>"), "{err}");
    }

    #[test]
    fn the_unreachable_message_is_actionable_and_pathless() {
        assert!(MSG_UNREACHABLE.contains("copypaste-daemon"));
        assert!(!MSG_UNREACHABLE.contains('/'));
    }

    #[test]
    fn items_deserialise_into_the_shared_wire_type() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":true,"data":[{"id":"a","content":"hi","content_type":"text/plain",
               "created_at":5,"pinned":true,"is_sensitive":false}]}"#,
        )
        .unwrap();
        let items = expect_items(into_data(response).unwrap()).unwrap();
        assert_eq!(items[0].content, "hi");
        assert!(items[0].pinned);
    }

    #[test]
    fn a_wrong_shape_is_reported_rather_than_silently_defaulted() {
        let response: Response = serde_json::from_str(r#"{"id":1,"ok":true,"data":{}}"#).unwrap();
        assert!(expect_items(into_data(response).unwrap()).is_err());
    }

    #[test]
    fn an_untagged_failure_still_becomes_an_error() {
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":false,"error":"unknown method"}"#).unwrap();
        assert!(into_data(response).is_err());
    }

    #[test]
    fn not_ready_reads_as_still_starting_up() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":false,"error":"still starting","error_code":"not_ready"}"#,
        )
        .unwrap();
        assert!(into_data(response).unwrap_err().contains("starting up"));
    }
}
