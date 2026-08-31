use copypaste_ipc::{PairingProgressData, PairingState};
use serde::Serialize;

use super::native_copy;
use super::resolve_pairing_semantics;
use super::semantics::PairingMessageId;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AndroidProgressPayload {
    pub(super) semantics: AndroidPairingSemantics,
    pub(super) copy: AndroidPairingCopy,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AndroidPairingSemantics {
    pub(super) message_id: PairingMessageId,
    pub(super) active: bool,
    pub(super) terminal: bool,
    pub(super) retry: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct AndroidPairingCopy {
    pub(super) title: &'static str,
    pub(super) detail: &'static str,
}

impl From<&PairingProgressData> for AndroidProgressPayload {
    fn from(progress: &PairingProgressData) -> Self {
        let semantics = resolve_pairing_semantics(progress.state, progress.error_code);
        let copy = native_copy::copy(semantics.message_id);
        Self {
            semantics: AndroidPairingSemantics {
                message_id: semantics.message_id,
                active: semantics.active,
                terminal: semantics.terminal,
                retry: semantics.retry,
            },
            copy: AndroidPairingCopy {
                title: copy.title,
                detail: copy.detail,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use copypaste_ipc::{ErrorCode, PairingRole};

    use super::*;

    fn progress(state: PairingState, error_code: Option<ErrorCode>) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("ceremony-1".into()),
            role: Some(PairingRole::Initiator),
            state,
            expires_in_ms: Some(60_000),
            sas: Some("123456".into()),
            peer_device_id: Some("device-secret".into()),
            peer_name: Some("Unverified Phone".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code,
        }
    }

    #[test]
    fn serialization_sends_canonical_safe_copy_not_raw_pairing_state() {
        let payload = AndroidProgressPayload::from(&progress(
            PairingState::Failed,
            Some(ErrorCode::PeerUnreachable),
        ));
        let json = serde_json::to_string(&payload).unwrap();

        assert!(json.contains("\"messageId\":\"unreachable\""), "{json}");
        assert!(json.contains("\"active\":false"), "{json}");
        assert!(json.contains("\"terminal\":true"), "{json}");
        assert!(json.contains("\"retry\":true"), "{json}");
        assert!(json.contains("Could not reach the other device"), "{json}");
        for forbidden in [
            "state",
            "error_code",
            "peer_unreachable",
            "123456",
            "device-secret",
            "Unverified Phone",
            "192.0.2.1",
        ] {
            assert!(!json.contains(forbidden), "pairing data leaked: {json}");
        }
    }

    #[test]
    fn every_pairing_state_serializes_a_canonical_message_and_safe_copy() {
        for (state, message_id) in [
            (PairingState::Idle, "ready"),
            (PairingState::WaitingForPeer, "waiting_for_peer"),
            (PairingState::Handshaking, "securing_connection"),
            (PairingState::AwaitingConfirmation, "compare_codes"),
            (PairingState::Confirmed, "paired"),
            (PairingState::Rejected, "rejected"),
            (PairingState::Cancelled, "cancelled"),
            (PairingState::TimedOut, "timed_out"),
            (PairingState::Failed, "failed"),
        ] {
            let payload = AndroidProgressPayload::from(&progress(state, None));
            let json = serde_json::to_string(&payload).unwrap();

            assert!(
                json.contains(&format!("\"messageId\":\"{message_id}\"")),
                "{json}"
            );
            assert!(json.contains("\"title\":"), "{json}");
            assert!(json.contains("\"detail\":"), "{json}");
            assert!(!json.contains("\"state\":"), "{json}");
        }
    }

    #[test]
    fn every_typed_error_code_serializes_a_resolved_safe_payload() {
        for error_code in [
            ErrorCode::NotFound,
            ErrorCode::InvalidRequest,
            ErrorCode::ProtocolMismatch,
            ErrorCode::NotReady,
            ErrorCode::AuthFailed,
            ErrorCode::KeyLocked,
            ErrorCode::KeyUnusable,
            ErrorCode::UnsupportedContent,
            ErrorCode::ContentTooLarge,
            ErrorCode::PairingCode,
            ErrorCode::PairingAddress,
            ErrorCode::RateLimited,
            ErrorCode::PeerUnreachable,
            ErrorCode::PairingLimit,
            ErrorCode::PeerFailed,
            ErrorCode::PeerVersion,
            ErrorCode::PeerNotFound,
            ErrorCode::Internal,
        ] {
            let payload =
                AndroidProgressPayload::from(&progress(PairingState::Failed, Some(error_code)));
            let json = serde_json::to_string(&payload).unwrap();

            assert!(json.contains("\"messageId\":"), "{json}");
            assert!(json.contains("\"title\":"), "{json}");
            assert!(json.contains("\"detail\":"), "{json}");
            assert!(!json.contains(error_code.as_str()), "{json}");
        }
    }
}
