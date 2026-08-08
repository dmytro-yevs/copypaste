//! Android clipboard capture — the four-rung ladder, rung 0 and rung 2.
//!
//! `docs/rewrite/android-clipboard-access.md` is the specification; this module
//! is its implementation, and ADR-0005 records what shape the implementation
//! took and why. The one-paragraph version: Android 10 removed the only moment
//! worth capturing, so rung 0 buys capture back with gestures that give the app
//! focus — the share sheet, the text-selection action, a Quick Settings tile,
//! plus the Mac's history over sync — and rung 2 buys real background capture
//! by making the binder call as the shell uid through Shizuku.
//!
//! # The split between here and Kotlin
//!
//! ADR-0002 deleted ~2,500 lines of Kotlin that no machine in this project
//! could compile. That failure is easy to repeat here, because the platform
//! genuinely needs a native layer. So the line is drawn hard:
//!
//! * **Kotlin reports facts.** Is Shizuku installed, is it running, did a read
//!   return, here is the text. It contains no policy and no user-facing
//!   sentence it invented.
//! * **[`model`] decides what the facts mean**, in ordinary Rust that compiles
//!   and is tested on a Linux host with no SDK, including every sentence the
//!   user reads.
//!
//! # Why a trait, when only one platform has a ladder
//!
//! The same reason [`crate::backend`] has one: `crate::commands` must contain
//! no `cfg`, so the command names and shapes are identical on both platforms
//! and the React side never branches. macOS answers with the one state it has —
//! the background service captures everything — and the status surface renders
//! that with the same component.
//!
//! # Unverified
//!
//! Everything Android-specific. This host has no SDK and no NDK — `cargo check
//! --target aarch64-linux-android` stops at SQLCipher for want of an NDK
//! `clang` — so [`android`] has never been compiled, the Kotlin under
//! `gen/android/app/src/main/java/` has never been compiled, and the central
//! claim (that a binder call as the shell uid reads the clipboard with no
//! focus) is derived from AOSP source and has never been observed.
//! `docs/rewrite/android-spike.md` is the checklist for the first device run.

use crate::backend::Result;

pub mod intake;
pub mod messages;
pub mod model;

pub use model::{CaptureSnapshot, Clip};

/// Everything the capture ladder can be asked to do.
///
/// Synchronous: every implementation is either a lock over an in-memory model
/// or a JNI call into the app's own process, and there is nothing here worth an
/// executor. The two operations that touch the database — accepting a captured
/// clip — live in [`intake`] instead, where they can await the backend.
pub trait CaptureControl: Send + Sync + 'static {
    /// The current state, without asking the platform anything.
    fn snapshot(&self) -> CaptureSnapshot;

    /// Re-ask the platform, then answer.
    ///
    /// Called on every entry point, not just startup: the android doc's §5
    /// rule 6, after v1 shipped a `remember(ctx)` that never refreshed once the
    /// user came back from system Settings. A grant can also disappear while
    /// the app is in the background, which is the whole subject of this module.
    fn refresh(&self) -> CaptureSnapshot;

    /// Ask for the permission if it is missing, otherwise register the clip
    /// listener and try one read.
    ///
    /// Both steps behind one call, so the setup screen and the loss
    /// notification have one button between them whatever state they find. The
    /// permission request returns immediately and is answered in Shizuku's own
    /// dialog; the refresh on the next resume picks the grant up.
    ///
    /// The read it takes is *not* proof — it happens with the app in front.
    /// See [`model::CaptureModel::record_read`].
    fn arm(&self) -> Result<CaptureSnapshot>;

    /// Unregister and stop. Does not revoke anything — Shizuku's permission is
    /// the user's to withdraw.
    fn disarm(&self) -> Result<CaptureSnapshot>;

    /// Rung 0: read the clipboard right now, from a window that has focus.
    ///
    /// `Ok(None)` means the clipboard was empty, which is not a failure.
    fn read_now(&self, source: model::CaptureSource) -> Result<Option<Clip>>;

    /// Take everything the platform has captured and not yet handed over.
    fn drain(&self) -> Result<Vec<Clip>>;

    /// Stop or resume capture at the platform queue boundary. Enabling drops
    /// the ambiguous pending batch before any later drain can replay it.
    fn set_private_mode(&self, enabled: bool) -> Result<()>;

    /// Turn background capture on or off. Off is a choice, not a fault.
    fn set_enabled(&self, enabled: bool) -> Result<CaptureSnapshot>;

    /// Suppress or restore the Android 12+ "Shell pasted from your clipboard"
    /// notice.
    ///
    /// `acknowledged` is the user having been shown [`model::TOAST_EXPLANATION`]
    /// and agreed to it. Enabling without it is refused rather than assumed:
    /// turning off one of the OS's privacy indicators on the user's behalf is
    /// precisely the move a clipboard manager must not make.
    fn set_toast_suppressed(&self, suppressed: bool, acknowledged: bool)
        -> Result<CaptureSnapshot>;

    /// Note that a clip reached the database, for the "last captured" line.
    fn note_stored(&self, at_ms: i64);

    /// Note clips that were captured and could not be stored.
    ///
    /// Surfaced in the snapshot rather than logged, because a copy that was not
    /// saved is exactly the thing the user must not have to discover for
    /// themselves.
    fn note_dropped(&self, count: u64);
}

#[cfg(target_os = "android")]
pub mod android;

#[cfg(not(target_os = "android"))]
pub mod desktop;

/// The capture control this build uses. One alias, resolved at compile time,
/// exactly as [`crate::backend::SelectedBackend`] is.
#[cfg(not(target_os = "android"))]
pub type SelectedCapture = desktop::DesktopCapture;

/// The capture control this build uses.
#[cfg(target_os = "android")]
pub type SelectedCapture = android::AndroidCapture;
