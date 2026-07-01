//! Popup positioning — multi-mode, clamped to visible screen frame.

use crate::config::PopupPosition;

use super::window::{POPUP_H_LOGICAL, POPUP_W_LOGICAL};

/// Position the popup window according to `mode`, clamping it onto the visible
/// screen frame so it never appears partially off-screen.
///
/// All arithmetic is in physical pixels:
///   - `cursor_position()` / `monitor.position()` / `monitor.size()` are physical px
///   - `monitor.scale_factor()` converts logical popup dims to physical px
///   - `set_position(PhysicalPosition)` places the window in physical px
pub(super) fn position_popup(win: &tauri::WebviewWindow, mode: &PopupPosition) {
    let monitors = win.available_monitors().unwrap_or_default();
    let primary = win.primary_monitor().ok().flatten();

    // Resolve the target monitor + raw (x, y) before clamping.
    let (target_monitor, raw_x, raw_y): (Option<tauri::Monitor>, i32, i32) = match mode {
        PopupPosition::Cursor => {
            // cursor_position() returns physical pixels.
            let cursor: tauri::PhysicalPosition<i32> = win
                .cursor_position()
                .map(|p| tauri::PhysicalPosition {
                    x: p.x as i32,
                    y: p.y as i32,
                })
                .unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });

            // Small offset so the popup doesn't sit right on the cursor tip.
            const OFFSET: i32 = 8;
            let rx = cursor.x + OFFSET;
            let ry = cursor.y + OFFSET;

            // Find the monitor whose physical bounds contain the cursor.
            // Iterate all monitors to handle negative coords on secondary displays.
            let mon = monitors
                .iter()
                .find(|m| {
                    let pos = m.position();
                    let size = m.size();
                    let (mx, my) = (pos.x, pos.y);
                    let (mw, mh) = (size.width as i32, size.height as i32);
                    cursor.x >= mx && cursor.x < mx + mw && cursor.y >= my && cursor.y < my + mh
                })
                .cloned()
                .or_else(|| primary.clone());

            (mon, rx, ry)
        }

        PopupPosition::Center => {
            // Center on the primary monitor (or first available).
            let mon = primary.clone().or_else(|| monitors.first().cloned());
            let (rx, ry) = if let Some(ref m) = mon {
                let pos = m.position();
                let size = m.size();
                let scale = m.scale_factor();
                let popup_w = (POPUP_W_LOGICAL * scale) as i32;
                let popup_h = (POPUP_H_LOGICAL * scale) as i32;
                let cx = pos.x + (size.width as i32 - popup_w) / 2;
                let cy = pos.y + (size.height as i32 - popup_h) / 2;
                (cx, cy)
            } else {
                (0, 0)
            };
            (mon, rx, ry)
        }

        PopupPosition::Menubar => {
            // Place below the tray / menu-bar area — top-right of the primary monitor.
            // macOS menu bar height is 24 pt logical; add a 4 pt gap.
            const MENUBAR_HEIGHT_LOGICAL: f64 = 24.0;
            const GAP_LOGICAL: f64 = 4.0;

            let mon = primary.clone().or_else(|| monitors.first().cloned());
            let (rx, ry) = if let Some(ref m) = mon {
                let pos = m.position();
                let size = m.size();
                let scale = m.scale_factor();
                let popup_w = (POPUP_W_LOGICAL * scale) as i32;
                let bar_h = ((MENUBAR_HEIGHT_LOGICAL + GAP_LOGICAL) * scale) as i32;
                // Align right edge with right edge of screen, 8 px inset.
                const RIGHT_INSET_LOGICAL: f64 = 8.0;
                let right_inset = (RIGHT_INSET_LOGICAL * scale) as i32;
                let rx = pos.x + size.width as i32 - popup_w - right_inset;
                let ry = pos.y + bar_h;
                (rx, ry)
            } else {
                (0, 0)
            };
            (mon, rx, ry)
        }
    };

    // Clamp raw position onto the monitor's frame so the popup is always fully visible.
    let (x, y) = if let Some(monitor) = target_monitor {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let popup_w = (POPUP_W_LOGICAL * scale) as i32;
        let popup_h = (POPUP_H_LOGICAL * scale) as i32;

        let mon_x = pos.x;
        let mon_y = pos.y;
        let mon_w = size.width as i32;
        let mon_h = size.height as i32;

        let max_x = mon_x + mon_w - popup_w;
        let max_y = mon_y + mon_h - popup_h;

        (
            raw_x.clamp(mon_x, max_x.max(mon_x)),
            raw_y.clamp(mon_y, max_y.max(mon_y)),
        )
    } else {
        (raw_x, raw_y)
    };

    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}
