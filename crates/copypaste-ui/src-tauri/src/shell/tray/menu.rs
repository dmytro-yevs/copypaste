//! The parts of the menu that change while the app runs.
//!
//! Built once, mutated afterwards: a refresh sets text on handles it already
//! holds and moves the same handles in and out of the submenu, so nothing
//! allocates a menu item per capture. v1 rebuilt its recent submenu every five
//! seconds.
//!
//! The clipping ids live here because a `MenuItem` remembers only what it
//! says, and the click handler must turn `recent.2` back into the row that
//! slot was showing when it was clicked.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager as _, Runtime};

use super::recent::{self, Clipping};
use super::text;
use crate::backend::{Backend as _, SelectedBackend};

pub const ID_TOGGLE: &str = "toggle";
pub const ID_AUTOSTART: &str = "autostart";
pub const ID_PRIVATE_MODE: &str = "private-mode";
pub const ID_QUIT: &str = "quit";
pub const ID_STATUS: &str = "status";
pub const RECENT_PREFIX: &str = "recent.";

/// Which clipping a `recent.N` id names, or `None` if the menu has moved on.
pub fn recent_slot(id: &str) -> Option<usize> {
    id.strip_prefix(RECENT_PREFIX)?.parse().ok()
}

/// What the submenu is currently showing.
#[derive(Debug, Default)]
struct Shown {
    slots: usize,
    placeholder: bool,
    ids: Vec<String>,
}

pub struct TrayMenu<R: Runtime> {
    pub menu: Menu<R>,
    pub autostart: CheckMenuItem<R>,
    pub private_mode: CheckMenuItem<R>,
    status: MenuItem<R>,
    recent: Submenu<R>,
    slots: Vec<MenuItem<R>>,
    placeholder: MenuItem<R>,
    shown: Mutex<Shown>,
}

impl<R: Runtime> TrayMenu<R> {
    pub fn build(app: &AppHandle<R>, autostart_enabled: bool) -> tauri::Result<Self> {
        // Disabled: it reports, it is not a verb. The one thing the tray could
        // not say before is why nothing is being captured.
        let status = MenuItem::with_id(app, ID_STATUS, text::STATUS_OFFLINE, false, None::<&str>)?;
        let toggle = MenuItem::with_id(app, ID_TOGGLE, text::SHOW, true, None::<&str>)?;

        let placeholder = MenuItem::new(app, text::RECENT_EMPTY, false, None::<&str>)?;
        let slots = (0..recent::SLOTS)
            .map(|i| {
                MenuItem::with_id(
                    app,
                    format!("{RECENT_PREFIX}{i}"),
                    // Replaced by the first refresh; a slot is only ever in the
                    // submenu once it has a clipping's text.
                    text::RECENT_EMPTY,
                    true,
                    None::<&str>,
                )
            })
            .collect::<tauri::Result<Vec<_>>>()?;
        let recent = Submenu::with_items(app, text::RECENT, true, &[&placeholder])?;

        let autostart = CheckMenuItem::with_id(
            app,
            ID_AUTOSTART,
            text::OPEN_AT_LOGIN,
            true,
            autostart_enabled,
            None::<&str>,
        )?;
        // Startup never waits for IPC. The refresh loop replaces this safe
        // unchecked default with the persisted daemon truth.
        let private_mode = CheckMenuItem::with_id(
            app,
            ID_PRIVATE_MODE,
            text::PRIVATE_MODE,
            true,
            false,
            None::<&str>,
        )?;
        let quit = MenuItem::with_id(app, ID_QUIT, text::QUIT, true, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[
                &status,
                &PredefinedMenuItem::separator(app)?,
                &toggle,
                &recent,
                &PredefinedMenuItem::separator(app)?,
                &private_mode,
                &PredefinedMenuItem::separator(app)?,
                &autostart,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;

        Ok(Self {
            menu,
            autostart,
            private_mode,
            status,
            recent,
            slots,
            placeholder,
            shown: Mutex::new(Shown {
                placeholder: true,
                ..Shown::default()
            }),
        })
    }

    /// The clipping id a slot is showing.
    ///
    /// `None` once the list has shrunk past that slot: a click landing on a
    /// slot the menu no longer offers copies nothing, rather than the row that
    /// used to be there.
    pub fn clipping_at(&self, slot: usize) -> Option<String> {
        self.shown.lock().ok()?.ids.get(slot).cloned()
    }

    /// Re-read the backend and put what it says into the menu.
    pub async fn refresh(&self, app: &AppHandle<R>) {
        let backend = app.state::<SelectedBackend>();

        let status = match backend.status().await {
            Ok(status) if status.capture_running => text::STATUS_CAPTURING,
            Ok(_) => text::STATUS_STOPPED,
            Err(_) => text::STATUS_OFFLINE,
        };
        let _ = self.status.set_text(status);

        if let Ok(config) = backend.get_config().await {
            let _ = self.private_mode.set_checked(config.config.private_mode);
        }

        // The page carries plaintext, which is exactly why `Clipping` and not
        // this function decides what a menu may say about it.
        match backend.list(recent::FETCH, None).await {
            Ok(page) => self.show(&recent::menu_clippings(&page.items), text::RECENT_EMPTY),
            Err(_) => self.show(&[], text::RECENT_OFFLINE),
        }
    }

    fn show(&self, clippings: &[Clipping], empty: &str) {
        let Ok(mut shown) = self.shown.lock() else {
            // A poisoned lock leaves the menu on its last good contents, which
            // is stale rather than wrong. Clearing it would be a menu that
            // silently emptied itself.
            tracing::error!("the tray menu lock is poisoned");
            return;
        };

        shown.ids = clippings.iter().map(|c| c.id().to_string()).collect();

        if clippings.is_empty() {
            while shown.slots > 0 {
                shown.slots -= 1;
                let _ = self.recent.remove(&self.slots[shown.slots]);
            }
            let _ = self.placeholder.set_text(empty);
            if !shown.placeholder {
                let _ = self.recent.append(&self.placeholder);
                shown.placeholder = true;
            }
            return;
        }

        if shown.placeholder {
            let _ = self.recent.remove(&self.placeholder);
            shown.placeholder = false;
        }
        for (slot, clipping) in clippings.iter().enumerate() {
            let _ = self.slots[slot].set_text(clipping.label());
        }
        // Membership last, so a slot never appears carrying the text it had a
        // refresh ago.
        while shown.slots > clippings.len() {
            shown.slots -= 1;
            let _ = self.recent.remove(&self.slots[shown.slots]);
        }
        while shown.slots < clippings.len() {
            let _ = self.recent.append(&self.slots[shown.slots]);
            shown.slots += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recent_id_names_its_slot_and_nothing_else_does() {
        assert_eq!(recent_slot("recent.0"), Some(0));
        assert_eq!(recent_slot("recent.4"), Some(4));
        assert_eq!(recent_slot("quit"), None);
        assert_eq!(recent_slot("recent."), None);
        assert_eq!(recent_slot("recent.x"), None);
    }
}
