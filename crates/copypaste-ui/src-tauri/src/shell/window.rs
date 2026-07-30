//! Popover-style window behaviour, and where the popover goes.
//!
//! A menu-bar clipboard manager is a popover, not a document window: it appears
//! on a gesture under the icon that summoned it, takes focus, and goes away when
//! the user looks elsewhere. Each of these is a defect if it is missing.
//!
//! * **Closing hides.** The window is the app's only surface, so closing it
//!   must not quit — quitting is a tray decision (manifest 06, INV-36).
//! * **Losing focus hides.** A popover that stays up after you click elsewhere
//!   is a window, and an always-on-top window that will not go away is worse
//!   than no popover at all.
//! * **Showing focuses.** A window shown without focus swallows the first
//!   keystroke, which for a search-first UI is the whole interaction.
//! * **Showing positions.** See [`anchor`].
//!
//! # Android
//!
//! This module compiles everywhere; `tray`, `hotkey` and `autostart` do not.
//! [`hide`] is a deliberate no-op there — the app *is* its window on a phone,
//! and hiding it would leave the user at the launcher with no way back.

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// The one window. Named in `tauri.conf.json`.
pub const MAIN: &str = "main";

/// Gap between the menu bar and the top of the popover, in physical pixels at
/// scale 1. Scaled by the target monitor's factor before use.
const GAP_PX: f64 = 6.0;

/// Keep this much of the screen edge clear, so the popover never sits flush
/// against the side of a display.
const EDGE_INSET_PX: f64 = 8.0;

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(MAIN)
}

/// Show and focus, in that order, under the menu-bar item.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    // Positioned before it is shown, so it never appears in one place and
    // jumps to another. Android has no tray to position under, and no
    // `tray_by_id` on `AppHandle` either — the window there is the activity.
    #[cfg(not(target_os = "android"))]
    position_under_tray(app, &window);
    let _ = window.show();
    // Focus is what makes the search field live. Requested after `show`
    // because focusing a hidden window is a no-op on macOS.
    let _ = window.set_focus();
}

/// Hide the window.
///
/// The only hide path. The frontend reaches it through the `hide_window`
/// command rather than touching the window itself (INV-25).
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if cfg!(target_os = "android") {
        return;
    }
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

/// What the hotkey and the tray's Show/Hide item both do.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    // `is_visible` failing is not a reason to do nothing: treating an
    // unanswerable window as hidden means the gesture still shows it, which is
    // the recoverable direction.
    if window.is_visible().unwrap_or(false) {
        hide(app);
    } else {
        show(app);
    }
}

/// Place the window under the menu-bar item, when we can find out where that is.
///
/// Every failure here is silent and leaves the window where it was. A popover
/// in the wrong place is a cosmetic problem; a popover that refuses to appear
/// because a monitor query returned `None` is not.
#[cfg(not(target_os = "android"))]
fn position_under_tray<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let Ok(Some(rect)) = tray.rect() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };

    // The tray rect arrives in whatever unit the platform reported. Both arms
    // are converted to physical pixels against the monitor the icon is on,
    // because mixing the two is how a window lands on the wrong display when
    // the built-in screen and an external one have different scale factors.
    let scale = window.scale_factor().unwrap_or(1.0);
    let icon = to_physical(rect, scale);

    let Ok(Some(monitor)) = window.monitor_from_point(icon.centre_x, icon.bottom) else {
        return;
    };
    let scale = monitor.scale_factor();
    let position = anchor(icon, size, monitor.position(), monitor.size(), scale);
    let _ = window.set_position(position);
}

/// A tray icon's screen rectangle, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconRect {
    pub centre_x: f64,
    pub bottom: f64,
}

#[cfg(not(target_os = "android"))]
fn to_physical(rect: tauri::Rect, scale: f64) -> IconRect {
    let position = match rect.position {
        tauri::Position::Physical(p) => PhysicalPosition::new(p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => PhysicalPosition::new(p.x * scale, p.y * scale),
    };
    let size = match rect.size {
        tauri::Size::Physical(s) => PhysicalSize::new(s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => PhysicalSize::new(s.width * scale, s.height * scale),
    };
    IconRect {
        centre_x: position.x + size.width / 2.0,
        bottom: position.y + size.height,
    }
}

/// Where the popover goes: centred under the icon, clamped onto the monitor.
///
/// Split out from [`position_under_tray`] because it is the only part with a
/// right answer that can be checked — everything around it is platform state
/// this host cannot produce.
///
/// The clamp is what makes this correct on a second display. An icon near the
/// right edge of a wide screen would otherwise put half the window off it, and
/// on a monitor whose origin is negative (a display arranged to the left of the
/// primary) an unclamped calculation lands the window on the wrong screen
/// entirely.
pub fn anchor(
    icon: IconRect,
    window: PhysicalSize<u32>,
    monitor_position: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
    scale: f64,
) -> PhysicalPosition<i32> {
    let width = f64::from(window.width);
    let height = f64::from(window.height);
    let inset = EDGE_INSET_PX * scale;

    let left = f64::from(monitor_position.x) + inset;
    let right = f64::from(monitor_position.x) + f64::from(monitor_size.width) - width - inset;
    let top = f64::from(monitor_position.y) + inset;
    let bottom = f64::from(monitor_position.y) + f64::from(monitor_size.height) - height - inset;

    // `max` then `min`: on a monitor narrower than the window the window is
    // pinned to the left edge rather than pushed off the left one.
    let x = (icon.centre_x - width / 2.0).max(left).min(right.max(left));
    let y = (icon.bottom + GAP_PX * scale).max(top).min(bottom.max(top));

    PhysicalPosition::new(x.round() as i32, y.round() as i32)
}

/// Window-event policy. Wired once in `crate::run`.
///
/// `Focused(false)` hides rather than closes so that the WebView keeps its
/// state — a popover that reloaded React on every dismissal would lose the
/// scroll position and the search text, and manifest 06's scroll-anchoring
/// requirements assume the list survives.
pub fn on_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        tauri::WindowEvent::Focused(false) => {
            let _ = window.hide();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1440×900 primary at the origin, and a 2560×1440 secondary arranged to
    /// its left — so the secondary's origin is negative, which is the case an
    /// unclamped calculation gets wrong.
    fn primary() -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        (PhysicalPosition::new(0, 0), PhysicalSize::new(1440, 900))
    }

    fn secondary_on_the_left() -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        (
            PhysicalPosition::new(-2560, 0),
            PhysicalSize::new(2560, 1440),
        )
    }

    const WINDOW: PhysicalSize<u32> = PhysicalSize {
        width: 420,
        height: 640,
    };

    #[test]
    fn the_window_is_centred_under_the_icon_and_hangs_below_it() {
        let (pos, size) = primary();
        let icon = IconRect {
            centre_x: 700.0,
            bottom: 24.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert_eq!(at.x, 700 - 210);
        assert_eq!(at.y, 24 + GAP_PX as i32);
    }

    /// The menu bar is at the right-hand end of the screen, which is where a
    /// menu-bar app's icon actually lives.
    #[test]
    fn an_icon_near_the_right_edge_does_not_push_the_window_off_screen() {
        let (pos, size) = primary();
        let icon = IconRect {
            centre_x: 1430.0,
            bottom: 24.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert!(
            at.x + 420 <= 1440,
            "the window ran off the right edge: x={}",
            at.x
        );
        assert_eq!(at.x, 1440 - 420 - EDGE_INSET_PX as i32);
    }

    /// The failure this clamp exists for: a display to the left of the primary
    /// has a negative origin, and an unclamped window lands on the wrong screen.
    #[test]
    fn a_monitor_with_a_negative_origin_keeps_the_window_on_that_monitor() {
        let (pos, size) = secondary_on_the_left();
        let icon = IconRect {
            centre_x: -2555.0,
            bottom: 24.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert!(at.x >= -2560, "{at:?}");
        assert!(at.x + 420 <= 0, "{at:?}");
    }

    /// A Retina display reports scale 2, so the gap and the inset have to
    /// double with it or the popover sits visibly tight to the menu bar.
    #[test]
    fn the_gap_and_the_inset_scale_with_the_display() {
        let (pos, size) = primary();
        let icon = IconRect {
            centre_x: 700.0,
            bottom: 48.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 2.0);
        assert_eq!(at.y, 48 + (GAP_PX * 2.0) as i32);
    }

    /// Degenerate rather than impossible: a very small external display, or a
    /// window sized larger than the screen it is being placed on. Pinning to
    /// the top-left keeps the title area reachable; the alternative pushes the
    /// window's origin off the screen and makes it unmovable.
    #[test]
    fn a_window_larger_than_the_monitor_is_pinned_to_the_top_left() {
        let pos = PhysicalPosition::new(0, 0);
        let size = PhysicalSize::new(320, 240);
        let icon = IconRect {
            centre_x: 160.0,
            bottom: 12.0,
        };
        let at = anchor(icon, WINDOW, &pos, &size, 1.0);
        assert_eq!(at.x, EDGE_INSET_PX as i32);
        assert_eq!(at.y, EDGE_INSET_PX as i32);
    }
}
