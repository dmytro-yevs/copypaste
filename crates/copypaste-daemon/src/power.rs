//! System resume notifications for the runtime's deadline task.
//!
//! Tokio timers run on a clock that may stop while the system sleeps, so a
//! deadline that came due during sleep would otherwise fire late. Each desktop
//! OS reports resume natively: macOS through the workspace wake notification,
//! Windows through `PowerRegisterSuspendResumeNotification`, Linux through
//! logind's `PrepareForSleep(false)`. Each one sweeps the deadlines. When no
//! subscription is possible the runtime caps short sleeps instead.

use std::sync::OnceLock;

use copypaste_runtime::ResumeWaker;

static WAKER: OnceLock<ResumeWaker> = OnceLock::new();

/// Called from a native resume notification, on any thread.
pub(crate) fn resumed() {
    if let Some(waker) = WAKER.get() {
        waker.resumed();
    }
}

/// Subscribe to resume notifications once. Answers whether one is active.
/// May block briefly while the subscription is set up.
pub(crate) fn install(waker: ResumeWaker) -> bool {
    if WAKER.set(waker).is_err() {
        return true;
    }
    platform::subscribe()
}

#[cfg(target_os = "macos")]
mod platform {
    /// The workspace observer, installed before the daemon runtime, already
    /// forwards the wake notification.
    pub(super) fn subscribe() -> bool {
        crate::macos_workspace::observes_wake()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
    };

    unsafe extern "system" fn on_power(
        _context: *const c_void,
        event: u32,
        _setting: *const c_void,
    ) -> u32 {
        if event == PBT_APMRESUMEAUTOMATIC || event == PBT_APMRESUMESUSPEND {
            super::resumed();
        }
        ERROR_SUCCESS
    }

    /// A callback registration, because the daemon's message-only windows
    /// never receive the broadcast `WM_POWERBROADCAST`.
    pub(super) fn subscribe() -> bool {
        // Registered for the life of the process, so the parameters live as
        // long and the registration handle is never closed.
        let parameters: &'static mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS =
            Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
                Callback: Some(on_power),
                Context: std::ptr::null_mut(),
            }));
        let mut registration: *mut c_void = std::ptr::null_mut();
        // SAFETY: `parameters` is valid for the rest of the process and the
        // callback only touches thread-safe state.
        let status = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                std::ptr::from_mut(parameters).cast(),
                &mut registration,
            )
        };
        if status != ERROR_SUCCESS {
            tracing::warn!(status, "could not subscribe to system resume");
        }
        status == ERROR_SUCCESS
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::sync::mpsc;
    use std::time::Duration;

    use zbus::blocking::{Connection, Proxy};

    const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

    /// One thread parked on logind's `PrepareForSleep`; zbus blocking runs
    /// its own executor and must stay off the Tokio threads.
    pub(super) fn subscribe() -> bool {
        let (ready, subscribed) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("resume-signal".into())
            .spawn(move || {
                let subscription = Connection::system().and_then(|connection| {
                    let proxy = Proxy::new(
                        &connection,
                        "org.freedesktop.login1",
                        "/org/freedesktop/login1",
                        "org.freedesktop.login1.Manager",
                    )?;
                    let signals = proxy.receive_signal("PrepareForSleep")?;
                    Ok((connection, proxy, signals))
                });
                let (_connection, _proxy, signals) = match subscription {
                    Ok(subscription) => {
                        let _ = ready.send(true);
                        subscription
                    }
                    Err(error) => {
                        tracing::warn!(%error, "could not subscribe to logind resume");
                        let _ = ready.send(false);
                        return;
                    }
                };
                for message in signals {
                    if message
                        .body()
                        .deserialize::<bool>()
                        .is_ok_and(|sleeping| !sleeping)
                    {
                        super::resumed();
                    }
                }
            });
        spawned.is_ok() && subscribed.recv_timeout(SUBSCRIBE_TIMEOUT).unwrap_or(false)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    pub(super) fn subscribe() -> bool {
        false
    }
}
