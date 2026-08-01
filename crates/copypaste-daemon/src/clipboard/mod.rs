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
//! - **§3.9** a short-lived frontmost-app cache, private-mode and exclusion
//!   gates; known password-manager origins are persisted as sensitive.
//! - **I-11** text capture is the only implemented payload path. Image and
//!   file changes are acknowledged without being materialised until encrypted
//!   binary storage and native paste-back land together.
//! - **I-18** `NSData.length` checked before the bytes are copied out.
//! - **I-39 / §6.5** rejections are counted and readable, not just logged.
//! - **§3.12** the invariant UTI strings are built once, not once per tick.
//! - **I-9** no clipboard content, and no paths, in logs or in errors.
//!
//! Rich-text, image and file capture remain deferred. The shared content-type
//! vocabulary nevertheless lets imported and remote rows retain their declared
//! type without routing a path or base64 string through text ingest.

mod change;
mod fake;
mod file_materialize;
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

/// Credential stores mark a capture sensitive even when its text does not
/// match a detector rule. Users may still explicitly exclude an app entirely.
pub(crate) fn is_password_manager_app(bundle_id: &str) -> bool {
    let bundle_id = bundle_id.to_ascii_lowercase();
    matches!(
        bundle_id.as_str(),
        "com.1password.1password"
            | "com.agilebits.onepassword7"
            | "com.bitwarden.desktop"
            | "org.keepassxc.keepassxc"
            | "com.dashlane.dashlane"
            | "com.lastpass.lastpass"
            | "com.apple.passwords"
    ) || bundle_id.contains("1password")
        || bundle_id.contains("bitwarden")
        || bundle_id.contains("keepass")
        || bundle_id.contains("dashlane")
        || bundle_id.contains("lastpass")
        || bundle_id.contains("protonpass")
        || bundle_id.contains("proton.pass")
        || bundle_id.contains("strongbox")
        || bundle_id.contains("secretive")
        || bundle_id.contains("keepassium")
}

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

    /// Write a native binary representation back to the platform pasteboard.
    /// A backend that cannot do this refuses rather than converting bytes to
    /// text and corrupting the user's clipboard.
    fn set_binary_contents(
        &mut self,
        _item_id: &str,
        _content_type: &str,
        _bytes: &[u8],
        _metadata: Option<&copypaste_core::FileMetadata>,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!(
            "this clipboard backend cannot write binary content"
        ))
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
    use super::{is_password_manager_app, MAX_TEXT_BYTES};
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
