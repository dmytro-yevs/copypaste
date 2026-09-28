//! Protected, first-party pairing WebView on macOS.
//!
//! Only this window may read an explicitly revealed invite or bound SAS. The
//! ordinary application and Quick Paste windows receive `PairingCeremony` only.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
use tauri::{AppHandle, Manager as _, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use zeroize::Zeroizing;

use super::invite::validate_native_invite_fields;
use super::macos_model::sas_digits;
use super::pairing_link::encode_pairing_link;
use super::{
    NativeAbort, NativePairingUi, NativePresentationOutcome, NativeScanOutcome, PairingDecision,
    PairingPresentationState,
};
use crate::backend::{BackendError, PairingBackend as _, Result};
use crate::commands::pairing::{
    PairingCeremony, SecureInviteView, SecurePairingView, SecureSasView,
};
use crate::SelectedBackend;

pub(crate) const WINDOW_LABEL: &str = "pairing";
const ROUTE: &str = "index.html?surface=pairing";
const WRONG_WINDOW: &str = "Protected pairing is unavailable in this window.";
const EXPIRED: &str = "This pairing session expired. Start again.";

#[derive(Clone, Default)]
pub(crate) struct SecurePairingSession {
    inner: Arc<Mutex<Session>>,
    operation: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Default)]
struct Session {
    generation: u64,
    phase: Phase,
    ceremony_id: Option<String>,
    invite: Option<Invite>,
    revealed_sas: Option<BoundSas>,
    decision_in_flight: bool,
}

struct DecisionPermit {
    session: SecurePairingSession,
    generation: u64,
}

impl Drop for DecisionPermit {
    fn drop(&mut self) {
        if let Ok(mut session) = self.session.inner.lock() {
            if session.generation == self.generation {
                session.decision_in_flight = false;
            }
        }
    }
}

struct BoundSas {
    value: Zeroizing<String>,
    expires_at: Instant,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Join,
    Invite,
    Progress,
    Confirm,
    Terminal,
}

struct Invite {
    code: Zeroizing<String>,
    address: Zeroizing<String>,
    link: Zeroizing<String>,
    expires_at: Instant,
}

impl SecurePairingSession {
    fn reset(&self, phase: Phase) {
        if let Ok(mut session) = self.inner.lock() {
            session.generation = session.generation.wrapping_add(1);
            session.phase = phase;
            session.ceremony_id = None;
            session.invite = None;
            session.revealed_sas = None;
            session.decision_in_flight = false;
        }
    }

    fn begin_invite(&self, invite: &PairingInviteData) -> bool {
        let Some(address) = invite.listen_addr.as_ref() else {
            return false;
        };
        let Some(link) = encode_pairing_link(invite) else {
            return false;
        };
        let Some(expires_at) =
            Instant::now().checked_add(Duration::from_secs(invite.expires_in_secs))
        else {
            return false;
        };
        if expires_at <= Instant::now() {
            return false;
        }
        let Ok(mut session) = self.inner.lock() else {
            return false;
        };
        session.generation = session.generation.wrapping_add(1);
        session.phase = Phase::Invite;
        session.ceremony_id = Some(invite.pairing_id.clone());
        session.invite = Some(Invite {
            code: Zeroizing::new(invite.code.clone()),
            address: Zeroizing::new(address.clone()),
            link,
            expires_at,
        });
        session.revealed_sas = None;
        session.decision_in_flight = false;
        true
    }

    fn progress(&self, progress: &PairingProgressData) {
        let _ = self.progress_if_generation(progress, None);
    }

    fn progress_if_generation(
        &self,
        progress: &PairingProgressData,
        expected: Option<u64>,
    ) -> bool {
        let Ok(mut session) = self.inner.lock() else {
            return false;
        };
        if expected.is_some_and(|generation| session.generation != generation) {
            return false;
        }
        if session.phase == Phase::Idle {
            return true;
        }
        if session.ceremony_id.is_some() && session.ceremony_id != progress.pairing_id {
            session.generation = session.generation.wrapping_add(1);
            session.invite = None;
            session.revealed_sas = None;
        }
        if progress.pairing_id.is_some() {
            session.ceremony_id = progress.pairing_id.clone();
        }
        session.phase = match progress.state {
            PairingState::WaitingForPeer if session.invite.is_some() => Phase::Invite,
            PairingState::Handshaking | PairingState::WaitingForPeer => Phase::Progress,
            PairingState::AwaitingConfirmation => Phase::Confirm,
            PairingState::Idle if session.phase == Phase::Join => Phase::Join,
            _ => Phase::Terminal,
        };
        if session.phase != Phase::Invite {
            session.invite = None;
        }
        if session.phase != Phase::Confirm {
            session.revealed_sas = None;
        }
        true
    }

    pub(crate) fn clear(&self) -> Option<(bool, u64, Option<String>)> {
        let Ok(mut session) = self.inner.lock() else {
            return None;
        };
        if session.decision_in_flight {
            return None;
        }
        let active = matches!(
            session.phase,
            Phase::Join | Phase::Invite | Phase::Progress | Phase::Confirm
        );
        let ceremony_id = session.ceremony_id.take();
        session.generation = session.generation.wrapping_add(1);
        session.phase = Phase::Idle;
        session.invite = None;
        session.revealed_sas = None;
        Some((active, session.generation, ceremony_id))
    }

    fn generation(&self) -> u64 {
        self.inner.lock().map_or(0, |session| session.generation)
    }

    fn is_generation(&self, generation: u64) -> bool {
        self.inner
            .lock()
            .is_ok_and(|session| session.generation == generation)
    }

    pub(crate) async fn lock_operation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.operation.lock().await
    }

    fn phase(&self) -> &'static str {
        self.inner
            .lock()
            .map_or("idle", |session| match session.phase {
                Phase::Idle => "idle",
                Phase::Join => "join",
                Phase::Invite => "invite",
                Phase::Progress => "progress",
                Phase::Confirm => "confirm",
                Phase::Terminal => "terminal",
            })
    }

    fn reveal_invite(
        &self,
        progress: &PairingProgressData,
        generation: u64,
    ) -> Result<SecureInviteView> {
        let session = self
            .inner
            .lock()
            .map_err(|_| BackendError::Invalid(EXPIRED))?;
        let invite = session
            .invite
            .as_ref()
            .ok_or(BackendError::Invalid(EXPIRED))?;
        if session.generation != generation
            || session.phase != Phase::Invite
            || progress.state != PairingState::WaitingForPeer
            || session.ceremony_id != progress.pairing_id
            || invite.expires_at <= Instant::now()
        {
            return Err(BackendError::Invalid(EXPIRED));
        }
        let qr_svg = qrcode::QrCode::new(invite.link.as_bytes())
            .map_err(|_| BackendError::Invalid(EXPIRED))?
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(256, 256)
            .build();
        let expires_in_ms = invite
            .expires_at
            .saturating_duration_since(Instant::now())
            .as_millis() as u64;
        Ok(SecureInviteView {
            generation,
            ceremony_id: session
                .ceremony_id
                .clone()
                .ok_or(BackendError::Invalid(EXPIRED))?,
            code: invite.code.to_string(),
            address: invite.address.to_string(),
            qr_svg,
            expires_in_ms,
        })
    }

    fn reveal_sas(&self, progress: &PairingProgressData, generation: u64) -> Result<SecureSasView> {
        let mut session = self
            .inner
            .lock()
            .map_err(|_| BackendError::Invalid(EXPIRED))?;
        let remaining = progress
            .expires_in_ms
            .filter(|ms| *ms > 0)
            .ok_or(BackendError::Invalid(EXPIRED))?;
        if session.generation != generation
            || session.phase != Phase::Confirm
            || progress.state != PairingState::AwaitingConfirmation
            || session.ceremony_id != progress.pairing_id
        {
            return Err(BackendError::Invalid(EXPIRED));
        }
        let sas = sas_digits(progress)
            .ok_or(BackendError::Invalid(EXPIRED))?
            .to_owned();
        session.revealed_sas = Some(BoundSas {
            value: Zeroizing::new(sas.clone()),
            expires_at: Instant::now() + Duration::from_millis(remaining),
        });
        Ok(SecureSasView {
            generation,
            ceremony_id: session
                .ceremony_id
                .clone()
                .ok_or(BackendError::Invalid(EXPIRED))?,
            sas,
            expires_in_ms: remaining,
        })
    }

    fn take_decision(
        &self,
        progress: &PairingProgressData,
        generation: u64,
    ) -> Result<DecisionPermit> {
        let mut session = self
            .inner
            .lock()
            .map_err(|_| BackendError::Invalid(EXPIRED))?;
        if session.generation != generation
            || session.phase != Phase::Confirm
            || session.decision_in_flight
            || session.revealed_sas.as_ref().is_none_or(|bound| {
                bound.expires_at <= Instant::now()
                    || progress.sas.as_deref() != Some(bound.value.as_str())
            })
            || progress.state != PairingState::AwaitingConfirmation
            || progress.expires_in_ms.is_none_or(|ms| ms == 0)
            || session.ceremony_id != progress.pairing_id
        {
            return Err(BackendError::Invalid(EXPIRED));
        }
        session.revealed_sas = None;
        session.decision_in_flight = true;
        Ok(DecisionPermit {
            session: self.clone(),
            generation,
        })
    }
}

pub(crate) struct MacOsPairingUi {
    app: AppHandle,
    session: SecurePairingSession,
    _abort: NativeAbort,
}

impl MacOsPairingUi {
    pub(crate) fn new(app: AppHandle, session: SecurePairingSession, abort: NativeAbort) -> Self {
        Self {
            app,
            session,
            _abort: abort,
        }
    }

    fn open(&self) -> bool {
        let window = match self.app.get_webview_window(WINDOW_LABEL) {
            Some(window) => window,
            None => match WebviewWindowBuilder::new(
                &self.app,
                WINDOW_LABEL,
                WebviewUrl::App(ROUTE.into()),
            )
            .title("Connect a device")
            .inner_size(560.0, 520.0)
            .resizable(false)
            .visible(false)
            .content_protected(true)
            .on_navigation(|url| {
                url.path() == "/index.html" && url.query() == Some("surface=pairing")
            })
            .build()
            {
                Ok(window) => window,
                Err(_) => return false,
            },
        };
        if window.set_content_protected(true).is_err()
            || window.show().is_err()
            || window.set_focus().is_err()
        {
            let _ = window.destroy();
            return false;
        }
        true
    }
}

impl NativePairingUi for MacOsPairingUi {
    fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome {
        if !self.session.begin_invite(invite) || !self.open() {
            self.session.clear();
            return NativePresentationOutcome::Unavailable;
        }
        NativePresentationOutcome::Presented
    }

    fn scan_invite(&self) -> NativeScanOutcome {
        self.session.reset(Phase::Join);
        if self.open() {
            NativeScanOutcome::Cancelled
        } else {
            self.session.clear();
            NativeScanOutcome::Unavailable
        }
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        self.session.progress(progress);
        PairingPresentationState::Presented
    }

    fn confirm(&self, _progress: &PairingProgressData) -> Option<PairingDecision> {
        // The protected WebView submits a separately checked, bound decision.
        Some(PairingDecision::Refresh)
    }
}

fn require_window(window: &WebviewWindow) -> Result<()> {
    if window.label() == WINDOW_LABEL {
        Ok(())
    } else {
        Err(BackendError::Invalid(WRONG_WINDOW))
    }
}

pub(crate) async fn state(window: WebviewWindow, app: AppHandle) -> Result<SecurePairingView> {
    require_window(&window)?;
    let session = app.state::<SecurePairingSession>();
    let _operation = session.lock_operation().await;
    let generation = session.generation();
    let backend = app.state::<SelectedBackend>();
    let progress = backend.pair_progress().await?;
    if !session.progress_if_generation(&progress, Some(generation)) {
        return Err(BackendError::Invalid(EXPIRED));
    }
    let presenter = app.state::<super::PairingPresenter>();
    Ok(SecurePairingView {
        generation: session.generation(),
        phase: session.phase().to_owned(),
        ceremony: PairingCeremony::from_progress(
            progress.clone(),
            presenter.state_for_progress(progress.state),
        ),
    })
}

pub(crate) async fn reveal_invite(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
) -> Result<SecureInviteView> {
    require_window(&window)?;
    let session = app.state::<SecurePairingSession>();
    let _operation = session.lock_operation().await;
    let backend = app.state::<SelectedBackend>();
    let progress = backend.pair_progress().await?;
    session.reveal_invite(&progress, generation)
}

pub(crate) async fn reveal_sas(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
) -> Result<SecureSasView> {
    require_window(&window)?;
    let session = app.state::<SecurePairingSession>();
    let _operation = session.lock_operation().await;
    let backend = app.state::<SelectedBackend>();
    let progress = backend.pair_progress().await?;
    session.reveal_sas(&progress, generation)
}

pub(crate) async fn join(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
    code: String,
    addr: String,
) -> Result<PairingCeremony> {
    require_window(&window)?;
    let session = app.state::<SecurePairingSession>();
    let _operation = session.lock_operation().await;
    if !session.is_generation(generation) || session.phase() != "join" {
        return Err(BackendError::Invalid(EXPIRED));
    }
    let scanned = validate_native_invite_fields(Zeroizing::new(code), Zeroizing::new(addr))
        .ok_or(BackendError::Invalid("Check the pairing code and address."))?;
    let backend = app.state::<SelectedBackend>();
    let progress = backend
        .pair_join(scanned.code.as_str(), scanned.addr.as_str())
        .await?;
    if !session.progress_if_generation(&progress, Some(generation)) {
        // A close while join was pending must not strand the new backend
        // ceremony. A later replacement session has a different generation.
        if session.is_generation(generation.wrapping_add(1)) {
            if let Ok(current) = backend.pair_progress().await {
                if current.pairing_id == progress.pairing_id
                    && matches!(
                        current.state,
                        PairingState::WaitingForPeer
                            | PairingState::Handshaking
                            | PairingState::AwaitingConfirmation
                    )
                {
                    let _ = backend.pair_cancel().await;
                }
            }
        }
        return Err(BackendError::Invalid(EXPIRED));
    }
    let presenter = app.state::<super::PairingPresenter>();
    crate::commands::pairing::reconcile_progress(&*backend, &presenter, progress, false).await
}

pub(crate) async fn decide(
    window: WebviewWindow,
    app: AppHandle,
    generation: u64,
    accept: bool,
) -> Result<PairingCeremony> {
    require_window(&window)?;
    let backend = app.state::<SelectedBackend>();
    let session = app.state::<SecurePairingSession>();
    let _operation = session.lock_operation().await;
    let progress = backend.pair_progress().await?;
    let _decision = session.take_decision(&progress, generation)?;
    let next = backend.pair_confirm(accept).await?;
    if !session.is_generation(generation) {
        return Err(BackendError::Invalid(EXPIRED));
    }
    let presenter = app.state::<super::PairingPresenter>();
    crate::commands::pairing::reconcile_progress(&*backend, &presenter, next, false).await
}

pub(crate) fn window_closed(app: &AppHandle) -> bool {
    let Some((active, closed_generation, ceremony_id)) =
        app.state::<SecurePairingSession>().clear()
    else {
        return false;
    };
    if active {
        let backend = app.clone();
        tauri::async_runtime::spawn(async move {
            let session = backend.state::<SecurePairingSession>();
            let _operation = session.lock_operation().await;
            let pairing = backend.state::<SelectedBackend>();
            let Ok(progress) = pairing.pair_progress().await else {
                return;
            };
            if session.is_generation(closed_generation)
                && matches!(
                    progress.state,
                    PairingState::WaitingForPeer
                        | PairingState::Handshaking
                        | PairingState::AwaitingConfirmation
                )
                && (ceremony_id.is_none() || progress.pairing_id == ceremony_id)
            {
                let _ = pairing.pair_cancel().await;
            }
        });
    }
    true
}

pub(crate) fn destroy_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::{PairingRole, PairingState};

    fn progress(state: PairingState, id: &str) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some(id.into()),
            role: Some(PairingRole::Initiator),
            state,
            expires_in_ms: Some(60_000),
            sas: Some("123456".into()),
            peer_device_id: None,
            peer_name: None,
            peer_addr: None,
            known_device: None,
            error_code: None,
        }
    }

    #[test]
    fn invite_reveal_requires_current_generation_ceremony_and_deadline() {
        let session = SecurePairingSession::default();
        let invite = PairingInviteData {
            code: "0123-4567-89AB-CDEF".into(),
            pairing_id: "one".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
            expires_in_secs: 120,
        };
        assert!(session.begin_invite(&invite));
        let generation = session.generation();
        let waiting = progress(PairingState::WaitingForPeer, "one");
        assert!(session.reveal_invite(&waiting, generation).is_ok());
        assert!(session
            .reveal_invite(&waiting, generation.wrapping_add(1))
            .is_err());
        assert!(session
            .reveal_invite(&progress(PairingState::WaitingForPeer, "other"), generation)
            .is_err());
        session.clear();
        assert!(session.reveal_invite(&waiting, generation).is_err());
    }

    #[test]
    fn decision_requires_the_exact_sas_revealed_for_this_generation() {
        let session = SecurePairingSession::default();
        session.reset(Phase::Join);
        let awaiting = progress(PairingState::AwaitingConfirmation, "one");
        session.progress(&awaiting);
        let generation = session.generation();
        assert!(session.take_decision(&awaiting, generation).is_err());
        let revealed = session
            .reveal_sas(&awaiting, generation)
            .expect("bound SAS");
        assert_eq!(revealed.sas, "123456");
        let mut changed = awaiting.clone();
        changed.sas = Some("654321".into());
        assert!(session.take_decision(&changed, generation).is_err());
        assert!(session.take_decision(&awaiting, generation).is_ok());
        assert!(session.take_decision(&awaiting, generation).is_err());
        session.reset(Phase::Join);
        session.progress(&progress(PairingState::AwaitingConfirmation, "two"));
        assert!(session.take_decision(&awaiting, generation).is_err());
    }

    #[test]
    fn close_waits_for_a_submitted_decision_and_unblocks_after_result() {
        let session = SecurePairingSession::default();
        session.reset(Phase::Join);
        let awaiting = progress(PairingState::AwaitingConfirmation, "one");
        session.progress(&awaiting);
        let generation = session.generation();
        session
            .reveal_sas(&awaiting, generation)
            .expect("bound SAS");
        let permit = session
            .take_decision(&awaiting, generation)
            .expect("submitted");
        assert!(
            session.clear().is_none(),
            "OS/custom close must wait for the backend"
        );
        assert!(session.is_generation(generation));

        session.progress(&progress(PairingState::Confirmed, "one"));
        drop(permit);
        assert!(session.clear().is_some(), "terminal result may close");

        session.reset(Phase::Join);
        let awaiting = progress(PairingState::AwaitingConfirmation, "two");
        session.progress(&awaiting);
        let generation = session.generation();
        session
            .reveal_sas(&awaiting, generation)
            .expect("bound SAS");
        let permit = session
            .take_decision(&awaiting, generation)
            .expect("submitted");
        assert!(session.clear().is_none());
        drop(permit);
        assert!(
            session.clear().is_some(),
            "backend error also releases close"
        );
    }
}
