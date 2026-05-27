// root_window.rs — RootWindowHandle: Rust side of the v0.4 MainWindow shell.
//
// T2.2: wraps the Slint-generated `MainWindow` type (exported from
// `ui/MainWindow.slint` via `ui/appui.slint`) and applies saved `UiPrefs`
// to the window's initial property state before first paint.
//
// Downstream tasks that will build on this handle:
//   T3.x  — wire sidebar tab callbacks, content-area views.
//   T3.5  — macOS vibrancy (`macos_vibrancy::apply_to_root_window`).
//   T4.x  — retire legacy HistoryWindow in favour of this shell.
//
// The handle intentionally owns the `MainWindow` value (not a `Weak`) so
// the window stays alive as long as the handle is in scope inside `main()`.

use crate::MainWindow;
use copypaste_ui::ui_prefs::UiPrefs;
use slint::ComponentHandle;

/// Rust-side handle for the v0.4 `MainWindow` shell.
///
/// Construction applies the persisted [`UiPrefs`] to window properties so
/// the sidebar state and accent colour are correct on the very first frame.
pub struct RootWindowHandle {
    window: MainWindow,
}

#[allow(dead_code)]
impl RootWindowHandle {
    /// Create the window and apply saved preferences.
    ///
    /// `_socket_path` is reserved for future IPC wiring (T3.x daemon-status
    /// dot); it is unused here so the argument stays in the public API without
    /// requiring a downstream refactor.
    pub fn new(prefs: &UiPrefs, _socket_path: &str) -> anyhow::Result<Self> {
        let window = MainWindow::new()?;

        // Apply persisted prefs to the initial property state.
        // sidebar-collapsed: driven by UiPrefs::sidebar_collapsed
        window.set_sidebar_collapsed(prefs.sidebar_collapsed);

        // app-version: injected from CARGO_PKG_VERSION at the call site.
        // (set separately via `set_app_version` after construction)

        // daemon-connected defaults to false; T3.x will wire a status probe.

        Ok(Self { window })
    }

    /// Set the app-version string shown in the sidebar footer.
    pub fn set_app_version(&self, version: &str) {
        self.window.set_app_version(version.into());
    }

    /// Make the window visible.
    ///
    /// Callers are responsible for calling `set_activation_policy_regular()`
    /// and `activate_app()` on macOS before or after this call, matching the
    /// pattern used for the legacy history window.
    pub fn show(&self) {
        let _ = self.window.show();
    }

    /// Hide the window without destroying it.
    pub fn hide(&self) {
        let _ = self.window.hide();
    }

    /// Borrow a weak reference for use in closures.
    pub fn as_weak(&self) -> slint::Weak<MainWindow> {
        self.window.as_weak()
    }
}
