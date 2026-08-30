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

#[allow(unsafe_code)]
fn with_accessible_property_services<T>(
    callback: impl FnOnce(&::windows::Win32::UI::Accessibility::IAccPropServices) -> T,
) -> Option<T> {
    use ::windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
    use ::windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use ::windows::Win32::UI::Accessibility::{CLSID_AccPropServices, IAccPropServices};

    // SAFETY: this function runs on the owned pairing UI thread; S_OK/S_FALSE
    // initialization is balanced below, while RPC_E_CHANGED_MODE explicitly
    // permits use of the already-initialized apartment without balancing it.
    let initialization = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let initialized = initialization == S_OK || initialization == S_FALSE;
    if !initialized && initialization != RPC_E_CHANGED_MODE {
        return None;
    }
    let result = {
        // SAFETY: CoCreateInstance returns an owned interface whose lexical
        // scope ends before CoUninitialize below.
        let service = match unsafe {
            CoCreateInstance::<_, IAccPropServices>(
                &CLSID_AccPropServices,
                None,
                CLSCTX_INPROC_SERVER,
            )
        } {
            Ok(service) => service,
            Err(_) => {
                if initialized {
                    // SAFETY: no interface was created on this path; this
                    // balances the successful apartment initialization.
                    unsafe { CoUninitialize() };
                }
                return None;
            }
        };
        callback(&service)
    };
    if initialized {
        // SAFETY: the service interface was dropped at the end of `result`.
        unsafe { CoUninitialize() };
    }
    Some(result)
}

#[allow(unsafe_code)]
pub(super) fn set_accessible_name(hwnd: &winsafe::HWND, name: &str) -> bool {
    use ::windows::core::PCWSTR;
    use ::windows::Win32::Foundation::HWND as WindowsHwnd;
    use ::windows::Win32::UI::Accessibility::Name_Property_GUID;
    use ::windows::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, OBJID_CLIENT};

    let mut name16: Vec<u16> = name.encode_utf16().collect();
    name16.push(0);
    with_accessible_property_services(|service| unsafe {
        // SAFETY: the caller passes a live, owned Edit HWND during WM_CREATE;
        // the NUL-terminated UTF-16 buffer and static property GUID live for
        // the duration of this synchronous call.
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

#[allow(unsafe_code)]
pub(super) fn clear_accessible_name(hwnd: &winsafe::HWND) -> bool {
    use ::windows::Win32::Foundation::HWND as WindowsHwnd;
    use ::windows::Win32::UI::Accessibility::Name_Property_GUID;
    use ::windows::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, OBJID_CLIENT};

    let properties = [Name_Property_GUID];
    with_accessible_property_services(|service| unsafe {
        // SAFETY: the caller passes the still-live Edit HWND before destroy;
        // the property slice is stack-owned and borrowed only for this call.
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

    fn production_source(source: &'static str) -> &'static str {
        source
            .split_once("\n#[cfg(test)]\nmod tests {")
            .map_or("", |(production, _)| production)
    }

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
        let source = production_source(include_str!("common.rs"));
        assert!(source.contains("SetHwndPropStr"));
        assert!(source.contains("ClearHwndProps"));
        assert!(source.contains("Name_Property_GUID"));
        assert!(!source.contains("ValuePattern"));
        assert_eq!(source.matches("#[allow(unsafe_code)]").count(), 3);
        assert!(source.contains("RPC_E_CHANGED_MODE"));
        assert!(source.contains("S_OK") && source.contains("S_FALSE"));
    }

    #[test]
    fn accessible_service_is_dropped_before_com_uninitializes() {
        let source = production_source(include_str!("common.rs"));
        let service_scope = source
            .split_once("let result = {")
            .unwrap()
            .1
            .split_once("\n    if initialized {")
            .unwrap()
            .0;
        assert!(service_scope.contains("let service = match"));
        assert!(service_scope.contains("callback(&service)"));
        assert!(service_scope.trim_end().ends_with("};"));
    }

    #[test]
    fn production_source_ignores_method_cfg_and_fails_closed_without_module_gate() {
        let fixture = concat!(
            "impl Type {\n",
            "    #[cfg(test)]\n",
            "    fn test_helper() {}\n",
            "}\n",
            "\n",
            "pub fn production() {}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    #[test]\n",
            "    fn regression() {}\n",
            "}\n",
        );
        let production = production_source(fixture);
        assert!(production.contains("fn test_helper()"));
        assert!(production.contains("pub fn production()"));
        assert!(!production.contains("fn regression()"));
        assert!(
            production_source("pub fn production() {}\n#[cfg(test)]\nfn helper() {}").is_empty()
        );
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
