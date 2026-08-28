use copypaste_ipc::PairingProgressData;

use super::native_copy::copy;
use super::semantics::resolve_pairing_semantics;

pub(super) struct ProgressCopy {
    pub title: &'static str,
    pub message: &'static str,
}

pub(super) fn progress_copy(progress: &PairingProgressData) -> ProgressCopy {
    let copy = copy(resolve_pairing_semantics(progress.state, progress.error_code).message_id);
    ProgressCopy {
        title: copy.title,
        message: copy.detail,
    }
}

pub(super) fn sas_digits(progress: &PairingProgressData) -> Option<&str> {
    progress
        .sas
        .as_deref()
        .filter(|sas| sas.len() == 6 && sas.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::{ErrorCode, PairingRole, PairingState};

    fn progress(state: PairingState) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("ceremony-secret".into()),
            role: Some(PairingRole::Initiator),
            state,
            expires_in_ms: (state == PairingState::AwaitingConfirmation).then_some(60_000),
            sas: Some("123456".into()),
            peer_device_id: Some("device-secret".into()),
            peer_name: Some("/Users/alice/Phone".into()),
            peer_addr: Some("/Users/alice/private.sock".into()),
            known_device: None,
            error_code: None,
        }
    }

    #[test]
    fn macos_uses_the_canonical_copy_table_without_peer_or_secret_data() {
        let copy = progress_copy(&progress(PairingState::AwaitingConfirmation));
        assert_eq!(copy.title, "Compare security codes");
        assert_eq!(
            copy.message,
            "Confirm the code in the native security prompt."
        );
        let visible = format!("{} {}", copy.title, copy.message);
        for forbidden in ["ceremony-secret", "device-secret", "/Users/alice", "123456"] {
            assert!(!visible.contains(forbidden), "{visible}");
        }
    }

    #[test]
    fn mismatch_uses_the_same_distinct_terminal_copy_as_windows() {
        let mut mismatch = progress(PairingState::Failed);
        mismatch.error_code = Some(ErrorCode::AuthFailed);
        assert_eq!(
            progress_copy(&mismatch).message,
            "The security codes did not match. No device was paired."
        );
    }

    #[test]
    fn only_six_ascii_digits_can_reach_the_confirmation_view() {
        let mut value = progress(PairingState::AwaitingConfirmation);
        assert_eq!(sas_digits(&value), Some("123456"));
        for invalid in ["12345", "1234567", "12 456", "１２３４５６"] {
            value.sas = Some(invalid.into());
            assert!(sas_digits(&value).is_none());
        }
    }
}
