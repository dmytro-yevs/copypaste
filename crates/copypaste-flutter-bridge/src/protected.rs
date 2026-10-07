//! Rust-held pairing ceremony state.
//!
//! Ordinary progress contains only a random ceremony id and sanitized state.
//! The active pairing inspector explicitly reveals a fresh invitation or bound
//! SAS. A user-entered join code is consumed with its endpoint and never logged.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use copypaste_ipc::{Method, PairingProgressData, ResponseData};
use uuid::Uuid;

use crate::api::{PairingCeremony, PairingInvitation, RuntimeError};

struct SecretCeremony {
    invitation: Option<copypaste_ipc::PairingInviteData>,
    pairing_id: String,
    native_context: Option<String>,
    generation: u64,
    decision_in_flight: bool,
}

static CEREMONIES: OnceLock<Mutex<HashMap<String, SecretCeremony>>> = OnceLock::new();

fn ceremonies() -> &'static Mutex<HashMap<String, SecretCeremony>> {
    CEREMONIES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) async fn create() -> Result<PairingCeremony, RuntimeError> {
    let response = crate::client::request(Method::PairCreateInvite).await?;
    let Some(ResponseData::PairingInvite(invitation)) = response.data else {
        return Err(RuntimeError::internal());
    };
    let ceremony_id = Uuid::new_v4().to_string();
    let pairing_id = invitation.pairing_id.clone();
    let public = PairingCeremony {
        ceremony_id: ceremony_id.clone(),
        state: "waiting_for_peer".into(),
        expires_in_ms: Some(invitation.expires_in_secs.saturating_mul(1_000)),
        peer_name: None,
        failure_message: None,
    };
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            ceremony_id,
            SecretCeremony {
                invitation: Some(invitation),
                pairing_id,
                native_context: None,
                generation: 1,
                decision_in_flight: false,
            },
        );
    Ok(public)
}

pub(crate) async fn join(code: String, addr: String) -> Result<PairingCeremony, RuntimeError> {
    let token = copypaste_p2p::PairingToken::parse(&code).map_err(|_| {
        RuntimeError::from_daemon(
            "pairing_invalid_code".into(),
            "That pairing code is not valid.".into(),
        )
    })?;
    let pairing_id = token.pairing_id();
    drop(token);
    let response = crate::client::request(Method::PairJoin { code, addr }).await?;
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return Err(RuntimeError::internal());
    };
    if progress.pairing_id.as_deref() != Some(pairing_id.as_str()) {
        return Err(RuntimeError::internal());
    }
    let ceremony_id = Uuid::new_v4().to_string();
    let public = sanitize(&ceremony_id, progress);
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            ceremony_id,
            SecretCeremony {
                invitation: None,
                pairing_id,
                native_context: None,
                generation: 1,
                decision_in_flight: false,
            },
        );
    Ok(public)
}

pub(crate) async fn join_uri(uri: String) -> Result<PairingCeremony, RuntimeError> {
    let link = copypaste_p2p::PairingLink::parse(&uri).map_err(|_| {
        RuntimeError::from_daemon(
            "pairing_invalid_link".into(),
            "That CopyPaste pairing link is not valid.".into(),
        )
    })?;
    let address = link.address().map_err(|_| {
        RuntimeError::from_daemon(
            "pairing_address_unavailable".into(),
            "That pairing invitation has no reachable device address.".into(),
        )
    })?;
    join(link.code().to_string(), address.to_string()).await
}

pub(crate) async fn status(ceremony_id: &str) -> Result<PairingCeremony, RuntimeError> {
    let pairing_id = pairing_id(ceremony_id)?;
    let response = crate::client::request(Method::PairProgress).await?;
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return Err(RuntimeError::internal());
    };
    if progress.pairing_id.as_deref() != Some(pairing_id.as_str()) {
        return Err(protected_action_rejected());
    }
    Ok(sanitize(ceremony_id, progress))
}

pub(crate) async fn confirm(
    ceremony_id: &str,
    verification_code: &str,
    accept: bool,
) -> Result<PairingCeremony, RuntimeError> {
    let pairing_id = pairing_id(ceremony_id)?;
    let progress_response = crate::client::request(Method::PairProgress).await?;
    let Some(ResponseData::PairingProgress(progress)) = progress_response.data else {
        return Err(RuntimeError::internal());
    };
    if progress.pairing_id.as_deref() != Some(pairing_id.as_str())
        || progress.state != copypaste_ipc::PairingState::AwaitingConfirmation
        || progress
            .expires_in_ms
            .is_none_or(|remaining| remaining == 0)
        || progress.sas.as_deref() != Some(verification_code)
    {
        return Err(protected_action_rejected());
    }
    let response = crate::client::request(Method::PairConfirm { accept }).await?;
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return Err(RuntimeError::internal());
    };
    let public = sanitize(ceremony_id, progress);
    if matches!(
        public.state.as_str(),
        "confirmed" | "rejected" | "cancelled" | "timed_out" | "failed"
    ) {
        remove(ceremony_id);
    }
    Ok(public)
}

pub(crate) async fn reveal_invitation_for_flutter(
    ceremony_id: &str,
) -> Result<PairingInvitation, RuntimeError> {
    reveal_invitation_with_progress(ceremony_id, || async {
        let response = crate::client::request(Method::PairProgress).await?;
        let Some(ResponseData::PairingProgress(progress)) = response.data else {
            return Err(RuntimeError::internal());
        };
        Ok(progress)
    })
    .await
}

async fn reveal_invitation_with_progress<F, Fut>(
    ceremony_id: &str,
    progress_request: F,
) -> Result<PairingInvitation, RuntimeError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<PairingProgressData, RuntimeError>>,
{
    let (expected_pairing_id, payload, code, address) = {
        let guard = ceremonies()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ceremony = guard
            .get(ceremony_id)
            .ok_or_else(RuntimeError::ceremony_not_found)?;
        let invitation = ceremony
            .invitation
            .as_ref()
            .ok_or_else(protected_action_rejected)?;
        let payload = invitation_uri(invitation).ok_or_else(RuntimeError::internal)?;
        (
            ceremony.pairing_id.clone(),
            payload,
            invitation.code.clone(),
            invitation.listen_addr.clone(),
        )
    };
    let progress = progress_request().await?;
    if !qr_progress_allows_reveal(&progress, &expected_pairing_id) {
        return Err(protected_action_rejected());
    }
    require(ceremony_id)?;
    Ok(PairingInvitation {
        qr_png: qr_png(&payload).ok_or_else(RuntimeError::internal)?,
        code,
        address,
    })
}

pub(crate) async fn reveal_sas_for_flutter(ceremony_id: &str) -> Result<String, RuntimeError> {
    let expected_pairing_id = pairing_id(ceremony_id)?;
    let response = crate::client::request(Method::PairProgress).await?;
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return Err(RuntimeError::internal());
    };
    if progress.pairing_id.as_deref() != Some(expected_pairing_id.as_str())
        || progress.state != copypaste_ipc::PairingState::AwaitingConfirmation
        || progress
            .expires_in_ms
            .is_none_or(|remaining| remaining == 0)
    {
        return Err(protected_action_rejected());
    }
    progress.sas.ok_or_else(protected_action_rejected)
}

pub(crate) async fn cancel(ceremony_id: &str) -> Result<PairingCeremony, RuntimeError> {
    require(ceremony_id)?;
    let response = crate::client::request(Method::PairCancel).await?;
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return Err(RuntimeError::internal());
    };
    let public = sanitize(ceremony_id, progress);
    remove(ceremony_id);
    Ok(public)
}

pub(crate) async fn dispose(ceremony_id: &str) -> Result<(), RuntimeError> {
    require(ceremony_id)?;
    let _ = crate::client::request(Method::PairCancel).await?;
    remove(ceremony_id);
    Ok(())
}

fn require(ceremony_id: &str) -> Result<(), RuntimeError> {
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(ceremony_id)
        .then_some(())
        .ok_or_else(RuntimeError::ceremony_not_found)
}

fn pairing_id(ceremony_id: &str) -> Result<String, RuntimeError> {
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(ceremony_id)
        .map(|ceremony| ceremony.pairing_id.clone())
        .ok_or_else(RuntimeError::ceremony_not_found)
}

fn protected_action_rejected() -> RuntimeError {
    RuntimeError::from_daemon(
        "pairing_action_rejected".into(),
        "That pairing action is no longer available.".into(),
    )
}

fn remove(ceremony_id: &str) {
    let _ = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(ceremony_id);
}

/// Binds one native protected-presenter context to one existing ceremony.
///
/// This is deliberately not exposed through FRB. A Dart page can carry a
/// ceremony id for safe progress, but cannot mint or attach a native context
/// capability that would authorize protected presentation.
pub(crate) fn attach_native_context(ceremony_id: &str, context_id: &str) -> Option<u64> {
    if !valid_context_id(context_id) {
        return None;
    }
    let mut guard = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ceremony = guard.get_mut(ceremony_id)?;
    match ceremony.native_context.as_deref() {
        None => {
            ceremony.generation = ceremony.generation.checked_add(1)?;
            ceremony.native_context = Some(context_id.to_string());
            Some(ceremony.generation)
        }
        Some(existing) if existing == context_id => Some(ceremony.generation),
        Some(_) => None,
    }
}

pub(crate) fn native_context_active(context_id: &str) -> bool {
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .any(|ceremony| ceremony.native_context.as_deref() == Some(context_id))
}

pub(crate) fn detach_native_context(context_id: &str) -> bool {
    let mut guard = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some((_, ceremony)) = guard
        .iter_mut()
        .find(|(_, ceremony)| ceremony.native_context.as_deref() == Some(context_id))
    else {
        return false;
    };
    ceremony.native_context = None;
    // Never reuse a lease after its native capability was released. In
    // particular, a presenter may reuse its opaque context id after teardown.
    // A stale callback from its earlier lifetime must remain rejected.
    ceremony.generation = ceremony.generation.saturating_add(1);
    true
}

pub(crate) async fn cancel_native_context(context_id: &str) -> bool {
    let ceremony_id = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|(_, ceremony)| {
            ceremony.native_context.as_deref() == Some(context_id) && !ceremony.decision_in_flight
        })
        .map(|(id, _)| id.clone());
    let Some(ceremony_id) = ceremony_id else {
        return false;
    };
    if crate::client::request(Method::PairCancel).await.is_err() {
        return false;
    }
    remove(&ceremony_id);
    true
}

pub(crate) async fn native_join(
    context_id: &str,
    generation: u64,
    code: String,
    addr: String,
) -> bool {
    if !context_matches(context_id, generation) {
        return false;
    }
    crate::client::request(Method::PairJoin { code, addr })
        .await
        .is_ok()
}

pub(crate) async fn native_join_uri(context_id: &str, generation: u64, uri: String) -> bool {
    let Ok(link) = copypaste_p2p::PairingLink::parse(&uri) else {
        return false;
    };
    let Ok(address) = link.address() else {
        return false;
    };
    native_join(
        context_id,
        generation,
        link.code().to_string(),
        address.to_string(),
    )
    .await
}

pub(crate) async fn native_status(context_id: &str, generation: u64) -> Option<(u32, u64)> {
    let pairing_id = context_pairing_id(context_id, generation)?;
    let response = crate::client::request(Method::PairProgress).await.ok()?;
    let ResponseData::PairingProgress(progress) = response.data? else {
        return None;
    };
    (progress.pairing_id.as_deref() == Some(pairing_id.as_str())).then_some((
        state_code(progress.state),
        progress.expires_in_ms.unwrap_or(0),
    ))
}

pub(crate) async fn native_decision(
    context_id: &str,
    generation: u64,
    sas: zeroize::Zeroizing<String>,
    accept: bool,
) -> bool {
    let pairing_id = context_pairing_id(context_id, generation);
    let Some(pairing_id) = pairing_id else {
        return false;
    };
    let response = match crate::client::request(Method::PairProgress).await {
        Ok(response) => response,
        Err(_) => return false,
    };
    let Some(ResponseData::PairingProgress(progress)) = response.data else {
        return false;
    };
    if progress.pairing_id.as_deref() != Some(pairing_id.as_str())
        || progress.state != copypaste_ipc::PairingState::AwaitingConfirmation
        || progress
            .expires_in_ms
            .is_none_or(|remaining| remaining == 0)
        || progress.sas.as_deref() != Some(sas.as_str())
    {
        return false;
    }
    {
        let mut guard = ceremonies()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(ceremony) = guard.values_mut().find(|ceremony| {
            ceremony.native_context.as_deref() == Some(context_id)
                && ceremony.generation == generation
        }) else {
            return false;
        };
        if ceremony.decision_in_flight {
            return false;
        }
        ceremony.decision_in_flight = true;
    }
    let result = crate::client::request(Method::PairConfirm { accept })
        .await
        .is_ok();
    if let Ok(mut guard) = ceremonies().lock() {
        if let Some(ceremony) = guard.values_mut().find(|ceremony| {
            ceremony.native_context.as_deref() == Some(context_id)
                && ceremony.generation == generation
        }) {
            ceremony.decision_in_flight = false;
        }
    }
    result
}

fn context_matches(context_id: &str, generation: u64) -> bool {
    context_pairing_id(context_id, generation).is_some()
}
fn context_pairing_id(context_id: &str, generation: u64) -> Option<String> {
    ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .find(|ceremony| {
            ceremony.native_context.as_deref() == Some(context_id)
                && ceremony.generation == generation
        })
        .map(|ceremony| ceremony.pairing_id.clone())
}
fn state_code(state: copypaste_ipc::PairingState) -> u32 {
    match state {
        copypaste_ipc::PairingState::Idle => 0,
        copypaste_ipc::PairingState::WaitingForPeer => 1,
        copypaste_ipc::PairingState::Handshaking => 2,
        copypaste_ipc::PairingState::AwaitingConfirmation => 3,
        copypaste_ipc::PairingState::Confirmed => 4,
        copypaste_ipc::PairingState::Rejected => 5,
        copypaste_ipc::PairingState::Cancelled => 6,
        copypaste_ipc::PairingState::TimedOut => 7,
        copypaste_ipc::PairingState::Failed => 8,
    }
}

pub(crate) async fn reveal_sas(context_id: &str, generation: u64) -> Option<Vec<u8>> {
    let expected_pairing_id = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|(_, ceremony)| {
            ceremony.native_context.as_deref() == Some(context_id)
                && ceremony.generation == generation
        })
        .map(|(_, ceremony)| ceremony.pairing_id.clone())?;
    let response = crate::client::request(Method::PairProgress).await.ok()?;
    let ResponseData::PairingProgress(progress) = response.data? else {
        return None;
    };
    (progress.pairing_id.as_deref() == Some(expected_pairing_id.as_str())
        && progress.state == copypaste_ipc::PairingState::AwaitingConfirmation
        && progress
            .expires_in_ms
            .is_some_and(|remaining| remaining > 0))
    .then_some(progress.sas?)
    .map(String::into_bytes)
}

pub(crate) async fn reveal_qr_png(
    ceremony_id: &str,
    context_id: &str,
    generation: u64,
) -> Option<Vec<u8>> {
    reveal_qr_png_with_progress(ceremony_id, context_id, generation, || async {
        let response = crate::client::request(Method::PairProgress).await.ok()?;
        let ResponseData::PairingProgress(progress) = response.data? else {
            return None;
        };
        Some(progress)
    })
    .await
}

async fn reveal_qr_png_with_progress<F, Fut>(
    ceremony_id: &str,
    context_id: &str,
    generation: u64,
    progress_request: F,
) -> Option<Vec<u8>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Option<PairingProgressData>>,
{
    let (pairing_id, payload) = {
        let guard = ceremonies()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ceremony = guard.get(ceremony_id)?;
        let invitation = ceremony.invitation.as_ref()?;
        if ceremony.native_context.as_deref() != Some(context_id)
            || ceremony.generation != generation
        {
            return None;
        }
        let payload = invitation_uri(invitation)?;
        (ceremony.pairing_id.clone(), payload)
    };

    let progress = progress_request().await?;
    if !qr_progress_allows_reveal(&progress, &pairing_id) {
        return None;
    }

    let guard = ceremonies()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ceremony = guard.get(ceremony_id)?;
    (ceremony.native_context.as_deref() == Some(context_id) && ceremony.generation == generation)
        .then(|| qr_png(&payload))?
}

fn qr_progress_allows_reveal(progress: &PairingProgressData, expected_pairing_id: &str) -> bool {
    progress.pairing_id.as_deref() == Some(expected_pairing_id)
        && progress.role == Some(copypaste_ipc::PairingRole::Responder)
        && progress.state == copypaste_ipc::PairingState::WaitingForPeer
        && progress
            .expires_in_ms
            .is_some_and(|remaining| remaining > 0)
}

fn invitation_uri(
    invitation: &copypaste_ipc::PairingInviteData,
) -> Option<zeroize::Zeroizing<String>> {
    let address = invitation
        .listen_addr
        .as_deref()
        .map(str::parse)
        .transpose()
        .ok()?;
    let link = copypaste_p2p::PairingLink::new(&invitation.code, address).ok()?;
    Some(zeroize::Zeroizing::new(link.to_uri()))
}

fn qr_png(payload: &str) -> Option<Vec<u8>> {
    use image::codecs::png::PngEncoder;
    use image::{ColorType, ImageEncoder, Luma};

    let qr = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    let image = qr
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(320, 320)
        .build();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::L8.into(),
        )
        .ok()?;
    Some(bytes)
}

fn valid_context_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn sanitize(ceremony_id: &str, progress: PairingProgressData) -> PairingCeremony {
    PairingCeremony {
        ceremony_id: ceremony_id.to_string(),
        state: match progress.state {
            copypaste_ipc::PairingState::Idle => "idle",
            copypaste_ipc::PairingState::WaitingForPeer => "waiting_for_peer",
            copypaste_ipc::PairingState::Handshaking => "handshaking",
            copypaste_ipc::PairingState::AwaitingConfirmation => "awaiting_confirmation",
            copypaste_ipc::PairingState::Confirmed => "confirmed",
            copypaste_ipc::PairingState::Rejected => "rejected",
            copypaste_ipc::PairingState::Cancelled => "cancelled",
            copypaste_ipc::PairingState::TimedOut => "timed_out",
            copypaste_ipc::PairingState::Failed => "failed",
        }
        .into(),
        expires_in_ms: progress.expires_in_ms,
        peer_name: progress.peer_name,
        failure_message: match progress.error_code {
            Some(copypaste_ipc::ErrorCode::PairingAlreadyExists) => {
                Some("Device is already paired.".into())
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_progress_has_no_sas_or_invitation_fields() {
        let ceremony = sanitize(
            "opaque-ceremony",
            PairingProgressData {
                pairing_id: Some("pairing-id".into()),
                role: None,
                state: copypaste_ipc::PairingState::AwaitingConfirmation,
                expires_in_ms: Some(1_000),
                sas: Some("123456".into()),
                peer_device_id: Some("peer-id".into()),
                peer_name: Some("Phone".into()),
                peer_addr: Some("192.0.2.1:1234".into()),
                known_device: None,
                error_code: None,
            },
        );
        let rendered = format!("{ceremony:?}");
        for secret in ["123456", "pairing-id", "peer-id", "192.0.2.1"] {
            assert!(
                !rendered.contains(secret),
                "secret escaped ceremony: {secret}"
            );
        }
    }

    #[test]
    fn duplicate_pairing_failure_has_one_safe_user_message() {
        let ceremony = sanitize(
            "opaque-ceremony",
            PairingProgressData {
                pairing_id: Some("new-pairing-id".into()),
                role: None,
                state: copypaste_ipc::PairingState::Failed,
                expires_in_ms: None,
                sas: None,
                peer_device_id: Some("existing-device-id".into()),
                peer_name: Some("Phone".into()),
                peer_addr: None,
                known_device: None,
                error_code: Some(copypaste_ipc::ErrorCode::PairingAlreadyExists),
            },
        );

        assert_eq!(
            ceremony.failure_message.as_deref(),
            Some("Device is already paired.")
        );
        let rendered = format!("{ceremony:?}");
        assert!(!rendered.contains("new-pairing-id"));
        assert!(!rendered.contains("existing-device-id"));
    }

    #[tokio::test]
    async fn join_rejects_an_invalid_code_before_contacting_the_runtime() {
        let error = join("not-a-pairing-code".into(), "192.0.2.10:47654".into())
            .await
            .unwrap_err();

        assert_eq!(error.code, "pairing_invalid_code");
    }

    #[tokio::test]
    async fn join_rejects_a_non_copypaste_uri_before_contacting_the_runtime() {
        let error = join_uri("https://example.com/pair".into())
            .await
            .unwrap_err();

        assert_eq!(error.code, "pairing_invalid_link");
    }

    #[test]
    fn invitation_qr_payload_is_the_versioned_application_uri() {
        let token = copypaste_p2p::PairingToken::generate();
        let invitation = copypaste_ipc::PairingInviteData {
            code: token.to_code(),
            pairing_id: token.pairing_id(),
            listen_addr: Some("192.0.2.10:47654".into()),
            expires_in_secs: 60,
        };

        let payload = invitation_uri(&invitation).unwrap();
        let parsed = copypaste_p2p::PairingLink::parse(&payload).unwrap();

        assert_eq!(parsed.pairing_id(), invitation.pairing_id);
        assert_eq!(parsed.address().unwrap().to_string(), "192.0.2.10:47654");
    }

    #[test]
    fn native_context_leases_rotate_on_attach_and_detach() {
        let id = "test-native-context";
        ceremonies().lock().unwrap().insert(
            id.into(),
            SecretCeremony {
                invitation: Some(copypaste_ipc::PairingInviteData {
                    code: "secret".into(),
                    pairing_id: "pair".into(),
                    listen_addr: None,
                    expires_in_secs: 60,
                }),
                pairing_id: "pair".into(),
                native_context: None,
                generation: 1,
                decision_in_flight: false,
            },
        );
        let first_generation = attach_native_context(id, "native-1").unwrap();
        assert_ne!(first_generation, 1);
        assert!(native_context_active("native-1"));
        assert!(attach_native_context(id, "native-2").is_none());
        assert!(context_matches("native-1", first_generation));
        assert!(!detach_native_context("native-2"));
        assert!(context_matches("native-1", first_generation));
        assert!(detach_native_context("native-1"));
        assert!(!native_context_active("native-1"));
        assert!(!context_matches("native-1", first_generation));
        let second_generation = attach_native_context(id, "native-1").unwrap();
        assert_ne!(second_generation, first_generation);
        assert!(!context_matches("native-1", first_generation));
        assert!(context_matches("native-1", second_generation));
        remove(id);
    }

    fn progress(
        pairing_id: &str,
        role: Option<copypaste_ipc::PairingRole>,
        state: copypaste_ipc::PairingState,
        expires_in_ms: Option<u64>,
    ) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some(pairing_id.into()),
            role,
            state,
            expires_in_ms,
            sas: None,
            peer_device_id: None,
            peer_name: None,
            peer_addr: None,
            known_device: None,
            error_code: None,
        }
    }

    #[tokio::test]
    async fn flutter_invitation_details_share_the_qr_lifecycle_guard() {
        let id = "test-flutter-invitation-details";
        let token = copypaste_p2p::PairingToken::generate();
        let code = token.to_code();
        let pairing_id = token.pairing_id();
        ceremonies().lock().unwrap().insert(
            id.into(),
            SecretCeremony {
                invitation: Some(copypaste_ipc::PairingInviteData {
                    code: code.clone(),
                    pairing_id: pairing_id.clone(),
                    listen_addr: Some("192.0.2.10:47654".into()),
                    expires_in_secs: 60,
                }),
                pairing_id: pairing_id.clone(),
                native_context: None,
                generation: 1,
                decision_in_flight: false,
            },
        );
        let result = reveal_invitation_with_progress(id, || async {
            Ok(progress(
                &pairing_id,
                Some(copypaste_ipc::PairingRole::Responder),
                copypaste_ipc::PairingState::WaitingForPeer,
                Some(1),
            ))
        })
        .await
        .unwrap();
        assert_eq!(result.code, code);
        assert_eq!(result.address.as_deref(), Some("192.0.2.10:47654"));
        assert_eq!(&result.qr_png[..8], b"\x89PNG\r\n\x1a\n");
        for state in [
            copypaste_ipc::PairingState::Handshaking,
            copypaste_ipc::PairingState::AwaitingConfirmation,
            copypaste_ipc::PairingState::TimedOut,
        ] {
            assert!(reveal_invitation_with_progress(id, || async {
                Ok(progress(
                    &pairing_id,
                    Some(copypaste_ipc::PairingRole::Responder),
                    state,
                    Some(1),
                ))
            })
            .await
            .is_err());
        }
        for (role, remaining) in [
            (copypaste_ipc::PairingRole::Initiator, 1),
            (copypaste_ipc::PairingRole::Responder, 0),
        ] {
            assert!(reveal_invitation_with_progress(id, || async {
                Ok(progress(
                    &pairing_id,
                    Some(role),
                    copypaste_ipc::PairingState::WaitingForPeer,
                    Some(remaining),
                ))
            })
            .await
            .is_err());
        }
        assert!(reveal_invitation_with_progress(id, || async {
            remove(id);
            Ok(progress(
                &pairing_id,
                Some(copypaste_ipc::PairingRole::Responder),
                copypaste_ipc::PairingState::WaitingForPeer,
                Some(1),
            ))
        })
        .await
        .is_err());
    }

    #[test]
    fn qr_reveal_requires_fresh_authoritative_invitation_owner_progress() {
        let id = "test-qr-progress";
        let token = copypaste_p2p::PairingToken::generate();
        let code = token.to_code();
        let pairing_id = token.pairing_id();
        ceremonies().lock().unwrap().insert(
            id.into(),
            SecretCeremony {
                invitation: Some(copypaste_ipc::PairingInviteData {
                    code,
                    pairing_id: pairing_id.clone(),
                    listen_addr: Some("192.0.2.10:47654".into()),
                    expires_in_secs: 60,
                }),
                pairing_id: pairing_id.clone(),
                native_context: None,
                generation: 1,
                decision_in_flight: false,
            },
        );
        let generation = attach_native_context(id, "native-qr").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let revealed = runtime.block_on(reveal_qr_png_with_progress(
            id,
            "native-qr",
            generation,
            || async {
                Some(progress(
                    &pairing_id,
                    Some(copypaste_ipc::PairingRole::Responder),
                    copypaste_ipc::PairingState::WaitingForPeer,
                    Some(1),
                ))
            },
        ));
        assert!(revealed.is_some());

        for state in [
            copypaste_ipc::PairingState::Handshaking,
            copypaste_ipc::PairingState::AwaitingConfirmation,
            copypaste_ipc::PairingState::Confirmed,
            copypaste_ipc::PairingState::Rejected,
            copypaste_ipc::PairingState::Cancelled,
            copypaste_ipc::PairingState::TimedOut,
            copypaste_ipc::PairingState::Failed,
        ] {
            assert!(runtime
                .block_on(reveal_qr_png_with_progress(
                    id,
                    "native-qr",
                    generation,
                    || async {
                        Some(progress(
                            &pairing_id,
                            Some(copypaste_ipc::PairingRole::Responder),
                            state,
                            Some(1),
                        ))
                    }
                ))
                .is_none());
        }
        assert!(runtime
            .block_on(reveal_qr_png_with_progress(
                id,
                "native-qr",
                generation,
                || async {
                    Some(progress(
                        &pairing_id,
                        Some(copypaste_ipc::PairingRole::Initiator),
                        copypaste_ipc::PairingState::WaitingForPeer,
                        Some(1),
                    ))
                }
            ))
            .is_none());
        assert!(runtime
            .block_on(reveal_qr_png_with_progress(
                id,
                "native-qr",
                generation,
                || async {
                    Some(progress(
                        &pairing_id,
                        Some(copypaste_ipc::PairingRole::Responder),
                        copypaste_ipc::PairingState::WaitingForPeer,
                        Some(0),
                    ))
                }
            ))
            .is_none());
        assert!(runtime
            .block_on(reveal_qr_png_with_progress(
                id,
                "native-qr",
                generation,
                || async {
                    Some(progress(
                        "other-pair",
                        Some(copypaste_ipc::PairingRole::Responder),
                        copypaste_ipc::PairingState::WaitingForPeer,
                        Some(1),
                    ))
                }
            ))
            .is_none());
        assert!(runtime
            .block_on(reveal_qr_png_with_progress(
                id,
                "native-qr",
                generation,
                || async {
                    assert!(detach_native_context("native-qr"));
                    Some(progress(
                        &pairing_id,
                        Some(copypaste_ipc::PairingRole::Responder),
                        copypaste_ipc::PairingState::WaitingForPeer,
                        Some(1),
                    ))
                }
            ))
            .is_none());
        remove(id);
    }
}
