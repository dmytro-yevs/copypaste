//! Desktop shell integration: menu-bar item, popover window, global hotkey,
//! launch at login.
//!
//! Everything here is desktop-only and everything here is a Tauri plugin or a
//! Tauri API — no native code is written in this crate. That was checked
//! before writing rather than after (CLAUDE.md rule 1):
//!
//! | need | what provides it |
//! |---|---|
//! | menu-bar item | `tauri`'s own `tray-icon` feature |
//! | popover show/hide/focus | `tauri`'s window API |
//! | global hotkey | `tauri-plugin-global-shortcut` → `global-hotkey` |
//! | launch at login | `tauri-plugin-autostart` → `auto-launch` |
//!
//! The one thing this module does add is a *policy* the plugins do not have:
//! [`hotkey::is_permission_free`], which refuses the shortcuts that would cost
//! an Accessibility grant. That is not a reimplementation of anything — it is a
//! constraint from ADR-0001 that no upstream crate knows about.
//!
//! # Android
//!
//! Everything except [`window`] and [`protection`] is desktop-only: Android has
//! no menu bar, no login items, and `tauri-plugin-global-shortcut` is itself
//! `#![cfg(not(any(target_os = "android", target_os = "ios")))]`.
//!
//! Those two compile everywhere because `crate::commands` may contain no `cfg`
//! — the command surface has to be the same text on both platforms (ADR-0002).
//! Each keeps its one platform difference inside itself, where it belongs:
//! `window::hide`, and `protection`'s two mechanisms for one setting.

#[cfg(not(target_os = "android"))]
pub mod autostart;
#[cfg(not(target_os = "android"))]
pub mod hotkey;
pub mod protection;
#[cfg(not(target_os = "android"))]
pub mod tray;
pub mod window;
