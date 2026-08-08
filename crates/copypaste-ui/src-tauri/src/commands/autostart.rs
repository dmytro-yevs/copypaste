//! Launch at login, as an IPC pair. Every platform mechanism and every `cfg`
//! lives in `shell::autostart`; this is the stable boundary Settings calls.

use tauri::AppHandle;

use crate::backend::BackendError;
use crate::shell::autostart;

#[tauri::command]
pub fn get_open_at_login(app: AppHandle) -> bool {
    autostart::is_enabled(&app)
}

/// Returns the state the system reports *after* the write, which is not always
/// the state that was asked for: on Windows the registry value can be set while
/// Task Manager's Startup override still holds the app off. A control that
/// echoed the request would then show a switch the machine disagrees with.
#[tauri::command]
pub fn set_open_at_login(app: AppHandle, enabled: bool) -> Result<bool, BackendError> {
    autostart::set_enabled(&app, enabled)?;
    Ok(autostart::is_enabled(&app))
}
