//! Two real processes, two real clipboard owners, one 750 ms window.
//!
//! The same-process `write_text` helper cannot show what production
//! attribution does: a process cannot make another process own the clipboard,
//! and opening it without a window leaves `GetClipboardOwner` with nothing to
//! name. Each child here creates its own window, takes ownership through it,
//! and stays alive until the parent has resolved it — so `GetClipboardOwner`,
//! `GetWindowThreadProcessId`, `OpenProcess` and `QueryFullProcessImageNameW`
//! all run against a distinct real writer, and the captures they produce go
//! through the real ingest (DMY-158).

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::{mem, ptr};

use clipboard_win::{raw, Clipboard};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, PeekMessageW, HWND_MESSAGE, MSG, PM_REMOVE,
};

use super::tests::serialised;
use super::{Capture, ClipboardSource, WindowsClipboard, OPEN_ATTEMPTS};
use crate::capture::ingest_capture;
use crate::testutil::test_state;

/// The child that stands in for a credential store. The production table in
/// `copypaste_core::sensitive` matches `1password`, so the sensitivity floor
/// has to fire on this writer's identity while its content looks like nothing
/// at all — which is the property a misattribution silently removes.
const CREDENTIAL_IMAGE: &str = "1password.exe";
const ORDINARY_IMAGE: &str = "copypaste-test-editor.exe";

const ORDINARY_TEXT: &str = "thursday agenda notes";
const CREDENTIAL_TEXT: &str = "correct horse battery staple";

/// Present only in a child process, and holding what it must put on the
/// clipboard. The parent runs the same binary, so the role has to be explicit.
const ROLE: &str = "COPYPASTE_CLIPBOARD_WRITER";
const CHILD_TEST: &str = "clipboard::windows::two_writers::child_owns_the_clipboard_until_released";

const RENDEZVOUS_TIMEOUT: Duration = Duration::from_secs(30);

/// DMY-158. Two processes write inside one poll period; each capture carries
/// its own writer, and the credential store's copy reaches neither full-text
/// search nor a sync session.
#[test]
#[ignore = "drives the real Windows clipboard and spawns two child processes"]
fn two_processes_get_distinct_attributions() {
    let _lock = serialised();
    let rendezvous = tempfile::tempdir().expect("a rendezvous directory");
    let mut ordinary = Writer::spawn(rendezvous.path(), ORDINARY_IMAGE, ORDINARY_TEXT);
    let mut credential = Writer::spawn(rendezvous.path(), CREDENTIAL_IMAGE, CREDENTIAL_TEXT);
    // Both children hold a window and wait, so the two writes below are as
    // close together as the clipboard will take them — not as far apart as
    // starting a process takes.
    ordinary.wait_for("ready");
    credential.wait_for("ready");

    let mut clipboard = WindowsClipboard::new().expect("a Windows clipboard");
    // Adopt whatever is on the clipboard now, so the writes below are the only
    // changes this test sees. That first poll resolves a writer of its own, so
    // the counts below are measured as a delta.
    let _ = clipboard.poll();
    let resolutions = clipboard.attribution.resolutions();
    let unattributed = clipboard.attribution.unattributed_count();

    let started = Instant::now();
    let first = ordinary.write_and_capture(&mut clipboard);
    let second = credential.write_and_capture(&mut clipboard);
    let elapsed = started.elapsed();

    ordinary.release();
    credential.release();

    let first_id = first
        .app_bundle_id
        .clone()
        .expect("the first writer was not identified");
    let second_id = second
        .app_bundle_id
        .clone()
        .expect("the second writer was not identified");
    assert_eq!(
        first_id, ORDINARY_IMAGE,
        "the first capture named {first_id} rather than the process that wrote it"
    );
    assert_eq!(
        second_id, CREDENTIAL_IMAGE,
        "the second capture named {second_id}; it inherited the first writer or fell back \
         to the foreground window"
    );
    assert_ne!(first_id, second_id);
    println!(
        "two writers resolved in {}ms: {first_id} then {second_id}",
        elapsed.as_millis()
    );
    assert_eq!(
        clipboard.attribution.resolutions() - resolutions,
        2,
        "the second change reused the first change's identity"
    );
    assert_eq!(
        clipboard.attribution.unattributed_count() - unattributed,
        0,
        "a writer that was alive and open could not be identified"
    );
    assert!(
        elapsed < Duration::from_millis(750),
        "the two writes and their attributions took {}ms; the property under test is that \
         the 750ms cache window can no longer collapse them",
        elapsed.as_millis()
    );

    // The same captures, through the real pipeline: what makes the second one
    // sensitive is its origin, since nothing in its text is a secret shape.
    let (state, _data) = test_state("two-writer-attribution");
    let now = copypaste_core::now_ms();
    let notes = ingest_capture(&state, first, now)
        .expect("the ordinary capture")
        .into_item();
    let secret = ingest_capture(&state, second, now + 1)
        .expect("the credential capture")
        .into_item();

    assert!(!notes.is_sensitive);
    assert!(
        secret.is_sensitive,
        "a credential store's copy was stored as ordinary text"
    );
    assert_eq!(secret.app_bundle_id.as_deref(), Some(CREDENTIAL_IMAGE));
    assert!(
        state
            .store
            .search("battery", 10)
            .expect("search")
            .is_empty(),
        "the password reached full-text search"
    );
    assert!(
        !state.store.search("agenda", 10).expect("search").is_empty(),
        "the ordinary item must still be findable"
    );

    let advertised: Vec<String> = state
        .store
        .summaries_since(0, None, 100)
        .expect("summaries")
        .into_iter()
        .map(|version| version.id)
        .collect();
    assert!(advertised.contains(&notes.id));
    assert!(
        !advertised.contains(&secret.id),
        "the password was offered to a sync session"
    );
}

/// The child role. Returns at once in the parent, which runs the same binary
/// and therefore also lists this test.
#[test]
#[ignore = "the child half of two_processes_get_distinct_attributions"]
fn child_owns_the_clipboard_until_released() {
    let Ok(text) = std::env::var(ROLE) else {
        return;
    };
    // The parent copied this binary into the rendezvous directory under the
    // image name it wants attributed, so the child's own path is both.
    let me = std::env::current_exe().expect("the child's own path");
    let directory = me.parent().expect("a rendezvous directory").to_path_buf();
    let label = me
        .file_stem()
        .expect("an image name")
        .to_string_lossy()
        .into_owned();
    let flag = |suffix: &str| directory.join(format!("{label}.{suffix}"));

    let window = OwnerWindow::create().expect("a child needs a window to own the clipboard");
    std::fs::write(flag("ready"), b"").expect("announce readiness");
    window
        .await_flag(&flag("go"), RENDEZVOUS_TIMEOUT)
        .expect("the parent never said go");

    {
        let _clipboard = Clipboard::new_attempts_for(window.0, OPEN_ATTEMPTS)
            .expect("the child could not open the clipboard");
        // `EmptyClipboard` is what assigns ownership to the window that opened
        // it, and ownership is what the parent resolves.
        raw::empty().expect("empty the clipboard");
        raw::set_string(&text).expect("write the clipboard");
    }
    std::fs::write(flag("done"), b"").expect("announce the write");

    // Held open deliberately: an exited process has no image name to query and
    // no window to own the clipboard, which is exactly why driving this with
    // `powershell -Command Set-Clipboard` proved nothing.
    window
        .await_flag(&flag("exit"), RENDEZVOUS_TIMEOUT)
        .expect("the parent never said exit");
    drop(window);
}

struct Writer {
    label: String,
    directory: PathBuf,
    process: Child,
}

impl Writer {
    fn spawn(directory: &Path, image_name: &str, text: &str) -> Self {
        let executable = directory.join(image_name);
        std::fs::copy(
            std::env::current_exe().expect("the test binary"),
            &executable,
        )
        .expect("copy the test binary under the image name to attribute");
        let process = Command::new(&executable)
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env(ROLE, text)
            .spawn()
            .expect("spawn a child writer");
        Self {
            label: image_name
                .strip_suffix(".exe")
                .unwrap_or(image_name)
                .to_owned(),
            directory: directory.to_path_buf(),
            process,
        }
    }

    fn flag(&self, suffix: &str) -> PathBuf {
        self.directory.join(format!("{}.{suffix}", self.label))
    }

    fn wait_for(&self, suffix: &str) {
        await_flag(&self.flag(suffix), RENDEZVOUS_TIMEOUT)
            .unwrap_or_else(|| panic!("the {} child never reported {suffix}", self.label));
    }

    fn write_and_capture(&self, clipboard: &mut WindowsClipboard) -> Capture {
        std::fs::write(self.flag("go"), b"").expect("release the child");
        self.wait_for("done");
        // The change is published by `CloseClipboard`, which has returned in
        // the child before it wrote its flag; a poll can still lose the race
        // for the clipboard lock itself.
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if let Some(capture) = clipboard.poll() {
                return capture;
            }
            assert!(
                Instant::now() < deadline,
                "the {} child's write was never captured",
                self.label
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn release(&mut self) {
        std::fs::write(self.flag("exit"), b"").expect("release the child");
        let status = self.process.wait().expect("wait for a child writer");
        assert!(
            status.success(),
            "the {} child failed: {status}",
            self.label
        );
    }
}

fn await_flag(flag: &Path, timeout: Duration) -> Option<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if flag.exists() {
            return Some(());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

/// A message-only window, which is all `GetClipboardOwner` needs to name.
struct OwnerWindow(*mut c_void);

impl OwnerWindow {
    /// Wait while dispatching messages, which is what makes this window a
    /// faithful clipboard owner rather than a hung one.
    ///
    /// `EmptyClipboard` sends `WM_DESTROYCLIPBOARD` to the current owner and
    /// waits for the answer, so the *next* writer stalls on the previous
    /// owner's message loop — measured at just over five seconds when this
    /// child slept instead of pumping, which is a poll period the property
    /// under test cannot be observed through.
    fn await_flag(&self, flag: &Path, timeout: Duration) -> Option<()> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.pump();
            if flag.exists() {
                return Some(());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    fn pump(&self) {
        let mut message: MSG = unsafe { mem::zeroed() };
        while unsafe { PeekMessageW(&raw mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe { DispatchMessageW(&message) };
        }
    }

    fn create() -> Option<Self> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        (!window.is_null()).then_some(Self(window))
    }
}

impl Drop for OwnerWindow {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.0) };
    }
}
