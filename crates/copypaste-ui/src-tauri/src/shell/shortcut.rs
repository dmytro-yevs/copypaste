//! The persisted desktop shortcut, separate from the hotkey mechanism.
//!
//! The shell owns this setting because changing a global registration is a
//! native operation.  React receives only the selected accelerator and never
//! has to infer whether a key is actually registered.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use copypaste_fs::Visibility;
use tauri::{AppHandle, Manager as _, Runtime};

use crate::backend::BackendError;

/// Kept as text at the boundary the WebView understands. `hotkey` parses and
/// validates it before any native registration is changed.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
const FILE_NAME: &str = "shortcut.json";
const SAVE_ERROR: &str = "CopyPaste couldn't save the shortcut setting.";

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

fn read(path: &Path) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let value: String = serde_json::from_str(&value).ok()?;
    #[cfg(not(target_os = "android"))]
    if super::hotkey::validate(&value).is_err() {
        return None;
    }
    Some(value)
}

fn write(path: &Path, value: &str) -> Result<(), BackendError> {
    let encoded = serde_json::to_vec(value).map_err(|_| save_error())?;
    // The error deliberately carries no path: the config directory discloses the
    // local username (`AGENTS.md` rule 4).
    copypaste_fs::write_atomically(path, &encoded, Visibility::OwnerOnly).map_err(|_| save_error())
}

fn save_error() -> BackendError {
    BackendError::Internal(SAVE_ERROR.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

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
    fn stored_value_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        write(&path, DEFAULT_SHORTCUT).unwrap();
        assert_eq!(read(&path).as_deref(), Some(DEFAULT_SHORTCUT));
    }

    #[test]
    fn concurrent_writers_publish_one_complete_value_without_collisions() {
        const WRITERS: usize = 16;

        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join(FILE_NAME));
        let barrier = Arc::new(Barrier::new(WRITERS));
        let values: Vec<String> = (0..WRITERS)
            .map(|writer| format!("writer-{writer}-{}", "x".repeat(256 * 1024)))
            .collect();

        std::thread::scope(|scope| {
            let handles: Vec<_> = values
                .iter()
                .map(|value| {
                    let path = Arc::clone(&path);
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        write(path.as_path(), value)
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap().unwrap();
            }
        });

        let stored: String = serde_json::from_slice(&fs::read(path.as_path()).unwrap()).unwrap();
        assert!(values.contains(&stored));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_write_hides_the_path() {
        // The config directory discloses the local username, so no failure here
        // may put it in a message a user sees (`AGENTS.md` rule 4). Induced with
        // a parent that is a regular file, so the directory cannot be created.
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("not-a-directory");
        fs::write(&blocked, "").unwrap();
        let path = blocked.join(FILE_NAME);

        let error = write(&path, DEFAULT_SHORTCUT).unwrap_err();

        assert_eq!(error.to_string(), SAVE_ERROR);
        assert!(!error.to_string().contains(&path.display().to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn stored_file_is_owner_private_after_every_write() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        write(&path, DEFAULT_SHORTCUT).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            copypaste_fs::OWNER_ONLY_MODE
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        write(&path, "CmdOrCtrl+Shift+B").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            copypaste_fs::OWNER_ONLY_MODE
        );
    }
}
