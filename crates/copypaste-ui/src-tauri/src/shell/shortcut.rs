//! The persisted desktop shortcut, separate from the hotkey mechanism.
//!
//! The shell owns this setting because changing a global registration is a
//! native operation.  React receives only the selected accelerator and never
//! has to infer whether a key is actually registered.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager as _, Runtime};

use crate::backend::BackendError;

/// Kept as text at the boundary the WebView understands. `hotkey` parses and
/// validates it before any native registration is changed.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
const FILE_NAME: &str = "shortcut.json";

#[derive(Debug)]
pub struct ShortcutSettings {
    value: Mutex<String>,
    path: PathBuf,
}

impl ShortcutSettings {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Self, BackendError> {
        let path = app
            .path()
            .app_config_dir()
            .map_err(|_| {
                BackendError::Internal("CopyPaste couldn't locate its settings folder.".into())
            })?
            .join(FILE_NAME);
        let value = read(&path).unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());
        Ok(Self {
            value: Mutex::new(value),
            path,
        })
    }

    pub fn current(&self) -> String {
        self.value
            .lock()
            .expect("shortcut setting lock poisoned")
            .clone()
    }

    /// Startup must remain usable when another application owns the saved
    /// shortcut (manifest INV-24). The preference is still returned to the
    /// control, and the tray remains available.
    #[cfg(not(target_os = "android"))]
    pub fn register_startup<R: Runtime>(&self, app: &AppHandle<R>) {
        let value = self.current();
        if let Err(message) = super::hotkey::register_text(app, &value) {
            tracing::warn!(%message, shortcut = %value, "the saved global shortcut was not registered");
        }
    }

    #[cfg(target_os = "android")]
    pub fn register_startup<R: Runtime>(&self, _app: &AppHandle<R>) {}

    #[cfg(not(target_os = "android"))]
    pub fn set<R: Runtime>(&self, app: &AppHandle<R>, next: &str) -> Result<(), BackendError> {
        let previous = self.current();
        super::hotkey::replace(app, &previous, next).map_err(BackendError::Invalid)?;
        if let Err(error) = write(&self.path, next) {
            // The registration and its persisted source of truth must move
            // together. If persisting fails, restore the old working binding.
            let _ = super::hotkey::replace(app, next, &previous);
            return Err(error);
        }
        *self.value.lock().expect("shortcut setting lock poisoned") = next.to_string();
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn set<R: Runtime>(&self, _app: &AppHandle<R>, _next: &str) -> Result<(), BackendError> {
        Err(BackendError::Unsupported(
            "Global shortcuts are available on desktop only.",
        ))
    }
}

fn read(path: &PathBuf) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let value: String = serde_json::from_str(&value).ok()?;
    #[cfg(not(target_os = "android"))]
    if super::hotkey::validate(&value).is_err() {
        return None;
    }
    Some(value)
}

fn write(path: &PathBuf, value: &str) -> Result<(), BackendError> {
    let parent = path.parent().ok_or(BackendError::Internal(
        "CopyPaste couldn't save the shortcut setting.".into(),
    ))?;
    fs::create_dir_all(parent).map_err(|_| {
        BackendError::Internal("CopyPaste couldn't save the shortcut setting.".into())
    })?;
    let encoded = serde_json::to_string(value).map_err(|_| {
        BackendError::Internal("CopyPaste couldn't save the shortcut setting.".into())
    })?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, encoded).map_err(|_| {
        BackendError::Internal("CopyPaste couldn't save the shortcut setting.".into())
    })?;
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        BackendError::Internal("CopyPaste couldn't save the shortcut setting.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_or_empty_persisted_value_uses_the_safe_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "not json").unwrap();
        assert_eq!(read(&path), None);

        fs::write(&path, "\"\"").unwrap();
        assert_eq!(read(&path), None);
    }

    #[test]
    fn stored_value_round_trips_without_exposing_a_path_in_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        write(&path, DEFAULT_SHORTCUT).unwrap();
        assert_eq!(read(&path).as_deref(), Some(DEFAULT_SHORTCUT));
    }
}
