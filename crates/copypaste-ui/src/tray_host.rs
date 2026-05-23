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

// ── v0.3 T3: tray menu polish ───────────────────────────────────────────────
//
// The tray gets a "Recent items" block above the existing commands, grouped
// by age bucket (Today / Yesterday / This Week / Older) with a per-type
// glyph prefix (📋 text, 🖼 image, 🔗 url). Items are pushed in via
// `update_recents` from the UI; the tray module owns *only* the menu
// rebuild + bucketing logic, not the IPC fetch.
//
// Caps: at most `MAX_TRAY_RECENTS` items shown (newest first) so the tray
// menu stays scannable. Older history lives in the full HistoryWindow.

/// Newest-first cap on items rendered in the tray's "Recent items" block.
/// macOS truncates very long menus with awkward submenu-arrows; 10 keeps
/// the menu one screen tall on any display.
pub const MAX_TRAY_RECENTS: usize = 10;

/// One row in the tray's "Recent items" block. Pushed in via
/// [`update_recents`] from the UI's IPC fetcher; the tray module never
/// touches the daemon socket itself.
#[derive(Debug, Clone)]
pub struct RecentTrayItem {
    /// Daemon-side history id — surfaced back through `on_paste_item` when
    /// the user clicks the menu row.
    pub id: String,
    /// Daemon `content_type` ("text", "image", "url", …). Drives the glyph
    /// prefix in [`type_icon`].
    pub content_type: String,
    /// Already-truncated preview (Slint side keeps a copy too).
    pub preview: String,
    /// Wall time in epoch milliseconds. Buckets are computed in
    /// [`bucket_label_for_age_ms`] against the current wall clock.
    pub wall_time_ms: i64,
}

/// Stable id prefix for dynamic recent-item menu entries. The drain handler
/// strips this prefix to recover the daemon's history id and forwards it
/// to `on_paste_item`.
const ID_RECENT_PREFIX: &str = "recent:";

/// v0.3 T3: classify `wall_time_ms` into one of five buckets relative to
/// `now_ms`. Pure function so the unit test suite can pin every boundary
/// without spinning a clock.
///
/// Buckets follow Apple's HIG-style grouping in Spotlight / Mail:
/// * "Today" — last 24 h (a rolling-window simplification — proper
///   calendar-day "today" would need chrono's `Local::today`, which we
///   skip to keep the dep-free build).
/// * "Yesterday" — 24-48 h ago.
/// * "This Week" — 2-7 days ago.
/// * "Older" — > 7 days.
/// * "Unknown" — `wall_time_ms <= 0` or in the future (clock skew).
pub fn bucket_label_for_age_ms(now_ms: i64, item_ms: i64) -> &'static str {
    if item_ms <= 0 || item_ms > now_ms {
        return "Unknown";
    }
    let age_ms = now_ms - item_ms;
    const HOUR_MS: i64 = 3_600_000;
    const DAY_MS: i64 = 24 * HOUR_MS;
    if age_ms < DAY_MS {
        "Today"
    } else if age_ms < 2 * DAY_MS {
        "Yesterday"
    } else if age_ms < 7 * DAY_MS {
        "This Week"
    } else {
        "Older"
    }
}

/// v0.3 T3: pick a one-glyph prefix for a tray row. URL detection is a
/// cheap `starts_with` because the daemon does not yet expose a `url`
/// content type — most URLs arrive as `text` with an `http(s)://` prefix.
///
/// ASCII fallbacks (`[T]`, `[I]`, `[U]`) are honoured via the `ascii`
/// flag so we can swap if a font on the target macOS lacks emoji glyphs
/// in the menu bar (rare on 12+, but the escape hatch is cheap).
pub fn type_icon(content_type: &str, preview: &str, ascii: bool) -> &'static str {
    if content_type == "image" {
        return if ascii { "[I]" } else { "🖼" };
    }
    // URL heuristic — works for both `text` and a future `url` content type.
    if content_type == "url"
        || preview.starts_with("http://")
        || preview.starts_with("https://")
    {
        return if ascii { "[U]" } else { "🔗" };
    }
    if ascii { "[T]" } else { "📋" }
}

/// Current wall-clock ms since Unix epoch, defaulting to 0 if the system
/// clock is before 1970 (impossible on macOS but defensive).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

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

/// Callback fired with the daemon-side history id when the user clicks a
/// row inside the tray's "Recent items" block.
pub type PasteCb = Box<dyn Fn(&str) + 'static>;

#[derive(Default)]
pub struct TrayCallbacks {
    pub on_open_history: Option<ActionCb>,
    pub on_open_preferences: Option<ActionCb>,
    pub on_quit: Option<ActionCb>,
    /// v0.3 T3: invoked when the user picks a row from the tray's
    /// "Recent items" block. The string argument is the daemon-side
    /// history id; the UI is expected to call `IpcClient::paste(id)`.
    pub on_paste_item: Option<PasteCb>,
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
    /// v0.3 T3: refs to the dynamic "Recent items" rows. Kept on the
    /// struct so muda doesn't garbage-collect them between rebuilds; the
    /// Vec is rebuilt wholesale by `rebuild_with_recents`.
    _recent_items: Vec<MenuItem>,
    /// v0.3 T3: bucket-header rows (Today / Yesterday / …). Disabled
    /// items, kept alive for the same reason as `_recent_items`.
    _recent_headers: Vec<MenuItem>,
}

impl TrayMenu {
    fn build(
        private_mode_on: bool,
        launch_at_login_on: bool,
        recents: &[RecentTrayItem],
    ) -> Result<Self, TrayHostError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build_inner(private_mode_on, launch_at_login_on, recents)
        }))
        .map_err(|_| TrayHostError::NotMainThread)
        .and_then(|inner| inner)
    }

    fn build_inner(
        private_mode_on: bool,
        launch_at_login_on: bool,
        recents: &[RecentTrayItem],
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

        // v0.3 T3: "Recent items" block — newest-first, capped at
        // MAX_TRAY_RECENTS, grouped by age bucket with a glyph prefix per
        // content type. Skipped entirely when the daemon hasn't shipped
        // any history yet (cold start) so the tray stays compact.
        let (recent_items, recent_headers) =
            append_recents_block(&menu, recents)?;

        menu.append(&PredefinedMenuItem::separator())?;
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
            _recent_items: recent_items,
            _recent_headers: recent_headers,
        })
    }
}

/// v0.3 T3: append a "Recent items" block (header per non-empty bucket
/// followed by its rows) to `menu`. Returns the rows and headers so the
/// caller can keep them alive across menu rebuilds — muda drops the menu
/// item when its `MenuItem` value goes out of scope.
///
/// Layout (skipped entirely when `recents` is empty):
///
/// ```text
/// ───────────────
/// Today                 (disabled header)
/// 📋 hello world
/// 🔗 https://…
/// Yesterday
/// 🖼 IMG 1920×1080
/// This Week
/// 📋 last week's note
/// ```
fn append_recents_block(
    menu: &Menu,
    recents: &[RecentTrayItem],
) -> Result<(Vec<MenuItem>, Vec<MenuItem>), TrayHostError> {
    if recents.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let now = now_ms();
    let capped = &recents[..recents.len().min(MAX_TRAY_RECENTS)];

    let mut items = Vec::with_capacity(capped.len());
    let mut headers: Vec<MenuItem> = Vec::new();
    let mut current_bucket: Option<&'static str> = None;

    menu.append(&PredefinedMenuItem::separator())?;

    for r in capped {
        let bucket = bucket_label_for_age_ms(now, r.wall_time_ms);
        if current_bucket != Some(bucket) {
            let header = MenuItem::with_id(
                format!("recent_header:{bucket}"),
                bucket,
                false,
                None,
            );
            menu.append(&header)?;
            headers.push(header);
            current_bucket = Some(bucket);
        }

        let glyph = type_icon(&r.content_type, &r.preview, false);
        let label = format!("{glyph}  {}", sanitize_for_menu(&r.preview));
        let row = MenuItem::with_id(
            format!("{ID_RECENT_PREFIX}{}", r.id),
            label,
            true,
            None,
        );
        menu.append(&row)?;
        items.push(row);
    }

    Ok((items, headers))
}

/// macOS NSMenu treats `\n`, `\r`, `\t` as literal whitespace which breaks
/// the row layout. Squash control chars to a single space and trim so
/// "hello\nworld" → "hello world".
fn sanitize_for_menu(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    // Collapse runs of whitespace + trim to keep menu rows compact.
    let mut out = String::with_capacity(cleaned.len());
    let mut last_space = false;
    for c in cleaned.trim().chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    // Truncate to keep the menu single-line — macOS truncates beyond ~40
    // chars anyway, so we pre-clip with an ellipsis for explicit feedback.
    const MAX: usize = 50;
    if out.chars().count() > MAX {
        let trimmed: String = out.chars().take(MAX).collect();
        format!("{trimmed}…")
    } else {
        out
    }
}

// ── Public entry point ──────────────────────────────────────────────────────

/// Mutable state held on the main thread between event-loop ticks.
struct TrayRuntime {
    /// v0.3 T3: live reference so `update_recents` can swap the menu via
    /// `TrayIcon::set_menu`. Previously held as `_tray` (drop-only) but
    /// dynamic recents need an actual handle to call `set_menu` on.
    tray: TrayIcon,
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
    let menu = TrayMenu::build(false, launch_at_login, &[])?;

    let icon = load_icon();
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu.menu.clone()))
        .with_tooltip("CopyPaste");
    if let Some(icon) = icon {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build()?;

    let runtime = Rc::new(RefCell::new(TrayRuntime {
        tray,
        menu,
        private_mode: false,
        launch_at_login,
        callbacks,
        quit_latch: Arc::new(AtomicBool::new(false)),
    }));

    // v0.3 T3: stash the runtime in a thread-local so the free function
    // `update_recents` can rebuild the menu from outside this module.
    // Thread-local because muda is main-thread-only on macOS — any caller
    // running off-thread would crash, so colocating the cell with the
    // main-thread enforcement is the right shape.
    TRAY_RUNTIME.with(|cell| {
        *cell.borrow_mut() = Some(runtime.clone());
    });

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

// v0.3 T3: thread-local handle so `update_recents` can find the runtime
// without forcing the caller to thread a TrayHandle through every layer.
thread_local! {
    static TRAY_RUNTIME: RefCell<Option<Rc<RefCell<TrayRuntime>>>> =
        const { RefCell::new(None) };
}

/// v0.3 T3: rebuild the tray menu with a fresh recents snapshot. Must be
/// called on the main thread (the same thread that called [`install`]) —
/// muda's menu APIs are main-thread-only on macOS.
///
/// Failure to find the runtime (no `install` yet, called from the wrong
/// thread) is logged and silently swallowed — the tray is non-critical
/// surface, the UI window keeps working.
pub fn update_recents(recents: Vec<RecentTrayItem>) {
    TRAY_RUNTIME.with(|cell| {
        let Some(runtime) = cell.borrow().clone() else {
            tracing::debug!("update_recents called before tray install — ignoring");
            return;
        };
        let mut rt = runtime.borrow_mut();
        let private_mode = rt.private_mode;
        let launch_at_login = rt.launch_at_login;
        match TrayMenu::build(private_mode, launch_at_login, &recents) {
            Ok(new_menu) => {
                // Swap the live menu before dropping the old one so the
                // OS always has a valid menu reference.
                rt.tray.set_menu(Some(Box::new(new_menu.menu.clone())));
                rt.menu = new_menu;
                tracing::debug!(count = recents.len(), "tray: recents updated");
            }
            Err(e) => {
                tracing::warn!(error = %e, "tray: recents rebuild failed");
            }
        }
    });
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
            other if other.starts_with(ID_RECENT_PREFIX) => {
                // v0.3 T3: dynamic recent row click — strip the prefix
                // back to the daemon-side history id and forward.
                let id = &other[ID_RECENT_PREFIX.len()..];
                if let Some(cb) = &rt.callbacks.on_paste_item {
                    cb(id);
                } else {
                    tracing::debug!(id, "tray: recent clicked but no on_paste_item bound");
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
        let outcome = std::panic::catch_unwind(|| TrayMenu::build(false, false, &[]));
        let result = outcome.expect("build must not panic");
        assert!(result.is_err(), "expected NotMainThread on test thread");
    }

    // ── v0.3 T3 bucket / icon / sanitize unit tests ─────────────────────

    const HOUR_MS: i64 = 3_600_000;
    const DAY_MS: i64 = 24 * HOUR_MS;

    #[test]
    fn bucket_today_for_recent_items() {
        let now = 10 * DAY_MS;
        assert_eq!(bucket_label_for_age_ms(now, now), "Today");
        assert_eq!(bucket_label_for_age_ms(now, now - HOUR_MS), "Today");
        assert_eq!(bucket_label_for_age_ms(now, now - DAY_MS + 1), "Today");
    }

    #[test]
    fn bucket_yesterday_at_day_boundary() {
        let now = 10 * DAY_MS;
        // Exactly DAY_MS old = Yesterday (boundary belongs to the next bucket).
        assert_eq!(bucket_label_for_age_ms(now, now - DAY_MS), "Yesterday");
        assert_eq!(bucket_label_for_age_ms(now, now - 2 * DAY_MS + 1), "Yesterday");
    }

    #[test]
    fn bucket_this_week_for_2_to_7_days() {
        let now = 30 * DAY_MS;
        assert_eq!(bucket_label_for_age_ms(now, now - 2 * DAY_MS), "This Week");
        assert_eq!(bucket_label_for_age_ms(now, now - 6 * DAY_MS), "This Week");
        assert_eq!(bucket_label_for_age_ms(now, now - 7 * DAY_MS + 1), "This Week");
    }

    #[test]
    fn bucket_older_after_a_week() {
        let now = 100 * DAY_MS;
        assert_eq!(bucket_label_for_age_ms(now, now - 7 * DAY_MS), "Older");
        assert_eq!(bucket_label_for_age_ms(now, now - 90 * DAY_MS), "Older");
    }

    #[test]
    fn bucket_unknown_for_zero_and_future() {
        let now = 10 * DAY_MS;
        assert_eq!(bucket_label_for_age_ms(now, 0), "Unknown");
        assert_eq!(bucket_label_for_age_ms(now, -1), "Unknown");
        assert_eq!(bucket_label_for_age_ms(now, now + HOUR_MS), "Unknown");
    }

    #[test]
    fn type_icon_picks_image_glyph() {
        assert_eq!(type_icon("image", "", false), "🖼");
        assert_eq!(type_icon("image", "", true), "[I]");
    }

    #[test]
    fn type_icon_picks_url_glyph_for_explicit_type() {
        assert_eq!(type_icon("url", "anything", false), "🔗");
    }

    #[test]
    fn type_icon_detects_url_in_text_preview() {
        assert_eq!(type_icon("text", "https://example.com", false), "🔗");
        assert_eq!(type_icon("text", "http://example.com", false), "🔗");
        assert_eq!(type_icon("text", "https://example.com", true), "[U]");
    }

    #[test]
    fn type_icon_defaults_to_text_glyph() {
        assert_eq!(type_icon("text", "hello", false), "📋");
        assert_eq!(type_icon("text", "hello", true), "[T]");
        // Non-URL prefixes don't accidentally hit the url branch.
        assert_eq!(type_icon("text", "httpx://nope", false), "📋");
    }

    #[test]
    fn sanitize_collapses_whitespace_and_control_chars() {
        assert_eq!(sanitize_for_menu("hello\nworld"), "hello world");
        assert_eq!(sanitize_for_menu("a\t\tb"), "a b");
        assert_eq!(sanitize_for_menu("  pad  "), "pad");
        assert_eq!(sanitize_for_menu("line1\r\nline2"), "line1 line2");
    }

    #[test]
    fn sanitize_truncates_long_strings_with_ellipsis() {
        let long = "x".repeat(80);
        let out = sanitize_for_menu(&long);
        assert!(out.ends_with('…'), "expected ellipsis on long input, got {out}");
        assert_eq!(out.chars().count(), 51); // 50 chars + …
    }

    #[test]
    fn sanitize_preserves_short_input() {
        assert_eq!(sanitize_for_menu("hi"), "hi");
    }
}
