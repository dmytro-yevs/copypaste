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
//! None of this is compiled there. Android has no menu bar, no login items, and
//! `tauri-plugin-global-shortcut` is itself
//! `#![cfg(not(any(target_os = "android", target_os = "ios")))]`. The module is
//! gated at its declaration in `lib.rs` so nothing here needs a `cfg` of its
//! own.

pub mod autostart;
pub mod hotkey;
pub mod tray;
pub mod window;
