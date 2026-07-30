//! The menu-bar item.
//!
//! On macOS this is the app's primary affordance: the window is a popover that
//! hides on blur, so the tray icon is the thing that is always there. It has to
//! carry everything the app cannot show while hidden — how to bring it back,
//! whether launch-at-login is on, and the only way to quit.
//!
//! # Left click shows, right click menus
//!
//! `show_menu_on_left_click(false)` is deliberate. The menu-bar convention for
//! an app with a popover is that clicking the icon *is* the gesture — making
//! the primary click open a menu adds a step to the most common action. The
//! menu stays on the secondary click, where the settings and Quit live.
//!
//! # Quit is here and nowhere else
//!
//! Closing the window hides it (manifest 06, INV-36), and the window has no
//! decorations, so this menu is the only exit. That makes "Quit CopyPaste"
//! load-bearing rather than conventional: without it the app can only be killed
//! from Activity Monitor.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};

use super::{autostart, window};

const ID_TOGGLE: &str = "toggle";
const ID_AUTOSTART: &str = "autostart";
const ID_QUIT: &str = "quit";

/// Build and install the tray icon.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, ID_TOGGLE, "Show CopyPaste", true, None::<&str>)?;
    // Reflects the current state on build. It is not re-read afterwards, so a
    // change made in System Settings while the app runs will not show here
    // until restart; that is a known gap rather than a claim of correctness.
    let launch_at_login = CheckMenuItem::with_id(
        app,
        ID_AUTOSTART,
        "Open at Login",
        true,
        autostart::is_enabled(app),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit CopyPaste", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &launch_at_login,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    // Cloned into the menu handler so the checkbox can be put back if the
    // plugin refuses the change. `CheckMenuItem` is a reference-counted handle,
    // so this is the same item and not a copy of it — `TrayIcon` exposes no way
    // to look a menu item up again afterwards.
    let checkbox = launch_at_login.clone();

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("CopyPaste")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            ID_TOGGLE => window::toggle(app),
            ID_AUTOSTART => {
                // The checkbox has already flipped itself, so the wanted state
                // is the opposite of what the plugin currently reports. If the
                // plugin refuses, put the tick back rather than leaving the
                // menu claiming something untrue.
                let wanted = !autostart::is_enabled(app);
                if autostart::set_enabled(app, wanted).is_err() {
                    tracing::warn!("could not change the launch-at-login setting");
                }
                let _ = checkbox.set_checked(autostart::is_enabled(app));
            }
            ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left click toggles the popover. `Down` rather than `Up` so the
            // window appears while the mouse is still held, which is how the
            // native menu-bar items behave.
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                window::toggle(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}
