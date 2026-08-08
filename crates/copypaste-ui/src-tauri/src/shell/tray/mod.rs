//! The menu-bar item, and the only exit: closing either app surface hides it
//! (manifest 06, INV-36); only the Quick Paste surface is frameless.
//!
//! `show_menu_on_left_click(false)` — with a popover, clicking the icon *is*
//! the gesture, so the menu stays on the secondary click.
//!
//! `IconMenuItem`'s native icons are a documented no-op away from macOS
//! (`muda-0.19.3/src/items/icon.rs`), so the Windows menu is the same rows
//! carrying the same text with no images. The labels already hold the state.
//!
//! UNVERIFIED: Linux host. The refresh loop, the slot reconciliation and the
//! click-to-copy path have never been run on macOS or Windows.

mod menu;
#[cfg(target_os = "macos")]
mod menu_image_visibility;
mod private_mode;
mod recent;
mod text;

use std::sync::Arc;
use std::time::Duration;

use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter as _, Listener as _, Manager as _, Runtime};
use tokio::sync::Notify;

#[cfg(target_os = "macos")]
pub(super) use menu_image_visibility::force_menu_image_visibility;

#[cfg(not(target_os = "macos"))]
pub(super) fn force_menu_image_visibility<R: Runtime>(_app: &AppHandle<R>) {}

use self::menu::TrayMenu;
use super::{autostart, notify, window};
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
            // Left click toggles the popover. `Down` rather than `Up` so the
            // window appears while the mouse is still held, which is how the
            // native menu-bar items behave.
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                window::toggle_quick_paste(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    force_menu_image_visibility(app);

    // The tray and Settings are independent readers of the same system state.
    let follower = Arc::clone(&tray_menu);
    let handle = app.clone();
    app.listen(autostart::EVENT_CHANGED, move |_| {
        follower.set_autostart(autostart::is_enabled(&handle));
    });

    private_mode::spawn(app.clone(), Arc::clone(&tray_menu), BACKSTOP);
    spawn_refresh(app.clone(), tray_menu);
    Ok(())
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, tray_menu: &Arc<TrayMenu<R>>, id: &str) {
    match id {
        // The menu item is named “Show CopyPaste”, so it must reveal the
        // primary application window. The menu-bar icon and global shortcut
        // remain the explicit Quick Paste affordances.
        menu::ID_TOGGLE => window::show_main(app),
        menu::ID_SETTINGS => window::show_main_settings(app),
        menu::ID_AUTOSTART => {
            // No optimistic label: `set_enabled` emits the state it can read
            // back afterwards, and the listener in `build` applies that. A
            // refusal emits nothing, so the menu keeps saying what is true.
            let wanted = !autostart::is_enabled(app);
            if let Err(error) = autostart::set_enabled(app, wanted) {
                tracing::warn!(%error, "could not change the launch-at-login setting");
            }
        }
        menu::ID_PRIVATE_MODE => {
            let previous = tray_menu.private_mode_enabled();
            let wanted = !previous;
            tray_menu.set_private_mode(wanted);
            let revision = tray_menu.private_mode_snapshot().1;
            let menu = Arc::clone(tray_menu);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let backend = app.state::<SelectedBackend>();
                let applied = match backend.set_private_mode(wanted).await {
                    Ok(applied) => Some(applied.private_mode),
                    Err(error) => {
                        tracing::warn!(%error, "could not change private mode");
                        None
                    }
                };
                let update = private_mode::toggle_update(previous, applied);
                if menu.confirm_private_mode(update.value, revision) && update.broadcast {
                    let _ = app.emit(private_mode::EVENT_CHANGED, update.value);
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
                match backend.copy(&item).await {
                    Ok(_) => notify::on_recent_copy(&app),
                    Err(error) => {
                        tracing::warn!(%error, "could not copy a clipping from the menu");
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_freshness_backstop_re_reads_the_recent_menu_every_five_seconds() {
        assert_eq!(BACKSTOP, Duration::from_secs(5));
    }
}
