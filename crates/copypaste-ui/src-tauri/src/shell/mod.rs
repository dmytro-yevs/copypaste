//! Platform shell integration: desktop tray, shortcuts and launch-at-login;
//! window, notification and screen-protection helpers on every platform.

#[cfg(not(target_os = "android"))]
pub mod autostart;
#[cfg(not(target_os = "android"))]
pub mod hotkey;
pub mod notify;
pub mod protection;
#[cfg(not(target_os = "android"))]
pub mod tray;
pub mod window;
