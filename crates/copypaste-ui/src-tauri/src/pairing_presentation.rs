#[cfg(any(target_os = "android", target_os = "macos", target_os = "windows"))]
use std::sync::Arc;
use std::sync::Mutex;

use copypaste_ipc::{ErrorCode, PairingInviteData, PairingProgressData, PairingState};
#[cfg(any(target_os = "android", target_os = "windows"))]
use tauri::{AppHandle, Manager as _};
use zeroize::Zeroizing;

mod native_copy;
pub use native_copy::PairingCopy;
pub(crate) mod semantics;
pub use semantics::{resolve_pairing_semantics, PairingSemantics};

#[cfg(any(test, target_os = "android"))]
mod android_payload;

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
            let _ = crate::commands::pairing::reconcile_progress(
                &*backend, &presenter, progress, false,
            )
            .await;
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

#[cfg(any(
    test,
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
))]
pub(crate) mod pairing_link;

#[cfg(target_os = "android")]
pub(crate) mod android;

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(any(test, target_os = "macos"))]
mod macos_model;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub fn windows_ui(abort: NativeAbort, refresh: NativeRefresh) -> impl NativePairingUi {
    windows::WindowsPairingUi::new(
        pairing_link::encode_pairing_link,
        invite::validate_native_invite_fields,
        abort,
        refresh,
    )
}

#[cfg(target_os = "macos")]
pub fn macos_ui(
    app: tauri::AppHandle,
    session: macos::SecurePairingSession,
    abort: NativeAbort,
) -> impl NativePairingUi {
    macos::MacOsPairingUi::new(app, session, abort)
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
    Cancelled,
    Unavailable,
    Refresh,
}

pub enum NativeScanOutcome {
    Scanned(ScannedPairing),
    Cancelled,
    Failed,
    Unavailable,
}

pub trait NativePairingUi: Send + Sync + 'static {
    fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome;
    fn scan_invite(&self) -> NativeScanOutcome;
    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState;
    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision>;
    fn take_pending_join(&self) -> Option<ScannedPairing> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfirmationToken {
    ceremony_id: Option<String>,
    generation: u64,
    sequence: u64,
    transition: PresentationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PresentationToken {
    ceremony_id: Option<String>,
    generation: u64,
    finishes_ceremony: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct NativeConfirmation {
    token: ConfirmationToken,
    pub(crate) decision: PairingDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressSignature {
    state: PairingState,
    error_code: Option<ErrorCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmationState {
    Ready,
    Prompting { sequence: u64 },
    Decided { sequence: u64 },
}

#[derive(Debug)]
struct CeremonyCoordinator {
    ceremony_id: Option<String>,
    generation: u64,
    signature: Option<ProgressSignature>,
    presentation: Option<PairingPresentationState>,
    confirmation: ConfirmationState,
}

impl Default for CeremonyCoordinator {
    fn default() -> Self {
        Self {
            ceremony_id: None,
            generation: 0,
            signature: None,
            presentation: None,
            confirmation: ConfirmationState::Ready,
        }
    }
}

impl CeremonyCoordinator {
    fn start(&mut self, ceremony_id: Option<String>) {
        if self.ceremony_id == ceremony_id {
            return;
        }
        self.ceremony_id = ceremony_id;
        self.generation = self.generation.wrapping_add(1);
        self.signature = None;
        self.presentation = None;
        self.confirmation = ConfirmationState::Ready;
    }

    fn started_invite(&mut self, pairing_id: &str) {
        self.start(Some(pairing_id.to_owned()));
        // The protected QR surface already presents the waiting phase.
        self.signature = Some(ProgressSignature {
            state: PairingState::WaitingForPeer,
            error_code: None,
        });
        self.presentation = Some(PairingPresentationState::Presented);
    }

    fn transition(&self, finishes_ceremony: bool) -> PresentationToken {
        PresentationToken {
            ceremony_id: self.ceremony_id.clone(),
            generation: self.generation,
            finishes_ceremony,
        }
    }

    fn next(
        &mut self,
        progress: &PairingProgressData,
        retry_confirmation: bool,
    ) -> PresentationAction {
        if progress.pairing_id.is_some() && progress.pairing_id != self.ceremony_id {
            self.start(progress.pairing_id.clone());
        }

        if progress.state == PairingState::Idle && self.ceremony_id.is_none() {
            return PresentationAction::None(Some(PairingPresentationState::Available));
        }

        let signature = ProgressSignature {
            state: progress.state,
            error_code: progress.error_code,
        };
        if self.signature == Some(signature) {
            if progress.state == PairingState::AwaitingConfirmation
                && retry_confirmation
                && self.confirmation == ConfirmationState::Ready
            {
                return self.confirm();
            }
            return PresentationAction::None(self.presentation);
        }
        self.signature = Some(signature);

        if progress.state != PairingState::AwaitingConfirmation {
            return PresentationAction::Progress(self.transition(
                progress.state == PairingState::Idle && progress.pairing_id.is_none(),
            ));
        }

        self.confirm()
    }

    fn confirm(&mut self) -> PresentationAction {
        match self.confirmation {
            ConfirmationState::Ready => {
                let sequence = self.generation.wrapping_add(1);
                let transition = self.transition(false);
                self.confirmation = ConfirmationState::Prompting { sequence };
                PresentationAction::Confirm(ConfirmationToken {
                    ceremony_id: self.ceremony_id.clone(),
                    generation: self.generation,
                    sequence,
                    transition,
                })
            }
            ConfirmationState::Prompting { .. } | ConfirmationState::Decided { .. } => {
                PresentationAction::None(self.presentation)
            }
        }
    }

    fn decision_finished(&mut self, token: &ConfirmationToken, decided: bool) {
        if self.matches(token) {
            self.confirmation = if decided {
                ConfirmationState::Decided {
                    sequence: token.sequence,
                }
            } else {
                ConfirmationState::Ready
            };
        }
    }

    fn record_presentation(
        &mut self,
        token: &PresentationToken,
        presentation: PairingPresentationState,
    ) {
        if self.matches_transition(token) {
            self.presentation = Some(presentation);
            if token.finishes_ceremony {
                self.ceremony_id = None;
                self.generation = self.generation.wrapping_add(1);
                self.signature = None;
                self.presentation = Some(PairingPresentationState::Available);
                self.confirmation = ConfirmationState::Ready;
            }
        }
    }

    fn matches(&self, token: &ConfirmationToken) -> bool {
        self.matches_transition(&token.transition)
            && self.ceremony_id == token.ceremony_id
            && self.generation == token.generation
            && matches!(
            self.confirmation,
                ConfirmationState::Prompting { sequence } | ConfirmationState::Decided { sequence }
                    if sequence == token.sequence
            )
    }

    fn matches_transition(&self, token: &PresentationToken) -> bool {
        self.ceremony_id == token.ceremony_id && self.generation == token.generation
    }
}

enum PresentationAction {
    None(Option<PairingPresentationState>),
    Progress(PresentationToken),
    Confirm(ConfirmationToken),
}

pub(crate) struct PairingPresentation {
    pub(crate) state: PairingPresentationState,
    pub(crate) confirmation: Option<NativeConfirmation>,
}

pub struct PairingPresenter {
    native: Box<dyn NativePairingUi>,
    available: bool,
    coordinator: Mutex<CeremonyCoordinator>,
}

impl Default for PairingPresenter {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        let native = Box::new(UnavailablePairingUi);
        #[cfg(target_os = "windows")]
        let native = Box::new(windows::WindowsPairingUi::new(
            pairing_link::encode_pairing_link,
            invite::validate_native_invite_fields,
            Arc::new(|| {}),
            Arc::new(|| {}),
        ));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let native = Box::new(UnavailablePairingUi);

        Self {
            native,
            available: cfg!(target_os = "windows"),
            coordinator: Mutex::new(CeremonyCoordinator::default()),
        }
    }
}

impl PairingPresenter {
    pub fn new(native: impl NativePairingUi) -> Self {
        Self {
            native: Box::new(native),
            available: true,
            coordinator: Mutex::new(CeremonyCoordinator::default()),
        }
    }

    /// Idle reports capability. An active ceremony exists only after its
    /// protected native surface presented successfully.
    pub fn state_for_progress(&self, state: PairingState) -> PairingPresentationState {
        if !self.available {
            PairingPresentationState::Unavailable
        } else if matches!(
            state,
            PairingState::WaitingForPeer
                | PairingState::Handshaking
                | PairingState::AwaitingConfirmation
        ) {
            PairingPresentationState::Presented
        } else {
            PairingPresentationState::Available
        }
    }

    pub fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome {
        let outcome = self.native.present_invite(invite);
        if outcome == NativePresentationOutcome::Presented {
            if let Ok(mut coordinator) = self.coordinator.lock() {
                coordinator.started_invite(&invite.pairing_id);
            }
        }
        outcome
    }

    pub fn scan_invite(&self) -> NativeScanOutcome {
        self.native.scan_invite()
    }

    pub(crate) fn present_progress(
        &self,
        progress: &PairingProgressData,
        retry_confirmation: bool,
    ) -> PairingPresentation {
        let action = self
            .coordinator
            .lock()
            .map(|mut coordinator| coordinator.next(progress, retry_confirmation))
            .unwrap_or(PresentationAction::None(None));

        match action {
            PresentationAction::None(state) => PairingPresentation {
                state: state.unwrap_or_else(|| self.state_for_progress(progress.state)),
                confirmation: None,
            },
            PresentationAction::Progress(token) => {
                let state = self.native.present_progress(progress);
                if let Ok(mut coordinator) = self.coordinator.lock() {
                    coordinator.record_presentation(&token, state);
                }
                PairingPresentation {
                    state,
                    confirmation: None,
                }
            }
            PresentationAction::Confirm(token) => {
                // The platform transition clears the QR/progress surface before
                // its single native SAS prompt opens.
                let progress_state = self.native.present_progress(progress);
                if let Ok(mut coordinator) = self.coordinator.lock() {
                    coordinator.record_presentation(&token.transition, progress_state);
                }
                let decision = self.native.confirm(progress);
                let state = if decision.is_some() {
                    PairingPresentationState::Presented
                } else {
                    PairingPresentationState::Unavailable
                };
                if let Ok(mut coordinator) = self.coordinator.lock() {
                    coordinator.decision_finished(&token, decision.is_some());
                }
                PairingPresentation {
                    state,
                    confirmation: decision.map(|decision| NativeConfirmation { token, decision }),
                }
            }
        }
    }

    pub(crate) fn confirmation_is_current(
        &self,
        confirmation: &NativeConfirmation,
        progress: &PairingProgressData,
    ) -> bool {
        progress.state == PairingState::AwaitingConfirmation
            && progress.pairing_id == confirmation.token.ceremony_id
            && self
                .coordinator
                .lock()
                .is_ok_and(|coordinator| coordinator.matches(&confirmation.token))
    }

    pub fn take_pending_join(&self) -> Option<ScannedPairing> {
        self.native.take_pending_join()
    }
}

#[cfg(not(target_os = "windows"))]
struct UnavailablePairingUi;

#[cfg(not(target_os = "windows"))]
impl NativePairingUi for UnavailablePairingUi {
    fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
        NativePresentationOutcome::Unavailable
    }

    fn scan_invite(&self) -> NativeScanOutcome {
        NativeScanOutcome::Unavailable
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use copypaste_ipc::PairingRole;

    struct ConfiguredPairingUi;

    struct RetryablePairingUi(AtomicUsize);

    struct RecordingPairingUi {
        progress_calls: Arc<AtomicUsize>,
        confirm_calls: Arc<AtomicUsize>,
        decision: Arc<Mutex<PairingDecision>>,
    }

    struct OrderedPairingUi(Arc<Mutex<Vec<&'static str>>>);

    impl NativePairingUi for ConfiguredPairingUi {
        fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
            NativePresentationOutcome::Unavailable
        }

        fn scan_invite(&self) -> NativeScanOutcome {
            NativeScanOutcome::Unavailable
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            PairingPresentationState::Unavailable
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            None
        }
    }

    impl NativePairingUi for RetryablePairingUi {
        fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
            if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
                NativePresentationOutcome::Cancelled
            } else {
                NativePresentationOutcome::Presented
            }
        }

        fn scan_invite(&self) -> NativeScanOutcome {
            NativeScanOutcome::Cancelled
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            PairingPresentationState::Available
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            None
        }
    }

    impl NativePairingUi for RecordingPairingUi {
        fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
            NativePresentationOutcome::Presented
        }

        fn scan_invite(&self) -> NativeScanOutcome {
            NativeScanOutcome::Cancelled
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            self.progress_calls.fetch_add(1, Ordering::Relaxed);
            PairingPresentationState::Presented
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            self.confirm_calls.fetch_add(1, Ordering::Relaxed);
            self.decision.lock().ok().map(|decision| *decision)
        }
    }

    impl NativePairingUi for OrderedPairingUi {
        fn present_invite(&self, _invite: &PairingInviteData) -> NativePresentationOutcome {
            NativePresentationOutcome::Presented
        }

        fn scan_invite(&self) -> NativeScanOutcome {
            NativeScanOutcome::Cancelled
        }

        fn present_progress(&self, _progress: &PairingProgressData) -> PairingPresentationState {
            self.0.lock().expect("events").push("progress");
            PairingPresentationState::Presented
        }

        fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
            self.0.lock().expect("events").push("confirm");
            Some(PairingDecision::Accept)
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

    fn idle_progress() -> PairingProgressData {
        let mut progress = progress("unused", PairingState::Idle);
        progress.pairing_id = None;
        progress
    }

    fn recording_presenter(
        decision: PairingDecision,
    ) -> (PairingPresenter, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let progress_calls = Arc::new(AtomicUsize::new(0));
        let confirm_calls = Arc::new(AtomicUsize::new(0));
        let presenter = PairingPresenter::new(RecordingPairingUi {
            progress_calls: Arc::clone(&progress_calls),
            confirm_calls: Arc::clone(&confirm_calls),
            decision: Arc::new(Mutex::new(decision)),
        });
        (presenter, progress_calls, confirm_calls)
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
        assert_eq!(
            presenter.state_for_progress(PairingState::Cancelled),
            PairingPresentationState::Available
        );
    }

    #[test]
    fn cancelling_a_native_invite_keeps_the_next_invite_available() {
        let presenter = PairingPresenter::new(RetryablePairingUi(AtomicUsize::new(0)));
        let invite = PairingInviteData {
            code: "code".into(),
            pairing_id: "pairing-id".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
            expires_in_secs: 120,
        };

        assert_eq!(
            presenter.present_invite(&invite),
            NativePresentationOutcome::Cancelled
        );
        assert_eq!(
            presenter.state_for_progress(PairingState::Cancelled),
            PairingPresentationState::Available
        );
        assert_eq!(
            presenter.present_invite(&invite),
            NativePresentationOutcome::Presented
        );
    }

    #[test]
    fn progress_reaches_the_native_surface_only_when_its_state_changes() {
        let (presenter, progress_calls, _) = recording_presenter(PairingDecision::Cancel);
        let waiting = progress("ceremony-1", PairingState::WaitingForPeer);

        assert_eq!(
            presenter.present_progress(&waiting, false).state,
            PairingPresentationState::Presented
        );
        assert_eq!(
            presenter.present_progress(&waiting, false).state,
            PairingPresentationState::Presented
        );
        assert_eq!(
            presenter
                .present_progress(&progress("ceremony-1", PairingState::Handshaking), false)
                .state,
            PairingPresentationState::Presented
        );
        assert_eq!(progress_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn initial_idle_never_opens_a_native_pairing_surface() {
        let (presenter, progress_calls, confirm_calls) =
            recording_presenter(PairingDecision::Cancel);

        assert_eq!(
            presenter.present_progress(&idle_progress(), false).state,
            PairingPresentationState::Available
        );
        assert_eq!(progress_calls.load(Ordering::Relaxed), 0);
        assert_eq!(confirm_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn compare_transition_updates_the_native_surface_before_sas() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let presenter = PairingPresenter::new(OrderedPairingUi(Arc::clone(&events)));

        presenter.present_progress(
            &progress("ceremony-1", PairingState::AwaitingConfirmation),
            false,
        );
        assert_eq!(
            events.lock().expect("events").as_slice(),
            ["progress", "confirm"]
        );
    }

    #[test]
    fn native_sas_is_once_per_ceremony_and_rearms_for_a_new_one() {
        let (presenter, _, confirm_calls) = recording_presenter(PairingDecision::Accept);
        let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
        let confirmation = presenter
            .present_progress(&awaiting, false)
            .confirmation
            .unwrap();

        assert!(presenter.confirmation_is_current(&confirmation, &awaiting));
        assert_eq!(confirm_calls.load(Ordering::Relaxed), 1);
        presenter.present_progress(&progress("ceremony-1", PairingState::WaitingForPeer), false);
        presenter.present_progress(&awaiting, false);
        assert_eq!(confirm_calls.load(Ordering::Relaxed), 1);

        let next = progress("ceremony-2", PairingState::AwaitingConfirmation);
        let next_confirmation = presenter
            .present_progress(&next, false)
            .confirmation
            .unwrap();
        assert!(presenter.confirmation_is_current(&next_confirmation, &next));
        assert_eq!(confirm_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_stale_native_decision_cannot_confirm_a_replacement_ceremony() {
        let (presenter, _, _) = recording_presenter(PairingDecision::Accept);
        let original = progress("ceremony-1", PairingState::AwaitingConfirmation);
        let confirmation = presenter
            .present_progress(&original, false)
            .confirmation
            .unwrap();
        let replacement = progress("ceremony-2", PairingState::AwaitingConfirmation);

        presenter.present_progress(&replacement, false);
        assert!(!presenter.confirmation_is_current(&confirmation, &replacement));
    }

    #[test]
    fn stale_platform_result_cannot_overwrite_a_new_ceremony_presentation() {
        let mut coordinator = CeremonyCoordinator::default();
        let original = progress("ceremony-1", PairingState::WaitingForPeer);
        let replacement = progress("ceremony-2", PairingState::WaitingForPeer);
        let PresentationAction::Progress(original_token) = coordinator.next(&original, false)
        else {
            panic!("original transition must present");
        };
        let PresentationAction::Progress(replacement_token) = coordinator.next(&replacement, false)
        else {
            panic!("replacement transition must present");
        };

        coordinator.record_presentation(&replacement_token, PairingPresentationState::Presented);
        coordinator.record_presentation(&original_token, PairingPresentationState::Unavailable);
        assert_eq!(
            coordinator.presentation,
            Some(PairingPresentationState::Presented)
        );
    }

    #[test]
    fn an_explicit_confirmation_retries_only_after_a_transient_native_failure() {
        let mut coordinator = CeremonyCoordinator::default();
        let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
        let PresentationAction::Confirm(token) = coordinator.next(&awaiting, false) else {
            panic!("first confirmation must prompt");
        };

        coordinator.decision_finished(&token, false);
        assert!(matches!(
            coordinator.next(&awaiting, false),
            PresentationAction::None(_)
        ));
        assert!(matches!(
            coordinator.next(&awaiting, true),
            PresentationAction::Confirm(_)
        ));
    }

    #[test]
    fn cancel_and_expiry_decisions_remain_native_only_and_do_not_reprompt() {
        for decision in [PairingDecision::Cancel, PairingDecision::Refresh] {
            let (presenter, progress_calls, confirm_calls) = recording_presenter(decision);
            let awaiting = progress("ceremony-1", PairingState::AwaitingConfirmation);
            let outcome = presenter.present_progress(&awaiting, false);

            assert_eq!(outcome.confirmation.unwrap().decision, decision);
            presenter.present_progress(&awaiting, false);
            assert_eq!(confirm_calls.load(Ordering::Relaxed), 1);
            assert_eq!(progress_calls.load(Ordering::Relaxed), 1);
        }
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
        let presentation = presenter.present_progress(&progress(), false);
        assert_eq!(presentation.state, PairingPresentationState::Unavailable);
        assert!(presentation.confirmation.is_none());
        assert!(matches!(
            presenter.scan_invite(),
            NativeScanOutcome::Unavailable
        ));
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
    fn pairing_secrets_are_scoped_to_the_protected_window_with_platform_entry_parity() {
        let sources = [
            production(include_str!("pairing_presentation/windows/mod.rs")),
            production(include_str!("pairing_presentation/windows/invite.rs")),
            production(include_str!("pairing_presentation/windows/entry.rs")),
            production(include_str!("pairing_presentation/windows/confirm.rs")),
            production(include_str!("pairing_presentation/windows/status.rs")),
            production(include_str!("pairing_presentation/android.rs")),
            production(include_str!("pairing_presentation/pairing_link.rs")),
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
        for required in [
            "const WINDOW_LABEL: &str = \"pairing\"",
            "WebviewUrl::App(ROUTE.into())",
            ".visible(false)",
            ".content_protected(true)",
            "window.set_content_protected(true)",
            "window.label() == WINDOW_LABEL",
            "session.generation != generation",
            "invite.expires_at <= Instant::now()",
            "session.revealed_sas",
            "progress.sas.as_deref()",
            "Zeroizing",
            "window_closed",
        ] {
            assert!(macos.contains(required), "missing macOS guard: {required}");
        }

        let windows = production(include_str!("pairing_presentation/windows/entry.rs"));
        assert_eq!(windows.matches("co::ES::PASSWORD").count(), 2);
        assert!(windows.contains("Pairing &code"));
        assert!(windows.contains("Pairing &address"));

        let android = production(include_str!("pairing_presentation/android.rs"));
        assert!(android.contains("\"scanInvite\""));
        assert!(android.contains("decode_pairing_payload"));
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
    fn qr_renderers_use_the_canonical_pairing_link_and_only_parse_legacy_json() {
        let macos = production(include_str!("pairing_presentation/macos.rs"));
        let presenter = production(include_str!("pairing_presentation.rs"));
        let android = production(include_str!("pairing_presentation/android.rs"));
        let deep_links =
            include_str!("../gen/android/app/src/main/java/com/copypaste/app/PairingDeepLinks.kt");
        let web_presentation =
            include_str!("../../src/features/pairing/model/pairingPresentation.ts");

        assert!(macos.contains("encode_pairing_link"));
        assert!(!macos.contains("encode_native_invite"));
        assert!(presenter.contains("windows::WindowsPairingUi::new("));
        assert!(presenter.contains("pairing_link::encode_pairing_link"));
        assert!(android.contains("encode_pairing_link"));
        assert!(deep_links.contains("uri.toString()"));
        assert!(!deep_links.contains("JSONObject"));
        assert!(!deep_links.contains("appendQueryParameter"));
        assert!(web_presentation.contains("semantics.copy.title"));
        assert!(web_presentation.contains("semantics.copy.detail"));
        assert!(!web_presentation.contains("devices.pairing.semantic"));
    }

    #[test]
    fn android_external_links_keep_the_payload_only_wire_shape() {
        let android = production(include_str!("pairing_presentation/android.rs"));
        let plugin = include_str!(
            "../gen/android/app/src/main/java/com/copypaste/app/PairingPresentationPlugin.kt"
        );

        assert!(android.contains("struct PendingLinkResult"));
        assert!(android.contains("self.call(\"takePendingLink\", ())?"));
        assert!(android.contains("result.payload.0.take()?"));
        assert!(plugin.contains("PairingDeepLinks.take()?.let { result.put(\"payload\", it) }"));
        assert!(!plugin.contains("result.put(\"outcome\", it)"));
    }

    #[test]
    fn native_deadlines_clear_secrets_and_leave_timeout_to_backend_progress() {
        let macos = production(include_str!("pairing_presentation/macos.rs"));
        assert!(macos.contains("expires_at: Instant"));
        assert!(macos.contains("progress.expires_in_ms"));
        assert!(macos.contains("session.invite = None"));
        assert!(macos.contains("session.revealed_sas = None"));
        assert!(macos.contains("PairingDecision::Refresh"));
        assert!(macos.contains("PairingState::AwaitingConfirmation"));
        assert!(macos.contains("session.is_generation(closed_generation)"));
        assert!(macos.contains("pairing.pair_cancel().await"));

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
        assert!(android.contains("qr.setImageDrawable(null)"));
        assert!(android.contains("qr.visibility = View.GONE"));
        assert!(android.contains("clearQr()"));
        assert!(android.contains("onRefresh?.invoke()"));
        assert!(android.contains("sasView?.removeAllViews()"));
        assert!(android.contains("deliver(\"refresh\")"));
        assert!(!android.contains("presentProgress(\"timed_out\")"));
    }

    #[test]
    fn windows_status_reads_the_semantics_owned_copy() {
        let status = production(include_str!("pairing_presentation/windows/status.rs"));
        assert!(status.contains("pub(super) fn copy(progress: &PairingProgressData)"));
        assert!(
            status.contains("resolve_pairing_semantics(progress.state, progress.error_code).copy")
        );
    }
}
