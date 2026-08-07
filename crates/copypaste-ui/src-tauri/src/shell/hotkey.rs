//! The global hotkey, and the per-platform policy about which keys it will
//! bind.
//!
//! # macOS: the finding this file exists to enforce
//!
//! ADR-0002 left one open question: whether `tauri-plugin-global-shortcut`
//! costs an Accessibility grant on macOS, which — for an ad-hoc-signed app
//! whose cdhash changes on every build — would be silently revoked on every
//! update (ADR-0001). The crate has now been read. The answer is
//! **mostly no, with one sharp edge**.
//!
//! `tauri-plugin-global-shortcut` 2.3.2 delegates to `global-hotkey` 0.8.0. In
//! `global-hotkey-0.8.0/src/platform_impl/macos/mod.rs`:
//!
//! * `GlobalHotKeyManager::new()` (line 44) installs a **Carbon** event handler
//!   via `InstallEventHandler` and nothing else. No tap is created at startup.
//! * `register()` (line 83) looks the key up in `key_to_scancode`, and for
//!   every ordinary key calls **`RegisterEventHotKey`** (line 116) — the one
//!   public global-hotkey API that needs no Accessibility, because the app is
//!   told about exactly one combination and never sees other input.
//! * **But** if `key_to_scancode` returns `None` and the key is a media key
//!   (line 140), `register` falls through to `start_watching_media_keys()`,
//!   which calls **`CGEventTapCreate`** (line 206) with
//!   `CGEventTapOptions::Default` — an *active* tap. That requires
//!   Accessibility.
//!
//! So the cost is not a property of the crate; it is a property of **which key
//! is registered**. A comment cannot prevent a shortcut recorder from offering
//! one of those five keys, but [`refusal`] can, and it is checked on the `Code`
//! rather than on a spelling so it cannot be sidestepped.
//!
//! `RegisterEventHotKey` also cannot bind modifier-only shortcuts, so no
//! recorder may offer "double-tap Control"; and the hotkey shows the window, it
//! never synthesises a paste.
//!
//! # Windows: a different platform, so a different policy
//!
//! `global-hotkey`'s Windows path is `RegisterHotKey`
//! (`platform_impl/windows/mod.rs` line 91), which needs no permission at all
//! and has `VK_MEDIA_*` codes for play/pause, stop and track skip. Refusing
//! those here would deny a Windows user a binding the OS supports, for a reason
//! that belongs to another operating system.
//!
//! What that table has no entry for is `MediaFastForward` and `MediaRewind`, so
//! `register` fails them with `Unknown VKCode`. They are refused up front
//! instead, because the alternative is reporting "already in use by another
//! app" about a key nothing has taken.
//!
//! # Unverified
//!
//! The source reading is exact and cited. Neither platform has been run: no
//! macOS host, and the Windows build is blocked. "No TCC prompt appears" and
//! "`RegisterHotKey` succeeds" are both inferred from the API used.

use std::str::FromStr;
use tauri::{AppHandle, Runtime};

use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

/// Why a shortcut was refused. `&'static str`, so no path can reach it.
pub const MSG_NEEDS_ACCESSIBILITY: &str =
    "Media keys can't be used as the shortcut: macOS would ask for Accessibility \
     access, and CopyPaste would lose it on every update. Pick another key.";

pub const MSG_NO_VIRTUAL_KEY: &str =
    "Windows has no key code for that media key, so it can't be bound. Pick another key.";

#[cfg(target_os = "windows")]
pub const MSG_NEEDS_MODIFIER: &str = "Choose a key combination with Ctrl, Alt, or Shift.";

#[cfg(not(target_os = "windows"))]
pub const MSG_NEEDS_MODIFIER: &str = "Choose a key combination with Cmd, Ctrl, Option, or Shift.";

/// Whether binding `code` avoids the `CGEventTap` path in `global-hotkey`.
///
/// The five media keys are the whole of the exception; see the module docs for
/// the file and line in the crate. Written as an explicit `matches!` on the
/// rejected set rather than an allowlist of accepted keys, because
/// `global-hotkey` decides by `key_to_scancode` returning `None` and the
/// rejected set is the part that is stable — a new ordinary key added upstream
/// should be allowed here without an edit, whereas a new *media* key must not
/// be.
#[cfg(not(target_os = "windows"))]
pub fn is_permission_free(code: Code) -> bool {
    !matches!(
        code,
        Code::MediaPlayPause
            | Code::MediaTrackNext
            | Code::MediaTrackPrevious
            | Code::MediaFastForward
            | Code::MediaRewind
    )
}

/// The reason this platform will not bind `code`, or `None` if it will.
#[cfg(not(target_os = "windows"))]
pub fn refusal(code: Code) -> Option<&'static str> {
    (!is_permission_free(code)).then_some(MSG_NEEDS_ACCESSIBILITY)
}

/// Only the two media keys `key_to_vk` has no `VK_` constant for. Everything
/// else, media keys included, goes to `RegisterHotKey` and costs nothing.
#[cfg(target_os = "windows")]
pub fn refusal(code: Code) -> Option<&'static str> {
    matches!(code, Code::MediaFastForward | Code::MediaRewind).then_some(MSG_NO_VIRTUAL_KEY)
}

/// Register the app's global shortcut.
///
/// Returns `Err` for a shortcut this app refuses to bind, or one the OS refused
/// (already taken by another app, typically). Both are reportable to a user;
/// neither is fatal, and the caller treats a failure as "no hotkey" rather than
/// as a reason not to start.
pub fn register<R: Runtime>(
    app: &AppHandle<R>,
    shortcut: Shortcut,
) -> std::result::Result<(), &'static str> {
    if let Some(reason) = refusal(shortcut.key) {
        return Err(reason);
    }

    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            // Fire on press only. `global-hotkey` sends both Pressed and
            // Released for a Carbon hotkey, and acting on both toggles the
            // window twice per keypress — it opens and immediately closes.
            if event.state == ShortcutState::Pressed {
                super::window::toggle_quick_paste(app);
            }
        })
        .map_err(|_| "That shortcut is already in use by another app.")
}

/// Validate user input before the old binding is disturbed. Parser details are
/// deliberately not returned: they are neither actionable nor stable across
/// plugin upgrades.
pub fn validate(accelerator: &str) -> std::result::Result<Shortcut, &'static str> {
    let shortcut = Shortcut::from_str(accelerator)
        .map_err(|_| "That key combination can't be used as a shortcut.")?;
    if shortcut.mods.is_empty() {
        return Err(MSG_NEEDS_MODIFIER);
    }
    if let Some(reason) = refusal(shortcut.key) {
        return Err(reason);
    }
    Ok(shortcut)
}

pub fn register_text<R: Runtime>(
    app: &AppHandle<R>,
    accelerator: &str,
) -> std::result::Result<(), &'static str> {
    register(app, validate(accelerator)?)
}

/// Swap registration transactionally as far as the native API permits. A
/// rejected new key restores the previous working registration before the
/// error reaches the UI.
pub fn replace<R: Runtime>(
    app: &AppHandle<R>,
    previous: &str,
    next: &str,
) -> std::result::Result<(), &'static str> {
    let next_shortcut = validate(next)?;
    if previous == next {
        return Ok(());
    }

    let was_registered = app.global_shortcut().is_registered(previous);
    if was_registered {
        app.global_shortcut()
            .unregister(previous)
            .map_err(|_| "CopyPaste couldn't update the global shortcut.")?;
    }
    if let Err(error) = register(app, next_shortcut) {
        if was_registered {
            let _ = register_text(app, previous);
        }
        return Err(error);
    }
    Ok(())
}

/// Install the plugin. Separate from [`register`] because the plugin has to be
/// built before the app exists, and the shortcut can only be bound after.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    /// The load-bearing test on the platforms that take the Carbon path. Each
    /// of these five reaches `start_watching_media_keys` in `global-hotkey`,
    /// which calls `CGEventTapCreate` and therefore requires Accessibility —
    /// the permission ADR-0001 says this app must never need.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn every_media_key_is_refused() {
        for code in [
            Code::MediaPlayPause,
            Code::MediaTrackNext,
            Code::MediaTrackPrevious,
            Code::MediaFastForward,
            Code::MediaRewind,
        ] {
            assert_eq!(
                refusal(code),
                Some(MSG_NEEDS_ACCESSIBILITY),
                "{code:?} takes the CGEventTap path and must be refused"
            );
        }
    }

    /// Windows binds media keys through `RegisterHotKey` at no permission cost,
    /// so refusing them would be borrowing another platform's constraint. The
    /// two exceptions are the ones `key_to_vk` cannot map at all.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_refuses_only_the_media_keys_with_no_virtual_key_code() {
        for code in [
            Code::MediaPlayPause,
            Code::MediaTrackNext,
            Code::MediaTrackPrevious,
        ] {
            assert_eq!(refusal(code), None, "{code:?} has a VK_ constant");
        }
        for code in [Code::MediaFastForward, Code::MediaRewind] {
            assert_eq!(refusal(code), Some(MSG_NO_VIRTUAL_KEY), "{code:?}");
        }
    }

    /// Everything with a scancode is fine everywhere, including the keys a
    /// shortcut recorder is most likely to offer.
    #[test]
    fn ordinary_keys_are_never_refused() {
        for code in [
            Code::KeyV,
            Code::KeyC,
            Code::Space,
            Code::Enter,
            Code::F1,
            Code::F12,
            Code::ArrowUp,
            Code::Digit1,
            Code::Escape,
            // Volume keys have scancodes in macOS's `key_to_scancode`
            // (0x48-0x4a) and are *not* in `is_media_key`, so they take Carbon;
            // Windows has `VK_VOLUME_*` for all three.
            Code::AudioVolumeUp,
            Code::AudioVolumeMute,
        ] {
            assert_eq!(refusal(code), None, "{code:?} should be allowed");
        }
    }

    #[test]
    fn the_persisted_default_is_one_we_are_willing_to_bind() {
        let shortcut = validate(crate::shell::shortcut::DEFAULT_SHORTCUT).unwrap();
        assert_eq!(refusal(shortcut.key), None);
        assert_eq!(shortcut.key, Code::KeyV);
        #[cfg(target_os = "macos")]
        assert!(shortcut.mods.contains(Modifiers::SUPER));
        #[cfg(not(target_os = "macos"))]
        assert!(shortcut.mods.contains(Modifiers::CONTROL));
        assert!(shortcut.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn frontend_spelling_parses_and_a_bare_key_does_not() {
        assert!(validate("CmdOrCtrl+Shift+V").is_ok());
        assert_eq!(validate("V"), Err(MSG_NEEDS_MODIFIER));
        assert!(validate("Alt+V").is_ok());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_media_key_is_refused_through_the_parser_too() {
        assert_eq!(
            validate("CmdOrCtrl+MediaPlayPause"),
            Err(MSG_NEEDS_ACCESSIBILITY)
        );
    }

    /// The refusal has to explain itself: a user told only "no" would try
    /// another media key.
    #[test]
    fn the_refusal_messages_name_the_reason_and_the_remedy() {
        assert!(MSG_NEEDS_ACCESSIBILITY.contains("Accessibility"));
        assert!(MSG_NEEDS_ACCESSIBILITY.contains("every update"));
        for message in [MSG_NEEDS_ACCESSIBILITY, MSG_NO_VIRTUAL_KEY] {
            assert!(message.contains("Pick another key"), "{message}");
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
