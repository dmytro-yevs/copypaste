//! Accessibility permission + native-appearance Tauri commands.

// ---------------------------------------------------------------------------
// Tauri commands — Accessibility + CGEventTap (macOS)
// ---------------------------------------------------------------------------

/// Returns `true` when the Accessibility permission is granted.
/// On non-macOS platforms this always returns `true` (no permission needed).
#[tauri::command]
pub(crate) fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::event_tap::accessibility_granted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open the System Settings Accessibility pane (macOS) and attempt to
/// (re-)install the CGEventTap if permission was just granted.
/// On non-macOS this is a no-op.
#[tauri::command]
pub(crate) fn request_accessibility_permission(handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri::Manager;

        crate::event_tap::open_accessibility_settings();
        // The user may already have granted permission before clicking;
        // try installing the tap in the background.
        let shortcut = {
            let state: tauri::State<crate::config::CurrentShortcut> = handle.state();
            let s = state.0.lock().expect("mutex poisoned").clone();
            s
        };
        super::focus::try_install_event_tap(&handle, &shortcut);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// Synchronise the macOS native NSWindow appearance with the CSS theme so that
/// `NSVisualEffectView` (vibrancy) renders the correct glass tint.
///
/// Without this, the sidebar NSVisualEffectView uses `effectiveAppearance`
/// inherited from the NSApplication — which follows the OS dark/light setting,
/// NOT the user's in-app theme choice.  Calling `[win setAppearance:]` pins the
/// window to the desired variant regardless of the OS preference.
///
/// Accepted values for `appearance`:
///   "light" → NSAppearanceNameAqua
///   "dark"  → NSAppearanceNameDarkAqua
///   anything else → no-op (leaves the window at OS default)
///
/// On non-macOS this command is a no-op so the invoke call is safe everywhere.
/// In browser/mock mode (`HAS_TAURI` is false in the frontend) this is never
/// called at all.
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(crate) fn set_native_appearance(appearance: String, handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2_app_kit::{
            NSAppearance, NSAppearanceName, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSView,
        };
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use tauri::Manager;

        let Some(win) = handle.get_webview_window("main") else {
            return;
        };

        // Resolve the target appearance name constant.
        // NSAppearanceNameAqua/DarkAqua are `&'static NSAppearanceName` extern
        // statics; reading an extern static is unsafe.
        // SAFETY: these are immutable Apple-provided string constants — reading
        // the static reference has no preconditions and no data races.
        let appearance_name: &NSAppearanceName = unsafe {
            match appearance.as_str() {
                "light" => NSAppearanceNameAqua,
                "dark" => NSAppearanceNameDarkAqua,
                // Unknown value — leave the window at its OS-default appearance.
                _ => return,
            }
        };

        // Look up the NSAppearance by name.
        let Some(ns_appearance) = NSAppearance::appearanceNamed(appearance_name) else {
            tracing::warn!(
                "set_native_appearance: NSAppearance::appearanceNamed({appearance:?}) returned nil"
            );
            return;
        };

        // Obtain the raw AppKit window handle.  Tauri's WebviewWindow implements
        // HasWindowHandle; on macOS the inner handle is AppKit (ns_view pointer).
        let raw = match win.window_handle() {
            Ok(h) => h.as_raw(),
            Err(e) => {
                tracing::warn!("set_native_appearance: window_handle failed: {e}");
                return;
            }
        };
        let RawWindowHandle::AppKit(appkit_handle) = raw else {
            return;
        };

        // SAFETY: appkit_handle.ns_view is a valid NSView* for the lifetime of
        // the window.  We access it only on the main thread (Tauri command
        // handlers on macOS run on the main thread).  The cast to &NSView is safe
        // because objc2-app-kit 0.2 NSView is repr(C) / ABI-compatible.
        unsafe {
            let ns_view: &NSView = appkit_handle.ns_view.cast().as_ref();
            if let Some(ns_window) = ns_view.window() {
                // setAppearance: is declared on NSWindow via NSAppearanceCustomization
                // but is not yet in the generated objc2-app-kit 0.2 bindings.
                // Use msg_send! directly — the selector name matches Apple's SDK.
                let _: () = msg_send![&*ns_window, setAppearance: &*ns_appearance];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}
