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

use tauri::menu::{IconMenuItem, Menu, NativeIcon, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager as _, Runtime};

use super::force_menu_image_visibility;
use super::recent::{self, Clipping};
use super::text;
use crate::backend::{Backend as _, SelectedBackend};

pub const ID_TOGGLE: &str = "toggle";
pub const ID_SETTINGS: &str = "settings";
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

#[derive(Debug, Default)]
struct PrivateModeState {
    enabled: bool,
    revision: u64,
}

impl PrivateModeState {
    fn apply(&mut self, enabled: bool, expected_revision: Option<u64>) -> Option<bool> {
        if expected_revision.is_some_and(|revision| revision != self.revision) {
            return None;
        }
        self.revision = self.revision.wrapping_add(1);
        let changed = self.enabled != enabled;
        self.enabled = enabled;
        Some(changed)
    }
}

pub struct TrayMenu<R: Runtime> {
    pub menu: Menu<R>,
    pub autostart: IconMenuItem<R>,
    pub private_mode: IconMenuItem<R>,
    status: IconMenuItem<R>,
    recent: Submenu<R>,
    slots: Vec<IconMenuItem<R>>,
    placeholder: IconMenuItem<R>,
    shown: Mutex<Shown>,
    private_mode_state: Mutex<PrivateModeState>,
}

impl<R: Runtime> TrayMenu<R> {
    pub fn build(app: &AppHandle<R>, autostart_enabled: bool) -> tauri::Result<Self> {
        // Disabled: it reports, it is not a verb. The one thing the tray could
        // not say before is why nothing is being captured.
        let status = IconMenuItem::with_id_and_native_icon(
            app,
            ID_STATUS,
            text::STATUS_OFFLINE,
            false,
            Some(NativeIcon::StatusUnavailable),
            None::<&str>,
        )?;
        let toggle = IconMenuItem::with_id_and_native_icon(
            app,
            ID_TOGGLE,
            text::SHOW,
            true,
            Some(NativeIcon::QuickLook),
            None::<&str>,
        )?;
        let settings = IconMenuItem::with_id_and_native_icon(
            app,
            ID_SETTINGS,
            text::SETTINGS,
            true,
            Some(NativeIcon::PreferencesGeneral),
            None::<&str>,
        )?;

        let placeholder = IconMenuItem::with_native_icon(
            app,
            text::RECENT_EMPTY,
            false,
            Some(NativeIcon::MultipleDocuments),
            None::<&str>,
        )?;
        let slots = (0..recent::SLOTS)
            .map(|i| {
                IconMenuItem::with_id_and_native_icon(
                    app,
                    format!("{RECENT_PREFIX}{i}"),
                    // Replaced by the first refresh; a slot is only ever in the
                    // submenu once it has a clipping's text.
                    text::RECENT_EMPTY,
                    true,
                    Some(NativeIcon::MultipleDocuments),
                    None::<&str>,
                )
            })
            .collect::<tauri::Result<Vec<_>>>()?;
        let recent = Submenu::new_with_native_icon(
            app,
            text::RECENT,
            true,
            Some(NativeIcon::MultipleDocuments),
        )?;
        recent.append(&placeholder)?;

        let autostart = IconMenuItem::with_id_and_native_icon(
            app,
            ID_AUTOSTART,
            autostart_label(autostart_enabled),
            true,
            Some(NativeIcon::PreferencesGeneral),
            None::<&str>,
        )?;
        // Startup never waits for IPC. The refresh loop replaces this safe
        // default with the persisted daemon truth.
        let private_mode = IconMenuItem::with_id_and_native_icon(
            app,
            ID_PRIVATE_MODE,
            private_mode_label(false),
            true,
            Some(NativeIcon::LockUnlocked),
            None::<&str>,
        )?;
        let quit = IconMenuItem::with_id_and_native_icon(
            app,
            ID_QUIT,
            text::QUIT,
            true,
            Some(NativeIcon::StopProgress),
            None::<&str>,
        )?;

        let menu = Menu::with_items(
            app,
            &[
                &status,
                &PredefinedMenuItem::separator(app)?,
                &toggle,
                &settings,
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
            private_mode_state: Mutex::new(PrivateModeState::default()),
        })
    }

    /// The toolbar API cannot show both a checkmark and an image on macOS.
    /// We keep the state in the visible label and pair it with the relevant
    /// native icon so toggle rows do not become the only iconless tray actions.
    pub fn set_autostart(&self, enabled: bool) {
        let _ = self.autostart.set_text(autostart_label(enabled));
    }

    pub fn private_mode_enabled(&self) -> bool {
        self.private_mode_snapshot().0
    }

    pub fn private_mode_snapshot(&self) -> (bool, u64) {
        self.private_mode_state
            .lock()
            .map(|state| (state.enabled, state.revision))
            .unwrap_or((false, 0))
    }

    pub fn set_private_mode(&self, enabled: bool) {
        self.apply_private_mode(enabled, None);
    }

    pub fn reconcile_private_mode(&self, enabled: bool, revision: u64) -> bool {
        self.apply_private_mode(enabled, Some(revision))
    }

    fn apply_private_mode(&self, enabled: bool, expected_revision: Option<u64>) -> bool {
        let Ok(mut state) = self.private_mode_state.lock() else {
            return false;
        };
        let Some(changed) = state.apply(enabled, expected_revision) else {
            return false;
        };
        if !changed {
            return false;
        }
        drop(state);
        let _ = self.private_mode.set_text(private_mode_label(enabled));
        let icon = if enabled {
            NativeIcon::LockLocked
        } else {
            NativeIcon::LockUnlocked
        };
        let _ = self.private_mode.set_native_icon(Some(icon));
        true
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

        let (status, icon) = match backend.status().await {
            Ok(status) if status.capture_running => {
                (text::STATUS_CAPTURING, NativeIcon::StatusAvailable)
            }
            Ok(_) => (text::STATUS_STOPPED, NativeIcon::StatusPartiallyAvailable),
            Err(_) => (text::STATUS_OFFLINE, NativeIcon::StatusUnavailable),
        };
        let _ = self.status.set_text(status);
        let _ = self.status.set_native_icon(Some(icon));

        // The page carries plaintext, which is exactly why `Clipping` and not
        // this function decides what a menu may say about it.
        match backend.list(recent::FETCH, None).await {
            Ok(page) => self.show(&recent::menu_clippings(&page.items), text::RECENT_EMPTY),
            Err(_) => self.show(&[], text::RECENT_OFFLINE),
        }
        force_menu_image_visibility(app);
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

fn autostart_label(enabled: bool) -> &'static str {
    if enabled {
        text::OPEN_AT_LOGIN_ON
    } else {
        text::OPEN_AT_LOGIN_OFF
    }
}

fn private_mode_label(enabled: bool) -> &'static str {
    if enabled {
        text::PRIVATE_MODE_ON
    } else {
        text::PRIVATE_MODE_OFF
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

    #[test]
    fn toggle_rows_keep_their_state_in_their_accessible_label() {
        assert_eq!(autostart_label(true), text::OPEN_AT_LOGIN_ON);
        assert_eq!(autostart_label(false), text::OPEN_AT_LOGIN_OFF);
        assert_eq!(private_mode_label(true), text::PRIVATE_MODE_ON);
        assert_eq!(private_mode_label(false), text::PRIVATE_MODE_OFF);
    }

    #[test]
    fn a_confirmed_broadcast_invalidates_an_older_reconciliation() {
        let mut state = PrivateModeState::default();
        let stale_revision = state.revision;

        assert_eq!(state.apply(true, None), Some(true));
        assert_eq!(state.apply(false, Some(stale_revision)), None);
        assert!(state.enabled);
    }
}
