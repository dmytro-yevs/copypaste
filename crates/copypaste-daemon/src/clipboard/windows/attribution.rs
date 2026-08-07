//! Which application put this on the clipboard.
//!
//! Manifest 07 wants the source as a sensitivity signal — a credential store's
//! copy is sensitive whatever its text looks like — and the settings want it as
//! an exclusion key. Nothing here is collected for its own sake, and the window
//! title is never read: it is user content, and a title on every history row is
//! a second copy of what the user was looking at.
//!
//! `GetClipboardOwner` names the window that wrote the data, which is the
//! question actually asked; the foreground window is the fallback for a writer
//! that opened the clipboard with no owner window.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::time::{Duration, Instant};

use tracing::{info, warn};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use super::WindowsClipboard;

/// Bounds how stale an exclusion decision can be, as on macOS.
const CACHE_FOR: Duration = Duration::from_millis(750);

/// Most process paths fit; `QueryFullProcessImageNameW` is retried once at the
/// documented ceiling for the ones that do not.
const PATH_ATTEMPTS: [usize; 2] = [260, 32_768];

#[derive(Clone)]
pub(super) struct SourceApp {
    /// The process image file name, lowercased: `1password.exe`.
    ///
    /// Windows has no bundle identifier. The file name is the only stable
    /// identity a process reliably has, and lowercasing it is what makes it
    /// comparable — `copypaste_core::sensitive::is_password_manager_app`
    /// lowercases before matching, and the exclusion list compares exactly.
    ///
    /// The *path* is discarded here rather than later: it contains the local
    /// user name (I-9), and this value is about to be stored on an item.
    pub(super) id: String,
    /// The same name without its extension, for display.
    pub(super) name: String,
}

impl WindowsClipboard {
    pub(super) fn source_app(&mut self) -> Option<SourceApp> {
        if let Some((seen, app)) = &self.source_cache {
            if seen.elapsed() < CACHE_FOR {
                return app.clone();
            }
        }
        let app = resolve();
        self.source_cache = Some((Instant::now(), app.clone()));
        app
    }

    /// Said once per transition, not once per poll: an exclusion list that
    /// silently matches nothing is the failure this makes visible.
    pub(super) fn note_attribution(&mut self, app: Option<&SourceApp>) {
        let available = app.is_some();
        if self.attribution_available == Some(available) {
            return;
        }
        self.attribution_available = Some(available);
        if available {
            info!("Windows capture source attribution is available");
        } else {
            warn!("Windows could not identify the source application for clipboard capture");
        }
    }
}

fn resolve() -> Option<SourceApp> {
    // Both crates spell a window handle `*mut core::ffi::c_void`.
    let window = clipboard_win::raw::get_owner()
        .map(|owner| owner.as_ptr())
        .or_else(foreground_window)?;
    let path = image_path(process_id(window)?)?;
    // `file_name`, never the path.
    let file_name = Path::new(&path).file_name()?.to_str()?.to_owned();
    let name = Path::new(&file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&file_name)
        .to_owned();
    Some(SourceApp {
        id: file_name.to_lowercase(),
        name,
    })
}

fn foreground_window() -> Option<HWND> {
    let window = unsafe { GetForegroundWindow() };
    (!window.is_null()).then_some(window)
}

fn process_id(window: HWND) -> Option<u32> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    (pid != 0).then_some(pid)
}

fn image_path(pid: u32) -> Option<OsString> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        // Ordinary rather than exceptional: an elevated process cannot be
        // opened by an unelevated daemon, and its copies are then attributed to
        // nothing at all — which a non-empty exclusion list treats as excluded.
        return None;
    }
    let path = query_image_path(process);
    unsafe { CloseHandle(process) };
    path
}

fn query_image_path(process: HANDLE) -> Option<OsString> {
    for capacity in PATH_ATTEMPTS {
        let mut buffer = vec![0u16; capacity];
        let mut length = capacity as u32;
        let queried = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                buffer.as_mut_ptr(),
                &mut length,
            )
        };
        if queried != 0 {
            return Some(OsString::from_wide(&buffer[..length as usize]));
        }
    }
    None
}
