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
use crate::model::{ui_items, UiItem};

type Result<T> = std::result::Result<T, BackendError>;

/// Most recent items, newest first; pinned ahead of unpinned.
#[tauri::command]
pub async fn list(
    backend: State<'_, SelectedBackend>,
    limit: u32,
    offset: u32,
) -> Result<Vec<UiItem>> {
    Ok(ui_items(backend.list(limit, offset).await?))
}

/// Full-text search. Sensitive items are never indexed and never returned, so
/// a result set cannot contain one — the `UiItem` conversion is the belt to
/// that pair of braces.
#[tauri::command]
pub async fn search(
    backend: State<'_, SelectedBackend>,
    query: String,
    limit: u32,
) -> Result<Vec<UiItem>> {
    Ok(ui_items(backend.search(&query, limit).await?))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `add_item` rejects blank content before spending a round trip, and the
    /// message it uses is one a user can act on.
    #[test]
    fn blank_content_is_rejected_with_an_actionable_message() {
        let err = BackendError::Invalid("There is nothing to add.");
        let shown = err.to_string();
        assert!(shown.ends_with('.'), "not a sentence: {shown}");
        assert!(!shown.contains('/'), "{shown}");
    }
}
