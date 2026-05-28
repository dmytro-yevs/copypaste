//! Headless PNG snapshot harness for the CopyPaste Slint desktop UI.
//!
//! # Why this crate exists
//!
//! Until now neither CI nor an AI agent could *see* the rendered UI. That blind
//! spot is exactly how the v0.4.0 visual-layout regressions shipped (a floating
//! global search field across views, a misplaced ⌘K badge in History, a cramped
//! Devices header). This crate closes that gap: it renders the existing Slint
//! views to PNG files entirely off-screen — no display server, no human, no
//! window manager — so layout can be inspected and regressions caught in CI.
//!
//! # How it works (zero changes to `copypaste-ui`)
//!
//! Slint ships a [`SoftwareRenderer`] that rasterises into a plain CPU pixel
//! buffer, plus a [`MinimalSoftwareWindow`] adapter that implements
//! [`slint::Window::take_snapshot`]. The default `i-slint-backend-testing`
//! backend (used by the UI crate's headless property tests) is *not* enough on
//! its own: its `TestingWindow` is its own renderer with stubbed text metrics
//! and does **not** implement `take_snapshot`, so it cannot produce pixels.
//!
//! Here we register our own [`slint::platform::Platform`] whose
//! `create_window_adapter` returns a `MinimalSoftwareWindow`. Slint wires the
//! renderer to the window automatically when a component is set on it, so
//! `Window::take_snapshot()` then returns a real `SharedPixelBuffer`.
//!
//! The `.slint` sources themselves are compiled in `build.rs` straight from the
//! `copypaste-ui` crate's `ui/` directory — they are read in place and never
//! modified or copied.
//!
//! [`SoftwareRenderer`]: slint::platform::software_renderer::SoftwareRenderer
//! [`MinimalSoftwareWindow`]: slint::platform::software_renderer::MinimalSoftwareWindow

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{PhysicalSize, PlatformError};

// Pull in the generated bindings for the copypaste-ui components compiled by
// build.rs (MainWindow, ClipItem, DeviceEntry, the legacy windows, ...).
slint::include_modules!();

/// Default render size for the desktop single-window shell.
///
/// `MainWindow` declares `preferred-width: 900px; preferred-height: 600px` and
/// switches to the sidebar (desktop) layout when `screen-width >= 600px`, so we
/// render at the preferred desktop size.
pub const DESKTOP_SIZE: (u32, u32) = (900, 600);

/// A headless Slint platform backed solely by the software renderer.
///
/// It owns no state: each component Slint constructs asks for a fresh
/// [`MinimalSoftwareWindow`], and the component handle keeps that window alive
/// for as long as the snapshot needs it (create component → set size → render).
struct HeadlessPlatform;

impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        // NewBuffer: each render targets a fresh, fully-painted buffer (no
        // partial-redraw dirty tracking) — exactly what a one-shot snapshot
        // wants. Slint calls `renderer().set_window_adapter()` when the
        // component is bound, so `take_snapshot` finds its window.
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }
}

/// Install the headless software-rendering platform for this process.
///
/// Must be called exactly once before any Slint component is constructed.
/// Idempotent across repeated calls *within reason*: `set_platform` returns an
/// error if a platform is already installed, which we treat as "already
/// initialised" and swallow so callers don't have to coordinate.
pub fn init_headless_platform() {
    // If a platform is already set (e.g. a previous call in the same process),
    // `set_platform` errors — that's fine, the headless platform is in place.
    let _ = slint::platform::set_platform(Box::new(HeadlessPlatform));
}

/// Render a Slint component to a PNG file at `path`, sized `width`x`height` px.
///
/// `component` must already be constructed (so its window exists). This sets the
/// window size, lets Slint settle pending layout/property updates, takes a
/// software-rendered snapshot, and encodes it as RGBA8 PNG via the `image`
/// crate.
pub fn render_component_to_png(
    component: &impl slint::ComponentHandle,
    width: u32,
    height: u32,
    path: &Path,
) -> Result<()> {
    let window = component.window();
    window.set_size(PhysicalSize::new(width, height));

    // Showing the window activates the component on the WindowInner: it runs
    // layout, resolves the window-item background/geometry, and marks the
    // frame dirty. Without this the software renderer paints against an
    // unlaid-out window item (size/background unresolved) and produces a
    // blank frame. The headless platform has no real event loop, so `show()`
    // just flips the visibility/activation state — it does not block.
    component
        .show()
        .map_err(|e| anyhow!("Window::show failed: {e}"))?;

    // Settle any timers / deferred property bindings the component queued
    // during construction/show so the first frame reflects the final layout.
    slint::platform::update_timers_and_animations();

    let buffer = window
        .take_snapshot()
        .map_err(|e| anyhow!("Window::take_snapshot failed: {e}"))?;

    // Tear the window back down so each snapshot starts from a clean window
    // (the headless platform reuses one MinimalSoftwareWindow adapter).
    component
        .hide()
        .map_err(|e| anyhow!("Window::hide failed: {e}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating snapshot directory {}", parent.display()))?;
    }

    // Slint's `take_snapshot` renders into an opaque RGB buffer, then widens it
    // to RGBA by copying only the RGB channels (see software_renderer.rs:872) —
    // it never writes the alpha byte, so every pixel comes back with alpha 0.
    // The colour data is correct; left as-is the PNG would composite as fully
    // transparent (i.e. look blank/white in a viewer). Force alpha to opaque so
    // the saved image matches what a user sees on screen.
    let mut rgba = buffer.as_bytes().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 0xff;
    }

    let img =
        image::RgbaImage::from_raw(buffer.width(), buffer.height(), rgba).ok_or_else(|| {
            anyhow!(
                "pixel buffer {}x{} does not match its byte length {} — cannot build RGBA image",
                buffer.width(),
                buffer.height(),
                buffer.as_bytes().len()
            )
        })?;

    img.save(path)
        .with_context(|| format!("encoding PNG to {}", path.display()))?;

    Ok(())
}

/// The four UI states the harness renders, matching the brief.
///
/// Each variant maps to a sidebar tab and a minimal set of view properties that
/// put the shared `MainWindow` shell into that state.
#[derive(Clone, Copy, Debug)]
pub enum View {
    /// History tab, empty list (no clipboard items yet).
    HistoryEmpty,
    /// Devices tab, no paired devices.
    DevicesEmpty,
    /// Settings tab.
    Settings,
    /// Pair tab.
    Pair,
}

impl View {
    /// All states rendered by the harness, in a stable order.
    pub fn all() -> [View; 4] {
        [
            View::HistoryEmpty,
            View::DevicesEmpty,
            View::Settings,
            View::Pair,
        ]
    }

    /// Output file stem, e.g. `history-empty` -> `history-empty.png`.
    pub fn file_stem(self) -> &'static str {
        match self {
            View::HistoryEmpty => "history-empty",
            View::DevicesEmpty => "devices-empty",
            View::Settings => "settings",
            View::Pair => "pair",
        }
    }

    /// `MainWindow.active-tab` index: 0=History 1=Devices 2=Pair 3=Settings.
    fn active_tab(self) -> i32 {
        match self {
            View::HistoryEmpty => 0,
            View::DevicesEmpty => 1,
            View::Pair => 2,
            View::Settings => 3,
        }
    }
}

/// Drive the shared `MainWindow` shell into `view`'s state with minimal mock
/// data (all owned by this crate, never injected into `copypaste-ui`).
///
/// History/Devices are rendered *empty* per the brief; Settings/Pair need no
/// list data. The sidebar footer is given a connected daemon + version string
/// so the footer renders realistically rather than as a warning dot.
pub fn build_main_window(view: View) -> Result<MainWindow> {
    let win = MainWindow::new().context("constructing MainWindow in headless mode")?;

    // Desktop sidebar layout (>= 600px) rather than the Android bottom-tab one.
    // `screen-width` is a Slint `length`, which the generated bindings expose as
    // a bare `f32` (logical px) — matching `copypaste-ui`'s own call site.
    win.set_screen_width(DESKTOP_SIZE.0 as f32);
    win.set_active_tab(view.active_tab());

    // Realistic, non-empty shell chrome.
    win.set_daemon_connected(true);
    win.set_app_version("v0.4.0".into());

    match view {
        View::HistoryEmpty => {
            // Empty model -> HistoryView renders its empty-state placeholder.
            let items: Rc<slint::VecModel<ClipItem>> = Rc::new(slint::VecModel::default());
            win.set_history_items(items.into());
            win.set_history_loading(false);
            win.set_history_error("".into());
        }
        View::DevicesEmpty => {
            // Empty model -> DevicesView renders its empty-state placeholder.
            let devices: Rc<slint::VecModel<DeviceEntry>> = Rc::new(slint::VecModel::default());
            win.set_devices(devices.into());
        }
        View::Settings => {
            // Settings view reads scalar prefs; defaults are fine, but seed a
            // fingerprint so the security section renders a real value.
            win.set_fingerprint("AB12 CD34 EF56 7890 AB12 CD34 EF56 7890".into());
        }
        View::Pair => {
            // Pair view default mode shows the code-entry / waiting state.
            win.set_pair_status_text("Waiting for peer…".into());
            win.set_pair_status_is_error(false);
        }
    }

    Ok(win)
}

/// Render all four states to `<out_dir>/<stem>.png` and return the written
/// paths in order.
pub fn render_all(out_dir: &Path) -> Result<Vec<PathBuf>> {
    init_headless_platform();

    let (w, h) = DESKTOP_SIZE;
    let mut written = Vec::with_capacity(View::all().len());

    for view in View::all() {
        let win = build_main_window(view)?;
        let path = out_dir.join(format!("{}.png", view.file_stem()));
        render_component_to_png(&win, w, h, &path)
            .with_context(|| format!("rendering {:?}", view))?;
        written.push(path);
    }

    Ok(written)
}
