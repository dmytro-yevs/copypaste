use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tracing::warn;
use winsafe::{co, gui, prelude::*};

use crate::pairing_presentation::NativeAbort;

#[derive(Clone)]
pub(super) struct CloseHandle {
    window: gui::WindowMain,
    programmatic: Arc<AtomicBool>,
}

impl CloseHandle {
    pub(super) fn new(window: gui::WindowMain) -> Self {
        Self {
            window,
            programmatic: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn programmatic_flag(&self) -> Arc<AtomicBool> {
        self.programmatic.clone()
    }

    pub(super) fn close(&self) {
        self.programmatic.store(true, Ordering::Release);
        if self.window.hwnd().IsWindow() {
            self.window.close();
        }
    }
}

pub(super) fn abort_if_user_dismissed(programmatic: &AtomicBool, abort: &NativeAbort) {
    if !programmatic.swap(true, Ordering::AcqRel) {
        abort();
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

/// Whether the session accepted a window's display affinity.
///
/// Injected rather than called in place because the answer is a property of the
/// session, not of the code: a test cannot make a real compositor refuse, and
/// the refusal branch is the one that keeps a pairing secret off a screen
/// recording.
pub(super) type Affinity = fn(&winsafe::HWND) -> bool;

pub(super) fn system_affinity(hwnd: &winsafe::HWND) -> bool {
    hwnd.SetWindowDisplayAffinity(co::WDA::EXCLUDEFROMCAPTURE)
        .is_ok()
}

/// INV-35 in Windows spelling, and it is asked rather than assumed.
///
/// `WDA_EXCLUDEFROMCAPTURE` needs Windows 10 2004 or later and can be refused
/// by the session — a remote desktop is the ordinary case. A window that shows
/// a pairing secret while the compositor declined to keep it out of screen
/// captures claims a protection it does not have, so a caller that gets `false`
/// here clears what it was about to show and closes instead. Called from
/// `WM_CREATE`, before the window is shown.
pub(super) fn protect_from_capture(hwnd: &winsafe::HWND, affinity: Affinity) -> bool {
    if affinity(hwnd) {
        return true;
    }
    // INV-13: the refusal, never the code, the invite or the peer.
    warn!("Windows refused screenshot protection for a pairing window; it was not shown");
    false
}

#[cfg(test)]
mod tests {
    use winsafe::prelude::Handle;

    use super::*;

    #[test]
    fn refusal_returns_false() {
        assert!(!protect_from_capture(&winsafe::HWND::NULL, |_| false));
    }

    #[test]
    fn acceptance_returns_true() {
        assert!(protect_from_capture(&winsafe::HWND::NULL, |_| true));
    }

    /// The window whose affinity is being set is the one that was asked about;
    /// a guard that protected a different window would still return `true`.
    #[test]
    fn the_window_reaches_the_affinity_call() {
        static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn record(hwnd: &winsafe::HWND) -> bool {
            SEEN.store(
                *hwnd == winsafe::HWND::NULL,
                std::sync::atomic::Ordering::Release,
            );
            true
        }
        assert!(protect_from_capture(&winsafe::HWND::NULL, record));
        assert!(SEEN.load(std::sync::atomic::Ordering::Acquire));
    }
}
