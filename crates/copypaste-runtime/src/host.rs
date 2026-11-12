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

/// How one clipboard write is marked for other clipboard readers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// Sensitive content: the platform's concealed marker (macOS
    /// `ConcealedType`, the Windows history exclusion formats, the KDE
    /// password-manager hint, Android `IS_SENSITIVE`).
    pub secret: bool,
    /// A delivery for one paste, not a copy the user keeps: other clipboard
    /// managers and OS clipboard history are asked not to record it (macOS
    /// `TransientType`, the Windows history exclusion formats). Linux has no
    /// such convention. Hosts give no copy feedback for it.
    pub transient: bool,
}

impl WriteOptions {
    /// An ordinary write of history content: concealed when `secret`, never
    /// transient.
    #[must_use]
    pub const fn copy(secret: bool) -> Self {
        Self {
            secret,
            transient: false,
        }
    }
}

pub trait RuntimeHost: Send + Sync {
    /// Write one authenticated payload to the native clipboard. `item_id`
    /// names the row the payload came from, for hosts that stage binary
    /// representations per item.
    fn write(
        &self,
        item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        options: WriteOptions,
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
        self.write(item_id, payload, content_type, WriteOptions::copy(secret))
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
    /// Write `payload` with the native markers `options` asks for.
    fn write(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        options: WriteOptions,
    ) -> Result<(), ClipboardWriteError>;

    /// Write received content and acknowledge success through the native host,
    /// including when no Flutter notification subscriber is running. Secret
    /// content must stay concealed in native notification previews.
    fn write_received(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), ClipboardWriteError> {
        self.write(payload, content_type, WriteOptions::copy(secret))
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
        options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        self.0.write(payload, content_type, options)
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
    fn write(
        &self,
        _: &ClipboardPayload,
        _: &str,
        _: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        Err(ClipboardWriteError::Failed)
    }
}
