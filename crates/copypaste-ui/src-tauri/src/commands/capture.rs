//! Capture commands: what rung is live, and how to move between them.
//!
//! Written against [`crate::capture::SelectedCapture`], so — like every other
//! command here — the names and shapes are identical on macOS and Android. On
//! macOS the state is constant and the mutating commands refuse, which is what
//! the shared status component renders without a platform branch.
//!
//! Every command that changes anything emits
//! [`crate::events::TauriEventName::CaptureState`] as well as returning the new
//! snapshot. The return value serves the caller; the event serves the history
//! screen, which has to show capture state without having asked for it (the
//! android doc's §5 rule 1).

use tauri::{AppHandle, Emitter, State};

use crate::backend::BackendError;
use crate::capture::intake;
use crate::capture::model::{CaptureSnapshot, CaptureSource, TOAST_EXPLANATION};
use crate::capture::{CaptureControl, SelectedCapture};
use crate::events::TauriEventName;
use crate::model::UiItem;

type Result<T> = std::result::Result<T, BackendError>;

/// The current state, as cheaply as possible.
#[tauri::command]
pub fn capture_state(capture: State<'_, SelectedCapture>) -> CaptureSnapshot {
    capture.snapshot()
}

/// Re-ask the platform and answer.
///
/// The frontend calls this on every resume, not only at startup. A permission
/// can lapse while the app is in the background — a reboot is the ordinary
/// case, and app hibernation the rare one — and v1 shipped a cached state that
/// never refreshed after a trip to system Settings.
#[tauri::command]
pub fn capture_refresh(app: AppHandle, capture: State<'_, SelectedCapture>) -> CaptureSnapshot {
    emit(&app, capture.refresh())
}

/// Register the background listener and try one read.
///
/// The one tap behind the "background capture stopped" notification, and the
/// last step of the rung 2 setup screen.
#[tauri::command]
pub fn capture_arm(app: AppHandle, capture: State<'_, SelectedCapture>) -> Result<CaptureSnapshot> {
    Ok(emit(&app, capture.arm()?))
}

#[tauri::command]
pub fn capture_disarm(
    app: AppHandle,
    capture: State<'_, SelectedCapture>,
) -> Result<CaptureSnapshot> {
    Ok(emit(&app, capture.disarm()?))
}

/// Turn background capture on or off.
///
/// Off leaves rung 0 running, which is why the state it produces is `Disabled`
/// and not a fault: the app is still capturing what the user copies inside it,
/// shares to it, or takes with the tile.
#[tauri::command]
pub fn capture_set_enabled(
    app: AppHandle,
    capture: State<'_, SelectedCapture>,
    enabled: bool,
) -> Result<CaptureSnapshot> {
    Ok(emit(&app, capture.set_enabled(enabled)?))
}

/// Rung 0: save whatever is on the clipboard right now.
///
/// `Ok(None)` means the clipboard was empty. Behind the Quick Settings tile,
/// and behind the in-app capture button — the tile's tap is what gives the
/// window focus, which is what makes the read legal on Android 10 and later.
#[tauri::command]
pub async fn capture_now(app: AppHandle, source: Option<CaptureSource>) -> Result<Option<UiItem>> {
    intake::capture_once(&app, source.unwrap_or(CaptureSource::InApp)).await
}

/// The text that must be on screen before suppression can be accepted.
///
/// A command rather than a string in the frontend so that the wording, the
/// consent gate and the test that checks the wording cannot drift apart.
#[tauri::command]
pub fn capture_toast_explanation() -> &'static str {
    TOAST_EXPLANATION
}

/// Suppress or restore Android's "Shell pasted from your clipboard" notice.
///
/// `acknowledged` must be `true` to turn suppression on, and the frontend may
/// only pass it after showing [`capture_toast_explanation`]. Turning it back
/// **off** needs no acknowledgement: restoring a privacy indicator is never the
/// thing to put a gate in front of.
#[tauri::command]
pub fn capture_set_toast_suppressed(
    app: AppHandle,
    capture: State<'_, SelectedCapture>,
    suppressed: bool,
    acknowledged: bool,
) -> Result<CaptureSnapshot> {
    Ok(emit(
        &app,
        capture.set_toast_suppressed(suppressed, acknowledged)?,
    ))
}

fn emit(app: &AppHandle, snapshot: CaptureSnapshot) -> CaptureSnapshot {
    let _ = app.emit(TauriEventName::CaptureState.as_str(), snapshot.clone());
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frontend sends `"tile"` and `"share"`; a rename on either side has
    /// to fail here rather than silently attributing every capture to the
    /// wrong surface.
    #[test]
    fn the_source_argument_deserialises_from_what_the_frontend_sends() {
        for (json, expected) in [
            (r#""in_app""#, CaptureSource::InApp),
            (r#""tile""#, CaptureSource::Tile),
            (r#""share""#, CaptureSource::Share),
            (r#""process_text""#, CaptureSource::ProcessText),
            (r#""background""#, CaptureSource::Background),
        ] {
            assert_eq!(
                serde_json::from_str::<CaptureSource>(json).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn the_explanation_command_returns_the_text_the_consent_gate_checks() {
        assert_eq!(capture_toast_explanation(), TOAST_EXPLANATION);
    }
}
