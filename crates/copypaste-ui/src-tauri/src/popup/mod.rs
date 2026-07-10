//! Popup window management — create, show, hide, position — plus macOS
//! helpers for focus tracking, CGEventTap, and synthetic paste events.
//!
//! This is a thin facade over the `popup/` submodules. `lib.rs` (and a few
//! sibling modules — `pairing.rs`, `tray.rs`) reference everything below as
//! `popup::<name>`; `generate_handler!` in particular depends on the exact
//! `#[tauri::command]` fn names/signatures re-exported here, so they are
//! FROZEN — do not rename without also updating the JS `invoke("...")`
//! callers.

mod commands_macos;
mod focus;
mod paste;
mod position;
mod setup;
mod state;
mod window;

// ---------------------------------------------------------------------------
// Re-exports — the `popup::` surface every external caller depends on.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub(crate) use state::{PriorApp, TapActive};

#[cfg(target_os = "macos")]
pub(crate) use focus::try_install_event_tap;
// `poll_until_frontmost` is extracted to be unit-testable without ObjC
// bindings; `lib.rs`'s `#[cfg(test)] mod tests` imports it as
// `popup::poll_until_frontmost` regardless of host target. The only consumer
// of *this* re-export is that test module — the non-test macOS caller
// (`paste.rs`) reaches the fn directly via `super::focus::poll_until_frontmost`,
// not through this facade — so gating on bare `#[cfg(test)]` (a subset of the
// underlying fn's `#[cfg(any(target_os = "macos", test))]`) is enough to
// compile everywhere and avoids an unused-import in non-test macOS builds.
#[cfg(test)]
pub(crate) use focus::poll_until_frontmost;

pub(crate) use window::toggle_popup;
// `hide_popup` itself has no direct Rust-level caller outside `generate_handler!`
// in `lib.rs`. That macro expands `popup::hide_popup` into both the hidden
// `__cmd__hide_popup!`/`__tauri_command_name_hide_popup!` macro invocations
// *and* passes the plain path `popup::hide_popup` itself as an argument to the
// `__cmd__hide_popup!` wrapper macro (see `tauri_macros::command::handler`),
// so the bare fn name must resolve too, or `generate_handler!` fails with E0425.
pub(crate) use window::hide_popup;
pub(crate) use window::{__cmd__hide_popup, __tauri_command_name_hide_popup};

#[cfg(target_os = "macos")]
pub(crate) use setup::setup_macos;
pub(crate) use setup::setup_main_window;

// `play_copy_sound` and `show_main` have genuine direct Rust callers
// (`tray.rs`, `pairing.rs`) in addition to `generate_handler!`. `focus_main_window`,
// `paste_plain_text`, and `paste_to_frontmost` are reached only via
// `generate_handler!` — but per the `hide_popup` note above, `generate_handler!`
// still needs the bare fn names to resolve (passed as an arg to the `__cmd__`
// wrapper macro), so all five plain fn names are re-exported alongside their
// `__cmd__`/`__tauri_command_name_` macros.
pub(crate) use paste::{
    __cmd__focus_main_window, __cmd__paste_plain_text, __cmd__paste_to_frontmost,
    __cmd__play_copy_sound, __tauri_command_name_focus_main_window,
    __tauri_command_name_paste_plain_text, __tauri_command_name_paste_to_frontmost,
    __tauri_command_name_play_copy_sound,
};
pub(crate) use paste::{focus_main_window, paste_plain_text, paste_to_frontmost};
pub(crate) use paste::{play_copy_sound, show_main};

// `check_accessibility_permission`, `request_accessibility_permission`, and
// `set_native_appearance` are reached only via `generate_handler!` — see the
// `hide_popup` note above for why the plain fn names must also be re-exported.
pub(crate) use commands_macos::{
    __cmd__check_accessibility_permission, __cmd__request_accessibility_permission,
    __cmd__set_native_appearance, __tauri_command_name_check_accessibility_permission,
    __tauri_command_name_request_accessibility_permission,
    __tauri_command_name_set_native_appearance,
};
pub(crate) use commands_macos::{
    check_accessibility_permission, request_accessibility_permission, set_native_appearance,
};

// `position` has no direct `popup::` caller — only `window::toggle_popup`
// reaches it internally via `super::position::position_popup` — so no
// re-export is needed here.
