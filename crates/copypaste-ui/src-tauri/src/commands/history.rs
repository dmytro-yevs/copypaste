//! History commands: list, search, add, copy, reveal, delete, delete_all,
//! set_pinned.
//!
//! # Naming
//!
//! Command names track `copypaste_ipc::Method` — `delete_all` for `DeleteAll`,
//! `set_pinned` for `Pin { pinned }` — rather than reading as English verbs.
//! That is deliberate: the wire enum is the single model of the contract, so a
//! name that matches it is one fewer mapping to keep straight, and
//! `crates/copypaste-ui/src/lib/ipc.ts` is already written against exactly
//! these names.
//!
//! The exception is [`reveal_item`], which has no `Method` behind it. See its
//! docs.

use tauri::State;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::{UiImagePreview, UiInstalledSourceApp, UiItem, UiPage, UiSourceAppIcon};
use crate::source_app_icon::SourceAppIconCache;

type Result<T> = std::result::Result<T, BackendError>;

/// Most recent items, newest first; pinned ahead of unpinned.
///
/// Returns a page rather than a bare array so the count of rows that would not
/// decrypt travels with them (parity finding 17) and so the marker for the next
/// page does.
///
/// `cursor` is the previous page's `next_cursor`, and `None` asks for the
/// first. Not an offset: the list grows at the top while it is being read, so a
/// row number taken for one page names a different boundary by the next, and
/// the second page repeats a row or skips one (B-1, `CopyPaste-8ebg.57`).
#[tauri::command]
pub async fn list(
    backend: State<'_, SelectedBackend>,
    limit: u32,
    cursor: Option<String>,
) -> Result<UiPage> {
    list_page(&*backend, limit, cursor.as_deref()).await
}

/// Load a page without making a second, unrelated round trip for its count.
///
/// A page that was already read must remain readable when a daemon restart
/// races the old count request. The live status query owns the global count;
/// [`UiPage::from`] supplies the loaded-page count for the compact Quick Paste
/// surface until its next refresh.
async fn list_page(backend: &impl Backend, limit: u32, cursor: Option<&str>) -> Result<UiPage> {
    Ok(backend.list(limit, cursor).await?.into())
}

/// Full-text search. Sensitive items are never indexed and never returned, so
/// a result set cannot contain one — the `UiItem` conversion is the belt to
/// that pair of braces.
#[tauri::command]
pub async fn search(
    backend: State<'_, SelectedBackend>,
    query: String,
    limit: u32,
) -> Result<UiPage> {
    Ok(backend.search(&query, limit).await?.into())
}

/// Add an item to history without going through the clipboard.
#[tauri::command]
pub async fn add_item(backend: State<'_, SelectedBackend>, content: String) -> Result<UiItem> {
    if content.trim().is_empty() {
        // Rejected here as well as in the backend so the round trip is not
        // spent learning something this side already knew.
        return Err(BackendError::Invalid("There is nothing to add."));
    }
    Ok(backend.add(&content).await?.into())
}

/// Put an item's content on the system clipboard.
///
/// Takes an id. The content never enters the WebView, which is what lets a
/// sensitive item be copied at all (ADR-0001: the user then presses Cmd+V —
/// the app never synthesises a paste).
#[tauri::command]
pub async fn copy_item(backend: State<'_, SelectedBackend>, id: String) -> Result<UiItem> {
    Ok(backend.copy(&id).await?.into())
}

/// Put an item on the clipboard as plain text, without exposing it to the WebView.
#[tauri::command]
pub async fn copy_item_as_plain_text(
    backend: State<'_, SelectedBackend>,
    id: String,
) -> Result<UiItem> {
    Ok(backend.copy_as_plain_text(&id).await?.into())
}

/// The deliberate reveal gesture: return one item's plaintext.
///
/// The single route back to a secret, and it is a command of its own so that it
/// is visible in the handler list, greppable, and impossible to reach by
/// accident from a list render. Everything else about a sensitive item —
/// copying it, pinning it, deleting it — goes by id and needs none of this.
///
/// Manifest 06 SCRH-7 requires a revealed item to re-hide on window blur and
/// after a timeout. That is the frontend's job, and it is only possible because
/// the plaintext arrives here as a one-off value rather than living in the list
/// state.
#[tauri::command]
pub async fn reveal_item(backend: State<'_, SelectedBackend>, id: String) -> Result<String> {
    Ok(backend.get(&id).await?.content)
}

/// A lazy thumbnail for an image row. This cannot return a sensitive image:
/// the backend refuses it before any bytes cross into the WebView.
#[tauri::command]
pub async fn get_image_preview(
    backend: State<'_, SelectedBackend>,
    id: String,
) -> Result<UiImagePreview> {
    Ok(backend.image_preview(&id).await?.into())
}

/// Resolve the captured source application's icon without exposing an app path
/// to the WebView. A missing or unqueryable app is a normal fallback state.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn get_source_app_icon(
    bundle_id: String,
    cache: State<'_, SourceAppIconCache>,
) -> Option<UiSourceAppIcon> {
    cache.resolve_desktop(&bundle_id)
}

/// Android-only installed application catalogue used by Settings → Service.
/// Desktop keeps its existing history/manual bundle-id workflow because macOS
/// does not expose the same installed-package catalogue to a sandboxed app.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn list_installed_source_apps(
    capture: State<'_, crate::capture::SelectedCapture>,
) -> Result<Vec<UiInstalledSourceApp>> {
    capture.installed_source_apps().map(|apps| {
        apps.into_iter()
            .map(|app| UiInstalledSourceApp::new(app.package_id, app.label))
            .collect()
    })
}

/// The command is registered on both platforms so the shared frontend keeps a
/// single IPC surface. It is intentionally empty on desktop, where settings
/// retains its bundle-id/manual flow instead of pretending macOS has Android's
/// package catalogue.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn list_installed_source_apps() -> Result<Vec<UiInstalledSourceApp>> {
    Ok(Vec::new())
}

/// Android resolves package icons through the application PackageManager.
/// Devices or package-visibility policies that cannot query the source simply
/// retain the semantic icon already rendered by the view.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn get_source_app_icon(
    bundle_id: String,
    cache: State<'_, SourceAppIconCache>,
    capture: State<'_, crate::capture::SelectedCapture>,
) -> Option<UiSourceAppIcon> {
    cache.resolve_with(&bundle_id, |package_id| {
        capture
            .source_app_icon(package_id)
            .and_then(|icon| UiSourceAppIcon::from_base64(icon.png_base64, icon.width, icon.height))
    })
}

/// `true` once the backend has confirmed the row is gone. An unknown id is a
/// not-found failure, not a quiet `false`.
#[tauri::command]
pub async fn delete_item(backend: State<'_, SelectedBackend>, id: String) -> Result<bool> {
    backend.delete(&id).await?;
    Ok(true)
}

/// Delete everything, and report how many rows went.
///
/// No confirmation here: the CLI prompts because a shell has nowhere else to
/// ask, but a dialog is the frontend's to own, and putting a second gate in the
/// bridge would mean two places could disagree about whether the user meant it.
#[tauri::command]
pub async fn delete_all(backend: State<'_, SelectedBackend>) -> Result<u64> {
    backend.clear().await
}

/// Pin or unpin an item, returning the updated item so the caller need not
/// re-list.
///
/// One command taking a boolean rather than a `pin` / `unpin` pair, because
/// that is the shape of `copypaste_ipc::Method::Pin` and of the frontend that
/// already calls it.
#[tauri::command]
pub async fn set_pinned(
    backend: State<'_, SelectedBackend>,
    id: String,
    pinned: bool,
) -> Result<UiItem> {
    Ok(backend.set_pinned(&id, pinned).await?.into())
}

/// Rewrite the order of the pinned section.
///
/// Takes the complete pinned list in the wanted order. Partial moves are not
/// offered: see `Backend::reorder_pinned` for why a full ordering is the only
/// shape that survives a concurrent pin.
#[tauri::command]
pub async fn reorder_pinned(backend: State<'_, SelectedBackend>, ids: Vec<String>) -> Result<()> {
    if ids.is_empty() {
        // Not an error worth a round trip, and not something to send: an empty
        // ordering would be indistinguishable from "unpin everything" to a
        // careless implementation on the other side.
        return Ok(());
    }
    backend.reorder_pinned(&ids).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Page, testing::FakeBackend};

    /// `add_item` rejects blank content before spending a round trip, and the
    /// message it uses is one a user can act on.
    #[test]
    fn blank_content_is_rejected_with_an_actionable_message() {
        let err = BackendError::Invalid("There is nothing to add.");
        let shown = err.to_string();
        assert!(shown.ends_with('.'), "not a sentence: {shown}");
        assert!(!shown.contains('/'), "{shown}");
    }

    #[tokio::test]
    async fn a_loaded_page_survives_an_independent_status_failure() {
        let backend = FakeBackend::failing().with_page(Page::default());

        // `FakeBackend::failing` rejects `status`. Listing must not depend on
        // it: the status query is separate and a daemon can restart between
        // two socket connections.
        let page = list_page(&backend, 50, None).await.unwrap();
        assert!(page.items().is_empty());
    }
}
