//! macOS CGEventTap — intercepts system-wide keyDown events to support
//! overriding OS-reserved shortcuts (e.g. Cmd+Space) for the popup toggle.
//!
//! The tap runs on a dedicated background thread, calls the provided callback
//! when the configured accelerator is pressed, and *swallows* the event
//! (returns `CallbackResult::Drop`) so the OS shortcut action does not fire.
//!
//! Requirements:
//!   * Accessibility permission: `AXIsProcessTrusted()` must return true.
//!     If false the tap cannot be installed; we fall back to
//!     `tauri-plugin-global-shortcut` which handles non-reserved combos.
//!   * The process must run a CFRunLoop on the thread that owns the tap;
//!     we spin one dedicated background thread for this.
//!
//! # Thread safety
//! `CURRENT_ACCEL` is a `Mutex<String>` written by the Tauri command thread
//! and read by the tap callback on every key event.
//!
//! # Shortcut recording
//! `start_recording` / `stop_recording` install a *separate* one-shot HID tap
//! at `kCGHIDEventTapLocation` — below the Hammerspoon session tap — that
//! captures the next raw keyDown (keycode + flags) and converts it to a Tauri
//! accelerator string.  This lets the recorder see physical keys even when
//! Hammerspoon has remapped them.

#![allow(non_upper_case_globals)]

use std::sync::{Mutex, OnceLock};

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};

// ---------------------------------------------------------------------------
// Popup-tap lifecycle handle (idempotency + teardown)
// ---------------------------------------------------------------------------

/// Owns the run loop of the live popup tap thread so installation can be made
/// idempotent and the tap can be torn down (which releases the `CGEventTap`,
/// its `CFRunLoopSource`, the underlying `CFMachPort`, and the boxed callback).
///
/// `CFRunLoop` is `Send + Sync`; stopping it from another thread makes the tap
/// thread's `CFRunLoop::run_current()` return, after which that thread drops
/// the stack-owned tap + source (the crate's `Drop for CGEventTap` invalidates
/// the `CFMachPort`).
struct TapHandle {
    /// The run loop that the tap thread is parked on.
    run_loop: CFRunLoop,
}

// SAFETY: `CFRunLoop` is declared `Send + Sync` by core-foundation, and we only
// ever call `stop()` on it from another thread, which CFRunLoopStop supports.
unsafe impl Send for TapHandle {}

/// Slot holding the live popup tap, if any. `Some` ⇒ a tap thread is running.
static TAP_HANDLE: OnceLock<Mutex<Option<TapHandle>>> = OnceLock::new();

fn tap_handle() -> &'static Mutex<Option<TapHandle>> {
    TAP_HANDLE.get_or_init(|| Mutex::new(None))
}

// ---------------------------------------------------------------------------
// Shared state — popup tap
// ---------------------------------------------------------------------------

/// The current accelerator string the tap should fire on.
static CURRENT_ACCEL: OnceLock<Mutex<String>> = OnceLock::new();

fn current_accel() -> &'static Mutex<String> {
    CURRENT_ACCEL.get_or_init(|| Mutex::new(String::new()))
}

/// Callback invoked (on the tap thread) when the shortcut fires.
type Callback = Box<dyn Fn() + Send + 'static>;
static CALLBACK: OnceLock<Mutex<Option<Callback>>> = OnceLock::new();

fn global_callback() -> &'static Mutex<Option<Callback>> {
    CALLBACK.get_or_init(|| Mutex::new(None))
}

// ---------------------------------------------------------------------------
// Shared state — recording tap
// ---------------------------------------------------------------------------

/// When `Some`, the recorder is active and the closure will receive the next
/// captured accelerator string then set the slot back to `None`.
type RecordCallback = Box<dyn FnOnce(String) + Send + 'static>;
static RECORD_CB: OnceLock<Mutex<Option<RecordCallback>>> = OnceLock::new();

fn record_cb() -> &'static Mutex<Option<RecordCallback>> {
    RECORD_CB.get_or_init(|| Mutex::new(None))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check whether the Accessibility permission is granted.
pub fn accessibility_granted() -> bool {
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    // SAFETY: AXIsProcessTrusted is a pure C function with no preconditions.
    unsafe { AXIsProcessTrusted() }
}

/// Open the macOS System Settings pane for Accessibility.
pub fn open_accessibility_settings() {
    let url = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
    let _ = std::process::Command::new("open").arg(url).spawn();
}

/// Update the shortcut accelerator that the tap should intercept.
/// Safe to call from any thread at any time — the tap picks it up on the
/// next key event.
pub fn update_tap_shortcut(accel: &str) {
    let mut guard = current_accel().lock().expect("mutex poisoned");
    *guard = accel.to_owned();
}

/// Start recording mode: installs a one-shot HID-level tap that captures the
/// next key chord (excluding bare modifiers) and delivers it as a Tauri
/// accelerator string to `on_capture`.
///
/// The tap is placed at `kCGHIDEventTapLocation` — below Hammerspoon's session
/// tap — so it sees raw physical keycodes before any remapping.
///
/// The captured event is *dropped* (swallowed) so it doesn't reach the app.
///
/// # Permissions required
/// Accessibility (`AXIsProcessTrusted()`) **and** Input Monitoring
/// (`kTCCServiceListenEvent`) must both be granted.  Without them
/// `CGEventTap::new` returns `Err(())`.
///
/// # Keys macOS will not deliver
/// Some hardware-level combos (e.g. Ctrl+Cmd+Q, or the Touch ID button) are
/// handled by the kernel/security layer before any HID tap and cannot be
/// captured.
pub fn start_recording(on_capture: impl FnOnce(String) + Send + 'static) -> Result<(), String> {
    if !accessibility_granted() {
        return Err("Accessibility permission not granted".into());
    }

    // Overwrite any previous pending recording callback.
    {
        let mut guard = record_cb().lock().expect("mutex poisoned");
        *guard = Some(Box::new(on_capture));
    }

    std::thread::Builder::new()
        .name("cgeventtap-recorder".into())
        .spawn(recording_tap_thread)
        .map_err(|e| format!("spawn recorder thread: {e}"))?;

    Ok(())
}

/// Cancel an in-progress recording session (no-op if not recording).
pub fn stop_recording() {
    let mut guard = record_cb().lock().expect("mutex poisoned");
    *guard = None;
}

/// Install the CGEventTap on a dedicated background thread.
///
/// `on_trigger` is called (on the tap thread) when the configured shortcut
/// fires.  Returns `Err` if Accessibility permission is not granted.
///
/// Idempotent: only one tap is ever installed.  Subsequent calls update the
/// callback and shortcut in place but do **not** spawn a second tap thread or
/// create a second `CGEventTap`/`CFRunLoopSource` (which would leak the old
/// ones, since each tap thread parks on an immortal run loop).  To replace the
/// live tap, call [`uninstall`] first.
pub fn install(initial_accel: &str, on_trigger: impl Fn() + Send + 'static) -> Result<(), String> {
    if !accessibility_granted() {
        return Err("Accessibility permission not granted".into());
    }

    update_tap_shortcut(initial_accel);
    {
        let mut cb = global_callback().lock().expect("mutex poisoned");
        *cb = Some(Box::new(on_trigger));
    }

    // Idempotency guard: if a tap thread is already live, we have just refreshed
    // its callback + shortcut above — do not spawn another one.
    if tap_already_live() {
        return Ok(());
    }

    std::thread::Builder::new()
        .name("cgeventtap-runloop".into())
        .spawn(tap_thread_main)
        .map_err(|e| format!("spawn tap thread: {e}"))?;

    Ok(())
}

/// Whether a popup tap thread is currently live (its run loop is parked and the
/// `CGEventTap` + `CFRunLoopSource` are held by that thread).
fn tap_already_live() -> bool {
    tap_handle().lock().expect("mutex poisoned").is_some()
}

/// Tear down the live popup tap, if any.
///
/// Stops the tap thread's run loop, which causes `tap_thread_main` to return
/// and drop the stack-owned `CGEventTap` (the crate's `Drop` invalidates the
/// `CFMachPort`) and its `CFRunLoopSource`.  Also reclaims the boxed trigger
/// callback so it is freed.  No-op if no tap is installed.
pub fn uninstall() {
    // Take the handle out first so a concurrent `install` sees "not live".
    let handle = tap_handle().lock().expect("mutex poisoned").take();
    if let Some(h) = handle {
        // Waking + stopping the run loop unblocks `CFRunLoop::run_current()`
        // on the tap thread, after which it removes the source and drops the
        // tap (releasing the CFMachPort) before exiting.
        h.run_loop.stop();
    }
    // Reclaim the boxed callback regardless, so it does not outlive the tap.
    if let Ok(mut cb) = global_callback().lock() {
        *cb = None;
    }
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

fn tap_thread_main() {
    // kCGSessionEventTapLocation intercepts events at the session level —
    // before they reach any application, which is what allows swallowing
    // OS-reserved combos such as Cmd+Space.
    let result = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::KeyDown],
        |_proxy, _etype, event| {
            let accel = {
                let g = current_accel().lock().expect("mutex poisoned");
                g.clone()
            };
            if accel.is_empty() {
                return CallbackResult::Keep;
            }
            let Some((target_flags, target_kc)) = parse_accelerator(&accel) else {
                return CallbackResult::Keep;
            };

            let event_kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let event_flags = event.get_flags();

            // Mask to only the four standard modifier bits.
            let modifier_mask = CGEventFlags::CGEventFlagCommand
                | CGEventFlags::CGEventFlagControl
                | CGEventFlags::CGEventFlagAlternate
                | CGEventFlags::CGEventFlagShift;
            let event_mods = event_flags & modifier_mask;
            let target_mods = target_flags & modifier_mask;

            if event_kc == target_kc && event_mods == target_mods {
                if let Ok(cb_guard) = global_callback().lock() {
                    if let Some(ref cb) = *cb_guard {
                        cb();
                    }
                }
                // Swallow the event so the OS action does not fire.
                return CallbackResult::Drop;
            }

            CallbackResult::Keep
        },
    );

    match result {
        Err(()) => {
            tracing::error!("CGEventTap::new failed — Accessibility permission revoked?");
        }
        Ok(tap) => {
            let Ok(source) = tap.mach_port().create_runloop_source(0) else {
                tracing::error!("CGEventTap: create_runloop_source failed");
                // `tap` drops here, invalidating the CFMachPort — nothing leaks.
                return;
            };
            let rl = CFRunLoop::get_current();
            rl.add_source(&source, unsafe { kCFRunLoopCommonModes });
            tap.enable();

            // Publish our run loop so `uninstall` can stop it (idempotency +
            // teardown).  If a tap is somehow already registered, bail out and
            // let our local tap/source drop rather than overwrite + leak it.
            {
                let mut slot = tap_handle().lock().expect("mutex poisoned");
                if slot.is_some() {
                    tracing::warn!("CGEventTap: a tap is already live; abandoning duplicate");
                    rl.remove_source(&source, unsafe { kCFRunLoopCommonModes });
                    return;
                }
                *slot = Some(TapHandle {
                    run_loop: rl.clone(),
                });
            }

            // Parks here until `uninstall` (or revoked permission) stops the loop.
            CFRunLoop::run_current();

            // Run loop stopped: remove the source and clear the published handle.
            // `tap` (and `source`) drop at end of scope; the crate's
            // `Drop for CGEventTap` invalidates the underlying CFMachPort.
            rl.remove_source(&source, unsafe { kCFRunLoopCommonModes });
            *tap_handle().lock().expect("mutex poisoned") = None;
            tracing::debug!("CGEventTap run loop exited; tap + source released");
        }
    }
}

/// One-shot recording tap at HID level (below Hammerspoon's session tap).
/// Captures the first non-modifier key chord, converts it to a Tauri
/// accelerator string, calls the stored callback, then exits.
fn recording_tap_thread() {
    let result = CGEventTap::new(
        // kCGHIDEventTapLocation = 0: lowest-level tap, before remapping.
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::KeyDown],
        |_proxy, _etype, event| {
            let kc = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let flags = event.get_flags();

            let modifier_mask = CGEventFlags::CGEventFlagCommand
                | CGEventFlags::CGEventFlagControl
                | CGEventFlags::CGEventFlagAlternate
                | CGEventFlags::CGEventFlagShift;

            // Ignore bare modifier keypresses.
            let is_bare_modifier = matches!(
                kc,
                0x37 | // Cmd (left)
                0x36 | // Cmd (right)
                0x3B | // Ctrl (left)
                0x3E | // Ctrl (right)
                0x38 | // Shift (left)
                0x3C | // Shift (right)
                0x3A | // Option (left)
                0x3D // Option (right)
            );
            if is_bare_modifier {
                return CallbackResult::Keep;
            }

            // Consume the callback (one-shot).
            let cb_opt = {
                let mut guard = record_cb().lock().expect("mutex poisoned");
                guard.take()
            };
            if let Some(cb) = cb_opt {
                let mods = flags & modifier_mask;
                let accel = keycode_flags_to_accelerator(kc, mods);
                cb(accel);
                // Stop the run loop after delivering.
                CFRunLoop::get_current().stop();
                return CallbackResult::Drop;
            }

            // Recording was cancelled; exit cleanly.
            CFRunLoop::get_current().stop();
            CallbackResult::Keep
        },
    );

    match result {
        Err(()) => {
            tracing::error!("Recording CGEventTap::new failed — permission denied?");
            // Clear callback so callers know recording did not start.
            let mut guard = record_cb().lock().expect("mutex poisoned");
            *guard = None;
        }
        Ok(tap) => {
            let source = tap
                .mach_port()
                .create_runloop_source(0)
                .expect("recorder: create_runloop_source failed");
            let rl = CFRunLoop::get_current();
            rl.add_source(&source, unsafe { kCFRunLoopCommonModes });
            tap.enable();
            CFRunLoop::run_current();
        }
    }
}

/// Convert a raw macOS virtual keycode + CGEventFlags to a Tauri accelerator
/// string (e.g. `"CmdOrCtrl+Shift+V"`).
fn keycode_flags_to_accelerator(kc: u16, flags: CGEventFlags) -> String {
    let mut parts: Vec<&str> = Vec::new();

    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        parts.push("CmdOrCtrl");
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        parts.push("Alt");
    }
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        parts.push("Shift");
    }
    if flags.contains(CGEventFlags::CGEventFlagControl) {
        parts.push("Ctrl");
    }

    let key_name = keycode_to_name(kc);
    let mut result = parts.join("+");
    if !result.is_empty() {
        result.push('+');
    }
    result.push_str(key_name.as_deref().unwrap_or("Unknown"));
    result
}

/// Map a macOS virtual keycode to its Tauri accelerator key name.
fn keycode_to_name(kc: u16) -> Option<String> {
    Some(
        match kc {
            0x31 => "Space",
            0x24 => "Return",
            0x35 => "Escape",
            0x30 => "Tab",
            0x33 => "Backspace",
            0x7A => "F1",
            0x78 => "F2",
            0x63 => "F3",
            0x76 => "F4",
            0x60 => "F5",
            0x61 => "F6",
            0x62 => "F7",
            0x64 => "F8",
            0x65 => "F9",
            0x6D => "F10",
            0x67 => "F11",
            0x6F => "F12",
            0x7E => "Up",
            0x7D => "Down",
            0x7B => "Left",
            0x7C => "Right",
            0x73 => "Home",
            0x77 => "End",
            0x74 => "PageUp",
            0x79 => "PageDown",
            _ => {
                // For letter/digit keys look up via the inverse of ascii_to_keycode.
                let c = keycode_to_ascii(kc)?;
                return Some(c.to_string());
            }
        }
        .to_owned(),
    )
}

/// Inverse of `ascii_to_keycode`: return the ASCII char for a keycode.
fn keycode_to_ascii(kc: u16) -> Option<char> {
    Some(match kc {
        0x00 => 'A',
        0x01 => 'S',
        0x02 => 'D',
        0x03 => 'F',
        0x04 => 'H',
        0x05 => 'G',
        0x06 => 'Z',
        0x07 => 'X',
        0x08 => 'C',
        0x09 => 'V',
        0x0B => 'B',
        0x0C => 'Q',
        0x0D => 'W',
        0x0E => 'E',
        0x0F => 'R',
        0x10 => 'Y',
        0x11 => 'T',
        0x12 => '1',
        0x13 => '2',
        0x14 => '3',
        0x15 => '4',
        0x16 => '6',
        0x17 => '5',
        0x19 => '9',
        0x1A => '7',
        0x1C => '8',
        0x1D => '0',
        0x1F => 'O',
        0x20 => 'U',
        0x22 => 'I',
        0x23 => 'P',
        0x25 => 'L',
        0x26 => 'J',
        0x28 => 'K',
        0x2D => 'N',
        0x2E => 'M',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Accelerator parser
// ---------------------------------------------------------------------------

/// Parse a Tauri accelerator string (e.g. `"CmdOrCtrl+Shift+V"`) into
/// `(CGEventFlags, macOS virtual key code)`.  Returns `None` if the key
/// part is not recognised.
fn parse_accelerator(accel: &str) -> Option<(CGEventFlags, u16)> {
    let mut flags = CGEventFlags::CGEventFlagNull;
    let mut key_name: Option<&str> = None;

    for part in accel.split('+') {
        match part {
            "CmdOrCtrl" | "Cmd" | "Command" => flags |= CGEventFlags::CGEventFlagCommand,
            "Ctrl" | "Control" => flags |= CGEventFlags::CGEventFlagControl,
            "Alt" | "Option" => flags |= CGEventFlags::CGEventFlagAlternate,
            "Shift" => flags |= CGEventFlags::CGEventFlagShift,
            other => key_name = Some(other),
        }
    }

    let key = key_name?;

    let kc: u16 = match key {
        "Space" => 0x31,
        "Return" | "Enter" => 0x24,
        "Escape" => 0x35,
        "Tab" => 0x30,
        "Delete" | "Backspace" => 0x33,
        "F1" => 0x7A,
        "F2" => 0x78,
        "F3" => 0x63,
        "F4" => 0x76,
        "F5" => 0x60,
        "F6" => 0x61,
        "F7" => 0x62,
        "F8" => 0x64,
        "F9" => 0x65,
        "F10" => 0x6D,
        "F11" => 0x67,
        "F12" => 0x6F,
        "Up" => 0x7E,
        "Down" => 0x7D,
        "Left" => 0x7B,
        "Right" => 0x7C,
        "Home" => 0x73,
        "End" => 0x77,
        "PageUp" => 0x74,
        "PageDown" => 0x79,
        k if k.len() == 1 => {
            let c = k.chars().next()?.to_ascii_uppercase();
            ascii_to_keycode(c)?
        }
        _ => return None,
    };

    Some((flags, kc))
}

/// Map an ASCII letter or digit to its macOS ANSI virtual key code.
fn ascii_to_keycode(c: char) -> Option<u16> {
    Some(match c {
        'A' => 0x00,
        'S' => 0x01,
        'D' => 0x02,
        'F' => 0x03,
        'H' => 0x04,
        'G' => 0x05,
        'Z' => 0x06,
        'X' => 0x07,
        'C' => 0x08,
        'V' => 0x09,
        'B' => 0x0B,
        'Q' => 0x0C,
        'W' => 0x0D,
        'E' => 0x0E,
        'R' => 0x0F,
        'Y' => 0x10,
        'T' => 0x11,
        '1' => 0x12,
        '2' => 0x13,
        '3' => 0x14,
        '4' => 0x15,
        '6' => 0x16,
        '5' => 0x17,
        '9' => 0x19,
        '7' => 0x1A,
        '8' => 0x1C,
        '0' => 0x1D,
        'O' => 0x1F,
        'U' => 0x20,
        'I' => 0x22,
        'P' => 0x23,
        'L' => 0x25,
        'J' => 0x26,
        'K' => 0x28,
        'N' => 0x2D,
        'M' => 0x2E,
        _ => return None,
    })
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    // These tests share the process-global tap state, so they must not run
    // concurrently. Serialize them under a dedicated mutex and always reset
    // state at the start so one test's residue cannot affect another.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_state() {
        *tap_handle().lock().expect("mutex poisoned") = None;
        *global_callback().lock().expect("mutex poisoned") = None;
    }

    /// A `CFRunLoop` handle obtained on the test thread is a valid stand-in for
    /// the tap thread's run loop for the purposes of guard/teardown logic.
    fn fake_handle() -> TapHandle {
        TapHandle {
            run_loop: CFRunLoop::get_current(),
        }
    }

    #[test]
    fn guard_reports_not_live_when_no_tap_registered() {
        let _g = TEST_LOCK.lock().expect("test lock");
        reset_state();
        assert!(!tap_already_live());
    }

    #[test]
    fn guard_reports_live_after_handle_registered() {
        let _g = TEST_LOCK.lock().expect("test lock");
        reset_state();
        *tap_handle().lock().expect("mutex poisoned") = Some(fake_handle());
        assert!(
            tap_already_live(),
            "install must treat an existing tap as live and not spawn a second"
        );
        reset_state();
    }

    #[test]
    fn uninstall_clears_handle_and_callback() {
        let _g = TEST_LOCK.lock().expect("test lock");
        reset_state();
        // Simulate a fully-installed tap: a live handle plus a boxed callback.
        *tap_handle().lock().expect("mutex poisoned") = Some(fake_handle());
        *global_callback().lock().expect("mutex poisoned") = Some(Box::new(|| {}) as Callback);

        uninstall();

        assert!(
            tap_handle().lock().expect("mutex poisoned").is_none(),
            "uninstall must take the handle so a re-install is not blocked"
        );
        assert!(
            global_callback().lock().expect("mutex poisoned").is_none(),
            "uninstall must reclaim the boxed callback so it does not outlive the tap"
        );
        // After teardown the guard must report not-live again (clean replace).
        assert!(!tap_already_live());
    }

    #[test]
    fn uninstall_is_noop_when_not_installed() {
        let _g = TEST_LOCK.lock().expect("test lock");
        reset_state();
        // Must not panic and must leave state empty.
        uninstall();
        assert!(!tap_already_live());
        assert!(global_callback().lock().expect("mutex poisoned").is_none());
    }
}
