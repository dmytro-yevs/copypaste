//! The background service's own settings.
//!
//! Everything Settings showed before this existed — theme, accent, preview
//! lines — is the app's own preference and never leaves the WebView. These two
//! commands are the other half: the values that govern what the service *does*,
//! which had no route into the app at all (CLAUDE.md rule 6).
//!
//! # A patch, not a record
//!
//! [`set_config`] takes a [`ConfigPatch`]. Sending a whole `ConfigData` back
//! would make two open Settings screens — or a screen and the CLI — a lost
//! update, because each would carry a stale copy of whatever the other had just
//! changed. The patch names only the field that moved.
//!
//! # `restart_required` is an answer, not a warning
//!
//! [`ConfigApplied::restart_required`] lists the fields whose new value the
//! running service has kept but not yet acted on. It comes back from the write
//! rather than being derived here, so the app cannot disagree with the service
//! about which those are; `ConfigData::field_liveness` is the one classification
//! and it lives beside the fields.
//!
//! Nothing here is secret and nothing here is a path, so both types cross the
//! bridge verbatim — unlike an item, which has plaintext to drop (`crate::model`).

use copypaste_ipc::{ConfigApplied, ConfigPatch};
use tauri::{AppHandle, Emitter, State};

use crate::backend::{Backend, BackendError, SelectedBackend};

type Result<T> = std::result::Result<T, BackendError>;

/// The service's effective settings.
#[tauri::command]
pub async fn get_config(backend: State<'_, SelectedBackend>) -> Result<ConfigApplied> {
    backend.get_config().await
}

/// Change the settings this patch names, and only those.
///
/// Rejected whole if any value is out of range: the service validates into a
/// new record and keeps the old one on failure, so a refused write leaves every
/// field — including the valid ones in the same patch — exactly as it was.
#[tauri::command]
pub async fn set_config(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    patch: ConfigPatch,
) -> Result<ConfigApplied> {
    if patch == ConfigPatch::default() {
        // An empty patch is a round trip that can only report what the caller
        // already had. Refusing it keeps a control that failed to read its own
        // value from silently "saving" nothing.
        return Err(BackendError::Invalid("There was nothing to change."));
    }
    let private_mode_changed = patch.private_mode.is_some();
    let applied = backend.set_config(patch).await?;
    if private_mode_changed {
        let _ = app.emit("private-mode-changed", applied.config.private_mode);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_patch_is_refused_before_it_costs_a_round_trip() {
        let err = BackendError::Invalid("There was nothing to change.");
        assert!(!err.to_string().contains('/'), "{err}");
    }

    /// The patch has to serialise with the fields it does *not* set absent, or
    /// the service reads an omission as "set this to null" and the lost update
    /// the patch exists to prevent comes back through serde.
    #[test]
    fn an_unset_field_is_absent_from_the_wire_form() {
        let json = serde_json::to_string(&ConfigPatch {
            lan_visibility: Some(false),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(json, r#"{"lan_visibility":false}"#);
    }
}
