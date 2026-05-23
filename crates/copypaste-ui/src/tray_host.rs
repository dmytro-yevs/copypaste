//! macOS menu-bar tray host for the Slint UI process (v0.3 only).
//!
//! Historically the tray lived in `copypaste-daemon` (`src/tray.rs`), but
//! daemons started by launchd cannot reliably bring up an `NSApplication`
//! main run loop — `tray-icon` / `muda::Menu` both *require* main-thread
//! affinity inside an active `NSApp`. The Slint UI process already runs an
//! `NSApp` event loop, so this is the correct host on macOS.
//!
//! The tray host is **drained from the Slint event loop** via
//! [`slint::Timer`] so menu events are polled on the main thread without
//! blocking it. This avoids spinning a second native run loop and keeps
//! the `MenuEvent` channel polling cheap (≤ 50 ms tick).
//!
//! Menu structure (matches the legacy daemon-side tray):
//!   CopyPaste            (title, disabled)
//!   ─────────────────
//!   Open History         → callback into the Slint UI
//!   Private Mode  ✓      → toggles local state (TODO: IPC to daemon)
//!   ─────────────────
//!   Launch at Login ✓    → shells out to `copypaste daemon install|uninstall`
//!   Preferences…         → callback into the Slint UI (settings window)
//!   ─────────────────
//!   Quit                 → `slint::quit_event_loop`
//!
//! Errors are degraded — if the platform refuses to register a tray (CI,
//! headless run) we log a warning and return without panicking so the UI
//! still functions as a window-only app.
//
// Gating: the module is declared as `#[cfg(target_os = "macos")] pub mod
// tray_host;` in `lib.rs`, so we don't need an inner `#![cfg(...)]` here.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

/// launchd label used by the daemon plist. Kept in sync with
/// `copypaste-daemon::launchd::LABEL`; UI only reads the file existence to
/// decide whether to render the "Launch at Login ✓" checkmark.
const LAUNCHD_LABEL: &str = "com.copypaste.daemon";

// ── Menu item IDs ────────────────────────────────────────────────────────────

const ID_OPEN_HISTORY: &str = "open_history";
const ID_PRIVATE_MODE: &str = "private_mode";
const ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
const ID_PREFERENCES: &str = "preferences";
const ID_QUIT: &str = "quit";

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TrayHostError {
    #[error("tray-icon menu append failed: {0}")]
    MenuAppend(#[from] tray_icon::menu::Error),

    #[error("tray-icon builder failed (no display / headless?): {0}")]
    Build(#[from] tray_icon::Error),

    #[error("tray menu must be built on the main thread")]
    NotMainThread,
}

// ── Callbacks the host exposes to the rest of the UI ───────────────────────

/// User actions surfaced by tray menu items. The Slint UI registers handlers
/// for the ones it cares about; unhandled actions log + no-op.
pub type ActionCb = Box<dyn Fn() + 'static>;

#[derive(Default)]
pub struct TrayCallbacks {
    pub on_open_history: Option<ActionCb>,
    pub on_open_preferences: Option<ActionCb>,
    pub on_quit: Option<ActionCb>,
}

// ── launchd LaunchAgent helpers ────────────────────────────────────────────
//
// Self-contained so the UI crate stays decoupled from `copypaste-daemon`.
// We only need (a) detect whether the plist exists and (b) shell out to the
// CLI to install / uninstall it. The CLI does the real work; the UI tray is
// just a control surface.

fn launch_agents_dir() -> Option<PathBuf> {
    home::home_dir().map(|h| h.join("Library/LaunchAgents"))
}

fn launchd_plist_path() -> Option<PathBuf> {
    launch_agents_dir().map(|d| d.join(format!("{LAUNCHD_LABEL}.plist")))
}

fn launchd_is_installed() -> bool {
    launchd_plist_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Locate the `copypaste` CLI binary next to the running UI binary. Falls
/// back to a bare command name so $PATH lookup is attempted as a last resort.
fn copypaste_cli_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("copypaste")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("copypaste"))
}

fn launchd_install() -> std::io::Result<()> {
    let cli = copypaste_cli_path();
    let status = std::process::Command::new(&cli)
        .args(["daemon", "install"])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "`{} daemon install` exited with status {status}",
            cli.display()
        )));
    }
    Ok(())
}

fn launchd_uninstall() -> std::io::Result<()> {
    let cli = copypaste_cli_path();
    let status = std::process::Command::new(&cli)
        .args(["daemon", "uninstall"])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "`{} daemon uninstall` exited with status {status}",
            cli.display()
        )));
    }
    Ok(())
}

// ── Icon loading ───────────────────────────────────────────────────────────

/// Probe a handful of locations next to the current executable for a tray
/// icon PNG. In the macOS .app bundle the icons live under
/// `Contents/Resources/icons/` (sibling of `Contents/MacOS/copypaste-ui`).
fn load_icon() -> Option<tray_icon::Icon> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))?;

    let candidates = [
        exe_dir.join("icons/tray-icon.png"),
        exe_dir.join("../Resources/icons/tray-icon.png"),
        exe_dir.join("../Resources/icons/tray-icon-idle.png"),
        exe_dir.join("tray-icon.png"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(bytes) = std::fs::read(candidate) {
                if let Some(rgba) = decode_png_rgba(&bytes) {
                    if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, 22, 22) {
                        return Some(icon);
                    }
                }
            }
        }
    }

    // Grey 22×22 placeholder fallback so the tray slot is still visible.
    let grey: Vec<u8> = std::iter::repeat_n([0x88u8, 0x88, 0x88, 0xff], 22 * 22)
        .flatten()
        .collect();
    tray_icon::Icon::from_rgba(grey, 22, 22).ok()
}

fn decode_png_rgba(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => Some(raw.to_vec()),
        png::ColorType::Rgb => Some(
            raw.chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 0xff])
                .collect(),
        ),
        png::ColorType::Grayscale => {
            Some(raw.iter().flat_map(|&g| [g, g, g, 0xff]).collect())
        }
        png::ColorType::GrayscaleAlpha => Some(
            raw.chunks_exact(2)
                .flat_map(|c| [c[0], c[0], c[0], c[1]])
                .collect(),
        ),
        _ => None,
    }
}

// ── Menu builder ────────────────────────────────────────────────────────────

struct TrayMenu {
    menu: Menu,
    private_mode: MenuItem,
    launch_at_login: MenuItem,
}

impl TrayMenu {
    fn build(
        private_mode_on: bool,
        launch_at_login_on: bool,
    ) -> Result<Self, TrayHostError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build_inner(private_mode_on, launch_at_login_on)
        }))
        .map_err(|_| TrayHostError::NotMainThread)
        .and_then(|inner| inner)
    }

    fn build_inner(
        private_mode_on: bool,
        launch_at_login_on: bool,
    ) -> Result<Self, TrayHostError> {
        let title = MenuItem::with_id("title", "CopyPaste", false, None);
        let open_history = MenuItem::with_id(ID_OPEN_HISTORY, "Open History", true, None);
        let private_mode = MenuItem::with_id(
            ID_PRIVATE_MODE,
            if private_mode_on { "Private Mode  ✓" } else { "Private Mode" },
            true,
            None,
        );
        let launch_at_login = MenuItem::with_id(
            ID_LAUNCH_AT_LOGIN,
            if launch_at_login_on { "Launch at Login  ✓" } else { "Launch at Login" },
            true,
            None,
        );
        let preferences = MenuItem::with_id(ID_PREFERENCES, "Preferences…", true, None);
        let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

        let menu = Menu::new();
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
            private_mode,
            launch_at_login,
        })
    }
}

// ── Public entry point ──────────────────────────────────────────────────────

/// Mutable state held on the main thread between event-loop ticks.
struct TrayRuntime {
    _tray: TrayIcon,
    menu: TrayMenu,
    private_mode: bool,
    launch_at_login: bool,
    callbacks: TrayCallbacks,
    /// Set when the user picks Quit. Latched so subsequent ticks can stop
    /// polling and forward the quit to Slint via the callback (if any).
    quit_latch: Arc<AtomicBool>,
}

/// Build the tray icon, register a Slint timer that drains tray menu events
/// from the main thread on every tick, and return `Ok(())`.
///
/// Must be called *before* `slint::run_event_loop` (i.e. before
/// `Window::run`). The returned `Result` allows callers to log + degrade
/// gracefully when the platform refuses the tray (CI, no display).
pub fn install(callbacks: TrayCallbacks) -> Result<(), TrayHostError> {
    let launch_at_login = launchd_is_installed();
    let menu = TrayMenu::build(false, launch_at_login)?;

    let icon = load_icon();
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu.menu.clone()))
        .with_tooltip("CopyPaste");
    if let Some(icon) = icon {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build()?;

    let runtime = Rc::new(RefCell::new(TrayRuntime {
        _tray: tray,
        menu,
        private_mode: false,
        launch_at_login,
        callbacks,
        quit_latch: Arc::new(AtomicBool::new(false)),
    }));

    // Slint timer ticks on the UI thread; safe to read menu events here.
    let timer = slint::Timer::default();
    let runtime_clone = runtime.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            drain_events(&runtime_clone);
        },
    );

    // Leak the timer into the Rc so it lives as long as the tray. Without
    // this the timer is dropped at end of scope and tray events silently
    // stop firing.
    std::mem::forget(timer);

    tracing::info!("ui tray host installed");
    Ok(())
}

fn drain_events(runtime: &Rc<RefCell<TrayRuntime>>) {
    let menu_channel = MenuEvent::receiver();
    while let Ok(event) = menu_channel.try_recv() {
        let mut rt = runtime.borrow_mut();
        match event.id.0.as_str() {
            ID_OPEN_HISTORY => {
                if let Some(cb) = &rt.callbacks.on_open_history {
                    cb();
                }
            }
            ID_PRIVATE_MODE => {
                // TODO(v0.3.x): plumb Private Mode through an IPC method on
                // the daemon. For now the UI just toggles the checkmark so
                // the menu state is testable.
                rt.private_mode = !rt.private_mode;
                let label = if rt.private_mode { "Private Mode  ✓" } else { "Private Mode" };
                rt.menu.private_mode.set_text(label);
                tracing::info!("tray: private_mode={}", rt.private_mode);
            }
            ID_LAUNCH_AT_LOGIN => {
                let want = !rt.launch_at_login;
                let result = if want { launchd_install() } else { launchd_uninstall() };
                match result {
                    Ok(()) => {
                        rt.launch_at_login = want;
                        let label = if want { "Launch at Login  ✓" } else { "Launch at Login" };
                        rt.menu.launch_at_login.set_text(label);
                        tracing::info!("tray: launch_at_login={}", want);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, want, "tray: launchd toggle failed");
                    }
                }
            }
            ID_PREFERENCES => {
                if let Some(cb) = &rt.callbacks.on_open_preferences {
                    cb();
                }
            }
            ID_QUIT => {
                rt.quit_latch.store(true, Ordering::Relaxed);
                if let Some(cb) = &rt.callbacks.on_quit {
                    cb();
                } else {
                    // Default: cleanly shut down the Slint loop so `window.run()` returns.
                    let _ = slint::quit_event_loop();
                }
            }
            other => {
                tracing::debug!(id = other, "tray: unknown menu event id");
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_png_rgba_returns_none_for_garbage() {
        assert!(decode_png_rgba(b"not a png").is_none());
    }

    #[test]
    fn launchd_plist_path_is_in_launch_agents() {
        if let Some(p) = launchd_plist_path() {
            let s = p.to_string_lossy();
            assert!(s.contains("Library/LaunchAgents"));
            assert!(s.ends_with("com.copypaste.daemon.plist"));
        }
    }

    #[test]
    fn tray_menu_build_off_main_thread_returns_err() {
        // muda::Menu::new() asserts main-thread on macOS; cargo test spawns
        // worker threads so this must surface as Err, never a panic.
        let outcome = std::panic::catch_unwind(|| TrayMenu::build(false, false));
        let result = outcome.expect("build must not panic");
        assert!(result.is_err(), "expected NotMainThread on test thread");
    }
}
