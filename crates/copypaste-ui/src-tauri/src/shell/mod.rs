//! Platform shell integration: desktop tray, shortcuts and launch-at-login;
//! window, notification and screen-protection helpers on every platform.

pub mod appearance;
pub mod autostart;
#[cfg(not(target_os = "android"))]
pub mod hotkey;
pub mod notify;
pub mod protection;
pub mod shortcut;
#[cfg(not(target_os = "android"))]
pub mod tray;
pub mod window;
