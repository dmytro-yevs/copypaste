//! A message-only clipboard listener. The sequence number remains authoritative.

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows_sys::Win32::System::Threading::{CreateEventW, GetCurrentThreadId, SetEvent, INFINITE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjects, PeekMessageW,
    PostThreadMessageW, HWND_MESSAGE, MSG, PM_REMOVE, QS_ALLINPUT, WM_CLIPBOARDUPDATE, WM_QUIT,
};

use super::super::ChangeSignal;

pub(super) struct ClipboardNotifications {
    signal: Arc<ChangeSignal>,
    stopping: Arc<AtomicBool>,
    // Owned until the worker joins. Store as usize so the owner remains Send.
    stop_event: usize,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl ClipboardNotifications {
    pub(super) fn start() -> std::io::Result<Self> {
        let stop_event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if stop_event.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let stop_event = stop_event as usize;
        let signal = Arc::new(ChangeSignal::new());
        let worker_signal = Arc::clone(&signal);
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = match thread::Builder::new()
            .name("clipboard-notifications".into())
            .spawn(move || unsafe {
                // STATIC is a built-in window class; no registration or global
                // callback state is needed for posted clipboard messages.
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let window = CreateWindowExW(
                    0, class.as_ptr(), ptr::null(), 0, 0, 0, 0, 0,
                    HWND_MESSAGE, ptr::null_mut(), ptr::null_mut(), ptr::null(),
                );
                if window.is_null() {
                    let _ = ready_tx.send(Err(std::io::Error::last_os_error()));
                    return;
                }
                if AddClipboardFormatListener(window) == 0 {
                    let error = std::io::Error::last_os_error();
                    DestroyWindow(window);
                    let _ = ready_tx.send(Err(error));
                    return;
                }
                let _ = ready_tx.send(Ok(GetCurrentThreadId()));
                let mut message: MSG = std::mem::zeroed();
                let handle = stop_event as *mut std::ffi::c_void;
                'pump: while !worker_stopping.load(Ordering::Acquire) {
                    let result = MsgWaitForMultipleObjects(1, &handle, 0, INFINITE, QS_ALLINPUT);
                    if result == WAIT_OBJECT_0 {
                        break;
                    }
                    if result != WAIT_OBJECT_0 + 1 {
                        tracing::warn!(error = %std::io::Error::last_os_error(), "clipboard notification wait failed");
                        break;
                    }
                    while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        if worker_stopping.load(Ordering::Acquire) || message.message == WM_QUIT {
                            break 'pump;
                        }
                        if message.message == WM_CLIPBOARDUPDATE {
                            worker_signal.notify.notify_one();
                        } else {
                            DispatchMessageW(&message);
                        }
                    }
                }
                RemoveClipboardFormatListener(window);
                DestroyWindow(window);
                worker_signal.available.store(false, Ordering::Release);
                worker_signal.notify.notify_one();
            }) {
            Ok(thread) => thread,
            Err(error) => {
                unsafe { CloseHandle(stop_event as *mut std::ffi::c_void) };
                return Err(error);
            }
        };
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                signal,
                stopping,
                stop_event,
                thread_id,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                unsafe { CloseHandle(stop_event as *mut std::ffi::c_void) };
                match result {
                    Ok(Err(error)) => Err(error),
                    _ => Err(std::io::Error::other(
                        "clipboard notification thread exited during startup",
                    )),
                }
            }
        }
    }

    pub(super) fn signal(&self) -> Arc<ChangeSignal> {
        Arc::clone(&self.signal)
    }
}

impl Drop for ClipboardNotifications {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if unsafe { SetEvent(self.stop_event as *mut std::ffi::c_void) } == 0 {
            tracing::error!(error = %std::io::Error::last_os_error(), "cannot stop clipboard notification thread");
            if unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) } == 0 {
                // Do not block shutdown if both independent OS wakeups fail.
                // The worker owns its other state, and the still-open event
                // remains valid until process exit.
                tracing::error!(error = %std::io::Error::last_os_error(), "cannot wake clipboard notification thread");
                self.thread.take();
                return;
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        unsafe { CloseHandle(self.stop_event as *mut std::ffi::c_void) };
    }
}
