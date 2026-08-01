//! The document window and the separate Quick Paste popover.

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const MAIN: &str = "main";
pub const QUICK_PASTE: &str = "quick-paste";

const GAP_PX: f64 = 6.0;
const EDGE_INSET_PX: f64 = 8.0;

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(main) = main_window(app) {
        hide_on_close(main);
    }

    if app.get_webview_window(QUICK_PASTE).is_none() {
        let popup =
            WebviewWindowBuilder::new(app, QUICK_PASTE, WebviewUrl::App("index.html".into()))
                .title("Quick Paste")
                .inner_size(420.0, 600.0)
                .min_inner_size(360.0, 400.0)
                .resizable(false)
                .always_on_top(true)
                .decorations(false)
                .transparent(true)
                .shadow(true)
                .skip_taskbar(true)
                .visible(false)
                .build()?;
        hide_on_close(popup);
    }

    Ok(())
}

fn hide_on_close<R: Runtime>(window: WebviewWindow<R>) {
    let handler_window = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = handler_window.hide();
        }
        WindowEvent::Focused(false) if handler_window.label() == QUICK_PASTE => {
            let _ = handler_window.hide();
        }
        _ => {}
    });
}

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(MAIN)
}

fn quick_paste_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(QUICK_PASTE)
}

pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if cfg!(target_os = "android") {
        return;
    }
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
    } else {
        show(app);
    }
}

/// Toggle the dedicated Quick Paste popover without changing the document window.
pub fn toggle_quick_paste<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = quick_paste_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    position_under_tray(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
}

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
    let scale = window.scale_factor().unwrap_or(1.0);
    let icon = to_physical(rect, scale);
    let Ok(Some(monitor)) = window.monitor_from_point(icon.centre_x, icon.bottom) else {
        return;
    };
    let position = anchor(
        icon,
        size,
        monitor.position(),
        monitor.size(),
        monitor.scale_factor(),
    );
    let _ = window.set_position(position);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconRect {
    pub centre_x: f64,
    pub bottom: f64,
}

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
    let x = (icon.centre_x - width / 2.0).max(left).min(right.max(left));
    let y = (icon.bottom + GAP_PX * scale).max(top).min(bottom.max(top));

    PhysicalPosition::new(x.round() as i32, y.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: PhysicalSize<u32> = PhysicalSize {
        width: 420,
        height: 600,
    };

    #[test]
    fn the_popup_is_centred_under_the_icon() {
        let at = anchor(
            IconRect {
                centre_x: 700.0,
                bottom: 24.0,
            },
            WINDOW,
            &PhysicalPosition::new(0, 0),
            &PhysicalSize::new(1440, 900),
            1.0,
        );
        assert_eq!(at, PhysicalPosition::new(490, 30));
    }

    #[test]
    fn the_popup_stays_on_a_negative_origin_monitor() {
        let at = anchor(
            IconRect {
                centre_x: -2555.0,
                bottom: 24.0,
            },
            WINDOW,
            &PhysicalPosition::new(-2560, 0),
            &PhysicalSize::new(2560, 1440),
            1.0,
        );
        assert!(at.x >= -2560, "{at:?}");
        assert!(at.x + WINDOW.width as i32 <= 0, "{at:?}");
    }

    #[test]
    fn the_popup_never_runs_off_the_right_edge() {
        let at = anchor(
            IconRect {
                centre_x: 1430.0,
                bottom: 24.0,
            },
            WINDOW,
            &PhysicalPosition::new(0, 0),
            &PhysicalSize::new(1440, 900),
            1.0,
        );
        assert_eq!(at.x + WINDOW.width as i32, 1440 - EDGE_INSET_PX as i32);
    }
}
