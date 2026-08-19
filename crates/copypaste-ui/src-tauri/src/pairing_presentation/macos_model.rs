use copypaste_ipc::{ErrorCode, PairingProgressData, PairingState};

pub(super) struct ProgressCopy {
    pub title: String,
    pub message: &'static str,
}

pub(super) fn progress_copy(progress: &PairingProgressData) -> ProgressCopy {
    let peer = progress
        .peer_name
        .as_deref()
        .map(sanitized_peer_name)
        .filter(|name| !name.is_empty());
    let title = match (progress.state, peer) {
        (PairingState::AwaitingConfirmation, Some(peer)) => {
            format!("Confirm pairing with {peer} (unverified)")
        }
        (PairingState::Confirmed, Some(peer)) => format!("Paired with {peer}"),
        _ => "Pair a device".to_owned(),
    };
    let message = match progress.state {
        PairingState::Idle => "Pairing ended — check the other device.",
        PairingState::WaitingForPeer => "Waiting for the other device…",
        PairingState::Handshaking => "Creating a secure connection…",
        PairingState::AwaitingConfirmation => {
            "The secure connection is ready. Compare the security code on both devices."
        }
        PairingState::Confirmed => "Paired ✓",
        PairingState::Rejected => "Pairing was rejected. No device was paired.",
        PairingState::Cancelled => "Pairing was cancelled. No device was paired.",
        PairingState::TimedOut => "Pairing timed out. No device was paired.",
        PairingState::Failed => failure_copy(progress.error_code),
    };
    ProgressCopy { title, message }
}

fn failure_copy(error: Option<ErrorCode>) -> &'static str {
    match error {
        Some(ErrorCode::AuthFailed | ErrorCode::PairingCode) => {
            "The security codes did not match. No device was paired."
        }
        Some(ErrorCode::ProtocolMismatch | ErrorCode::PeerVersion) => {
            "The other device uses an incompatible version. No device was paired."
        }
        Some(ErrorCode::PairingAddress | ErrorCode::PeerUnreachable) => {
            "Could not reach the other device. Check that both devices are on the same network."
        }
        Some(ErrorCode::RateLimited) => "Another pairing is already in progress.",
        _ => "Pairing failed. No device was paired.",
    }
}

fn sanitized_peer_name(name: &str) -> String {
    let name = name
        .chars()
        .filter(|character| !character.is_control())
        .take(64)
        .collect::<String>()
        .trim()
        .to_owned();
    if name.contains(['/', '\\'])
        || name
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
    {
        "Other device".to_owned()
    } else {
        name
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
    use copypaste_ipc::PairingRole;

    fn progress(state: PairingState) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("ceremony-secret".into()),
            role: Some(PairingRole::Initiator),
            state,
            sas: Some("123456".into()),
            peer_device_id: Some("device-secret".into()),
            peer_name: Some("/Users/alice/Phone".into()),
            peer_addr: Some("/Users/alice/private.sock".into()),
            known_device: None,
            error_code: None,
        }
    }

    #[test]
    fn every_progress_message_is_fixed_copy_without_secrets_or_paths() {
        for state in [
            PairingState::Idle,
            PairingState::WaitingForPeer,
            PairingState::Handshaking,
            PairingState::AwaitingConfirmation,
            PairingState::Confirmed,
            PairingState::Rejected,
            PairingState::Cancelled,
            PairingState::TimedOut,
            PairingState::Failed,
        ] {
            let copy = progress_copy(&progress(state));
            let visible = format!("{} {}", copy.title, copy.message);
            for forbidden in [
                "ceremony-secret",
                "device-secret",
                "/Users/alice",
                "192.0.2.1",
                "123456",
            ] {
                assert!(
                    !visible.contains(forbidden),
                    "leaked {forbidden}: {visible}"
                );
            }
        }
    }

    #[test]
    fn timeout_and_mismatch_have_explicit_terminal_copy() {
        assert_eq!(
            progress_copy(&progress(PairingState::TimedOut)).message,
            "Pairing timed out. No device was paired."
        );
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

    #[test]
    fn native_secret_surfaces_are_protected_and_use_the_shared_codec() {
        let source = include_str!("macos.rs");
        for required in [
            "encode_native_invite",
            "decode_native_invite",
            "QrCode::new",
            "NSSecureTextField",
            "setAccessibilityProtectedContent(true)",
            "NSWindowSharingType::NSWindowSharingNone",
            "Security code: {spoken}",
            "setSelectable(false)",
            "for (index, digit) in sas.chars().enumerate()",
            "NativeAbort",
            "(self.abort)()",
            "PairingPresentationState::Unavailable",
        ] {
            assert!(
                source.contains(required),
                "missing native guard: {required}"
            );
        }
        for forbidden in ["copypaste://", "url::", "tracing::", "emit("] {
            assert!(
                !source.contains(forbidden),
                "forbidden native path: {forbidden}"
            );
        }
    }

    #[test]
    fn progress_close_aborts_and_awaiting_skips_blocking_alert() {
        let source = include_str!("macos.rs");
        let present = source
            .split("fn present_progress")
            .nth(1)
            .and_then(|body| body.split("fn confirm").next())
            .expect("present_progress");
        assert!(present.contains("AwaitingConfirmation"));
        assert!(present.contains("PairingPresentationState::Presented"));
        assert!(present.contains("(self.abort)()"));
        assert!(present.contains("PairingPresentationState::Unavailable"));
        assert!(
            present.find("AwaitingConfirmation").unwrap() < present.find("(self.abort)()").unwrap(),
            "awaiting confirmation must return before aborting Close"
        );
    }
}
