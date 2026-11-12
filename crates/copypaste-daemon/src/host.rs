//! The desktop platform pieces the shared runtime asks of this daemon.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::{
    CapabilityGuarantee, CapabilityState, CapabilityStatus, DiagnosticCounters, PlatformCapability,
};
use copypaste_runtime::{PlatformStatus, RuntimeHost, Settings, SettingsTransition, WriteOptions};

use crate::clipboard::{CapturePolicy, ClipboardSource};

pub type SharedClipboard = Arc<Mutex<Box<dyn ClipboardSource>>>;

/// Lock recovery rather than propagation: a poisoned clipboard mutex means a
/// previous pasteboard call panicked, not that the handle is unusable.
pub fn lock(clipboard: &SharedClipboard) -> MutexGuard<'_, Box<dyn ClipboardSource>> {
    clipboard
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct DaemonHost {
    clipboard: SharedClipboard,
    settings: Arc<Settings>,
    backend_name: &'static str,
    started_at: Instant,
}

impl DaemonHost {
    pub fn new(clipboard: SharedClipboard, settings: Arc<Settings>) -> Self {
        let backend_name = lock(&clipboard).backend_name();
        Self {
            clipboard,
            settings,
            backend_name,
            started_at: Instant::now(),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}

impl RuntimeHost for DaemonHost {
    fn write(
        &self,
        item_id: &str,
        payload: &ClipboardPayload,
        content_type: &str,
        options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        lock(&self.clipboard).write_payload(item_id, payload, content_type, options)
    }

    fn copy_feedback(&self) {
        crate::notify::on_copy(self.settings.get().sound_on_copy, self.backend_name);
    }

    /// One clipboard lock for the transport probes and both adapter counters.
    fn platform_status(&self) -> PlatformStatus {
        let clipboard = lock(&self.clipboard);
        let mut capabilities = clipboard.platform_capabilities();
        let status = PlatformStatus {
            capabilities: BTreeMap::new(),
            clipboard_available: clipboard.is_available(),
            clipboard_backend: clipboard.backend_name().to_owned(),
            counters: DiagnosticCounters {
                rejected_too_large: clipboard.rejected_too_large_count(),
                lost_intermediates: clipboard.lost_intermediates_count(),
                uptime_secs: self.started_at.elapsed().as_secs(),
            },
        };
        drop(clipboard);
        let unprobed = CapabilityStatus {
            state: CapabilityState::Error,
            backend: None,
            reason: Some("No daemon-side probe is registered for this capability.".into()),
            permissions: Vec::new(),
            guarantee: CapabilityGuarantee::None,
            mime_types: Vec::new(),
            max_bytes: None,
        };
        for kind in [
            PlatformCapability::GlobalShortcuts,
            PlatformCapability::QuickPaste,
            PlatformCapability::DesktopIntegration,
            PlatformCapability::Updates,
        ] {
            capabilities.insert(kind, unprobed.clone());
        }
        capabilities.insert(PlatformCapability::SecureStorage, secure_storage());
        PlatformStatus {
            capabilities,
            ..status
        }
    }

    /// Resuming capture advances the native cursor without reading or
    /// replacing what was copied while private mode was on.
    fn settings_applied(&self, transition: &SettingsTransition) {
        if !transition.resumes_capture() {
            return;
        }
        let mut private_policy = transition.config().clone();
        private_policy.private_mode = true;
        lock(&self.clipboard).poll_with_policy(CapturePolicy::new(&private_policy));
    }
}

/// The device key backend this build loaded at startup. Release builds use
/// the platform keystore; debug macOS and Linux builds use a development file.
fn secure_storage() -> CapabilityStatus {
    let backend = if cfg!(target_os = "windows") {
        Some("dpapi")
    } else if cfg!(debug_assertions) {
        None
    } else if cfg!(target_os = "macos") {
        Some("keychain")
    } else if cfg!(target_os = "linux") {
        Some("secret_service")
    } else {
        None
    };
    CapabilityStatus {
        state: CapabilityState::Available,
        backend: Some(backend.unwrap_or("development_file").into()),
        reason: Some(match backend {
            Some(_) => {
                "Device key loaded at daemon startup; current lock state was not probed.".into()
            }
            None => "Development build keeps the device key in a local file.".into(),
        }),
        permissions: Vec::new(),
        guarantee: if backend.is_some() {
            CapabilityGuarantee::BestEffort
        } else {
            CapabilityGuarantee::None
        },
        mime_types: Vec::new(),
        max_bytes: None,
    }
}
