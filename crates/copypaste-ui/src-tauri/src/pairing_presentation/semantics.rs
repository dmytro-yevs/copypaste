use copypaste_ipc::{ErrorCode, PairingState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingMessageId {
    Ready,
    WaitingForPeer,
    SecuringConnection,
    CompareCodes,
    Paired,
    Rejected,
    Cancelled,
    TimedOut,
    CodeMismatch,
    IncompatibleVersion,
    Unreachable,
    Busy,
    Limit,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "camelCase")]
pub enum PairingIcon {
    ShieldCheck,
    Spinner,
    CheckCircle,
    Close,
    Alert,
    WifiOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingLive {
    Status,
    Alert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PairingSemantics {
    pub message_id: PairingMessageId,
    pub icon: PairingIcon,
    pub tone: PairingTone,
    pub live: PairingLive,
    pub active: bool,
    pub terminal: bool,
    pub needs_devices: bool,
    pub review_secure: bool,
    pub retry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeCopy {
    pub title: &'static str,
    pub detail: &'static str,
}

pub fn resolve_pairing_semantics(
    state: PairingState,
    error: Option<ErrorCode>,
) -> PairingSemantics {
    let message_id = match state {
        PairingState::Idle => PairingMessageId::Ready,
        PairingState::WaitingForPeer => PairingMessageId::WaitingForPeer,
        PairingState::Handshaking => PairingMessageId::SecuringConnection,
        PairingState::AwaitingConfirmation => PairingMessageId::CompareCodes,
        PairingState::Confirmed => PairingMessageId::Paired,
        PairingState::Rejected => PairingMessageId::Rejected,
        PairingState::Cancelled => PairingMessageId::Cancelled,
        PairingState::TimedOut => PairingMessageId::TimedOut,
        PairingState::Failed => failed_message(error),
    };
    let mut semantics = semantics_for(message_id);
    if state == PairingState::Failed && error == Some(ErrorCode::ContentTooLarge) {
        semantics.retry = false;
    }
    semantics
}

fn failed_message(error: Option<ErrorCode>) -> PairingMessageId {
    match error {
        Some(ErrorCode::AuthFailed | ErrorCode::PairingCode) => PairingMessageId::CodeMismatch,
        Some(ErrorCode::ProtocolMismatch | ErrorCode::PeerVersion) => {
            PairingMessageId::IncompatibleVersion
        }
        Some(ErrorCode::PairingAddress | ErrorCode::PeerUnreachable) => {
            PairingMessageId::Unreachable
        }
        Some(ErrorCode::RateLimited) => PairingMessageId::Busy,
        Some(ErrorCode::PairingLimit) => PairingMessageId::Limit,
        Some(
            ErrorCode::NotFound
            | ErrorCode::InvalidRequest
            | ErrorCode::NotReady
            | ErrorCode::KeyLocked
            | ErrorCode::KeyUnusable
            | ErrorCode::UnsupportedContent
            | ErrorCode::ContentTooLarge
            | ErrorCode::PeerFailed
            | ErrorCode::PeerNotFound
            | ErrorCode::Internal,
        ) => PairingMessageId::Failed,
        None => PairingMessageId::Failed,
    }
}

fn semantics_for(message_id: PairingMessageId) -> PairingSemantics {
    match message_id {
        PairingMessageId::Ready => semantic(
            message_id,
            PairingIcon::ShieldCheck,
            PairingTone::Neutral,
            PairingLive::Status,
            false,
            false,
            false,
            false,
            false,
        ),
        PairingMessageId::WaitingForPeer => semantic(
            message_id,
            PairingIcon::Spinner,
            PairingTone::Info,
            PairingLive::Status,
            true,
            false,
            false,
            false,
            false,
        ),
        PairingMessageId::SecuringConnection => semantic(
            message_id,
            PairingIcon::Spinner,
            PairingTone::Info,
            PairingLive::Status,
            true,
            false,
            true,
            false,
            false,
        ),
        PairingMessageId::CompareCodes => semantic(
            message_id,
            PairingIcon::ShieldCheck,
            PairingTone::Warning,
            PairingLive::Status,
            true,
            false,
            true,
            true,
            false,
        ),
        PairingMessageId::Paired => semantic(
            message_id,
            PairingIcon::CheckCircle,
            PairingTone::Success,
            PairingLive::Status,
            false,
            true,
            false,
            false,
            false,
        ),
        PairingMessageId::Rejected => semantic(
            message_id,
            PairingIcon::Close,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::Cancelled => semantic(
            message_id,
            PairingIcon::Close,
            PairingTone::Neutral,
            PairingLive::Status,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::TimedOut => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::CodeMismatch => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::IncompatibleVersion => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::Unreachable => semantic(
            message_id,
            PairingIcon::WifiOff,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::Busy => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
        PairingMessageId::Limit => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            false,
        ),
        PairingMessageId::Failed => semantic(
            message_id,
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            true,
            false,
            false,
            true,
        ),
    }
}

const fn semantic(
    message_id: PairingMessageId,
    icon: PairingIcon,
    tone: PairingTone,
    live: PairingLive,
    active: bool,
    terminal: bool,
    needs_devices: bool,
    review_secure: bool,
    retry: bool,
) -> PairingSemantics {
    PairingSemantics {
        message_id,
        icon,
        tone,
        live,
        active,
        terminal,
        needs_devices,
        review_secure,
        retry,
    }
}

pub(super) fn native_copy(message_id: PairingMessageId) -> NativeCopy {
    match message_id {
        PairingMessageId::Ready => NativeCopy {
            title: "Pair a device",
            detail: "No device pairing is in progress.",
        },
        PairingMessageId::WaitingForPeer => NativeCopy {
            title: "Waiting for a device",
            detail: "Waiting for the other device to join.",
        },
        PairingMessageId::SecuringConnection => NativeCopy {
            title: "Securing the connection",
            detail: "Keep both devices nearby while CopyPaste establishes a secure connection.",
        },
        PairingMessageId::CompareCodes => NativeCopy {
            title: "Compare security codes",
            detail: "Confirm the code in the native security prompt.",
        },
        PairingMessageId::Paired => NativeCopy {
            title: "Paired",
            detail: "The device was paired successfully.",
        },
        PairingMessageId::Rejected => NativeCopy {
            title: "Pairing rejected",
            detail: "The security codes did not match, so no pairing was saved.",
        },
        PairingMessageId::Cancelled => NativeCopy {
            title: "Pairing cancelled",
            detail: "No pairing was saved.",
        },
        PairingMessageId::TimedOut => NativeCopy {
            title: "Pairing timed out",
            detail: "Check that both devices are on the same network and try again.",
        },
        PairingMessageId::CodeMismatch => NativeCopy {
            title: "Pairing didn't match",
            detail: "The security codes did not match. No device was paired.",
        },
        PairingMessageId::IncompatibleVersion => NativeCopy {
            title: "Pairing couldn't finish",
            detail: "The other device uses an incompatible version. No device was paired.",
        },
        PairingMessageId::Unreachable => NativeCopy {
            title: "Pairing couldn't finish",
            detail:
                "Could not reach the other device. Check that both devices are on the same network.",
        },
        PairingMessageId::Busy => NativeCopy {
            title: "Pairing is busy",
            detail: "Another pairing is already in progress.",
        },
        PairingMessageId::Limit => NativeCopy {
            title: "Pairing limit reached",
            detail: "Remove or revoke a paired device before trying again.",
        },
        PairingMessageId::Failed => NativeCopy {
            title: "Pairing failed",
            detail: "Pairing failed. No device was paired.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_and_failure_code_has_one_safe_semantic() {
        let states = [
            PairingState::Idle,
            PairingState::WaitingForPeer,
            PairingState::Handshaking,
            PairingState::AwaitingConfirmation,
            PairingState::Confirmed,
            PairingState::Rejected,
            PairingState::Cancelled,
            PairingState::TimedOut,
        ];
        for state in states {
            let semantics = resolve_pairing_semantics(state, None);
            assert_ne!(native_copy(semantics.message_id).title, "");
        }
        for error in [
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
            let semantics = resolve_pairing_semantics(PairingState::Failed, Some(error));
            assert_ne!(native_copy(semantics.message_id).detail, "");
        }
    }

    #[test]
    fn oversized_failure_is_terminal_and_nonretryable() {
        let semantics =
            resolve_pairing_semantics(PairingState::Failed, Some(ErrorCode::ContentTooLarge));
        assert_eq!(semantics.message_id, PairingMessageId::Failed);
        assert!(semantics.terminal);
        assert!(!semantics.retry);
    }

    #[test]
    fn terminal_outcomes_remain_distinct() {
        let ids = [
            PairingState::TimedOut,
            PairingState::Cancelled,
            PairingState::Rejected,
            PairingState::Failed,
        ]
        .map(|state| resolve_pairing_semantics(state, None).message_id);
        assert_eq!(
            ids,
            [
                PairingMessageId::TimedOut,
                PairingMessageId::Cancelled,
                PairingMessageId::Rejected,
                PairingMessageId::Failed
            ]
        );
    }

    #[test]
    fn native_copy_is_closed_and_secret_free() {
        for message_id in [
            PairingMessageId::Ready,
            PairingMessageId::WaitingForPeer,
            PairingMessageId::SecuringConnection,
            PairingMessageId::CompareCodes,
            PairingMessageId::Paired,
            PairingMessageId::Rejected,
            PairingMessageId::Cancelled,
            PairingMessageId::TimedOut,
            PairingMessageId::CodeMismatch,
            PairingMessageId::IncompatibleVersion,
            PairingMessageId::Unreachable,
            PairingMessageId::Busy,
            PairingMessageId::Limit,
            PairingMessageId::Failed,
        ] {
            let copy = native_copy(message_id);
            let visible = format!("{} {}", copy.title, copy.detail);
            for forbidden in ["/", "\\\\", "123456", "secret", "token"] {
                assert!(
                    !visible.to_ascii_lowercase().contains(forbidden),
                    "{visible}"
                );
            }
        }
    }

    #[test]
    fn busy_and_limit_have_distinct_remedies() {
        let busy = resolve_pairing_semantics(PairingState::Failed, Some(ErrorCode::RateLimited));
        let limit = resolve_pairing_semantics(PairingState::Failed, Some(ErrorCode::PairingLimit));
        assert_eq!(busy.message_id, PairingMessageId::Busy);
        assert!(busy.retry);
        assert_eq!(limit.message_id, PairingMessageId::Limit);
        assert!(!limit.retry);
        assert_ne!(
            native_copy(busy.message_id).detail,
            native_copy(limit.message_id).detail
        );
    }
}
