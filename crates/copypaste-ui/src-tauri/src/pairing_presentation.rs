#[cfg(any(target_os = "android", target_os = "macos", target_os = "windows"))]
use std::sync::Arc;

use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
#[cfg(any(target_os = "android", target_os = "windows"))]
use tauri::{AppHandle, Manager as _};
use zeroize::Zeroizing;

mod native_copy;
pub(crate) mod semantics;
pub use semantics::{resolve_pairing_semantics, PairingSemantics};

#[cfg(any(target_os = "android", target_os = "macos", target_os = "windows"))]
pub type NativeAbort = Arc<dyn Fn() + Send + Sync>;

#[cfg(any(target_os = "android", target_os = "windows"))]
pub type NativeRefresh = Arc<dyn Fn() + Send + Sync>;

#[cfg(any(target_os = "android", target_os = "windows"))]
pub fn native_refresh(app: AppHandle) -> NativeRefresh {
    Arc::new(move || {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            use crate::backend::PairingBackend as _;

            let backend = app.state::<crate::SelectedBackend>();
            let Ok(progress) = backend.pair_progress().await else {
                tracing::warn!("pairing deadline refresh failed");
                return;
            };
            let presenter = app.state::<PairingPresenter>();
            presenter.present_progress(&progress);
        });
    })
}

#[cfg(any(
    test,
    feature = "dev-web-bridge",
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
))]
pub(crate) mod invite;

#[cfg(target_os = "android")]
pub(crate) mod android;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "macos"))]
mod macos_model;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub fn windows_ui(abort: NativeAbort, refresh: NativeRefresh) -> impl NativePairingUi {
    windows::WindowsPairingUi::new(
        invite::encode_native_invite,
        invite::validate_native_invite_fields,
        abort,
        refresh,
    )
}

#[cfg(target_os = "macos")]
pub fn macos_ui(abort: NativeAbort) -> impl NativePairingUi {
    macos::MacOsPairingUi::new(abort)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingPresentationState {
    Available,
    Presented,
    Unavailable,
}

pub struct ScannedPairing {
    pub code: Zeroizing<String>,
    pub addr: Zeroizing<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingDecision {
    Accept,
    Reject,
    Cancel,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationOutcome {
    Presented,
    Unavailable,
    Refresh,
}

pub trait NativePairingUi: Send + Sync + 'static {
    fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome;
    fn scan_invite(&self) -> Option<ScannedPairing>;
    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState;
    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision>;
}

pub struct PairingPresenter {
    native: Box<dyn NativePairingUi>,
    available: bool,
}

impl Default for PairingPresenter {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        let native = Box::new(macos::MacOsPairingUi::new(Arc::new(|| {})));
        #[cfg(target_os = "windows")]
        let native = Box::new(windows::WindowsPairingUi::new(
            invite::encode_native_invite,
            invite::validate_native_invite_fields,
            Arc::new(|| {}),
            Arc::new(|| {}),
        ));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let native = Box::new(UnavailablePairingUi);

        Self {
            native,
            available: cfg!(any(target_os = "macos", target_os = "windows")),
        }
    }
}

impl PairingPresenter {
    pub fn new(native: impl NativePairingUi) -> Self {
        Self {
            native: Box::new(native),
            available: true,
        }
    }

    /// Idle reports capability. An active ceremony exists only after its
    /// protected native surface presented successfully.
    pub fn state_for_progress(&self, state: PairingState) -> PairingPresentationState {
        if !self.available {
            PairingPresentationState::Unavailable
        } else if state == PairingState::Idle {
            PairingPresentationState::Available
        } else {
            PairingPresentationState::Presented
        }
    }

    pub fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome {
        self.native.present_invite(invite)
    }

    pub fn scan_invite(&self) -> Option<ScannedPairing> {
        self.native.scan_invite()
    }

    pub fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        self.native.present_progress(progress)
    }

    pub fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision> {
        self.native.confirm(progress)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
struct UnavailablePairingUi;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl NativePairingUi for UnavailablePairingUi {
    fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
        NativePresentationOutcome::Unavailable
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
        None
    }

    fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
        PairingPresentationState::Unavailable
    }

    fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
        None
    }
}

#[cfg(test)]
mod presenter_tests {
    use super::*;

    struct ConfiguredPairingUi;

    impl NativePairingUi for ConfiguredPairingUi {
        fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
            NativePresentationOutcome::Unavailable
        }

        fn scan_invite(&self) -> Option<ScannedPairing> {
            None
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            PairingPresentationState::Unavailable
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            None
        }
    }

    #[test]
    fn configured_renderers_are_available_before_a_ceremony_starts() {
        let presenter = PairingPresenter::new(ConfiguredPairingUi);

        assert_eq!(
            presenter.state_for_progress(PairingState::Idle),
            PairingPresentationState::Available
        );
        assert_eq!(
            presenter.state_for_progress(PairingState::WaitingForPeer),
            PairingPresentationState::Presented
        );
    }
}

#[cfg(all(test, not(any(target_os = "macos", target_os = "windows"))))]
mod tests {
    use super::*;
    use copypaste_ipc::{PairingRole, PairingState};

    fn progress() -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("ceremony-1".into()),
            role: Some(PairingRole::Responder),
            state: PairingState::AwaitingConfirmation,
            expires_in_ms: Some(60_000),
            sas: Some("123456".into()),
            peer_device_id: Some("device-1".into()),
            peer_name: Some("Phone".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code: None,
        }
    }

    #[test]
    fn missing_platform_renderers_are_an_explicit_state() {
        let presenter = PairingPresenter::default();
        let invite = PairingInviteData {
            code: "SECRET-CODE".into(),
            pairing_id: "ceremony-1".into(),
            listen_addr: Some("192.0.2.2:47654".into()),
            expires_in_secs: 120,
        };

        assert_eq!(
            presenter.present_invite(&invite),
            NativePresentationOutcome::Unavailable
        );
        assert_eq!(
            presenter.state_for_progress(PairingState::Idle),
            PairingPresentationState::Unavailable
        );
        assert_eq!(
            presenter.present_progress(&progress()),
            PairingPresentationState::Unavailable
        );
        assert!(presenter.scan_invite().is_none());
        assert!(presenter.confirm(&progress()).is_none());
    }
}

#[cfg(test)]
mod native_pairing_source_contracts {
    fn production(source: &'static str) -> &'static str {
        source
            .split_once("#[cfg(test)]")
            .map_or(source, |part| part.0)
    }

    #[test]
    fn native_pairing_keeps_credentials_off_the_webview_with_platform_entry_parity() {
        let sources = [
            production(include_str!("pairing_presentation/macos.rs")),
            production(include_str!("pairing_presentation/windows/mod.rs")),
            production(include_str!("pairing_presentation/windows/invite.rs")),
            production(include_str!("pairing_presentation/windows/entry.rs")),
            production(include_str!("pairing_presentation/windows/confirm.rs")),
            production(include_str!("pairing_presentation/windows/status.rs")),
            production(include_str!("pairing_presentation/android.rs")),
        ];
        let joined = sources.join("\n");
        for forbidden in ["WebviewWindow", ".eval(", "write_text("] {
            assert!(
                !joined.contains(forbidden),
                "forbidden pairing sink: {forbidden}"
            );
        }
        assert!(joined.contains("encode_native_invite"));
        assert!(joined.contains("decode_native_invite"));
        assert!(joined.contains("validate_native_invite_fields"));
        assert!(joined.contains("Zeroizing"));

        let macos = production(include_str!("pairing_presentation/macos.rs"));
        assert!(macos.matches("NSSecureTextField").count() >= 2);
        assert!(macos.contains("Pairing code"));
        assert!(macos.contains("Pairing address"));
        assert!(macos.contains("setSelectable(false)"));
        for required in [
            "setAccessibilityProtectedContent(true)",
            "NSWindowSharingType::NSWindowSharingNone",
            "Security code: {spoken}",
            "for (index, digit) in sas.chars().enumerate()",
            "NativeAbort",
            "let abort = self.abort.clone()",
            "(self.abort)()",
        ] {
            assert!(macos.contains(required), "missing macOS guard: {required}");
        }

        let windows = production(include_str!("pairing_presentation/windows/entry.rs"));
        assert_eq!(windows.matches("co::ES::PASSWORD").count(), 2);
        assert!(windows.contains("Pairing &code"));
        assert!(windows.contains("Pairing &address"));

        let android = production(include_str!("pairing_presentation/android.rs"));
        assert!(android.contains("\"scanInvite\""));
        assert!(android.contains("decode_native_invite(payload)"));
        let android_plugin = include_str!(
            "../gen/android/app/src/main/java/com/copypaste/app/PairingPresentationPlugin.kt"
        );
        let android_dialog = include_str!(
            "../gen/android/app/src/main/java/com/copypaste/app/PairingDialogController.kt"
        );
        assert!(android_plugin.contains("GmsBarcodeScanning.getClient"));
        assert!(android_plugin.contains("Barcode.FORMAT_QR_CODE"));
        assert!(android_dialog.contains("WindowManager.LayoutParams.FLAG_SECURE"));
    }

    #[test]
    fn native_deadlines_remove_secrets_then_refresh_rust_progress() {
        let macos = production(include_str!("pairing_presentation/macos.rs"));
        assert!(macos.contains("ModalDeadline::arm"));
        assert!(macos.contains("abortModal"));
        assert!(macos.contains("PairingDecision::Refresh"));
        let progress = macos
            .split("fn present_progress")
            .nth(1)
            .and_then(|body| body.split("fn confirm").next())
            .expect("macOS progress implementation");
        assert!(progress.contains("AwaitingConfirmation"));
        assert!(progress.contains("(self.abort)()"));
        assert!(
            progress.find("AwaitingConfirmation").unwrap()
                < progress.find("(self.abort)()").unwrap(),
            "awaiting confirmation must return before Close aborts the ceremony"
        );

        let windows_invite = production(include_str!("pairing_presentation/windows/invite.rs"));
        assert!(windows_invite.contains("SetWindowText(\"\")"));
        assert!(windows_invite.contains("refresh();"));
        let windows_confirm = production(include_str!("pairing_presentation/windows/confirm.rs"));
        assert!(windows_confirm.contains("PairingDecision::Refresh"));
        assert!(!windows_confirm.contains("SAS_TIMEOUT"));
        assert!(!windows_confirm.contains("Pairing timed out"));

        let android = include_str!(
            "../gen/android/app/src/main/java/com/copypaste/app/PairingDialogController.kt"
        );
        assert!(android.contains("reveal.setOnClickListener(null)"));
        assert!(android.contains("sasView?.removeAllViews()"));
        assert!(android.contains("deliver(\"refresh\")"));
        assert!(!android.contains("presentProgress(\"timed_out\")"));
    }
}
