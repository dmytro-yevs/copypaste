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
use std::sync::mpsc::{self, Receiver, Sender};
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
    launchd_plist_path().map(|p| p.exists()).unwrap_or(false)
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
                if let Some((rgba, w, h)) = decode_png_rgba(&bytes) {
                    if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, w, h) {
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

/// Decode a PNG byte slice into `(rgba_bytes, width, height)`.
///
/// Returning the actual image dimensions (instead of assuming 22×22) lets the
/// caller pass them straight into `tray_icon::Icon::from_rgba`, which
/// validates `data.len() == width * height * 4`. The bundled tray PNGs are
/// 32×32; hardcoding 22×22 here caused `from_rgba` to return
/// `Err(BadImageBufferSize)` and the tray fell through to the grey placeholder.
fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    use std::io::Cursor;
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => raw.to_vec(),
        png::ColorType::Rgb => raw
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 0xff])
            .collect(),
        png::ColorType::Grayscale => raw.iter().flat_map(|&g| [g, g, g, 0xff]).collect(),
        png::ColorType::GrayscaleAlpha => raw
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        _ => return None,
    };
    Some((rgba, info.width, info.height))
}

// ── Menu builder ────────────────────────────────────────────────────────────

struct TrayMenu {
    menu: Menu,
    private_mode: MenuItem,
    launch_at_login: MenuItem,
}

impl TrayMenu {
    fn build(private_mode_on: bool, launch_at_login_on: bool) -> Result<Self, TrayHostError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build_inner(private_mode_on, launch_at_login_on)
        }))
        .map_err(|_| TrayHostError::NotMainThread)
        .and_then(|inner| inner)
    }

    fn build_inner(private_mode_on: bool, launch_at_login_on: bool) -> Result<Self, TrayHostError> {
        let title = MenuItem::with_id("title", "CopyPaste", false, None);
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

/// Outcome of an off-thread `launchctl bootstrap`/`bootout` job, posted
/// back to the UI thread via [`TrayRuntime::launchd_rx`] and applied by
/// [`drain_events`] on the next tick.
struct LaunchdResult {
    /// What the user requested: `true` = install, `false` = uninstall.
    want: bool,
    /// `Ok(())` on success, otherwise the error rendered as a String — owned
    /// `String` is `Send` without ceremony, unlike `std::io::Error`.
    outcome: Result<(), String>,
}

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
    /// `true` while a `launchctl bootstrap`/`bootout` job is running on a
    /// background thread. Debounces repeated clicks on "Launch at Login" —
    /// `launchctl` can take seconds and a second concurrent invocation
    /// would race with the first.
    launchd_in_flight: bool,
    /// Sender half handed to each worker thread doing the blocking
    /// `Command::status()`. Cloned on dispatch; kept here so it stays
    /// alive for the lifetime of the tray runtime.
    launchd_tx: Sender<LaunchdResult>,
    /// Receiver drained by [`drain_events`] each tick — applies the
    /// result and re-enables the menu item.
    launchd_rx: Receiver<LaunchdResult>,
    /// Slint timer driving [`drain_events`]. Stored here (instead of
    /// `mem::forget`-ed) so its `Drop` runs when the runtime itself is
    /// dropped, stopping the timer cleanly. Without ownership the timer
    /// would be reclaimed at end of `install()` and tray events would
    /// silently stop firing — keeping it inside the runtime preserves
    /// that lifetime contract without leaking on every `install()` call.
    _drain_timer: Option<slint::Timer>,
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

    let (launchd_tx, launchd_rx) = mpsc::channel::<LaunchdResult>();

    let runtime = Rc::new(RefCell::new(TrayRuntime {
        _tray: tray,
        menu,
        private_mode: false,
        launch_at_login,
        callbacks,
        quit_latch: Arc::new(AtomicBool::new(false)),
        launchd_in_flight: false,
        launchd_tx,
        launchd_rx,
        _drain_timer: None,
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

    // Park the timer inside the runtime so it lives as long as the tray
    // and is dropped (stopped) cleanly when the runtime is dropped.
    // Replaces the previous `mem::forget(timer)`, which leaked one Slint
    // timer per `install()` call.
    runtime.borrow_mut()._drain_timer = Some(timer);

    tracing::info!("ui tray host installed");
    Ok(())
}

fn drain_events(runtime: &Rc<RefCell<TrayRuntime>>) {
    // Apply any pending results from off-thread launchd jobs *first*. Done
    // in its own scope so no `RefMut<TrayRuntime>` is held while we
    // subsequently dispatch user callbacks.
    apply_pending_launchd_results(runtime);

    let menu_channel = MenuEvent::receiver();
    while let Ok(event) = menu_channel.try_recv() {
        // For every event we first mutate any pure runtime state inside a
        // short-lived `RefMut`, then drop the borrow *before* invoking any
        // user callback. Holding a `RefMut<TrayRuntime>` across user code
        // panics `already borrowed` the moment that callback re-enters the
        // runtime synchronously (e.g. an `on_quit` handler that touches
        // another tray method).
        //
        // `ActionCb` is `Box<dyn Fn>` and not cloneable, so we use
        // `Option::take` to move the callback OUT of the runtime, invoke
        // it with no borrow held, then put it back. The window where the
        // slot is `None` is bounded to one tick and re-entrancy simply
        // sees `None` and no-ops — the correct behaviour.
        match event.id.0.as_str() {
            ID_OPEN_HISTORY => invoke_callback(runtime, |c| &mut c.on_open_history),
            ID_PRIVATE_MODE => {
                // TODO(v0.3.x): plumb Private Mode through an IPC method on
                // the daemon. For now the UI just toggles the checkmark so
                // the menu state is testable. No user callback — borrow is
                // confined to this arm.
                let mut rt = runtime.borrow_mut();
                rt.private_mode = !rt.private_mode;
                let label = if rt.private_mode {
                    "Private Mode  ✓"
                } else {
                    "Private Mode"
                };
                rt.menu.private_mode.set_text(label);
                tracing::info!("tray: private_mode={}", rt.private_mode);
            }
            ID_LAUNCH_AT_LOGIN => {
                // `launchctl bootstrap`/`bootout` can take several seconds
                // on a cold launchd. Running it inline froze the Slint
                // tick (and therefore the whole UI) mid-frame. Spawn a
                // worker thread and update the menu when it reports back
                // via the mpsc channel drained by
                // `apply_pending_launchd_results` on the next tick.
                let want = {
                    let mut rt = runtime.borrow_mut();
                    if rt.launchd_in_flight {
                        // Debounce: a previous toggle is still running.
                        // The menu item is also disabled below, so this
                        // path only fires if the user races a click
                        // through before the disable lands.
                        tracing::debug!("tray: launchd toggle ignored (in flight)");
                        continue;
                    }
                    let want = !rt.launch_at_login;
                    rt.launchd_in_flight = true;
                    // Disable the menu item while the worker is running so
                    // the user can't queue duplicate launchctl invocations.
                    rt.menu.launch_at_login.set_enabled(false);
                    want
                };
                spawn_launchd_toggle(runtime, want);
            }
            ID_PREFERENCES => invoke_callback(runtime, |c| &mut c.on_open_preferences),
            ID_QUIT => {
                runtime.borrow().quit_latch.store(true, Ordering::Relaxed);
                let cb = runtime.borrow_mut().callbacks.on_quit.take();
                match cb {
                    Some(cb) => {
                        cb();
                        // Restore so a subsequent quit event still sees the
                        // handler. If the callback already reassigned it
                        // (unlikely), don't clobber.
                        let mut rt = runtime.borrow_mut();
                        if rt.callbacks.on_quit.is_none() {
                            rt.callbacks.on_quit = Some(cb);
                        }
                    }
                    None => {
                        // Default: cleanly shut down the Slint loop so
                        // `window.run()` returns.
                        let _ = slint::quit_event_loop();
                    }
                }
            }
            other => {
                tracing::debug!(id = other, "tray: unknown menu event id");
            }
        }
    }
}

/// Temporarily move a callback OUT of `TrayRuntime`, invoke it with no
/// borrow held, then restore it. The `selector` returns a `&mut
/// Option<ActionCb>` pointing at the desired field. This is the only
/// re-entrancy-safe way to call a `Box<dyn Fn>` stored inside a `RefCell`
/// without panicking `already borrowed` when the callback synchronously
/// touches the same runtime.
fn invoke_callback<F>(runtime: &Rc<RefCell<TrayRuntime>>, selector: F)
where
    F: Fn(&mut TrayCallbacks) -> &mut Option<ActionCb>,
{
    let cb = {
        let mut rt = runtime.borrow_mut();
        selector(&mut rt.callbacks).take()
    };
    let Some(cb) = cb else { return };
    cb();
    let mut rt = runtime.borrow_mut();
    let slot = selector(&mut rt.callbacks);
    if slot.is_none() {
        *slot = Some(cb);
    }
}

/// Spawn a worker thread that runs the blocking `copypaste daemon
/// install|uninstall` CLI. The result is posted to `launchd_tx` for the
/// next [`drain_events`] tick to apply on the UI thread. Replaces the
/// previous inline `Command::status()` call, which blocked the Slint
/// event loop for the duration of `launchctl bootstrap` (seconds).
fn spawn_launchd_toggle(runtime: &Rc<RefCell<TrayRuntime>>, want: bool) {
    let tx = runtime.borrow().launchd_tx.clone();
    std::thread::spawn(move || {
        let outcome = if want {
            launchd_install()
        } else {
            launchd_uninstall()
        };
        let payload = LaunchdResult {
            want,
            outcome: outcome.map_err(|e| e.to_string()),
        };
        if let Err(e) = tx.send(payload) {
            // Receiver was dropped — runtime is gone (app shutdown). Best
            // effort: log so we notice if this fires during normal use.
            tracing::warn!(error = %e, "tray: launchd result channel closed");
        }
    });
}

/// Drain any pending [`LaunchdResult`]s posted from worker threads, apply
/// them to `TrayRuntime`, and re-enable the "Launch at Login" menu item.
fn apply_pending_launchd_results(runtime: &Rc<RefCell<TrayRuntime>>) {
    // Pull results out of the receiver under an immutable borrow first so
    // we never hold the channel borrow longer than necessary. No user
    // callbacks fire inside this function — the RefMut below stays
    // confined.
    let results: Vec<LaunchdResult> = {
        let rt = runtime.borrow();
        let mut out = Vec::new();
        while let Ok(r) = rt.launchd_rx.try_recv() {
            out.push(r);
        }
        out
    };
    if results.is_empty() {
        return;
    }
    let mut rt = runtime.borrow_mut();
    for r in results {
        match r.outcome {
            Ok(()) => {
                rt.launch_at_login = r.want;
                let label = if r.want {
                    "Launch at Login  ✓"
                } else {
                    "Launch at Login"
                };
                rt.menu.launch_at_login.set_text(label);
                tracing::info!("tray: launch_at_login={}", r.want);
            }
            Err(e) => {
                tracing::error!(error = %e, want = r.want, "tray: launchd toggle failed");
            }
        }
        rt.launchd_in_flight = false;
        rt.menu.launch_at_login.set_enabled(true);
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
    fn decode_png_rgba_reports_actual_dimensions() {
        // Minimal 2×2 RGBA PNG built via the `png` crate so the test stays
        // self-contained — guards the regression where dimensions were
        // hardcoded to 22×22 and `tray_icon::Icon::from_rgba` rejected the
        // buffer with BadImageBufferSize.
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("encode header");
            // 2x2 = 4 px × 4 bytes RGBA = 16 bytes
            writer.write_image_data(&[0xff; 16]).expect("encode pixels");
        }
        let (rgba, w, h) = decode_png_rgba(&buf).expect("decode round-trip");
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
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
