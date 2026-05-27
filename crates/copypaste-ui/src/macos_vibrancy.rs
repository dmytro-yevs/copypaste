//! macOS NSVisualEffectView vibrancy shim (T3.5).
//!
//! Applies `NSVisualEffectMaterial::Sidebar` blending to the backing
//! `NSWindow` of a Slint component so the sidebar/panel region renders with
//! the platform-native translucent material instead of a flat colour.
//!
//! # Activation
//!
//! Call [`apply_to_window`] once, after the Slint component is constructed
//! and *before* `slint::run_event_loop()`.  The call is a no-op on non-macOS
//! targets and degrades gracefully (log + continue) if the AppKit call fails
//! at runtime, because vibrancy is purely cosmetic.
//!
//! # Feature gate
//!
//! The full implementation requires `objc2-app-kit` feature `NSVisualEffectView`
//! (plus `NSView`, `NSWindow`, `NSViewGeometry`).  Add them to
//! `crates/copypaste-ui/Cargo.toml` under
//! `[target.'cfg(target_os = "macos")'.dependencies]` to unlock the real path:
//!
//! ```toml
//! objc2-app-kit = { version = "0.3", default-features = false, features = [
//!     "NSApplication", "NSResponder", "NSRunningApplication",
//!     "NSVisualEffectView", "NSView", "NSWindow",
//! ] }
//! ```
//!
//! Until those features are enabled the function is a documented stub.

#![cfg(target_os = "macos")]

use slint::ComponentHandle;

/// Apply `NSVisualEffectMaterial::Sidebar` vibrancy to the window that backs
/// `window`.
///
/// Safe to call from the main thread before `slint::run_event_loop`.
/// On failure (headless CI, missing entitlement, …) the error is logged at
/// `warn` level and the function returns normally — the app continues without
/// vibrancy.
///
/// # Type parameter
///
/// `T` is any Slint-generated component that implements [`ComponentHandle`]
/// (e.g. `HistoryWindow`, `SettingsWindow`).  Pass `window.as_weak()` or a
/// fresh `Weak` clone obtained from the component before passing ownership to
/// the event loop.
pub fn apply_to_window<T: ComponentHandle>(_window: &slint::Weak<T>) {
    // ── Full implementation (requires NSVisualEffectView feature) ────────────
    //
    // When the feature is available the real path looks like:
    //
    // ```rust
    // use objc2::rc::Retained;
    // use objc2_app_kit::{
    //     NSApplication, NSView, NSVisualEffectBlendingMode,
    //     NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    //     NSWindow,
    // };
    // use objc2_foundation::NSRect;
    //
    // let Some(window) = _window.upgrade() else {
    //     tracing::warn!("macos_vibrancy: window already dropped, skipping");
    //     return;
    // };
    //
    // // Safety: Slint runs on the main thread; AppKit requires main-thread access.
    // unsafe {
    //     let app = NSApplication::sharedApplication();
    //     let Some(ns_window) = app.mainWindow() else {
    //         tracing::warn!("macos_vibrancy: no mainWindow yet, vibrancy skipped");
    //         return;
    //     };
    //     let content_view: Retained<NSView> = ns_window.contentView()
    //         .expect("NSWindow contentView must exist");
    //
    //     let effect_view = NSVisualEffectView::new();
    //     let frame = content_view.frame();
    //     effect_view.setFrame(frame);
    //     effect_view.setMaterial(NSVisualEffectMaterial::Sidebar);
    //     effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    //     effect_view.setState(NSVisualEffectState::FollowsWindowActiveState);
    //     effect_view.setAutoresizingMask(
    //         objc2_app_kit::NSAutoresizingMaskOptions::NSViewWidthSizable
    //         | objc2_app_kit::NSAutoresizingMaskOptions::NSViewHeightSizable,
    //     );
    //
    //     // Insert the effect view as the bottom-most subview of the content view
    //     // so Slint's own layer renders on top.
    //     content_view.addSubview_positioned_relativeTo(
    //         &effect_view,
    //         objc2_app_kit::NSWindowOrderingMode::NSWindowBelow,
    //         None,
    //     );
    //     tracing::debug!("macos_vibrancy: NSVisualEffectView (Sidebar) applied");
    // }
    // ```
    //
    // ── Stub path ────────────────────────────────────────────────────────────
    //
    // TODO(T3.5): enable `NSVisualEffectView` (+ `NSView`, `NSWindow`) features
    // in crates/copypaste-ui/Cargo.toml and replace this stub with the block
    // above.
    tracing::debug!(
        "macos_vibrancy: stub — add NSVisualEffectView feature to Cargo.toml to enable"
    );
}
