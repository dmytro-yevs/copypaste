use std::sync::{Mutex, MutexGuard};

use tracing::warn;
use winsafe::{co, gui, prelude::*};

#[derive(Clone)]
pub(super) struct CloseHandle(gui::WindowMain);

impl CloseHandle {
    pub(super) fn new(window: gui::WindowMain) -> Self {
        Self(window)
    }

    pub(super) fn close(&self) {
        if self.0.hwnd().IsWindow() {
            self.0.close();
        }
    }
}

static UI_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn lock_ui() -> Option<MutexGuard<'static, ()>> {
    UI_LOCK.lock().ok()
}

pub(super) fn window(title: &str, size: (i32, i32)) -> gui::WindowMain {
    gui::WindowMain::new(gui::WindowMainOpts {
        title,
        size: gui::dpi(size.0, size.1),
        ..Default::default()
    })
}

pub(super) fn label(
    parent: &(impl GuiParent + 'static),
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> gui::Label {
    gui::Label::new(
        parent,
        gui::LabelOpts {
            text,
            position: gui::dpi(position.0, position.1),
            size: gui::dpi(size.0, size.1),
            ..Default::default()
        },
    )
}

pub(super) fn button(
    parent: &(impl GuiParent + 'static),
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
    ctrl_id: u16,
) -> gui::Button {
    gui::Button::new(
        parent,
        gui::ButtonOpts {
            text,
            position: gui::dpi(position.0, position.1),
            width: gui::dpi_x(size.0),
            height: gui::dpi_y(size.1),
            ctrl_id,
            ..Default::default()
        },
    )
}

pub(super) fn hide(hwnd: &winsafe::HWND) {
    hwnd.ShowWindow(co::SW::HIDE);
}

/// INV-35 in Windows spelling, and it is asked rather than assumed.
///
/// `WDA_EXCLUDEFROMCAPTURE` needs Windows 10 2004 or later and can be refused
/// by the session — a remote desktop is the ordinary case. A window that shows
/// a pairing secret while the compositor declined to keep it out of screen
/// captures claims a protection it does not have, so a caller that gets `false`
/// here clears what it was about to show and closes instead. Called from
/// `WM_CREATE`, before the window is shown.
pub(super) fn protect_from_capture(hwnd: &winsafe::HWND) -> bool {
    if hwnd
        .SetWindowDisplayAffinity(co::WDA::EXCLUDEFROMCAPTURE)
        .is_ok()
    {
        return true;
    }
    // INV-13: the refusal, never the code, the invite or the peer.
    warn!("Windows refused screenshot protection for a pairing window; it was not shown");
    false
}
