//! The one verb behind the "Allow screenshots" setting (INV-35).

/// Let screen recorders see this window, or stop them.
///
/// The frontend calls this once at startup with the stored preference and again
/// whenever the user changes it. It is deliberately infallible and deliberately
/// one-way in effect: the window is already protected when this is first
/// reached, so a call that does not arrive — a bridge that is down, a platform
/// that refused — leaves the user protected rather than exposed.
#[tauri::command]
pub fn set_allow_screenshots(app: tauri::AppHandle, allow: bool) {
    crate::shell::protection::apply(&app, allow);
}
