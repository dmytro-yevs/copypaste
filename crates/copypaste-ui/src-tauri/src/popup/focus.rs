//! macOS focus/activation helpers + CGEventTap install.

#[cfg(target_os = "macos")]
use super::state::TapActive;

/// Return the bundle identifier of the currently frontmost application,
/// or `None` if it cannot be determined.
#[cfg(target_os = "macos")]
pub(crate) fn frontmost_bundle_id() -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    // SAFETY: ObjC calls are safe when the types and selectors are correct;
    // objc2-app-kit guarantees these bindings.
    unsafe {
        let ws = NSWorkspace::sharedWorkspace();
        let app = ws.frontmostApplication()?;
        // bundleIdentifier returns Option<Retained<NSString>>; Display impl calls
        // autoreleasepool_leaking internally so we can call .to_string() safely.
        let nsstring = app.bundleIdentifier()?;
        Some(nsstring.to_string())
    }
}

/// Poll until the app identified by `target_bundle_id` is the frontmost
/// application, or until `max_iter` iterations have been exhausted.
///
/// `probe` is called on each iteration and should return the current frontmost
/// bundle ID (or `None` if indeterminate).  Between probes the caller sleeps
/// for `interval`.
///
/// Returns `true` when a probe matches `target_bundle_id`, `false` on timeout.
///
/// # Why poll instead of a fixed sleep?
/// A fixed 80 ms sleep is racy: on a slow or heavily-loaded system the prior
/// app may not have become frontmost yet, so the synthesised Cmd+V fires into
/// the wrong window and the paste is silently dropped.  Polling terminates as
/// soon as the OS confirms the transition, making the happy path faster while
/// the bounded timeout (≈ 300 ms with default params) prevents an indefinite hang
/// on pathological cases where activation never completes.
///
/// This function is extracted so it can be unit-tested without macOS ObjC bindings.
// Only the macOS activation path calls this; on other platforms the sole caller
// is `#[cfg(target_os = "macos")]`, so gate the fn (plus `test` for the
// platform-independent unit test) to avoid a dead_code error under -D warnings.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn poll_until_frontmost(
    target_bundle_id: &str,
    mut probe: impl FnMut() -> Option<String>,
    max_iter: u32,
    interval: std::time::Duration,
) -> bool {
    for _ in 0..max_iter {
        if probe().as_deref() == Some(target_bundle_id) {
            return true;
        }
        std::thread::sleep(interval);
    }
    false
}

/// Activate the app with the given bundle identifier using NSRunningApplication.
#[cfg(target_os = "macos")]
pub(crate) fn activate_app_by_bundle_id(bundle_id: &str) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    use objc2_foundation::NSString;

    // NSApplicationActivateIgnoringOtherApps is deprecated in macOS 14 but
    // remains the correct approach for cross-process activation of a specific
    // app by bundle ID; there is no non-deprecated replacement that provides
    // the same behaviour for background processes.
    #[allow(deprecated)]
    unsafe {
        let bid = NSString::from_str(bundle_id);
        let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&bid);
        if let Some(app) = apps.firstObject() {
            let _ = app.activateWithOptions(
                NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
            );
        }
    }
}

/// Try to install the CGEventTap.  Silently logs and falls back to the
/// plugin-based shortcut if Accessibility is not granted.
#[cfg(target_os = "macos")]
pub(crate) fn try_install_event_tap(handle: &tauri::AppHandle, accel: &str) {
    use tauri::Manager;

    let tap_state = handle.try_state::<TapActive>();
    // Already installed?
    if let Some(ref ts) = tap_state {
        if *ts.0.lock().expect("mutex poisoned") {
            // Just update the accelerator.
            crate::event_tap::update_tap_shortcut(accel);
            return;
        }
    }

    let handle_clone = handle.clone();
    match crate::event_tap::install(accel, move || {
        super::window::toggle_popup(&handle_clone);
    }) {
        Ok(()) => {
            tracing::info!("CGEventTap installed — OS-reserved shortcuts can now be overridden");
            if let Some(ts) = tap_state {
                *ts.0.lock().expect("mutex poisoned") = true;
            }
        }
        Err(e) => {
            tracing::info!("CGEventTap not installed ({e}); using plugin-global-shortcut fallback");
        }
    }
}
