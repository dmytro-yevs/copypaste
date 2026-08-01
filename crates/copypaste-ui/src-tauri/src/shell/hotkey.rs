//! The global hotkey, and the guard that keeps it permission-free.
//!
//! # The finding this file exists to enforce
//!
//! ADR-0002 left one open question: whether `tauri-plugin-global-shortcut`
//! costs an Accessibility grant on macOS, which — for an ad-hoc-signed app
//! whose cdhash changes on every build — would be silently revoked on every
//! update (ADR-0001). The crate has now been read. The answer is
//! **mostly no, with one sharp edge**, and the edge is why this is a module
//! rather than a line in the ADR.
//!
//! `tauri-plugin-global-shortcut` 2.3.2 delegates to `global-hotkey` 0.8.0. In
//! `global-hotkey-0.8.0/src/platform_impl/macos/mod.rs`:
//!
//! * `GlobalHotKeyManager::new()` (line 44) installs a **Carbon** event handler
//!   via `InstallEventHandler` and nothing else. No tap is created at startup.
//! * `register()` (line 83) looks the key up in `key_to_scancode`, and for
//!   every ordinary key calls **`RegisterEventHotKey`** (line 116) — the one
//!   public global-hotkey API that needs no Accessibility, because the app is
//!   told about exactly one combination and never sees other input. This is the
//!   path ADR-0001's table already assumed.
//! * **But** if `key_to_scancode` returns `None` and the key is a media key
//!   (line 140), `register` falls through to `start_watching_media_keys()`,
//!   which calls **`CGEventTapCreate`** (line 206) at
//!   `CGEventTapLocation::Session` with `CGEventTapOptions::Default` — an
//!   *active* tap, not a listener. That requires Accessibility.
//!
//! So the permission cost is not a property of the crate; it is a property of
//! **which key is registered**. Five keys — `MediaPlayPause`,
//! `MediaTrackNext`, `MediaTrackPrevious`, `MediaFastForward`, `MediaRewind` —
//! take the tap. Every other key takes Carbon.
//!
//! # Why a guard rather than a note
//!
//! The plugin parses shortcut strings straight through
//! `Shortcut::from_str`, and `global-hotkey`'s parser accepts `MEDIAPLAYPAUSE`
//! and friends (`hotkey.rs` lines 335-340). So the moment the app grows a
//! user-configurable shortcut recorder, a user typing a media key would cause a
//! `CGEventTap` to be created and a TCC prompt to appear — and after the next
//! `brew upgrade` the grant would be gone and the hotkey would silently stop
//! working. That is precisely the outcome ADR-0002 says not to ship.
//!
//! A comment cannot prevent that. [`is_permission_free`] can, and
//! [`register`] refuses anything it rejects. The check is on the key rather
//! than on a blocklist of strings so that it cannot be sidestepped by
//! spelling — `Code` is what `register` actually branches on.
//!
//! # Cost we accept, unchanged from ADR-0001
//!
//! `RegisterEventHotKey` cannot bind modifier-only shortcuts, so a future
//! recorder must not offer "double-tap Control" and similar. And the hotkey
//! shows the window; it never synthesises a paste.
//!
//! # Unverified
//!
//! The source reading is exact and cited. What has **not** been done is running
//! it: this host is Linux with no macOS SDK, so no build here has ever called
//! `RegisterEventHotKey`. The claim "no TCC prompt appears" is inferred from
//! the API used, not observed.

use std::str::FromStr;
use tauri::{AppHandle, Runtime};

use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

/// Whether binding `code` avoids the `CGEventTap` path in `global-hotkey`.
///
/// The five media keys are the whole of the exception; see the module docs for
/// the file and line in the crate. Written as an explicit `matches!` on the
/// rejected set rather than an allowlist of accepted keys, because
/// `global-hotkey` decides by `key_to_scancode` returning `None` and the
/// rejected set is the part that is stable — a new ordinary key added upstream
/// should be allowed here without an edit, whereas a new *media* key must not
/// be.
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

/// Why a shortcut was refused. `&'static str`, so no path can reach it.
pub const MSG_NEEDS_ACCESSIBILITY: &str =
    "Media keys can't be used as the shortcut: macOS would ask for Accessibility \
     access, and CopyPaste would lose it on every update. Pick another key.";

pub const MSG_NEEDS_MODIFIER: &str = "Choose a key combination with Cmd, Ctrl, Option, or Shift.";

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
    if !is_permission_free(shortcut.key) {
        return Err(MSG_NEEDS_ACCESSIBILITY);
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
    if !is_permission_free(shortcut.key) {
        return Err(MSG_NEEDS_ACCESSIBILITY);
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

    /// The load-bearing test. Each of these five reaches
    /// `start_watching_media_keys` in `global-hotkey`, which calls
    /// `CGEventTapCreate` and therefore requires Accessibility — the permission
    /// ADR-0001 says this app must never need.
    #[test]
    fn every_media_key_is_refused() {
        for code in [
            Code::MediaPlayPause,
            Code::MediaTrackNext,
            Code::MediaTrackPrevious,
            Code::MediaFastForward,
            Code::MediaRewind,
        ] {
            assert!(
                !is_permission_free(code),
                "{code:?} takes the CGEventTap path and must be refused"
            );
        }
    }

    /// Everything with a Carbon scancode is fine, including the keys a
    /// shortcut recorder is most likely to offer.
    #[test]
    fn ordinary_keys_are_permission_free() {
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
            // Volume keys have scancodes in `key_to_scancode` (0x48-0x4a) and
            // are *not* in `is_media_key`, so they take Carbon too.
            Code::AudioVolumeUp,
            Code::AudioVolumeMute,
        ] {
            assert!(is_permission_free(code), "{code:?} should be allowed");
        }
    }

    #[test]
    fn the_persisted_default_is_one_we_are_willing_to_bind() {
        let shortcut = validate(crate::shell::shortcut::DEFAULT_SHORTCUT).unwrap();
        assert!(is_permission_free(shortcut.key));
        assert_eq!(shortcut.key, Code::KeyV);
        #[cfg(target_os = "macos")]
        assert!(shortcut.mods.contains(Modifiers::SUPER));
        #[cfg(not(target_os = "macos"))]
        assert!(shortcut.mods.contains(Modifiers::CONTROL));
        assert!(shortcut.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn frontend_spelling_parses_and_media_keys_do_not() {
        assert!(validate("CmdOrCtrl+Shift+V").is_ok());
        assert_eq!(validate("V"), Err(MSG_NEEDS_MODIFIER));
        assert!(validate("Alt+V").is_ok());
        assert!(matches!(
            validate("CmdOrCtrl+MediaPlayPause"),
            Err(MSG_NEEDS_ACCESSIBILITY)
        ));
    }

    /// The refusal has to explain itself: a user told only "no" would try
    /// another media key.
    #[test]
    fn the_refusal_message_names_the_reason_and_the_remedy() {
        assert!(MSG_NEEDS_ACCESSIBILITY.contains("Accessibility"));
        assert!(MSG_NEEDS_ACCESSIBILITY.contains("every update"));
        assert!(MSG_NEEDS_ACCESSIBILITY.contains("Pick another key"));
        assert!(!MSG_NEEDS_ACCESSIBILITY.contains('/'));
    }
}
