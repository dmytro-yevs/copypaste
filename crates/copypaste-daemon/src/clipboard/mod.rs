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
//! Text capture only — the MVP's single representation. Frontmost-app
//! attribution (§3.9), private mode and the app-exclusion gate are *not* in this
//! module; they are separate work. The pieces that are here are implemented
//! against the manifest, not approximated:
//!
//! - **I-1..I-4** change detection driven by `changeCount`, never by content
//!   comparison, with the cursor advanced on every drop path.
//! - **§3.2** burst handling — the manifest's highest-value rule. A burst never
//!   replaces the content; the surviving clipboard value is always returned.
//! - **§3.3** the two-sided self-write sentinel, including the *conditional*
//!   post-stamp (CopyPaste-8yzf) and the reset-on-failure path.
//! - **I-5 / §3.4** the three `org.nspasteboard.*` opt-out markers, probed
//!   before any representation is read.
//! - **I-18** `NSData.length` checked before the bytes are copied out.
//! - **I-39 / §6.5** rejections are counted and readable, not just logged.
//! - **§3.12** the invariant UTI strings are built once, not once per tick.
//! - **I-9** no clipboard content, and no paths, in logs or in errors.
//!
//! ## Image, file and rich-text capture: what is done and what is not
//!
//! **Done — the type plumbing.** `copypaste_ipc::content_type` is the one place
//! the vocabulary is spelled, `Capture::content_type` and the storage column
//! carry it end to end, `Item::content_type` reaches every client, and a
//! non-text row already renders as its label rather than as mojibake
//! (`cli::render::item_preview`). A row of any of these types can already
//! *arrive* — from a peer, from a cloud account, from an import — and is
//! stored, listed and excluded from an export with a count (`skipped_non_text`).
//!
//! **Not done — the capture path, and it is the larger half.** Nothing below
//! this line is written, and none of it can be tested on a Linux host:
//!
//! * **I-11** representation priority `text > image > file`, chosen once per
//!   change, with the lower-priority ones dropped rather than queued.
//! * **I-12/I-13** probing image *presence* with `availableTypeFromArray`
//!   before any `dataForType`, `public.png` then `public.tiff`. A multi-MB
//!   image accompanying text must never enter the heap.
//! * **I-14/§3.6** `public.file-url` — strip the scheme, percent-decode with a
//!   maintained crate, require absolute; invalid `%` passes through.
//! * **§3.5 (`CopyPaste-q5ab`)** `NSFilenamesPboardType` is a **binary plist**,
//!   not a string: read it with a data accessor and parse an array of bare
//!   POSIX paths. v1 called `stringForType` and stripped `file://`, and every
//!   file copied from Finder was silently discarded with no error and no log.
//! * **I-15/I-16/I-19** absolute-path check, the read moved off the reactor,
//!   and `stat`+`read` in one blocking operation (`CopyPaste-b5iz`, TOCTOU).
//! * **§3.7/§3.8** promised files stay unsupported; the unsupported-type probe
//!   is a fixed allowlist logged once per kind.
//!
//! And three things outside this module that the capture path would need:
//! [`Capture::content`] is a `String`, so image bytes have nowhere to go;
//! `copypaste_core::ingest_into` takes `&str`; and `Item::content` is a
//! `String` on the wire. Carrying bytes is a change to all three, and it is the
//! decision that should be taken first — base64 on the JSON wire, a side
//! channel, or a content-addressed blob beside the database — because the
//! pasteboard code above is shaped by the answer.

mod change;
mod fake;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "macos"))]
pub use fake::FakeClipboard;

/// Read gate for text, in bytes (§4, "Max text (default)" = 10 MiB).
///
/// Kept under the 16 MiB wire-frame cap so anything storable is transportable.
/// Configuration is not wired yet; when it is, §3.10 applies — this gate and the
/// storage layer's gate must be driven by the *same* user-configured value and
/// hot-reload together.
const MAX_TEXT_BYTES: usize = 10 * 1024 * 1024;

/// One captured clipboard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// A `String`, which is why image and file capture is not merely a matter
    /// of writing the pasteboard code — see the module header.
    pub content: String,
    /// One of `copypaste_ipc::content_type`. Always
    /// [`copypaste_ipc::content_type::TEXT`] today; the field is typed as the
    /// shared vocabulary rather than a bare literal so a new representation
    /// names itself the same way everywhere it travels.
    pub content_type: String,
}

impl Capture {
    fn text(content: String) -> Self {
        Self {
            content,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
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
