//! Release-evidence accessibility bootstrap for macOS CI.
//!
//! WKWebView only materializes its AX tree when an assistive client is detected.
//! System Events on the Anka runners otherwise sees an empty `AXWebArea`, so
//! cloud UI evidence cannot find Settings or Sync. Opt in with
//! `COPYPASTE_EVIDENCE_AX=1` from the evidence launcher only.

#![allow(unsafe_code)]

use tauri::{AppHandle, Runtime};

/// Loosen screen-capture protection and ask AppKit to publish the full AX tree.
pub fn bootstrap<R: Runtime>(app: &AppHandle<R>) {
    if std::env::var_os("COPYPASTE_EVIDENCE_AX").is_none() {
        return;
    }
    super::protection::apply(app, true);
    enable_enhanced_user_interface();
}

fn enable_enhanced_user_interface() {
    use dispatch2::DispatchQueue;
    use objc2::msg_send;
    use objc2::runtime::Bool;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let run = || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // VoiceOver sets this; without it WKWebView keeps an empty AXWebArea.
        let app = NSApplication::sharedApplication(mtm);
        let _: () = unsafe { msg_send![&*app, setAccessibilityEnhancedUserInterface: Bool::YES] };
    };

    if MainThreadMarker::new().is_some() {
        run();
        return;
    }
    DispatchQueue::main().exec_sync(run);
}
