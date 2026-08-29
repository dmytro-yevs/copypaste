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

    #[cfg(test)]
    pub(super) fn close_from_user_for_test(&self) {
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

fn with_accessible_property_services<T>(
    callback: impl FnOnce(&::windows::Win32::UI::Accessibility::IAccPropServices) -> T,
) -> Option<T> {
    use ::windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use ::windows::Win32::UI::Accessibility::{CLSID_AccPropServices, IAccPropServices};

    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let service = match unsafe {
        CoCreateInstance::<_, IAccPropServices>(&CLSID_AccPropServices, None, CLSCTX_INPROC_SERVER)
    } {
        Ok(service) => service,
        Err(_) => {
            if initialized {
                unsafe { CoUninitialize() };
            }
            return None;
        }
    };
    let result = callback(&service);
    if initialized {
        unsafe { CoUninitialize() };
    }
    Some(result)
}

pub(super) fn set_accessible_name(hwnd: &winsafe::HWND, name: &str) -> bool {
    use ::windows::core::PCWSTR;
    use ::windows::Win32::Foundation::HWND as WindowsHwnd;
    use ::windows::Win32::UI::Accessibility::Name_Property_GUID;
    use ::windows::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, OBJID_CLIENT};

    let mut name16: Vec<u16> = name.encode_utf16().collect();
    name16.push(0);
    with_accessible_property_services(|service| unsafe {
        service
            .SetHwndPropStr(
                WindowsHwnd(hwnd.ptr()),
                OBJID_CLIENT.0 as u32,
                CHILDID_SELF,
                Name_Property_GUID,
                PCWSTR::from_raw(name16.as_ptr()),
            )
            .is_ok()
    })
    .unwrap_or(false)
}

pub(super) fn clear_accessible_name(hwnd: &winsafe::HWND) -> bool {
    use ::windows::Win32::Foundation::HWND as WindowsHwnd;
    use ::windows::Win32::UI::Accessibility::Name_Property_GUID;
    use ::windows::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, OBJID_CLIENT};

    let properties = [Name_Property_GUID];
    with_accessible_property_services(|service| unsafe {
        service
            .ClearHwndProps(
                WindowsHwnd(hwnd.ptr()),
                OBJID_CLIENT.0 as u32,
                CHILDID_SELF,
                &properties,
            )
            .is_ok()
    })
    .unwrap_or(false)
}

pub(super) fn hide(hwnd: &winsafe::HWND) {
    hwnd.ShowWindow(co::SW::HIDE);
}

pub(super) fn show(hwnd: &winsafe::HWND) {
    hwnd.ShowWindow(co::SW::SHOW);
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

    #[test]
    fn accessible_names_are_annotated_without_value_access() {
        let source = include_str!("common.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("SetHwndPropStr"));
        assert!(source.contains("ClearHwndProps"));
        assert!(source.contains("Name_Property_GUID"));
        assert!(!source.contains("ValuePattern"));
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
