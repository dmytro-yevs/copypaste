//! Desktop global-shortcut commands. The shell owns persistence and native
//! registration; this file is only the small, stable IPC boundary.

use tauri::{AppHandle, State};

use crate::backend::BackendError;
use crate::shell::shortcut::{DEFAULT_SHORTCUT, ShortcutSettings};

type Result<T> = std::result::Result<T, BackendError>;

#[tauri::command]
pub fn get_default_shortcut() -> &'static str {
    DEFAULT_SHORTCUT
}

#[tauri::command]
pub fn get_shortcut(settings: State<'_, ShortcutSettings>) -> String {
    settings.current()
}

#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    settings: State<'_, ShortcutSettings>,
    accelerator: String,
) -> Result<()> {
    settings.set(&app, accelerator.trim())
}
