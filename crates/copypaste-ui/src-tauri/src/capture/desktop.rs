//! The macOS answer: there is no ladder.
//!
//! The background service reads the pasteboard with no permission at all
//! (ADR-0001), so every rung question has one answer and none of them is a
//! thing the user can act on. This exists so `crate::commands` can be written
//! once — see [`super`] — not because macOS has capture states to manage.

use std::sync::Mutex;

use crate::backend::{BackendError, Result};

use super::model::{CaptureModel, CaptureSnapshot, CaptureSource, Clip};
use super::CaptureControl;

/// Why every mutating operation refuses here.
///
/// A structural refusal rather than a silent success: a frontend that reached
/// one of these has a platform branch it should not have, and a quiet `Ok`
/// would hide that until someone noticed the button did nothing.
const MSG_NO_LADDER: &str = "The background service already captures everything you copy here.";

#[derive(Debug)]
pub struct DesktopCapture {
    model: Mutex<CaptureModel>,
}

impl Default for DesktopCapture {
    fn default() -> Self {
        Self {
            model: Mutex::new(CaptureModel::desktop()),
        }
    }
}

impl CaptureControl for DesktopCapture {
    fn snapshot(&self) -> CaptureSnapshot {
        self.model.lock().expect("capture model").snapshot()
    }

    fn refresh(&self) -> CaptureSnapshot {
        self.snapshot()
    }

    fn arm(&self) -> Result<CaptureSnapshot> {
        Err(BackendError::Unsupported(MSG_NO_LADDER))
    }

    fn disarm(&self) -> Result<CaptureSnapshot> {
        Err(BackendError::Unsupported(MSG_NO_LADDER))
    }

    fn read_now(&self, _source: CaptureSource) -> Result<Option<Clip>> {
        Err(BackendError::Unsupported(MSG_NO_LADDER))
    }

    /// Empty rather than a refusal: the drain loop runs on both platforms and
    /// an error here would be a warning logged once a second forever.
    fn drain(&self) -> Result<Vec<Clip>> {
        Ok(Vec::new())
    }

    fn set_enabled(&self, _enabled: bool) -> Result<CaptureSnapshot> {
        Err(BackendError::Unsupported(MSG_NO_LADDER))
    }

    fn set_toast_suppressed(
        &self,
        _suppressed: bool,
        _acknowledged: bool,
    ) -> Result<CaptureSnapshot> {
        // There is no such notice on macOS, and nothing to suppress.
        Err(BackendError::Unsupported(MSG_NO_LADDER))
    }

    fn note_stored(&self, at_ms: i64) {
        self.model
            .lock()
            .expect("capture model")
            .record_capture(at_ms);
    }

    fn note_dropped(&self, count: u64) {
        self.model
            .lock()
            .expect("capture model")
            .record_dropped(count);
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::Rung;
    use super::*;

    #[test]
    fn the_desktop_reports_one_state_and_offers_no_setup() {
        let capture = DesktopCapture::default();
        let snapshot = capture.snapshot();
        assert_eq!(snapshot.rung, Rung::Desktop);
        assert!(snapshot.detail.is_none());
        assert!(matches!(
            capture.arm().unwrap_err(),
            BackendError::Unsupported(_)
        ));
    }

    /// The drain task runs on both platforms; on this one it has to be quiet
    /// rather than an error to log.
    #[test]
    fn the_drain_is_empty_rather_than_an_error() {
        assert!(DesktopCapture::default().drain().unwrap().is_empty());
    }

    #[test]
    fn no_refusal_names_a_path() {
        let capture = DesktopCapture::default();
        let shown = capture.arm().unwrap_err().to_string();
        assert!(!shown.contains('/'), "{shown}");
        assert!(shown.ends_with('.'), "{shown}");
    }
}
