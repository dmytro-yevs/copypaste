use tauri::AppHandle;

use crate::shell::appearance::{self, NativeTheme};

/// Apply the resolved web appearance to the platform-owned chrome.
#[tauri::command]
pub fn set_native_theme(app: AppHandle, theme: NativeTheme) {
    appearance::apply(&app, theme);
}

#[tauri::command]
pub fn system_accent(app: AppHandle) -> appearance::SystemAccent {
    appearance::system_accent(&app)
}
