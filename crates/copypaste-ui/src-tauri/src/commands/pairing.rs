use copypaste_ipc::{PairingProgressData, PairingRole, PairingState};
use serde::Serialize;
use tauri::{AppHandle, Manager as _, State, WebviewWindow};

use crate::backend::{PairingBackend, Result};
use crate::pairing_presentation::{
    resolve_pairing_semantics, NativePresentationOutcome, NativeScanOutcome, PairingDecision,
    PairingPresentationState, PairingPresenter, PairingSemantics,
};
use crate::SelectedBackend;

pub(crate) async fn reconcile_progress<B: PairingBackend + ?Sized>(
    backend: &B,
    presenter: &PairingPresenter,
    progress: PairingProgressData,
    retry_confirmation: bool,
) -> Result<PairingCeremony> {
    let presentation = presenter.present_progress(&progress, retry_confirmation);
    let Some(confirmation) = presentation.confirmation else {
        return Ok(PairingCeremony::from_progress(progress, presentation.state));
    };

    // A native decision can outlive a polling response. Check the backend still
    // names this exact ceremony before allowing a decision to reach it.
    let current = backend.pair_progress().await?;
    if !presenter.confirmation_is_current(&confirmation, &current) {
        let presentation = presenter.state_for_progress(current.state);
        return Ok(PairingCeremony::from_progress(current, presentation));
    }

    let next = match confirmation.decision {
        PairingDecision::Accept => backend.pair_confirm(true).await?,
        PairingDecision::Reject => backend.pair_confirm(false).await?,
        PairingDecision::Cancel => backend.pair_cancel().await?,
        PairingDecision::Refresh => current,
    };
    let presentation = presenter.present_progress(&next, false);
    Ok(PairingCeremony::from_progress(next, presentation.state))
}

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

/// Secrets are returned only to the dedicated, capture-protected pairing
/// window. The ordinary PairingCeremony DTO remains secret-free.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SecureInviteView {
    pub(crate) generation: u64,
    pub(crate) ceremony_id: String,
    pub(crate) code: String,
    pub(crate) address: String,
    pub(crate) qr_svg: String,
    pub(crate) expires_in_ms: u64,
}

#[derive(Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SecureSasView {
    pub(crate) generation: u64,
    pub(crate) ceremony_id: String,
    pub(crate) sas: String,
    pub(crate) expires_in_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SecurePairingView {
    pub(crate) generation: u64,
    pub(crate) phase: String,
    pub(crate) ceremony: PairingCeremony,
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
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let invite = backend.pair_create_invite().await?;
    match presenter.present_invite(&invite) {
        NativePresentationOutcome::Unavailable => backend.pair_cancel().await.map(|progress| {
            PairingCeremony::from_progress(progress, PairingPresentationState::Unavailable)
        }),
        NativePresentationOutcome::Cancelled => {
            let progress = backend.pair_cancel().await?;
            reconcile_progress(&*backend, &presenter, progress, false).await
        }
        NativePresentationOutcome::Presented => {
            let progress = backend.pair_progress().await?;
            reconcile_progress(&*backend, &presenter, progress, false).await
        }
        NativePresentationOutcome::Refresh => {
            let progress = backend.pair_progress().await?;
            reconcile_progress(&*backend, &presenter, progress, false).await
        }
    }
}

#[tauri::command]
pub async fn pair_scan_invite(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let scanned = match presenter.scan_invite() {
        NativeScanOutcome::Scanned(scanned) => scanned,
        NativeScanOutcome::Cancelled | NativeScanOutcome::Failed => {
            let progress = backend.pair_progress().await?;
            return reconcile_progress(&*backend, &presenter, progress, false).await;
        }
        NativeScanOutcome::Unavailable => return Ok(PairingCeremony::unavailable()),
    };
    let progress = backend
        .pair_join(scanned.code.as_str(), scanned.addr.as_str())
        .await?;
    reconcile_progress(&*backend, &presenter, progress, false).await
}

#[tauri::command]
pub async fn pair_progress(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let progress = backend.pair_progress().await?;
    if progress.state == PairingState::Idle {
        if let Some(scanned) = presenter.take_pending_join() {
            let progress = backend
                .pair_join(scanned.code.as_str(), scanned.addr.as_str())
                .await?;
            return reconcile_progress(&*backend, &presenter, progress, false).await;
        }
    }
    reconcile_progress(&*backend, &presenter, progress, false).await
}

#[tauri::command]
pub async fn pair_present(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let progress = backend.pair_progress().await?;
    reconcile_progress(&*backend, &presenter, progress, false).await
}

#[tauri::command]
pub async fn pair_confirm(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let progress = backend.pair_progress().await?;
    reconcile_progress(&*backend, &presenter, progress, true).await
}

#[tauri::command]
pub async fn pair_reject(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let progress = backend.pair_confirm(false).await?;
    reconcile_progress(&*backend, &presenter, progress, false).await
}

#[tauri::command]
pub async fn pair_cancel(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
    presenter: State<'_, PairingPresenter>,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    let session = app.state::<crate::pairing_presentation::macos::SecurePairingSession>();
    #[cfg(target_os = "macos")]
    let _operation = session.lock_operation().await;
    #[cfg(not(target_os = "macos"))]
    let _ = &app;
    let progress = backend.pair_cancel().await?;
    reconcile_progress(&*backend, &presenter, progress, false).await
}

#[tauri::command]
pub async fn pair_secure_state(window: WebviewWindow, app: AppHandle) -> Result<SecurePairingView> {
    #[cfg(target_os = "macos")]
    {
        return crate::pairing_presentation::macos::state(window, app).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[tauri::command]
pub async fn pair_secure_reveal_invite(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
) -> Result<SecureInviteView> {
    #[cfg(target_os = "macos")]
    {
        return crate::pairing_presentation::macos::reveal_invite(window, app, generation).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app, generation);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[tauri::command]
pub async fn pair_secure_reveal_sas(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
) -> Result<SecureSasView> {
    #[cfg(target_os = "macos")]
    {
        return crate::pairing_presentation::macos::reveal_sas(window, app, generation).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app, generation);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[tauri::command]
pub async fn pair_secure_join(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
    code: String,
    addr: String,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    {
        return crate::pairing_presentation::macos::join(window, app, generation, code, addr).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app, generation, code, addr);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[tauri::command]
pub async fn pair_secure_decide(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
    accept: bool,
) -> Result<PairingCeremony> {
    #[cfg(target_os = "macos")]
    {
        return crate::pairing_presentation::macos::decide(window, app, generation, accept).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app, generation, accept);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[tauri::command]
pub async fn pair_secure_close(window: WebviewWindow, app: AppHandle) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if window.label() != crate::pairing_presentation::macos::WINDOW_LABEL {
            return Err(crate::backend::BackendError::Invalid(
                "Protected pairing is unavailable in this window.",
            ));
        }
        if !crate::pairing_presentation::macos::window_closed(&app) {
            return Err(crate::backend::BackendError::Invalid(
                "Wait for the pairing decision to finish.",
            ));
        }
        crate::pairing_presentation::macos::destroy_window(&app);
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, app);
        Err(crate::backend::BackendError::Invalid(
            "Protected pairing is unavailable.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use copypaste_ipc::{ErrorCode, PeerInfo};
    struct FakeBackend {
        progress: Mutex<VecDeque<PairingProgressData>>,
        confirm_result: PairingProgressData,
        cancel_result: PairingProgressData,
        confirmations: Mutex<Vec<bool>>,
        cancellations: AtomicUsize,
    }

    impl FakeBackend {
        fn new(
            progress: impl IntoIterator<Item = PairingProgressData>,
            confirm_result: PairingProgressData,
            cancel_result: PairingProgressData,
        ) -> Self {
            Self {
                progress: Mutex::new(progress.into_iter().collect()),
                confirm_result,
                cancel_result,
                confirmations: Mutex::new(Vec::new()),
                cancellations: AtomicUsize::new(0),
            }
        }
    }

    impl PairingBackend for FakeBackend {
        async fn pair_create_invite(&self) -> Result<copypaste_ipc::PairingInviteData> {
            unreachable!("not used by coordinator tests")
        }

        async fn pair_join(&self, _code: &str, _addr: &str) -> Result<PairingProgressData> {
            unreachable!("not used by coordinator tests")
        }

        async fn pair_progress(&self) -> Result<PairingProgressData> {
            Ok(self
                .progress
                .lock()
                .expect("progress queue")
                .pop_front()
                .expect("expected progress read"))
        }

        async fn pair_confirm(&self, accept: bool) -> Result<PairingProgressData> {
            self.confirmations
                .lock()
                .expect("confirmations")
                .push(accept);
            Ok(self.confirm_result.clone())
        }

        async fn pair_cancel(&self) -> Result<PairingProgressData> {
            self.cancellations.fetch_add(1, Ordering::Relaxed);
            Ok(self.cancel_result.clone())
        }
    }

    struct RecordingPairingUi {
        decision: PairingDecision,
        confirmation_calls: Arc<AtomicUsize>,
    }

    impl crate::pairing_presentation::NativePairingUi for RecordingPairingUi {
        fn present_invite(
            &self,
            _invite: &copypaste_ipc::PairingInviteData,
        ) -> NativePresentationOutcome {
            NativePresentationOutcome::Presented
        }

        fn scan_invite(&self) -> NativeScanOutcome {
            NativeScanOutcome::Cancelled
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            PairingPresentationState::Presented
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            self.confirmation_calls.fetch_add(1, Ordering::Relaxed);
            Some(self.decision)
        }
    }

    fn progress(pairing_id: &str, state: PairingState) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some(pairing_id.into()),
            role: Some(PairingRole::Initiator),
            state,
            expires_in_ms: Some(60_000),
            sas: Some("123456".into()),
            peer_device_id: Some("device-secret".into()),
            peer_name: Some("Unverified device".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code: None,
        }
    }

    fn presenter(decision: PairingDecision) -> (PairingPresenter, Arc<AtomicUsize>) {
        let confirmation_calls = Arc::new(AtomicUsize::new(0));
        (
            PairingPresenter::new(RecordingPairingUi {
                decision,
                confirmation_calls: Arc::clone(&confirmation_calls),
            }),
            confirmation_calls,
        )
    }

    #[tokio::test]
    async fn native_accepts_once_then_waits_for_the_remote_decision() {
        let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
        let waiting = progress("ceremony-1", PairingState::WaitingForPeer);
        let backend = FakeBackend::new([awaiting.clone()], waiting.clone(), waiting.clone());
        let (presenter, confirmation_calls) = presenter(PairingDecision::Accept);

        let result = reconcile_progress(&backend, &presenter, awaiting, false)
            .await
            .unwrap();
        assert_eq!(result.state, PairingState::WaitingForPeer);
        assert_eq!(
            backend
                .confirmations
                .lock()
                .expect("confirmations")
                .as_slice(),
            &[true]
        );
        reconcile_progress(&backend, &presenter, waiting, false)
            .await
            .unwrap();
        assert_eq!(confirmation_calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn stale_native_outcome_cannot_confirm_a_replacement_ceremony() {
        let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
        let replacement = progress("ceremony-2", PairingState::AwaitingConfirmation);
        let backend = FakeBackend::new(
            [replacement.clone()],
            progress("ceremony-1", PairingState::Confirmed),
            progress("ceremony-1", PairingState::Cancelled),
        );
        let (presenter, _) = presenter(PairingDecision::Accept);

        let result = reconcile_progress(&backend, &presenter, awaiting, false)
            .await
            .unwrap();
        assert_eq!(result.ceremony_id.as_deref(), Some("ceremony-2"));
        assert!(backend
            .confirmations
            .lock()
            .expect("confirmations")
            .is_empty());
    }

    #[tokio::test]
    async fn native_cancel_and_expiry_fail_closed_without_confirmation() {
        for decision in [PairingDecision::Cancel, PairingDecision::Refresh] {
            let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
            let cancelled = progress("ceremony-1", PairingState::Cancelled);
            let backend = FakeBackend::new([awaiting.clone()], cancelled.clone(), cancelled);
            let (presenter, _) = presenter(decision);

            let result = reconcile_progress(&backend, &presenter, awaiting, false)
                .await
                .unwrap();
            assert!(backend
                .confirmations
                .lock()
                .expect("confirmations")
                .is_empty());
            match decision {
                PairingDecision::Cancel => {
                    assert_eq!(result.state, PairingState::Cancelled);
                    assert_eq!(backend.cancellations.load(Ordering::Relaxed), 1);
                }
                PairingDecision::Refresh => {
                    assert_eq!(result.state, PairingState::AwaitingConfirmation);
                    assert_eq!(backend.cancellations.load(Ordering::Relaxed), 0);
                }
                PairingDecision::Accept | PairingDecision::Reject => unreachable!(),
            }
        }
    }

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
            ErrorCode::NotFound,
            ErrorCode::InvalidRequest,
            ErrorCode::ProtocolMismatch,
            ErrorCode::NotReady,
            ErrorCode::RateLimited,
            ErrorCode::AuthFailed,
            ErrorCode::KeyLocked,
            ErrorCode::KeyUnusable,
            ErrorCode::UnsupportedContent,
            ErrorCode::PairingCode,
            ErrorCode::PairingAddress,
            ErrorCode::PairingLimit,
            ErrorCode::PeerVersion,
            ErrorCode::PeerUnreachable,
            ErrorCode::PeerFailed,
            ErrorCode::PeerNotFound,
            ErrorCode::Internal,
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
            assert_eq!(value["semantics"]["terminal"], true);
            assert_eq!(value["semantics"]["retry"], code.retryable());
            assert_eq!(value["error"]["retryable"], code.retryable());
        }

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
                error_code: None,
            },
            PairingPresentationState::Presented,
        );
        let value = serde_json::to_value(ceremony).unwrap();
        assert_eq!(value["semantics"]["terminal"], true);
        assert_eq!(value["semantics"]["retry"], false);
        assert!(value["error"].is_null());
    }
}
