use copypaste_ipc::{PairingProgressData, PairingRole, PairingState};
use serde::Serialize;
use tauri::State;

use crate::backend::{PairingBackend, Result};
use crate::pairing_presentation::{
    resolve_pairing_semantics, NativePresentationOutcome, PairingDecision,
    PairingPresentationState, PairingPresenter, PairingSemantics,
};
use crate::SelectedBackend;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PairingCeremony {
    ceremony_id: Option<String>,
    role: Option<PairingRole>,
    state: PairingState,
    semantics: PairingSemantics,
    presentation: PairingPresentationState,
    known_device: Option<PairedDevice>,
    error: Option<crate::backend::UiError>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PairedDevice {
    name: String,
    last_seen_ms: i64,
    online: bool,
}

impl PairingCeremony {
    pub(crate) fn from_progress(
        progress: PairingProgressData,
        presentation: PairingPresentationState,
    ) -> Self {
        let semantics = resolve_pairing_semantics(progress.state, progress.error_code);
        let known_device = if progress.state == PairingState::Confirmed {
            progress.known_device.map(|peer| PairedDevice {
                name: peer.name,
                last_seen_ms: peer.last_seen_ms,
                online: peer.online,
            })
        } else {
            None
        };
        Self {
            ceremony_id: progress.pairing_id,
            role: progress.role,
            state: progress.state,
            semantics,
            presentation,
            known_device,
            error: progress
                .error_code
                .map(|code| crate::backend::UiError::from_error_code(Some(code))),
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            ceremony_id: None,
            role: None,
            state: PairingState::Idle,
            semantics: resolve_pairing_semantics(PairingState::Idle, None),
            presentation: PairingPresentationState::Unavailable,
            known_device: None,
            error: None,
        }
    }
}

#[tauri::command]
pub async fn pair_create_invite(
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    let invite = backend.pair_create_invite().await?;
    match presenter.present_invite(&invite) {
        NativePresentationOutcome::Unavailable => backend.pair_cancel().await.map(|progress| {
            PairingCeremony::from_progress(progress, PairingPresentationState::Unavailable)
        }),
        NativePresentationOutcome::Presented => backend.pair_progress().await.map(|progress| {
            PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
        }),
        NativePresentationOutcome::Refresh => {
            let progress = backend.pair_progress().await?;
            let presentation = presenter.present_progress(&progress);
            Ok(PairingCeremony::from_progress(progress, presentation))
        }
    }
}

#[tauri::command]
pub async fn pair_scan_invite(
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    let Some(scanned) = presenter.scan_invite() else {
        return Ok(PairingCeremony::unavailable());
    };
    let progress = backend
        .pair_join(scanned.code.as_str(), scanned.addr.as_str())
        .await?;
    let presentation = presenter.present_progress(&progress);
    Ok(PairingCeremony::from_progress(progress, presentation))
}

#[tauri::command]
pub async fn pair_progress(
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    backend.pair_progress().await.map(|progress| {
        let presentation = presenter.state_for_progress(progress.state);
        PairingCeremony::from_progress(progress, presentation)
    })
}

#[tauri::command]
pub async fn pair_present(
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    let progress = backend.pair_progress().await?;
    let presentation = presenter.present_progress(&progress);
    Ok(PairingCeremony::from_progress(progress, presentation))
}

#[tauri::command]
pub async fn pair_confirm(
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    let progress = backend.pair_progress().await?;
    let Some(decision) = presenter.confirm(&progress) else {
        return Ok(PairingCeremony::from_progress(
            progress,
            PairingPresentationState::Unavailable,
        ));
    };
    let progress = match decision {
        PairingDecision::Accept => backend.pair_confirm(true).await?,
        PairingDecision::Reject => backend.pair_confirm(false).await?,
        PairingDecision::Cancel => backend.pair_cancel().await?,
        PairingDecision::Refresh => {
            let progress = backend.pair_progress().await?;
            let presentation = presenter.present_progress(&progress);
            return Ok(PairingCeremony::from_progress(progress, presentation));
        }
    };
    Ok(PairingCeremony::from_progress(
        progress,
        PairingPresentationState::Presented,
    ))
}

#[tauri::command]
pub async fn pair_reject(backend: State<'_, SelectedBackend>) -> Result<PairingCeremony> {
    backend.pair_confirm(false).await.map(|progress| {
        PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
    })
}

#[tauri::command]
pub async fn pair_cancel(backend: State<'_, SelectedBackend>) -> Result<PairingCeremony> {
    backend.pair_cancel().await.map(|progress| {
        PairingCeremony::from_progress(progress, PairingPresentationState::Unavailable)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::{ErrorCode, PeerInfo};

    #[test]
    fn inv_13_webview_contract_has_no_raw_pairing_material() {
        let generated = include_str!("../../../src/generated/ipc.ts");
        let declaration = generated
            .lines()
            .find(|line| line.starts_with("export type PairingCeremony ="))
            .unwrap();

        for forbidden in [
            "code:",
            "sas:",
            "addr:",
            "peer_device_id:",
            "peer_name:",
            "qr_payload:",
            "socket_path:",
        ] {
            assert!(!declaration.contains(forbidden), "{declaration}");
        }
        assert!(!generated.contains("export type PairingInviteData"));
        assert!(!generated.contains("export type PairingProgressData"));
    }

    #[test]
    fn pairing_progress_discards_every_preconfirmation_secret() {
        let safe = PairingCeremony::from_progress(
            PairingProgressData {
                pairing_id: Some("ceremony-1".into()),
                role: Some(PairingRole::Initiator),
                state: PairingState::AwaitingConfirmation,
                expires_in_ms: Some(60_000),
                sas: Some("123456".into()),
                peer_device_id: Some("device-secret".into()),
                peer_name: Some("Unverified Phone".into()),
                peer_addr: Some("192.0.2.1:47654".into()),
                known_device: None,
                error_code: Some(ErrorCode::PeerFailed),
            },
            PairingPresentationState::Presented,
        );
        let json = serde_json::to_string(&safe).unwrap();

        for forbidden in ["123456", "device-secret", "Unverified Phone", "192.0.2.1"] {
            assert!(!json.contains(forbidden), "pairing material leaked: {json}");
        }
        assert!(json.contains("ceremony-1"), "{json}");
        assert!(json.contains("peer_failed"), "{json}");
    }

    #[test]
    fn confirmed_output_keeps_only_safe_known_device_metadata() {
        let safe = PairingCeremony::from_progress(
            PairingProgressData {
                pairing_id: Some("ceremony-1".into()),
                role: Some(PairingRole::Responder),
                state: PairingState::Confirmed,
                expires_in_ms: None,
                sas: Some("654321".into()),
                peer_device_id: Some("device-secret".into()),
                peer_name: Some("Unverified Phone".into()),
                peer_addr: Some("192.0.2.1:47654".into()),
                known_device: Some(PeerInfo {
                    pairing_id: "pairing-secret".into(),
                    name: "Phone".into(),
                    last_addr: Some("198.51.100.2:47654".into()),
                    last_seen_ms: 42,
                    online: true,
                    details: None,
                }),
                error_code: None,
            },
            PairingPresentationState::Presented,
        );
        let json = serde_json::to_string(&safe).unwrap();

        assert!(json.contains("Phone"), "{json}");
        assert!(json.contains("\"last_seen_ms\":42"), "{json}");
        for forbidden in [
            "654321",
            "device-secret",
            "192.0.2.1",
            "198.51.100.2",
            "pairing-secret",
        ] {
            assert!(!json.contains(forbidden), "pairing material leaked: {json}");
        }
    }

    #[test]
    fn failed_ceremony_serialization_uses_the_typed_retry_policy() {
        for code in [
            ErrorCode::ContentTooLarge,
            ErrorCode::RateLimited,
            ErrorCode::AuthFailed,
            ErrorCode::PairingCode,
            ErrorCode::PeerVersion,
            ErrorCode::PeerUnreachable,
            ErrorCode::PeerFailed,
            ErrorCode::Internal,
            ErrorCode::NotReady,
            ErrorCode::KeyLocked,
        ] {
            let ceremony = PairingCeremony::from_progress(
                PairingProgressData {
                    pairing_id: None,
                    role: None,
                    state: PairingState::Failed,
                    expires_in_ms: None,
                    sas: None,
                    peer_device_id: None,
                    peer_name: None,
                    peer_addr: None,
                    known_device: None,
                    error_code: Some(code),
                },
                PairingPresentationState::Presented,
            );
            let value = serde_json::to_value(ceremony).unwrap();
            assert_eq!(value["semantics"]["retry"], code.retryable());
            assert_eq!(value["error"]["retryable"], code.retryable());
        }
    }
}
