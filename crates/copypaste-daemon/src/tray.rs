//! macOS system tray icon integration using the `tray-icon` crate.
//!
//! Provides a menubar icon with the following menu structure:
//!   CopyPaste          (title, disabled)
//!   ─────────────────
//!   Open History       (shows clipboard history window — stub)
//!   Private Mode  ✓    (toggle with checkmark)
//!   ─────────────────
//!   Launch at Login ✓  (toggles launchd install/uninstall)
//!   Preferences…       (stub)
//!   ─────────────────
//!   Quit
//!
//! This module must be called from the **main thread** on macOS.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

use crate::launchd;

/// Errors raised while constructing the tray menu / icon.
///
/// These are returned instead of panicking so that headless / sandboxed
/// environments (CI, SSH sessions, test runners) can degrade gracefully and
/// continue running the daemon without a tray icon.
#[derive(Debug, thiserror::Error)]
pub enum TrayInitError {
    #[error("tray-icon menu append failed: {0}")]
    MenuAppend(#[from] tray_icon::menu::Error),

    #[error("tray-icon builder failed (no display / headless?): {0}")]
    Build(#[from] tray_icon::Error),

    #[error("tray menu can only be built on the main thread (platform requirement)")]
    NotMainThread,
}

// ── Menu item IDs ────────────────────────────────────────────────────────────

const ID_OPEN_HISTORY: &str = "open_history";
const ID_PRIVATE_MODE: &str = "private_mode";
const ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
const ID_PREFERENCES: &str = "preferences";
const ID_QUIT: &str = "quit";

// ── Shared tray state ────────────────────────────────────────────────────────

/// State shared between the tray event loop and the rest of the daemon.
#[derive(Debug)]
pub struct TrayState {
    /// Whether Private Mode is enabled (no new items are stored).
    pub private_mode: Arc<AtomicBool>,
    /// Whether the user has requested a quit.
    pub quit_requested: Arc<AtomicBool>,
}

impl TrayState {
    pub fn new() -> Self {
        Self {
            private_mode: Arc::new(AtomicBool::new(false)),
            quit_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Icon loading ─────────────────────────────────────────────────────────────

/// Load the tray icon from the bundled PNG bytes.
///
/// The PNG must be 22×22 pixels, RGBA. If the bundled icon cannot be decoded
/// the function falls back to a 22×22 solid grey placeholder so the tray icon
/// still appears. If even the placeholder cannot be constructed (extremely
/// unlikely — would mean `tray-icon` rejected a valid 22×22 RGBA buffer) the
/// function returns `None` and the caller skips the icon entirely.
fn load_icon() -> Option<tray_icon::Icon> {
    // TODO: embed the real icon at build time via include_bytes! once the
    // asset pipeline bundles icons to a known absolute path. For now we look
    // next to the binary and fall back to a grey placeholder.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(dir) = exe_dir {
        // Try several candidate locations
        let candidates = [
            dir.join("icons/tray-icon.png"),
            dir.join("../Resources/icons/tray-icon.png"),
            dir.join("tray-icon.png"),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                if let Ok(bytes) = std::fs::read(candidate) {
                    if let Ok(icon) = tray_icon::Icon::from_rgba(
                        decode_png_rgba(&bytes).unwrap_or_default(),
                        22,
                        22,
                    ) {
                        return Some(icon);
                    }
                }
            }
        }
    }

    // Fallback: 22×22 grey RGBA pixels.
    let grey: Vec<u8> = std::iter::repeat([0x88u8, 0x88, 0x88, 0xffu8])
        .take(22 * 22)
        .flatten()
        .collect();
    match tray_icon::Icon::from_rgba(grey, 22, 22) {
        Ok(icon) => Some(icon),
        Err(e) => {
            tracing::warn!(error = %e, "tray placeholder icon build failed — continuing without icon");
            None
        }
    }
}

/// Minimal PNG RGBA decoder — delegates to the `png` crate if available,
/// otherwise returns `None` so the fallback path is used.
fn decode_png_rgba(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];

    // Convert to RGBA if needed
    match info.color_type {
        png::ColorType::Rgba => Some(raw.to_vec()),
        png::ColorType::Rgb => {
            let rgba: Vec<u8> = raw
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 0xff])
                .collect();
            Some(rgba)
        }
        png::ColorType::Grayscale => {
            let rgba: Vec<u8> = raw
                .iter()
                .flat_map(|&g| [g, g, g, 0xff])
                .collect();
            Some(rgba)
        }
        png::ColorType::GrayscaleAlpha => {
            let rgba: Vec<u8> = raw
                .chunks_exact(2)
                .flat_map(|c| [c[0], c[0], c[0], c[1]])
                .collect();
            Some(rgba)
        }
        _ => None,
    }
}

// ── Menu builder ─────────────────────────────────────────────────────────────

/// Build the tray menu. Returns the menu and the individual item handles so
/// we can read/update their state on menu events.
struct TrayMenu {
    menu: Menu,
    open_history: MenuItem,
    private_mode: MenuItem,
    launch_at_login: MenuItem,
    preferences: MenuItem,
    quit: MenuItem,
}

impl TrayMenu {
    /// Build the tray menu, returning [`TrayInitError`] on any `menu.append`
    /// failure instead of panicking. Callers can degrade gracefully (skip the
    /// tray) when the platform refuses to register a menu (headless / no
    /// display server / unsupported platform shim).
    ///
    /// On macOS `muda::Menu::new()` *asserts* it is called from the main
    /// thread and aborts via `panic!` otherwise. We catch that panic so the
    /// caller receives a recoverable `Err` and the daemon can continue
    /// without a tray (e.g. on test runners or background helper threads).
    pub fn build(
        private_mode_on: bool,
        launch_at_login_on: bool,
    ) -> Result<Self, TrayInitError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build_inner(private_mode_on, launch_at_login_on)
        }))
        .map_err(|_| TrayInitError::NotMainThread)
        .and_then(|inner| inner)
    }

    fn build_inner(
        private_mode_on: bool,
        launch_at_login_on: bool,
    ) -> Result<Self, TrayInitError> {
        let open_history = MenuItem::with_id(ID_OPEN_HISTORY, "Open History", true, None);
        let private_mode = MenuItem::with_id(
            ID_PRIVATE_MODE,
            if private_mode_on {
                "Private Mode  ✓"
            } else {
                "Private Mode"
            },
            true,
            None,
        );
        let launch_at_login = MenuItem::with_id(
            ID_LAUNCH_AT_LOGIN,
            if launch_at_login_on {
                "Launch at Login  ✓"
            } else {
                "Launch at Login"
            },
            true,
            None,
        );
        let preferences = MenuItem::with_id(ID_PREFERENCES, "Preferences…", true, None);
        let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

        let menu = Menu::new();
        // Title row (disabled)
        let title = MenuItem::with_id("title", "CopyPaste", false, None);
        menu.append(&title)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&open_history)?;
        menu.append(&private_mode)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&launch_at_login)?;
        menu.append(&preferences)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit)?;

        Ok(Self {
            menu,
            open_history,
            private_mode,
            launch_at_login,
            preferences,
            quit,
        })
    }
}

/// Public, test-friendly wrapper around [`TrayMenu::build`] that constructs
/// just the menu without attaching it to a tray icon. Used by `init_safety`
/// tests to assert that a build failure surfaces as `Err`, not a panic.
pub fn build_tray_menu(
    private_mode_on: bool,
    launch_at_login_on: bool,
) -> Result<(), TrayInitError> {
    TrayMenu::build(private_mode_on, launch_at_login_on).map(|_| ())
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Run the tray icon event loop on the **current (main) thread**.
///
/// This function blocks until the user clicks Quit or `state.quit_requested`
/// is set to `true` from another thread.
///
/// The caller is responsible for running the async daemon logic on a separate
/// thread before calling this function.
pub fn run_tray(state: Arc<TrayState>) {
    let private_mode_on = state.private_mode.load(Ordering::Relaxed);
    let launch_at_login_on = launchd::is_installed();

    let tray_menu = match TrayMenu::build(private_mode_on, launch_at_login_on) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "tray menu build failed — running without tray UI");
            // Park the thread until quit is requested; daemon continues headless.
            while !state.quit_requested.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            return;
        }
    };

    let icon = load_icon();

    // Build tray icon — keep _tray alive for the duration of the event loop.
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu.menu))
        .with_tooltip("CopyPaste");
    if let Some(icon) = icon {
        builder = builder.with_icon(icon);
    }
    let _tray: TrayIcon = match builder.build() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "tray icon build failed (headless?) — running without tray UI");
            while !state.quit_requested.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            return;
        }
    };

    let menu_channel = MenuEvent::receiver();

    // Track mutable state locally (private_mode, launch_at_login)
    let mut private_mode = private_mode_on;
    let mut launch_at_login = launch_at_login_on;

    tracing::info!("tray icon started");

    // Spin the NSRunLoop / CFRunLoop while processing menu events.
    // We check for quit on every iteration.
    loop {
        if state.quit_requested.load(Ordering::Relaxed) {
            break;
        }

        // Drain all pending menu events
        while let Ok(event) = menu_channel.try_recv() {
            match event.id.0.as_str() {
                ID_OPEN_HISTORY => {
                    tracing::info!("Open History triggered (stub)");
                    // TODO: show Slint history window
                }
                ID_PRIVATE_MODE => {
                    private_mode = !private_mode;
                    state.private_mode.store(private_mode, Ordering::Relaxed);
                    tray_menu.private_mode.set_text(if private_mode {
                        "Private Mode  ✓"
                    } else {
                        "Private Mode"
                    });
                    tracing::info!("private_mode={private_mode}");
                }
                ID_LAUNCH_AT_LOGIN => {
                    launch_at_login = !launch_at_login;
                    if launch_at_login {
                        match launchd::install() {
                            Ok(()) => tracing::info!("launchd: installed"),
                            Err(e) => {
                                tracing::error!("launchd install failed: {e}");
                                launch_at_login = false; // revert on failure
                            }
                        }
                    } else {
                        match launchd::uninstall() {
                            Ok(()) => tracing::info!("launchd: uninstalled"),
                            Err(e) => {
                                tracing::error!("launchd uninstall failed: {e}");
                                launch_at_login = true; // revert on failure
                            }
                        }
                    }
                    tray_menu.launch_at_login.set_text(if launch_at_login {
                        "Launch at Login  ✓"
                    } else {
                        "Launch at Login"
                    });
                }
                ID_PREFERENCES => {
                    tracing::info!("Preferences triggered (stub)");
                    // TODO: open preferences window
                }
                ID_QUIT => {
                    tracing::info!("Quit triggered from tray");
                    state.quit_requested.store(true, Ordering::Relaxed);
                    break;
                }
                other => {
                    tracing::debug!("unknown menu event id: {other}");
                }
            }
        }

        // Yield briefly to avoid busy-spin; on macOS the CFRunLoop
        // is still active via tray-icon's platform layer.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    tracing::info!("tray icon stopped");
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_state_defaults() {
        let state = TrayState::new();
        assert!(!state.private_mode.load(Ordering::Relaxed));
        assert!(!state.quit_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn tray_state_private_mode_toggle() {
        let state = TrayState::new();
        assert!(!state.private_mode.load(Ordering::Relaxed));

        state.private_mode.store(true, Ordering::Relaxed);
        assert!(state.private_mode.load(Ordering::Relaxed));

        state.private_mode.store(false, Ordering::Relaxed);
        assert!(!state.private_mode.load(Ordering::Relaxed));
    }

    #[test]
    fn tray_state_quit_flag() {
        let state = TrayState::new();
        assert!(!state.quit_requested.load(Ordering::Relaxed));
        state.quit_requested.store(true, Ordering::Relaxed);
        assert!(state.quit_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn decode_png_rgba_returns_none_for_garbage() {
        let result = decode_png_rgba(b"not a png");
        assert!(result.is_none());
    }

    /// Regression test for Wave 2.6 best-prac HIGH #3 / #4.
    ///
    /// `TrayMenu::build` previously used 7× `.unwrap()` on `menu.append(...)`
    /// which would abort the daemon if the platform menu shim refused an
    /// item (headless CI, unsupported platform, sandboxed). After the fix
    /// the function must return `Result<_, TrayInitError>` and the public
    /// `build_tray_menu` helper must never panic regardless of environment.
    #[test]
    fn tray_init_failure_logs_does_not_panic() {
        // Wrap in catch_unwind: even if the underlying platform_impl path
        // happens to succeed under test, the assertion we care about is the
        // absence of panic.
        let outcome =
            std::panic::catch_unwind(|| build_tray_menu(false, false));

        let result = match outcome {
            Ok(r) => r,
            Err(_) => panic!("build_tray_menu must not panic — it must return Result"),
        };

        // Either Ok or Err is acceptable on macOS test runners; we only
        // require that the function does not panic and returns the
        // documented Result type. On macOS test threads we expect
        // `NotMainThread` because `muda::Menu::new()` requires main-thread
        // affinity and cargo test spawns worker threads.
        match result {
            Ok(()) => {}
            Err(TrayInitError::MenuAppend(_))
            | Err(TrayInitError::Build(_))
            | Err(TrayInitError::NotMainThread) => {}
        }
    }
}
