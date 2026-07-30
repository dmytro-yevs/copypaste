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
//! ## What is implemented here
//!
//! Text capture only — the MVP's single representation. The manifest's
//! representation-priority rules (I-11..I-16), image/file ingest, frontmost-app
//! attribution (§3.9), private mode and the app-exclusion gate are *not* in this
//! file; they are separate work. The pieces that are here are implemented
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
//! ## Deviation from §3.11 (stated, not silent)
//!
//! The manifest says non-macOS is a silent no-op with no polling cost. We keep
//! the *contract* — a default [`FakeClipboard`] never panics, never errors and
//! returns `None` forever — but we deliberately make it drivable, from
//! `COPYPASTE_FAKE_CLIPBOARD` or from `push_external`, so the MVP is
//! demonstrable on the Linux host we develop on and so the acceptance tests
//! become real tests. [`ClipboardSource::backend_name`] is surfaced over IPC
//! (`StatusResponse::clipboard_backend`) so a demo cannot be mistaken for the
//! real thing.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Constants — manifest §4. Values are quoted with the rationale that pins them;
// changing one without changing the manifest is a regression.
// ---------------------------------------------------------------------------

/// `changeCount` delta at which a burst is reported (§4).
///
/// 1 = a normal copy, 2 = a paste-back pair (`clearContents` + `set…:forType:`),
/// so >= 3 is a genuine burst. This is a *telemetry* threshold only: crossing it
/// never changes what is captured (§3.2).
const BURST_THRESHOLD: i64 = 3;

/// "No pending self-write", and the initial change-count cursor (§4).
///
/// Must be outside the valid `changeCount` domain, which is non-negative and
/// monotonically increasing. The shared value also gives I-2 for free: the first
/// poll after startup cannot be mistaken for a burst.
const COUNT_NONE: i64 = -1;

/// A self-write moves `changeCount` by exactly this much (§4): `clearContents`
/// (+1) then `set…:forType:` (+1).
const SELF_WRITE_DELTA: i64 = 2;

/// Read gate for text, in bytes (§4, "Max text (default)" = 10 MiB).
///
/// Kept under the 16 MiB wire-frame cap so anything storable is transportable.
/// Configuration is not wired yet; when it is, §3.10 applies — this gate and the
/// storage layer's gate must be driven by the *same* user-configured value and
/// hot-reload together.
const MAX_TEXT_BYTES: usize = 10 * 1024 * 1024;

/// Environment variable naming a file the [`FakeClipboard`] watches.
const FAKE_CLIPBOARD_ENV: &str = "COPYPASTE_FAKE_CLIPBOARD";

// ---------------------------------------------------------------------------
// The port
// ---------------------------------------------------------------------------

/// One captured clipboard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub content: String,
    /// `"text"` for the MVP. Image and file representations (I-11..I-16) are
    /// not captured yet.
    pub content_type: String,
}

impl Capture {
    fn text(content: String) -> Self {
        Self {
            content,
            content_type: "text".to_string(),
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
    /// `allow(dead_code)`: the daemon is a binary, so until the status handler
    /// reads this the compiler cannot see a caller. Delete the attribute — do
    /// not delete the method — when `StatusResponse` carries it.
    #[allow(dead_code)]
    fn rejected_too_large_count(&self) -> u64 {
        0
    }

    /// How many clipboard values were overwritten before we could observe them.
    ///
    /// §3.1/§3.2: `changeCount` is lossy — a delta above 1 means intermediate
    /// values existed and are irrecoverable. This is the telemetry side-channel
    /// the manifest asks for; a burst is *never* modelled as a content value
    /// (§6.1). Same `allow` rationale as above.
    #[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// Change detection — written once, used by every backend
// ---------------------------------------------------------------------------

/// What a `changeCount` observation means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Nothing moved. The caller MUST NOT read any representation (I-1).
    Unchanged,
    /// Our own write, coming back to us. Acknowledged and dropped (§3.3 step 6).
    SelfWrite,
    /// A genuine change. `lost_intermediates` is telemetry only — the caller
    /// falls through and captures the current value regardless (§3.2).
    Fresh { lost_intermediates: u64 },
}

/// The self-write sentinel of §3.3.
///
/// One atomic cell with acquire/release ordering, deliberately behind an `Arc`
/// and deliberately a *single* primitive: the manifest requires that sync
/// auto-apply and relay auto-apply reuse this same cell rather than growing
/// their own (CopyPaste-7ub), so remotely-synced items written to the pasteboard
/// are not re-captured as local ones. Clone it to share it; do not copy the
/// protocol.
#[derive(Debug, Clone)]
struct SelfWriteSentinel(Arc<AtomicI64>);

impl SelfWriteSentinel {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(COUNT_NONE)))
    }

    /// Step 2 — pre-stamp `pre + 2` *before* touching the pasteboard.
    ///
    /// v1 stamped after the write; a poll landing in that window recorded the
    /// item we had just pasted as a brand-new capture (Fix-4 / "DUP-ON-COPY").
    fn arm(&self, pre: i64) {
        self.0.store(pre + SELF_WRITE_DELTA, Ordering::Release);
    }

    /// Step 4 — post-stamp, but only if the write landed exactly where we
    /// predicted.
    ///
    /// CopyPaste-8yzf: if a third-party app wrote between our `set…` and this
    /// read, `actual > pre + 2`. Storing `actual` unconditionally would stamp
    /// *their* change count and make the monitor suppress *their* content as if
    /// it were ours — silently dropping a genuine user copy. So the mismatch
    /// branch leaves the pre-stamped expectation alone.
    fn confirm(&self, pre: i64, actual: i64) {
        if actual == pre + SELF_WRITE_DELTA {
            self.0.store(actual, Ordering::Release);
        }
    }

    /// Step 5 — any write failure resets the cell.
    ///
    /// A stale sentinel that is never consumed permanently suppresses whichever
    /// future genuine capture happens to land on that change count.
    fn clear(&self) {
        self.0.store(COUNT_NONE, Ordering::Release);
    }

    /// Step 6 — the suppression is for exactly one change.
    fn consume_if(&self, count: i64) -> bool {
        count >= 0
            && self
                .0
                .compare_exchange(count, COUNT_NONE, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
}

/// The change-count cursor plus the sentinel: everything a backend needs to
/// decide *whether* to read the clipboard, and nothing about reading it.
#[derive(Debug)]
struct ChangeTracker {
    /// I-2: starts at a sentinel distinguishable from every valid change count.
    cursor: i64,
    sentinel: SelfWriteSentinel,
    lost_intermediates: u64,
}

impl ChangeTracker {
    fn new() -> Self {
        Self {
            cursor: COUNT_NONE,
            sentinel: SelfWriteSentinel::new(),
            lost_intermediates: 0,
        }
    }

    /// Classify a `changeCount` reading.
    ///
    /// Every branch that returns something other than `Fresh` has already
    /// advanced the cursor (I-3): skipping without advancing re-offers the same
    /// change forever, and advancing without capturing is the intended
    /// "acknowledge and drop".
    fn observe(&mut self, count: i64) -> Change {
        // I-1: the comparison is the first thing the poll does, so an unchanged
        // clipboard costs one field read and zero allocations.
        if count == self.cursor {
            return Change::Unchanged;
        }

        // §3.3 step 6: consume the sentinel exactly once.
        if self.sentinel.consume_if(count) {
            self.cursor = count;
            return Change::SelfWrite;
        }

        // I-4: the delta comes from the *old* cursor value, before advancing.
        // I-2: a cursor still at the sentinel is startup, not a burst.
        let lost = if self.cursor < 0 {
            0
        } else {
            let delta = count - self.cursor;
            if delta >= BURST_THRESHOLD {
                // The current value survives; only the ones in between are lost.
                (delta - 1) as u64
            } else {
                0
            }
        };
        self.cursor = count;
        self.lost_intermediates += lost;

        // §3.2: fall through to capture. Never return "a burst happened"
        // *instead of* the content — that bug permanently lost the most recent
        // item every time a user copied three things quickly.
        Change::Fresh {
            lost_intermediates: lost,
        }
    }
}

// ---------------------------------------------------------------------------
// FakeClipboard — the test/dev backend
// ---------------------------------------------------------------------------

/// Test/dev backend. Polls the file named by `COPYPASTE_FAKE_CLIPBOARD` (if
/// set) and otherwise holds content in memory. Public so tests and the demo can
/// drive it.
///
/// It models the same machine the real backend does — a monotonic change
/// counter, one current value, no history — so tests written against it hold
/// for `NSPasteboard`: copying three things quickly leaves only the third, and
/// the fake will not pretend otherwise.
pub struct FakeClipboard {
    tracker: ChangeTracker,
    /// The fake's `changeCount`. Bumped by `push_external` (+1), by
    /// `set_contents` (+2, mirroring clear+set) and by an observed external
    /// edit of the watched file (+1).
    change_count: i64,
    /// The single current clipboard value. `None` means "empty clipboard", or
    /// "the last change was dropped by a gate" (I-3: the cursor still moved).
    contents: Option<String>,
    watched: Option<WatchedFile>,
    rejected_too_large: u64,
}

/// The watched-file half of the fake.
struct WatchedFile {
    path: PathBuf,
    /// Cheap stamp so an unchanged file is never re-read — the fake's analogue
    /// of I-1 ("no representation is read when nothing changed").
    last_seen: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FakeClipboard {
    pub fn new() -> Self {
        match std::env::var_os(FAKE_CLIPBOARD_ENV) {
            Some(path) if !path.is_empty() => Self::watching(PathBuf::from(path)),
            _ => Self::in_memory(),
        }
    }

    fn in_memory() -> Self {
        Self {
            tracker: ChangeTracker::new(),
            change_count: 0,
            contents: None,
            watched: None,
            rejected_too_large: 0,
        }
    }

    fn watching(path: PathBuf) -> Self {
        Self {
            watched: Some(WatchedFile {
                path,
                last_seen: None,
            }),
            ..Self::in_memory()
        }
    }

    /// Simulate an external app copying something.
    ///
    /// Public API of the port even though the daemon binary usually drives the
    /// fake over IPC instead.
    #[allow(dead_code)]
    pub fn push_external(&mut self, text: &str) {
        self.contents = Some(text.to_string());
        self.change_count += 1;
    }

    /// Fold any external edit of the watched file into the change counter.
    ///
    /// Returns without reading anything when the file has not moved, which is
    /// the property I-1 asks of the real backend.
    fn sync_watched_file(&mut self) {
        let Some(watched) = self.watched.as_mut() else {
            return;
        };
        // A missing or unreadable file is simply "no change" — the fake must be
        // a silent no-op when nothing is driving it (§3.11).
        let Ok(meta) = std::fs::metadata(&watched.path) else {
            return;
        };
        let stamp = FileStamp {
            modified: meta.modified().ok(),
            len: meta.len(),
        };
        if watched.last_seen == Some(stamp) {
            return;
        }
        watched.last_seen = Some(stamp);

        // I-18's shape: the size is checked before the bytes are copied in, and
        // the rejection is counted rather than dropped in silence (§6.5).
        if stamp.len > MAX_TEXT_BYTES as u64 {
            self.rejected_too_large += 1;
            warn!(
                bytes = stamp.len,
                cap = MAX_TEXT_BYTES,
                "clipboard text exceeds the size cap; dropped"
            );
            // I-3: acknowledge the change so it is not re-offered forever.
            self.contents = None;
            self.change_count += 1;
            return;
        }

        match std::fs::read(&watched.path) {
            Ok(bytes) => {
                // I-9: byte counts, never content, and never the path.
                let text = String::from_utf8_lossy(&bytes).into_owned();
                // Fake-only affordance: `echo hi > file` is the demo, and the
                // trailing newline is the shell's, not the user's.
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                self.contents = Some(text);
                self.change_count += 1;
            }
            Err(_) => {
                debug!("fake clipboard file could not be read; change ignored");
            }
        }
    }
}

impl Default for FakeClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardSource for FakeClipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.sync_watched_file();

        match self.tracker.observe(self.change_count) {
            Change::Unchanged => return None,
            Change::SelfWrite => {
                debug!("suppressed our own clipboard write");
                return None;
            }
            Change::Fresh { lost_intermediates } => {
                if lost_intermediates > 0 {
                    warn!(
                        lost = lost_intermediates,
                        "clipboard burst: intermediate values are irrecoverable"
                    );
                }
            }
        }

        // §3.2: whatever the burst arithmetic said, the surviving value is
        // returned. §3.20: no content dedup here — re-copying the same text is
        // a real change and ingest is what collapses it into a recency bump.
        let content = self.contents.clone()?;
        if content.is_empty() {
            return None;
        }
        Some(Capture::text(content))
    }

    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        let pre = self.change_count;
        // §3.3 step 2: pre-stamp before touching the clipboard.
        self.tracker.sentinel.arm(pre);

        if let Some(watched) = self.watched.as_mut() {
            if let Err(err) = std::fs::write(&watched.path, text.as_bytes()) {
                // §3.3 step 5: a failed write must not leave a stale sentinel.
                self.tracker.sentinel.clear();
                // I-9 / CLAUDE.md rule 4: the message carries no path.
                return Err(anyhow::anyhow!(
                    "could not write the fake clipboard file: {}",
                    err.kind()
                ));
            }
            // Our own write must not also be seen as an external edit.
            watched.last_seen = std::fs::metadata(&watched.path).ok().map(|m| FileStamp {
                modified: m.modified().ok(),
                len: m.len(),
            });
        }

        self.contents = Some(text.to_string());
        self.change_count = pre + SELF_WRITE_DELTA;
        // §3.3 step 4: conditional post-stamp.
        self.tracker.sentinel.confirm(pre, self.change_count);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        if self.watched.is_some() {
            "fake-file"
        } else {
            "fake-memory"
        }
    }

    fn rejected_too_large_count(&self) -> u64 {
        self.rejected_too_large
    }

    fn lost_intermediates_count(&self) -> u64 {
        self.tracker.lost_intermediates
    }
}

// ---------------------------------------------------------------------------
// macOS — NSPasteboard
// ---------------------------------------------------------------------------

/// The real backend.
///
/// **Unverified on this host.** CopyPaste develops on Linux; nothing in this
/// module has been compiled or run. The logic is written against manifest 01
/// and the shared [`ChangeTracker`] it uses *is* tested. The binding-level
/// assumptions to confirm on a mac are listed on [`macos::MacOsClipboard`].
#[cfg(target_os = "macos")]
mod macos {
    // objc2 0.2/0.5 marks a shifting set of pasteboard accessors `unsafe`.
    // Wrapping every call and allowing the redundant ones keeps this module
    // compiling across binding revisions instead of flipping with each bump.
    #![allow(unused_unsafe)]

    use super::{
        Capture, Change, ChangeTracker, ClipboardSource, MAX_TEXT_BYTES, SELF_WRITE_DELTA,
    };

    use objc2::rc::{autoreleasepool, Retained};
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSArray, NSString};
    use tracing::{debug, warn};

    /// UTIs spelled literally rather than pulled from `NSPasteboardType*`
    /// statics: the values are frozen by the OS and by nspasteboard.org, and
    /// this way the module does not depend on which binding revision exports
    /// which constant.
    const UTI_TEXT: &str = "public.utf8-plain-text";
    /// §3.4 — the three `org.nspasteboard.*` opt-out markers, probed as a set.
    /// `ConcealedType` is how 1Password, Bitwarden, KeePassXC and friends say
    /// "this is a secret, do not persist it". Ignoring these writes users'
    /// master passwords into a searchable history.
    const UTI_MARKERS: [&str; 3] = [
        "org.nspasteboard.TransientType",
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.AutoGeneratedType",
    ];

    /// §3.12 (CopyPaste-pbre): the invariant UTI strings are process-lifetime
    /// constants, built once and reused. v1 allocated ~12 fresh Cocoa strings on
    /// every changed tick.
    ///
    /// A `thread_local` rather than a `static`: these are strong references, so
    /// they are deliberately *not* in the per-tick autorelease pool, and this
    /// form needs no `Send`/`Sync` claim about `NSString` from whichever
    /// binding revision is in the tree.
    struct Utis {
        text: Retained<NSString>,
        text_probe: Retained<NSArray<NSString>>,
        markers: Retained<NSArray<NSString>>,
    }

    impl Utis {
        fn new() -> Self {
            let text = NSString::from_str(UTI_TEXT);
            let markers: Vec<Retained<NSString>> =
                UTI_MARKERS.iter().map(|s| NSString::from_str(s)).collect();
            let marker_refs: Vec<&NSString> = markers.iter().map(|s| &**s).collect();
            Self {
                text_probe: NSArray::from_slice(&[&*text]),
                markers: NSArray::from_slice(&marker_refs),
                text,
            }
        }
    }

    thread_local! {
        static UTIS: Utis = Utis::new();
    }

    /// `NSPasteboard`-backed clipboard source.
    ///
    /// Holds no Cocoa objects: the general pasteboard is a cheap singleton
    /// lookup performed inside each poll's autorelease pool, which keeps this
    /// type unconditionally `Send` for the poll task.
    ///
    /// Binding-level assumptions to confirm on a mac (the *logic* is manifest,
    /// the *spelling* is not): `NSPasteboard::generalPasteboard`,
    /// `changeCount`, `clearContents`, `availableTypeFromArray`, `dataForType`,
    /// `setString_forType`, `NSArray::from_slice`, `NSData::length`/`bytes`.
    pub struct MacOsClipboard {
        tracker: ChangeTracker,
        rejected_too_large: u64,
    }

    impl MacOsClipboard {
        pub fn new() -> Self {
            Self {
                tracker: ChangeTracker::new(),
                rejected_too_large: 0,
            }
        }
    }

    impl ClipboardSource for MacOsClipboard {
        fn poll(&mut self) -> Option<Capture> {
            // I-17: one autorelease pool around the entire Cocoa interaction.
            // Without it, autoreleased NSString/NSData accumulate on the async
            // worker thread and reserved memory grows without bound.
            autoreleasepool(|_pool| {
                let pb = unsafe { NSPasteboard::generalPasteboard() };

                // I-1: the change-count comparison is the first thing we do; an
                // unchanged pasteboard performs zero reads and zero allocations.
                let count = unsafe { pb.changeCount() } as i64;
                match self.tracker.observe(count) {
                    Change::Unchanged => return None,
                    Change::SelfWrite => {
                        debug!(change_count = count, "suppressed our own pasteboard write");
                        return None;
                    }
                    Change::Fresh { lost_intermediates } => {
                        if lost_intermediates > 0 {
                            // §3.1/§3.2: telemetry only. NSPasteboard keeps no
                            // history, so the intermediates are gone — but the
                            // surviving value is captured below, never replaced
                            // by a "burst happened" result.
                            warn!(
                                lost = lost_intermediates,
                                change_count = count,
                                "pasteboard burst: intermediate values are irrecoverable"
                            );
                        }
                    }
                }

                // I-5 / §3.4: probe the opt-out markers BEFORE reading any
                // representation, so a password never enters process memory at
                // all. "Read then discard" is not compliant. A pasteboard
                // carrying a marker *and* a normal string is dropped entirely;
                // that is intentional.
                let marked =
                    UTIS.with(|utis| unsafe { pb.availableTypeFromArray(&utis.markers).is_some() });
                if marked {
                    // I-9: no content, no type name.
                    debug!(
                        change_count = count,
                        "pasteboard change carries an org.nspasteboard.* opt-out marker; dropped"
                    );
                    return None;
                }

                // I-11: text is the highest-priority representation. Image and
                // file capture are not implemented yet; when they are, they go
                // below this branch, and image *presence* must be probed
                // without materialising the bytes (I-12).
                let data = UTIS.with(|utis| unsafe {
                    if pb.availableTypeFromArray(&utis.text_probe).is_none() {
                        return None;
                    }
                    pb.dataForType(&utis.text)
                })?;

                // I-18 (CopyPaste-1f5c): `length` is a field read, `to_vec` on a
                // multi-GiB item is a multi-GiB allocation. Check first.
                let len = unsafe { data.length() };
                if len > MAX_TEXT_BYTES {
                    self.rejected_too_large += 1;
                    // I-39 / §6.5: counted, not silently dropped.
                    warn!(
                        bytes = len,
                        cap = MAX_TEXT_BYTES,
                        "pasteboard text exceeds the size cap; dropped"
                    );
                    return None;
                }

                let bytes = unsafe { data.bytes() }.to_vec();
                // public.utf8-plain-text is UTF-8 by definition; malformed input
                // is a third-party app's bug. §3.6's precedent is lossy
                // conversion rather than dropping the user's copy, and I-37
                // forbids panicking on a malformed payload.
                let content = String::from_utf8_lossy(&bytes).into_owned();
                if content.is_empty() {
                    return None;
                }
                Some(Capture::text(content))
            })
        }

        fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
            // I-17 again: the paste-back path needs the pool as much as the poll.
            autoreleasepool(|_pool| {
                let pb = unsafe { NSPasteboard::generalPasteboard() };
                let pre = unsafe { pb.changeCount() } as i64;

                // §3.3 step 2: pre-stamp `pre + 2` BEFORE touching the
                // pasteboard. Stamping afterwards leaves a window in which a
                // poll records the item we just pasted as a fresh capture.
                self.tracker.sentinel.arm(pre);

                let value = NSString::from_str(text);
                let ok = UTIS.with(|utis| unsafe {
                    // clearContents (+1) then setString:forType: (+1) = the
                    // expected delta of 2.
                    let _ = pb.clearContents();
                    pb.setString_forType(&value, &utis.text)
                });
                if !ok {
                    // §3.3 step 5.
                    self.tracker.sentinel.clear();
                    // I-9 / CLAUDE.md rule 4: no paths, no content.
                    return Err(anyhow::anyhow!("the pasteboard rejected the write"));
                }

                // §3.3 step 4 (CopyPaste-8yzf): conditional post-stamp. If a
                // third-party app wrote between our `setString` and this read,
                // `actual > pre + 2`, and storing it would suppress *their*
                // content as if it were ours.
                let actual = unsafe { pb.changeCount() } as i64;
                self.tracker.sentinel.confirm(pre, actual);
                if actual != pre + SELF_WRITE_DELTA {
                    debug!(
                        expected = pre + SELF_WRITE_DELTA,
                        actual, "another app wrote to the pasteboard during our write"
                    );
                }
                Ok(())
            })
        }

        fn backend_name(&self) -> &'static str {
            "nspasteboard"
        }

        fn rejected_too_large_count(&self) -> u64 {
            self.rejected_too_large
        }

        fn lost_intermediates_count(&self) -> u64 {
            self.tracker.lost_intermediates
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// These are the acceptance tests of manifest §5 that the port makes reachable.
// v1 could not write any of them: it had no seam, so its "contract tests"
// re-stated the rule in a comment and asserted something trivially true (§6.7).

#[cfg(test)]
mod tests {
    use super::*;

    fn fake() -> FakeClipboard {
        FakeClipboard::in_memory()
    }

    // -- the fake, driven as a clipboard ------------------------------------

    /// T-1 — an idle clipboard is free, and stays quiet however often it is
    /// polled.
    #[test]
    fn no_change_returns_none() {
        let mut cb = fake();
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.backend_name(), "fake-memory");
    }

    /// T-2 — the first change after startup is captured and is not a burst.
    #[test]
    fn push_surfaces_exactly_once() {
        let mut cb = fake();
        cb.push_external("hello");

        let captured = cb.poll().expect("a pushed value must surface");
        assert_eq!(captured.content, "hello");
        assert_eq!(captured.content_type, "text");
        assert_eq!(cb.lost_intermediates_count(), 0);

        // The change was acknowledged, not re-offered.
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
    }

    /// T-8 — a self-write is suppressed. T-9 — the suppression is one-shot, so
    /// the next genuine copy is still captured.
    #[test]
    fn set_contents_does_not_self_capture() {
        let mut cb = fake();
        cb.set_contents("pasted by us").expect("in-memory write");

        assert_eq!(cb.poll(), None, "our own write must not be re-captured");
        assert_eq!(cb.poll(), None);

        cb.push_external("copied by a user");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("copied by a user".to_string()),
            "suppression must be for exactly one change"
        );
    }

    /// §3.2 / T-5 — the manifest's highest-value rule. Three copies with one
    /// poll: the intermediates are gone (NSPasteboard keeps no history) but the
    /// survivor MUST be captured. v1 advanced the cursor and returned a
    /// burst-only result, so the most recent item was lost permanently.
    #[test]
    fn burst_does_not_eat_the_survivor() {
        let mut cb = fake();
        // Prime the cursor: I-2 means the very first poll is never a burst, so
        // the burst arithmetic is only meaningful on a running daemon.
        cb.push_external("earlier");
        assert_eq!(cb.poll().map(|c| c.content), Some("earlier".to_string()));

        cb.push_external("one");
        cb.push_external("two");
        cb.push_external("three");

        let captured = cb.poll().expect("the surviving value must be captured");
        assert_eq!(captured.content, "three");
        assert_eq!(
            cb.lost_intermediates_count(),
            2,
            "reported as telemetry only"
        );

        // T-6 — normal capture resumes afterwards.
        cb.push_external("after-burst");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("after-burst".to_string())
        );
    }

    /// Several rapid pushes, each polled: every one surfaces, in order, exactly
    /// once.
    #[test]
    fn rapid_pushes_each_surface() {
        let mut cb = fake();
        for text in ["a", "b", "c", "d"] {
            cb.push_external(text);
            assert_eq!(cb.poll().map(|c| c.content), Some(text.to_string()));
            assert_eq!(cb.poll(), None);
        }
        assert_eq!(cb.lost_intermediates_count(), 0);
    }

    /// §3.20 — the change-detection layer must not dedup by content. Re-copying
    /// the same text is a real change; ingest is what collapses it into a
    /// recency bump, and it can only do that if it sees it.
    #[test]
    fn recopying_the_same_text_is_re_emitted() {
        let mut cb = fake();
        cb.push_external("same");
        assert_eq!(cb.poll().map(|c| c.content), Some("same".to_string()));
        cb.push_external("same");
        assert_eq!(cb.poll().map(|c| c.content), Some("same".to_string()));
    }

    // -- the fake, driven from a file ---------------------------------------

    #[test]
    fn watched_file_is_captured_once_per_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());
        assert_eq!(cb.backend_name(), "fake-file");

        // A missing file is a silent no-op, never an error (§3.11's contract).
        assert_eq!(cb.poll(), None);

        // The trailing newline `echo` adds is the shell's, not the user's.
        std::fs::write(&path, b"from a file\n").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("from a file".to_string())
        );
        assert_eq!(cb.poll(), None);

        std::fs::write(&path, b"second").expect("write");
        assert_eq!(cb.poll().map(|c| c.content), Some("second".to_string()));
        assert_eq!(cb.poll(), None);
    }

    /// The write side of the file backend: `set_contents` puts the text where an
    /// external reader can see it, and still suppresses the self-write.
    #[test]
    fn file_backed_set_contents_does_not_self_capture() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());

        cb.set_contents("pasted by us").expect("file write");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read back"),
            "pasted by us"
        );
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);

        std::fs::write(&path, b"a user copy").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("a user copy".to_string())
        );
    }

    /// I-39 / §6.5 — an oversized value is rejected, counted, and acknowledged
    /// (not re-offered forever). v1 dropped oversized images in complete
    /// silence.
    #[test]
    fn oversized_content_is_rejected_and_counted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());

        std::fs::write(&path, vec![b'x'; MAX_TEXT_BYTES + 1]).expect("write");
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.rejected_too_large_count(), 1);
        // I-3: acknowledged, so it is not offered again.
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.rejected_too_large_count(), 1);

        std::fs::write(&path, b"small again").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("small again".to_string())
        );
    }

    // -- the change tracker: the invariants that used to be untestable -------
    //
    // This is the state machine the macOS backend runs; testing it here is what
    // makes I-2, I-3, I-4, §3.2 and the whole §3.3 sentinel protocol verifiable
    // on a host with no window server.

    #[test]
    fn unchanged_count_is_never_re_offered() {
        let mut t = ChangeTracker::new();
        assert!(matches!(t.observe(7), Change::Fresh { .. }));
        assert_eq!(t.observe(7), Change::Unchanged);
        assert_eq!(t.observe(7), Change::Unchanged);
    }

    /// I-2 / T-2 — a first poll at an arbitrary change count is not a burst.
    #[test]
    fn first_observation_is_not_a_burst() {
        let mut t = ChangeTracker::new();
        assert_eq!(
            t.observe(4321),
            Change::Fresh {
                lost_intermediates: 0
            }
        );
        assert_eq!(t.lost_intermediates, 0);
    }

    /// T-7 — the threshold boundary. Delta 2 is a paste-back pair, not a burst.
    #[test]
    fn burst_threshold_boundary() {
        let mut t = ChangeTracker::new();
        t.observe(10);
        assert_eq!(
            t.observe(12),
            Change::Fresh {
                lost_intermediates: 0
            },
            "a delta of 2 is the clear+set pair, not a burst"
        );
        assert_eq!(
            t.observe(15),
            Change::Fresh {
                lost_intermediates: 2
            },
            "a delta of 3 loses the two values in between"
        );
    }

    /// I-4 — the delta is computed from the old cursor, and the cursor advances
    /// afterwards.
    #[test]
    fn cursor_advances_after_the_delta_is_computed() {
        let mut t = ChangeTracker::new();
        t.observe(100);
        assert_eq!(
            t.observe(110),
            Change::Fresh {
                lost_intermediates: 9
            }
        );
        assert_eq!(t.cursor, 110);
        assert_eq!(t.observe(110), Change::Unchanged);
    }

    /// T-8 / T-9 — the sentinel suppresses exactly one change and then a
    /// genuine copy is captured. I-3 — the drop path advanced the cursor.
    #[test]
    fn self_write_sentinel_suppresses_exactly_one_change() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let pre = 10;
        t.sentinel.arm(pre); // step 2
        t.sentinel.confirm(pre, pre + SELF_WRITE_DELTA); // step 4

        assert_eq!(t.observe(12), Change::SelfWrite);
        assert_eq!(t.observe(12), Change::Unchanged, "cursor advanced (I-3)");
        assert!(matches!(t.observe(13), Change::Fresh { .. }));
    }

    /// T-11 / CopyPaste-8yzf — a third-party app writing between our `set…` and
    /// our post-write read must not have its content suppressed as if it were
    /// ours.
    #[test]
    fn third_party_write_during_ours_is_still_captured() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let pre = 10;
        t.sentinel.arm(pre);
        // The post-write count is pre+3: someone else wrote in between.
        t.sentinel.confirm(pre, pre + 3);

        // Our own write is still suppressed …
        assert_eq!(t.observe(12), Change::SelfWrite);
        // … and theirs is captured. An unconditional post-stamp would have
        // stored 13 here and silently dropped a genuine user copy.
        assert!(matches!(t.observe(13), Change::Fresh { .. }));
    }

    /// T-12 — a failed write clears the sentinel, so it cannot suppress some
    /// unrelated future capture that happens to land on that change count.
    #[test]
    fn failed_write_clears_the_sentinel() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        t.sentinel.arm(10);
        t.sentinel.clear(); // step 5

        assert!(matches!(t.observe(12), Change::Fresh { .. }));
    }

    /// The sentinel is one shared primitive (§3.3, CopyPaste-7ub): sync
    /// auto-apply arms the same cell the poller consumes, rather than growing a
    /// second copy of the protocol.
    #[test]
    fn sentinel_is_shared_not_duplicated() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let elsewhere = t.sentinel.clone();
        elsewhere.arm(10);
        elsewhere.confirm(10, 12);

        assert_eq!(t.observe(12), Change::SelfWrite);
        // Consumed once, by whoever polls first.
        assert!(!elsewhere.consume_if(12));
    }
}
