//! What a platform host supplies to the shared runtime.
//!
//! The runtime owns history, settings, sync and the request dispatch. A host
//! owns only the native pieces: the clipboard writer, copy feedback, platform
//! status probes and the effects a settings change has on native capture.

use std::collections::BTreeMap;
use std::sync::Arc;

use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::{CapabilityStatus, DiagnosticCounters, PlatformCapability};

use crate::settings::SettingsTransition;

/// Platform facts reported by `status`. Probes, never clipboard content.
#[derive(Debug, Clone)]
pub struct PlatformStatus {
    pub capabilities: BTreeMap<PlatformCapability, CapabilityStatus>,
    pub clipboard_available: bool,
    pub clipboard_backend: String,
    pub counters: DiagnosticCounters,
}

pub trait RuntimeHost: Send + Sync {
    /// Write one authenticated history payload to the native clipboard.
    /// `item_id` names the row the payload came from, for hosts that stage
    /// binary representations per item.
    fn write(
        &self,
        item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError>;

    /// Write content received from another device. Hosts that acknowledge
    /// receipt natively do so here; secret content stays concealed.
    fn write_received(
        &self,
        item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        self.write(item_id, payload, content_type, secret)
    }

    /// Feedback after a successful clipboard write, such as the copy sound.
    fn copy_feedback(&self) {}

    /// Whether a copied file may be written as its platform content URI.
    fn writes_content_uris(&self) -> bool {
        false
    }

    fn platform_status(&self) -> PlatformStatus;

    /// Native effects of an applied settings change, run before the next one
    /// may begin.
    fn settings_applied(&self, _transition: &SettingsTransition) {}
}

/// The writer-only host contract an in-process platform supplies.
pub trait ClipboardWriter: Send + Sync {
    fn write(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
    ) -> Result<(), ClipboardWriteError>;

    /// Preserve classification on hosts that support native sensitive previews.
    fn write_classified(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        let _ = secret;
        self.write(payload, content_type)
    }

    /// Write received content and acknowledge success through the native host,
    /// including when no Flutter notification subscriber is running. Secret
    /// content must stay concealed in native notification previews.
    fn write_received(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        self.write_classified(payload, content_type, secret)
    }
}

/// The Android host: native capture and feedback stay in the platform
/// service, so status reports only the in-process transport.
pub(crate) struct InProcessHost(pub(crate) Arc<dyn ClipboardWriter>);

impl RuntimeHost for InProcessHost {
    fn write(
        &self,
        _item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        self.0.write_classified(payload, content_type, secret)
    }

    fn write_received(
        &self,
        _item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        self.0.write_received(payload, content_type, secret)
    }

    fn writes_content_uris(&self) -> bool {
        true
    }

    fn platform_status(&self) -> PlatformStatus {
        PlatformStatus {
            capabilities: BTreeMap::new(),
            clipboard_available: true,
            clipboard_backend: "android".to_owned(),
            counters: DiagnosticCounters::default(),
        }
    }
}

pub(crate) struct UnavailableClipboard;
impl ClipboardWriter for UnavailableClipboard {
    fn write(&self, _: &ClipboardPayload, _: &str) -> Result<(), ClipboardWriteError> {
        Err(ClipboardWriteError::Failed)
    }
}
