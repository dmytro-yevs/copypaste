#[cfg(any(target_os = "android", target_os = "macos", target_os = "windows"))]
use std::sync::Arc;

use copypaste_ipc::{PairingInviteData, PairingProgressData};
use zeroize::Zeroizing;

#[cfg(any(target_os = "android", target_os = "macos", target_os = "windows"))]
pub type NativeAbort = Arc<dyn Fn() + Send + Sync>;

#[cfg(any(
    test,
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
))]
mod invite;

#[cfg(target_os = "android")]
pub(crate) mod android;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "macos"))]
mod macos_model;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub fn windows_ui(abort: NativeAbort) -> impl NativePairingUi {
    windows::WindowsPairingUi::new(
        invite::encode_native_invite,
        invite::decode_native_invite,
        abort,
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
}

pub trait NativePairingUi: Send + Sync + 'static {
    fn present_invite(&self, invite: &PairingInviteData) -> PairingPresentationState;
    fn scan_invite(&self) -> Option<ScannedPairing>;
    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState;
    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision>;
}

pub struct PairingPresenter {
    native: Box<dyn NativePairingUi>,
}

impl Default for PairingPresenter {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        let native = Box::new(macos::MacOsPairingUi::new(Arc::new(|| {})));
        #[cfg(target_os = "windows")]
        let native = Box::new(windows::WindowsPairingUi::new(
            invite::encode_native_invite,
            invite::decode_native_invite,
            Arc::new(|| {}),
        ));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let native = Box::new(UnavailablePairingUi);

        Self { native }
    }
}

impl PairingPresenter {
    pub fn new(native: impl NativePairingUi) -> Self {
        Self {
            native: Box::new(native),
        }
    }

    pub fn present_invite(&self, invite: &PairingInviteData) -> PairingPresentationState {
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
    fn present_invite(&self, _invite: &PairingInviteData) -> PairingPresentationState {
        PairingPresentationState::Unavailable
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

#[cfg(all(test, not(any(target_os = "macos", target_os = "windows"))))]
mod tests {
    use super::*;
    use copypaste_ipc::{PairingRole, PairingState};

    fn progress() -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("ceremony-1".into()),
            role: Some(PairingRole::Responder),
            state: PairingState::AwaitingConfirmation,
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
    fn windows_and_android_pairing_keep_credentials_off_the_webview() {
        let sources = [
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
        assert!(joined.contains("Zeroizing"));
    }
}
