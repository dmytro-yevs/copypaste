//! Main-window surfacing + paste-synthesis Tauri commands.

use tauri::Manager;

#[cfg(target_os = "macos")]
use super::state::PriorApp;

// ---------------------------------------------------------------------------
// Tauri commands — window management
// ---------------------------------------------------------------------------

/// Show the main CopyPaste window.
pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Bring the main CopyPaste window to the foreground.
///
/// Called by the React layer when an incoming pairing request (role=responder,
/// state=awaiting_sas) is detected so the SAS confirmation modal is visible
/// to the user immediately, without requiring them to open the app manually.
#[tauri::command]
pub(crate) fn focus_main_window(handle: tauri::AppHandle) {
    show_main(&handle);
}

/// Activate the previously-focused application (restoring focus) and then
/// synthesise a Cmd+V paste event so the clipboard content lands in the
/// target app.  Call this after `api.copyItem` and before hiding the popup.
#[tauri::command]
pub(crate) fn paste_to_frontmost(handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, KeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        use std::thread;
        use std::time::Duration;

        // Read the saved prior-app bundle ID.
        let bundle_id: Option<String> = handle
            .try_state::<PriorApp>()
            .and_then(|s| s.0.lock().ok().map(|g| g.clone()))
            .flatten();

        // Activate the prior app, then wait for it to become frontmost before
        // synthesising Cmd+V.  The wait uses a bounded polling loop rather than
        // a fixed sleep so that:
        //   • On fast systems the paste fires as soon as the OS confirms the
        //     transition (typically 10–30 ms), not after a blind 80 ms delay.
        //   • On slow or loaded systems the old 80 ms was racy — paste landed
        //     in the wrong window.  The poll backs off up to 300 ms total
        //     (37 × 8 ms) then proceeds unconditionally so we never hang.
        //
        // CopyPaste-78hg fix: replaced fixed 80 ms sleep.
        thread::spawn(move || {
            // Activate the prior app by bundle ID (best effort).
            if let Some(ref bid) = bundle_id {
                super::focus::activate_app_by_bundle_id(bid);
            }

            // Poll until the prior app is frontmost or the timeout expires.
            // 37 iterations × 8 ms ≈ 296 ms maximum wait.
            // When no prior-app bundle is known (popup opened but no external
            // app ever focused), skip the poll and proceed after a minimal wait
            // so the synthetic Cmd+V still fires even if it may land nowhere.
            const MAX_ITER: u32 = 37;
            const INTERVAL: Duration = Duration::from_millis(8);
            if let Some(ref bid) = bundle_id {
                let bid_clone = bid.clone();
                let matched = super::focus::poll_until_frontmost(
                    bid,
                    super::focus::frontmost_bundle_id,
                    MAX_ITER,
                    INTERVAL,
                );
                if !matched {
                    tracing::debug!(
                        "paste_to_frontmost: '{}' did not become frontmost within \
                         {}ms; proceeding with Cmd+V anyway",
                        bid_clone,
                        MAX_ITER * INTERVAL.as_millis() as u32,
                    );
                }
            } else {
                // No prior app recorded; wait one interval before firing.
                thread::sleep(INTERVAL);
            }

            // Synthesise Cmd+V (key down + key up).
            //
            // CGEvent::new_keyboard_event returns Err only in pathological cases
            // (e.g. event source exhaustion). This runs on a detached background
            // thread, so a panic here would abort the process silently from the
            // user's point of view — handle the error and log instead.
            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                let v_kc = KeyCode::ANSI_V;
                match (
                    CGEvent::new_keyboard_event(source.clone(), v_kc, true),
                    CGEvent::new_keyboard_event(source, v_kc, false),
                ) {
                    (Ok(kd), Ok(ku)) => {
                        kd.set_flags(CGEventFlags::CGEventFlagCommand);
                        kd.post(core_graphics::event::CGEventTapLocation::Session);
                        ku.set_flags(CGEventFlags::CGEventFlagCommand);
                        ku.post(core_graphics::event::CGEventTapLocation::Session);
                    }
                    _ => {
                        tracing::warn!(
                            "paste_to_frontmost: failed to synthesise Cmd+V keyboard event"
                        );
                    }
                }
            } else {
                tracing::warn!("paste_to_frontmost: failed to create CGEventSource");
            }
        });
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = handle;
        Ok(())
    }
}

/// Write `text` as plain UTF-8 to the system clipboard (strips all rich
/// formatting / MIME context), then activate the prior app and synthesise
/// Cmd+V — the Option+Enter "paste as plain text" shortcut (F1).
///
/// The caller is responsible for hiding the popup first so the prior app
/// receives focus before `paste_to_frontmost` fires Cmd+V.
///
/// On non-macOS this is a no-op (returns Ok).
#[tauri::command]
pub(crate) fn paste_plain_text(text: String, handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
        use objc2_foundation::NSString;

        // Write plain text to the general pasteboard, clearing all prior types.
        //
        // SAFETY: NSPasteboard / NSString ObjC bindings are correct; these calls
        // are safe on any thread that has an autorelease pool. Tauri command
        // handlers run on the Tokio runtime which provides an autorelease pool on
        // macOS, so this is safe here.
        unsafe {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            let ns_text = NSString::from_str(&text);
            // setString_forType returns bool — false means the write was rejected
            // (e.g. pasteboard is owned by another process). Log and continue; the
            // Cmd+V will paste whatever was on the clipboard before (graceful
            // degradation).
            let ok = pb.setString_forType(&ns_text, NSPasteboardTypeString);
            if !ok {
                tracing::warn!("paste_plain_text: NSPasteboard setString_forType returned false");
            }
        }

        // Reuse the existing paste-to-frontmost logic (activate prior app + Cmd+V).
        paste_to_frontmost(handle)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, handle);
        Ok(())
    }
}

/// Play a soft system sound (NSSound "Tink") after a successful copy.
///
/// Maccy parity: Maccy plays "Funk" or "Pop" depending on version; we use
/// "Tink" because it is shorter and less intrusive. The sound plays on the
/// main run-loop via `[NSSound play]` which is non-blocking from the Rust
/// perspective. Any failure (sound file missing, audio device unavailable) is
/// silently ignored so it never disrupts the copy flow.
///
/// The command is cross-platform safe: on non-macOS it is a no-op.
#[tauri::command]
pub(crate) fn play_copy_sound() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSSound;
        use objc2_foundation::NSString;

        // SAFETY: NSSound and NSString bindings are correct; ObjC calls are
        // safe when invoked from a thread that has an autorelease pool. Tauri
        // command handlers run on the Tokio runtime, which drives an ObjC
        // autorelease pool on macOS — so this is safe here.
        unsafe {
            let name = NSString::from_str("Tink");
            if let Some(sound) = NSSound::soundNamed(&name) {
                // play returns bool; ignore the result — best-effort only.
                let _ = sound.play();
            } else {
                tracing::debug!("play_copy_sound: NSSound 'Tink' not found (non-fatal)");
            }
        }
    }
}
