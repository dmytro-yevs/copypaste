//! Putting text the WebView already holds onto the system clipboard.
//!
//! # Why this is a command and not a plugin permission
//!
//! `capabilities/default.json` withholds `clipboard-manager:allow-write-text`,
//! and the argument there is sound: granting it hands *any* script in the
//! WebView a direct line to the system clipboard, with nothing of ours in the
//! path. The property that file states — every clipboard write goes through a
//! command — is what this preserves. The write happens in Rust, through the
//! plugin's own API, which capability files do not gate; the command is in
//! `generate_handler!`, so the complete list of things that can write the
//! clipboard stays greppable in one place.
//!
//! It is not the same job as `commands::history::copy_item`, which takes an
//! *id* so that a sensitive item's plaintext never has to enter the WebView at
//! all. Here the text is already on screen — the pairing code the user is
//! reading — so there is nothing to keep out; what is left is not handing the
//! capability to everything else running in the page.

use tauri::{AppHandle, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::backend::BackendError;

type Result<T> = std::result::Result<T, BackendError>;

const MSG_NOTHING: &str = "There is nothing to copy.";
const MSG_FAILED: &str = "That couldn't be copied to the clipboard.";
const MSG_READ_FAILED: &str = "That pairing detail couldn't be read from the clipboard.";

/// Put `text` on the system clipboard.
///
/// A failure is returned, never swallowed. The one caller is the pairing
/// dialog's copy button, and a copy that silently does nothing sends the user
/// to the other device to paste a code that is not there.
#[tauri::command]
pub async fn copy_text<R: Runtime>(app: AppHandle<R>, text: String) -> Result<()> {
    if text.is_empty() {
        return Err(BackendError::Invalid(MSG_NOTHING));
    }
    app.clipboard()
        // The platform error is dropped rather than rendered: it is written by
        // the OS, and this string is shown to a user (CLAUDE.md rule 4).
        .write_text(text)
        .map_err(|e| {
            tracing::warn!(error = %e, "a clipboard write failed");
            BackendError::Internal(MSG_FAILED.to_string())
        })
}

/// Read the pairing payload after the user expressly selects Paste in the
/// pairing ceremony. The plugin permission remains withheld from the WebView.
#[tauri::command]
pub async fn read_pairing_text<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.clipboard().read_text().map_err(|e| {
        tracing::warn!(error = %e, "a pairing clipboard read failed");
        BackendError::Internal(MSG_READ_FAILED.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both sentences are complete and name no path — they are rendered
    /// verbatim by the frontend.
    #[test]
    fn the_messages_are_sentences_with_nothing_to_leak() {
        for message in [MSG_NOTHING, MSG_FAILED, MSG_READ_FAILED] {
            assert!(message.ends_with('.'), "{message}");
            assert!(!message.contains('/'), "{message}");
        }
    }
}
