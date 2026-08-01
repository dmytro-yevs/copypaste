//! The menu-bar item, and the only exit: closing the window hides it
//! (manifest 06, INV-36) and the window has no decorations.
//!
//! `show_menu_on_left_click(false)` — with a popover, clicking the icon *is*
//! the gesture, so the menu stays on the secondary click.
//!
//! UNVERIFIED: Linux host. The refresh loop, the slot reconciliation and the
//! click-to-copy path have never been run.

mod menu;
mod recent;
mod text;

use std::sync::Arc;
use std::time::Duration;

use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter as _, Listener as _, Manager as _, Runtime};
use tokio::sync::Notify;

use self::menu::TrayMenu;
use super::{autostart, window};
use crate::backend::{Backend as _, SelectedBackend};
use crate::service::push::{EVENT_CHANGED, EVENT_PUSH_STATE};

/// Coalescing window after a change before the menu is re-read.
///
/// A paste-heavy minute emits one event per capture. `Notify` already collapses
/// a burst into one wake; this stops the one refresh that follows from racing
/// the tail of the burst.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// How often the menu is re-read with no event to prompt it.
///
/// The change stream can die in ways neither end notices — `service::push` says
/// so, and the frontend keeps a slow poll for the same reason. A tray that
/// stopped updating because it believed it was subscribed would show yesterday's
/// clippings indefinitely.
const BACKSTOP: Duration = Duration::from_secs(30);

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // Read once at build: a change made in System Settings while the app runs
    // does not show until restart. A known gap, not a claim of correctness.
    let tray_menu = Arc::new(TrayMenu::build(app, autostart::is_enabled(app))?);

    let handler = Arc::clone(&tray_menu);
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("CopyPaste")
        .menu(&tray_menu.menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| on_menu_event(app, &handler, event.id().as_ref()))
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

    spawn_refresh(app.clone(), tray_menu);
    Ok(())
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, tray_menu: &Arc<TrayMenu<R>>, id: &str) {
    match id {
        menu::ID_TOGGLE => window::toggle(app),
        menu::ID_AUTOSTART => {
            // The checkbox has already flipped itself, so the wanted state is
            // the opposite of what the plugin currently reports. If the plugin
            // refuses, put the tick back rather than leaving the menu claiming
            // something untrue.
            let wanted = !autostart::is_enabled(app);
            if autostart::set_enabled(app, wanted).is_err() {
                tracing::warn!("could not change the launch-at-login setting");
            }
            let _ = tray_menu.autostart.set_checked(autostart::is_enabled(app));
        }
        menu::ID_PRIVATE_MODE => {
            // Tauri toggles a check item before delivering the event, so its
            // current state is the requested one. Confirm it from persisted
            // daemon state below and roll it back if that write fails.
            let wanted = tray_menu.private_mode.is_checked().unwrap_or(false);
            let menu = Arc::clone(tray_menu);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let backend = app.state::<SelectedBackend>();
                match backend
                    .set_config(copypaste_ipc::ConfigPatch {
                        private_mode: Some(wanted),
                        ..Default::default()
                    })
                    .await
                {
                    Ok(applied) => {
                        let _ = menu.private_mode.set_checked(applied.config.private_mode);
                        let _ = app.emit("private-mode-changed", applied.config.private_mode);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "could not change private mode");
                        let _ = menu.private_mode.set_checked(!wanted);
                    }
                }
            });
        }
        menu::ID_QUIT => app.exit(0),
        id => {
            let Some(slot) = menu::recent_slot(id) else {
                return;
            };
            // By id: the backend puts the text on the pasteboard, so it never
            // has to be held here.
            let Some(item) = tray_menu.clipping_at(slot) else {
                return;
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let backend = app.state::<SelectedBackend>();
                if let Err(error) = backend.copy(&item).await {
                    tracing::warn!(%error, "could not copy a clipping from the menu");
                }
            });
        }
    }
}

fn spawn_refresh<R: Runtime>(app: AppHandle<R>, tray_menu: Arc<TrayMenu<R>>) {
    let wake = Arc::new(Notify::new());

    // `Notify::notify_one` stores at most one permit, so a burst of captures
    // costs one refresh rather than one per event.
    for event in [EVENT_CHANGED, EVENT_PUSH_STATE] {
        let on_change = Arc::clone(&wake);
        app.listen(event, move |_| on_change.notify_one());
    }
    // The second is not redundant: the app starts the service after the tray
    // exists, so without it the menu would report "not running" until the
    // backstop, on every launch.

    tauri::async_runtime::spawn(async move {
        loop {
            tray_menu.refresh(&app).await;
            tokio::select! {
                () = wake.notified() => tokio::time::sleep(DEBOUNCE).await,
                () = tokio::time::sleep(BACKSTOP) => {}
            }
        }
    });
}
