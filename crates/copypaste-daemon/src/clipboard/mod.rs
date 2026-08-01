//! The clipboard source port — the seam v1 never had.
//!
//! Port manifest 01 (`docs/rewrite/port-manifest/01-clipboard-capture.md`) is
//! binding in full for this subsystem; dropping v0.4.x compatibility does not
//! touch it, because it describes macOS behaviour rather than our formats.
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
//! - **§3.9** a short-lived frontmost-app cache, private-mode and app gates;
//!   excluded and known password-manager apps are acknowledged without
//!   materialising their clipboard data.
//! - **I-11..I-14** one representation per change, in the strict order text,
//!   image (PNG then TIFF), file URL then legacy filename plist.
//! - **I-18** `NSData.length` checked before the bytes are copied out.
//! - **I-39 / §6.5** rejections are counted and readable, not just logged.
//! - **§3.12** the invariant UTI strings are built once, not once per tick.
//! - **I-9** no clipboard content, and no paths, in logs or in errors.
//!
//! Rich-text UTIs remain explicitly unsupported by the binding manifest: they
//! are not mistaken for a file or image, and are never allowed to outrank one.
//! The shared content-type vocabulary nevertheless lets imported and remote
//! rows retain their declared type.

mod change;
mod fake;
mod format;

/// Decisions that must be made before a clipboard representation is read.
///
/// The backend owns the change cursor, so applying these gates here rather
/// than after `poll` is what acknowledges a skipped value without first
/// materialising its plaintext.
#[derive(Debug, Clone, Copy)]
pub struct CapturePolicy<'a> {
    pub private_mode: bool,
    pub excluded_app_bundle_ids: &'a [String],
    /// The live storage cap. Backends reject before copying more than this
    /// many bytes (or more than this many encoded bytes for a binary payload).
    pub max_item_bytes: u64,
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "macos"))]
pub use fake::FakeClipboard;

/// Absolute read gate, in bytes.
///
/// §4 gives 10 MiB and gives its reason as "kept under the wire-frame cap so a
/// storable item is always transportable" — the number was v1's *default*
/// `max_item_bytes`, and the reason is the rule. Both now come from one place:
/// this is the ceiling `max_item_bytes` may be set to, so reading further can
/// only ever end in a rejection. §3.10 still applies — this gate and the
/// storage layer's gate move together.
const MAX_TEXT_BYTES: usize = copypaste_ipc::MAX_CONTENT_BYTES;

/// Credential stores are never captured, even if the user has not added their
/// bundle id to the configurable exclusion list. Their pasteboards often have
/// an opt-out marker as well, but attribution is a second independent gate.
pub(crate) fn is_password_manager_app(bundle_id: &str) -> bool {
    matches!(
        bundle_id,
        "com.1password.1password"
            | "com.agilebits.onepassword7"
            | "com.bitwarden.desktop"
            | "org.keepassxc.keepassxc"
            | "com.dashlane.dashlane"
            | "com.lastpass.lastpass"
    )
}

/// One captured clipboard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// UTF-8 text, a base64 image payload, or an absolute file reference.
    pub content: String,
    /// One of `copypaste_ipc::content_type`.
    pub content_type: String,
    /// The frontmost app at capture time, when the platform could resolve it.
    pub app_bundle_id: Option<String>,
}

impl Capture {
    #[allow(dead_code)]
    fn text(content: String) -> Self {
        Self {
            content,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            app_bundle_id: None,
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

    /// Poll with privacy policy. Legacy sources that cannot attribute an app
    /// retain their ordinary polling semantics; macOS and the drivable fake
    /// override this to gate before representation extraction.
    fn poll_with_policy(&mut self, _policy: CapturePolicy<'_>) -> Option<Capture> {
        self.poll()
    }

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

/// macOS -> `NSPasteboard`. Everything else -> the fake.
pub fn new_source() -> Box<dyn ClipboardSource> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsClipboard::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(FakeClipboard::new())
    }
}

#[cfg(test)]
mod tests {
    use super::MAX_TEXT_BYTES;
    use copypaste_ipc::{ConfigData, ConfigPatch};

    /// §3.10, from the other end: the read gate and the storable maximum were
    /// separate numbers, and the gate was the larger. Everything it let through
    /// above `max_item_bytes`' ceiling was read off the pasteboard, copied into
    /// the heap, and then refused by ingest — or, worse, stored under a config
    /// no client could read back.
    #[test]
    fn a_capture_at_the_read_gate_is_a_size_a_user_may_store() {
        let at_the_gate = ConfigPatch {
            max_item_bytes: Some(MAX_TEXT_BYTES as u64),
            ..Default::default()
        }
        .apply(&ConfigData::default())
        .expect("the config ceiling must admit everything the read gate passes");
        assert_eq!(at_the_gate.max_item_bytes, MAX_TEXT_BYTES as u64);
    }

    /// And nothing it passes can outgrow a reply frame.
    #[test]
    fn the_read_gate_stays_inside_the_wire_contract() {
        const { assert!(MAX_TEXT_BYTES <= copypaste_ipc::MAX_CONTENT_BYTES) };
    }
}
