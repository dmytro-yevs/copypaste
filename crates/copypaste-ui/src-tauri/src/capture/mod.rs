//! Android clipboard capture orchestration for rung 0 and rung 2.
//!
//! `docs/rewrite/android-clipboard-access.md` specifies the ladder. Kotlin
//! reports platform facts through `contract`; [`model`] owns every decision
//! and user-facing sentence. [`CaptureControl`] keeps commands platform-neutral.
//!
//! The bridge DTOs compile in Android debug unit-test variants. The Shizuku
//! binder path still needs paired-device evidence; `docs/rewrite/android-spike.md`
//! is that checklist.

use crate::backend::Result;

#[cfg(any(target_os = "android", test))]
mod contract;
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

    /// Ask for the permission if it is missing, otherwise start the background
    /// reader and try one read.
    ///
    /// Both steps behind one call, so the setup screen and the loss
    /// notification have one button between them whatever state they find. The
    /// permission request returns immediately and is answered in Shizuku's own
    /// dialog; the refresh on the next resume picks the grant up.
    ///
    /// The read it takes is *not* proof — it happens with the app in front.
    /// See [`model::CaptureModel::record_read`].
    fn arm(&self) -> Result<CaptureSnapshot>;

    /// Stop. Does not revoke anything — Shizuku's permission is
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

    /// Synchronise the source-exclusion gate that runs before Android reads
    /// clipboard content. `None` pauses implicit external reads while a config
    /// change is in flight; `Some(&[])` is the deliberate fail-open default.
    fn set_excluded_app_bundle_ids(&self, bundle_ids: Option<&[String]>) -> Result<()>;

    /// Queue native acknowledgement playback. Preference policy stays in the
    /// shared shell feedback service.
    fn play_feedback(&self) -> Result<()>;

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

    /// Open Shizuku's app surface on Android.
    fn open_shizuku(&self) -> Result<()>;

    /// Open Android's Developer options so the user can reach Wireless debugging.
    fn open_developer_options(&self) -> Result<()>;

    /// Request Android's battery-optimization exemption for this app.
    fn request_battery_exemption(&self) -> Result<()>;

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
