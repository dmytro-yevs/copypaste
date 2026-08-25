//! The clipboard source port — the seam v1 never had.
//!
//! Port manifest 01 (`docs/rewrite/port-manifest/01-clipboard-capture.md`) is
//! binding for v2's text-capture scope. Its image and file rules are reference.
//!
//! Its own top finding (§6.7) is that v1's monitor called
//! `NSPasteboard.generalPasteboard` directly, so roughly two thirds of the
//! invariants in §2 could not be tested at all — which is precisely why several
//! of them regressed. This module is the fix: one trait ([`ClipboardSource`]),
//! one real backend, one fake, and the change-detection state machine written
//! **once** and shared by both, so the rules that used to be untestable are
//! exercised by ordinary `cargo test` on any host.
//!
//! ## Layout
//!
//! The split follows that fix rather than the platforms:
//!
//! * [`change`] — the change-count cursor and the self-write sentinel. Pure,
//!   platform-free, and the file both backends run. Every §3.3 and §3.2 test
//!   lives with it.
//! * [`fake`] — the test/dev backend, and the acceptance tests of §5 that it
//!   makes reachable.
//! * `macos` — `NSPasteboard`, compiled only on macOS. It contains the Cocoa
//!   spelling and nothing else that could have been tested off a mac.
//! * `windows` — the same shape against `GetClipboardSequenceNumber` and the
//!   four do-not-record clipboard formats, compiled only on Windows. Its
//!   opt-out decision is in `windows_optout`, which is built everywhere.
//!
//! This file keeps only what both backends must agree on: the port itself, the
//! captured value, and the size gate.
//!
//! ## What is implemented here
//!
//! The capture boundary implements the manifest's privacy and representation
//! decisions before a payload is copied from the pasteboard:
//!
//! - **I-1..I-4** change detection driven by `changeCount`, never by content
//!   comparison, with the cursor advanced on every drop path.
//! - **§3.2** burst handling — the manifest's highest-value rule. A burst never
//!   replaces the content; the surviving clipboard value is always returned.
//! - **§3.3** the two-sided self-write sentinel, including the *conditional*
//!   post-stamp (CopyPaste-8yzf) and the reset-on-failure path.
//! - **I-5 / §3.4** the three `org.nspasteboard.*` opt-out markers, probed
//!   before any representation is read.
//! - **§3.9** a short-lived frontmost-app cache, private-mode and exclusion
//!   gates; known password-manager origins are persisted as sensitive.
//! - **I-11** text is captured when offered; non-text-only changes are ignored.
//! - **I-18** `NSData.length` checked before the bytes are copied out.
//! - **I-39 / §6.5** rejections are counted and readable, not just logged.
//! - **§3.12** the invariant UTI strings are built once, not once per tick.
//! - **I-9** no clipboard content, and no paths, in logs or in errors.
//!
//! The shared content-type vocabulary keeps captured, imported, and remote rows
//! on the same dispatch without routing a path or base64 string through text ingest.

mod change;
mod fake;
// `test` so the module is exercised off macOS, but only where it compiles:
// every syscall in it is `rustix::fs`, which has no Windows implementation.
#[cfg(any(target_os = "macos", all(test, unix)))]
mod file_materialize;
pub(crate) mod format;
/// The Windows opt-out vocabulary, and which application a change belongs to.
/// Built everywhere, like `change`, because the decisions they encode are the
/// ones that must not regress unnoticed.
///
/// `windows_attribution` is crate-visible because the capture path's
/// end-to-end test drives it: what the attribution decides and what reaches
/// the search index are in different files, and the regression DMY-158 records
/// only exists between them.
#[cfg(any(target_os = "windows", test))]
pub(crate) mod windows_attribution;
#[cfg(any(target_os = "windows", test))]
mod windows_optout;

/// Decisions that must be made before a clipboard representation is read.
///
/// The backend owns the change cursor, so applying these gates here rather
/// than after `poll` is what acknowledges a skipped value without first
/// materialising its plaintext.
#[derive(Debug, Clone, Copy)]
pub struct CapturePolicy<'a> {
    pub settings: &'a copypaste_ipc::ConfigData,
}

impl<'a> CapturePolicy<'a> {
    pub fn new(settings: &'a copypaste_ipc::ConfigData) -> Self {
        Self { settings }
    }

    /// The shared IPC selector applies both the content-specific live setting
    /// and the current storage/transport hard bound.
    pub fn limit_bytes(self, content_type: &str) -> u64 {
        self.settings.capture_limit_bytes(content_type)
    }
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use fake::FakeClipboard;

/// Current storage and transport hard bound, in bytes.
#[cfg(any(target_os = "macos", test))]
const MAX_CAPTURE_BYTES: usize = copypaste_ipc::MAX_CONTENT_BYTES;

/// Credential stores mark a capture sensitive even when its text does not
/// match a detector rule. Users may still explicitly exclude an app entirely.
pub(crate) use copypaste_core::sensitive::is_password_manager_app;

/// One captured clipboard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// UTF-8 text captured from the system pasteboard.
    pub content: String,
    /// Raw bytes for image/file capture.  Text stays in `content`; the fields
    /// are mutually exclusive so no caller can accidentally feed binary to a
    /// string-only consumer.
    pub binary_content: Option<Vec<u8>>,
    /// An absolute local file reference.  The tick opens it on the blocking
    /// worker; polling itself must never read a file's bytes (manifest 01 I-16).
    pub file_path: Option<std::path::PathBuf>,
    pub file_metadata: Option<copypaste_core::FileMetadata>,
    /// One of `copypaste_ipc::content_type`.
    pub content_type: String,
    /// The frontmost app at capture time, when the platform could resolve it.
    pub app_bundle_id: Option<String>,
    /// Display name reported by the originating platform. This is metadata,
    /// never a value inferred from a bundle/package identifier.
    pub app_name: Option<String>,
}

impl Capture {
    #[allow(dead_code)]
    fn text(content: String) -> Self {
        Self {
            content,
            binary_content: None,
            file_path: None,
            file_metadata: None,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            app_bundle_id: None,
            app_name: None,
        }
    }
}

/// The seam between the daemon and the system clipboard.
///
/// `Send` so the poll loop can own one inside a spawned task.
pub trait ClipboardSource: Send {
    /// Return `Some(capture)` when the clipboard changed since the last poll.
    /// Must return `None` when nothing changed — the caller polls on a timer.
    fn poll(&mut self) -> Option<Capture>;

    /// Apply privacy policy before extracting a clipboard representation.
    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture>;

    /// Whether [`ClipboardSource::poll`] has anything to report, without
    /// consuming it.
    ///
    /// The capture loop calls this on the async thread and only hands off to
    /// the blocking pool when it answers `true`, which is what keeps an idle
    /// clipboard from costing six thread wakeups every poll.
    ///
    /// Defaults to `true`: a backend that cannot answer cheaply must be polled,
    /// never skipped. It must also answer `true` for a pending self-write —
    /// only `poll` consumes the sentinel, and one left armed suppresses a later
    /// genuine copy.
    fn changed(&mut self) -> bool {
        true
    }

    /// Write text to the clipboard. Must suppress the resulting self-write so
    /// the next poll does not re-capture our own content.
    fn set_contents(&mut self, text: &str) -> anyhow::Result<()>;

    /// Write one authenticated history payload without consulting its display
    /// label. The payload enum is shared with Android's embedded backend, so
    /// every platform must make the same exhaustive content decision.
    fn write_payload(
        &mut self,
        item_id: &str,
        payload: &copypaste_core::ClipboardPayload,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::{ClipboardPayload, ClipboardWriteError};

        match payload {
            ClipboardPayload::Text(text) => self
                .set_contents(text)
                .map_err(|_| ClipboardWriteError::Failed),
            ClipboardPayload::Image {
                content_type,
                bytes,
            } => self.set_binary_contents(item_id, content_type, bytes, None),
            ClipboardPayload::File { bytes, metadata } => self.set_binary_contents(
                item_id,
                copypaste_ipc::content_type::FILE,
                bytes,
                metadata.as_ref(),
            ),
            ClipboardPayload::Unsupported { .. } => Err(ClipboardWriteError::UnsupportedContent),
        }
    }

    /// Write a native binary representation back to the platform pasteboard.
    /// A backend that cannot do this refuses rather than converting bytes to
    /// text and corrupting the user's clipboard.
    fn set_binary_contents(
        &mut self,
        _item_id: &str,
        _content_type: &str,
        _bytes: &[u8],
        _metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        Err(copypaste_core::ClipboardWriteError::UnsupportedContent)
    }

    /// Identifies the live backend, surfaced over IPC so a demo cannot be
    /// mistaken for the real thing.
    fn backend_name(&self) -> &'static str;

    /// How many changes were dropped for exceeding the size cap.
    ///
    /// I-39 and §6.5: v1 wired a rejection counter that nothing ever read, and
    /// the image path bypassed it entirely, so an oversized clipboard item was
    /// dropped in complete silence — indistinguishable from the daemon being
    /// broken. The counter is on the port so a status response can surface it.
    /// Defaulted so a backend may omit it, never so it can be forgotten.
    ///
    /// Read by `AppState::counters` onto [`copypaste_ipc::DiagnosticCounters`].
    fn rejected_too_large_count(&self) -> u64 {
        0
    }

    /// How many clipboard values were overwritten before we could observe them.
    ///
    /// §3.1/§3.2: `changeCount` is lossy — a delta above 1 means intermediate
    /// values existed and are irrecoverable. This is the telemetry side-channel
    /// the manifest asks for; a burst is *never* modelled as a content value
    /// (§6.1). Surfaced the same way as the counter above.
    fn lost_intermediates_count(&self) -> u64 {
        0
    }
}

/// macOS -> `NSPasteboard`. Windows -> the system clipboard. Everything else ->
/// the fake.
pub fn new_source(data_dir: &std::path::Path) -> std::io::Result<Box<dyn ClipboardSource>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOsClipboard::new(data_dir)?))
    }
    #[cfg(target_os = "windows")]
    {
        // No staging directory: this backend refuses a file paste-back rather
        // than materialising plaintext it has no sweeper for.
        let _ = data_dir;
        Ok(Box::new(windows::WindowsClipboard::new()?))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = data_dir;
        Ok(Box::new(FakeClipboard::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_password_manager_app, CapturePolicy, MAX_CAPTURE_BYTES};
    use copypaste_ipc::ConfigData;

    #[test]
    fn policy_selects_each_payload_cap_before_applying_the_hard_bound() {
        let settings = ConfigData {
            max_text_size_bytes: 64 * 1024,
            max_image_size_bytes: 2 * 1024 * 1024,
            max_file_size_bytes: 3 * 1024 * 1024,
            ..Default::default()
        };
        let policy = CapturePolicy::new(&settings);
        assert_eq!(
            policy.limit_bytes(copypaste_ipc::content_type::TEXT),
            64 * 1024
        );
        assert_eq!(
            policy.limit_bytes(copypaste_ipc::content_type::IMAGE_PNG),
            2 * 1024 * 1024
        );
        assert_eq!(
            policy.limit_bytes(copypaste_ipc::content_type::FILE),
            3 * 1024 * 1024
        );
    }

    /// And nothing it passes can outgrow a reply frame.
    #[test]
    fn the_read_gate_stays_inside_the_wire_contract() {
        let settings = ConfigData::default();
        let policy = CapturePolicy::new(&settings);
        for content_type in [
            copypaste_ipc::content_type::TEXT,
            copypaste_ipc::content_type::IMAGE_PNG,
            copypaste_ipc::content_type::FILE,
        ] {
            assert_eq!(policy.limit_bytes(content_type), MAX_CAPTURE_BYTES as u64);
        }
    }

    #[test]
    fn credential_store_match_is_case_insensitive_and_covers_supported_apps() {
        for bundle_id in [
            "COM.1PASSWORD.1PASSWORD",
            "com.apple.Passwords",
            "com.proton.pass",
            "com.strongbox.passwordsafe",
            "com.mortenjust.secretive",
            "com.keepassium.keepassium",
        ] {
            assert!(is_password_manager_app(bundle_id), "{bundle_id}");
        }
        assert!(!is_password_manager_app("com.apple.TextEdit"));
    }
}
