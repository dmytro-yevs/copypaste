//! Which application put this on the clipboard, asked of Win32.
//!
//! Settings use the source as an exclusion key. Nothing here is collected for
//! its own sake, and the window
//! title is never read: it is user content, and a title on every history row is
//! a second copy of what the user was looking at.
//!
//! `GetClipboardOwner` names the window that wrote the data, which is the
//! question actually asked. A writer without an owner window is unknown, not
//! the foreground application: a non-empty exclusion list must fail closed
//! before clipboard data is read. What is done with the answer is
//! [`super::super::windows_attribution`], which runs on every host.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_INSUFFICIENT_BUFFER, HANDLE, HWND,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use super::super::windows_attribution::SourceApp;
use super::WindowsClipboard;

/// `MAX_PATH` covers almost every process; the retry is at the documented
/// ceiling for a long path, and happens only for the one error that a larger
/// buffer can fix.
const PATH_ATTEMPTS: [usize; 2] = [260, 32_768];

impl WindowsClipboard {
    pub(super) fn source_app(&mut self, change: i64) -> Option<SourceApp> {
        self.attribution.for_change(change, resolve)
    }
}

fn resolve() -> Option<SourceApp> {
    // Both crates spell a window handle `*mut core::ffi::c_void`.
    let owner = clipboard_win::raw::get_owner().map(|owner| owner.as_ptr());
    source_app_for_owner(owner)
}

fn source_app_for_owner(owner: Option<HWND>) -> Option<SourceApp> {
    // A missing owner is deliberately not attributed to the foreground
    // application. Foreground state describes the poll, not the clipboard
    // writer, and would let an excluded writer bypass the fail-closed gate.
    let window = owner?;
    let path = image_path(process_id(window)?)?;
    SourceApp::from_image_path(&path.to_string_lossy())
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
        // A 32 KiB allocation cannot answer `ERROR_ACCESS_DENIED` or an exited
        // process, and retrying one is how a per-tick syscall becomes two.
        if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ownerless_clipboard_has_no_foreground_source_fallback() {
        assert!(source_app_for_owner(None).is_none());
    }
}
