//! Menu-bar integration for the desktop app.

mod menu;
mod recent;
mod text;

use std::sync::Arc;
use std::time::Duration;

use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Listener as _, Manager as _, Runtime};
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
const BACKSTOP: Duration = Duration::from_secs(5);

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
            // A left click toggles the main window.
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                window::toggle_quick_paste(tray.app_handle());
            }
        });

    #[cfg(target_os = "macos")]
    {
        let template =
            tauri::image::Image::from_bytes(include_bytes!("../../../icons/trayTemplate.png"))?;
        tray = tray.icon(template).icon_as_template(true);
    }

    #[cfg(not(target_os = "macos"))]
    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    window::install(app)?;

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
                } else {
                    crate::shell::notify::on_recent_copy(&app);
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
