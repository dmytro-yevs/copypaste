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
    if state == PairingState::Failed {
        semantics.terminal = true;
        semantics.retry = error.is_some_and(ErrorCode::retryable);
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
    let semantic = |icon, tone, live, active, terminal, needs_devices, review_secure, retry| {
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
    };
    match message_id {
        PairingMessageId::Ready => semantic(
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
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            false,
            false,
            false,
            true,
        ),
        PairingMessageId::IncompatibleVersion => semantic(
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            false,
            false,
            false,
            true,
        ),
        PairingMessageId::Unreachable => semantic(
            PairingIcon::WifiOff,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            false,
            false,
            false,
            true,
        ),
        PairingMessageId::Busy => semantic(
            PairingIcon::Alert,
            PairingTone::Warning,
            PairingLive::Alert,
            false,
            false,
            false,
            false,
            true,
        ),
        PairingMessageId::Limit => semantic(
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
            PairingIcon::Alert,
            PairingTone::Danger,
            PairingLive::Alert,
            false,
            false,
            false,
            false,
            true,
        ),
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
            assert!(matches!(
                semantics.message_id,
                PairingMessageId::Ready
                    | PairingMessageId::WaitingForPeer
                    | PairingMessageId::SecuringConnection
                    | PairingMessageId::CompareCodes
                    | PairingMessageId::Paired
                    | PairingMessageId::Rejected
                    | PairingMessageId::Cancelled
                    | PairingMessageId::TimedOut
            ));
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
            assert!(matches!(
                semantics.message_id,
                PairingMessageId::CodeMismatch
                    | PairingMessageId::IncompatibleVersion
                    | PairingMessageId::Unreachable
                    | PairingMessageId::Busy
                    | PairingMessageId::Limit
                    | PairingMessageId::Failed
            ));
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
    fn busy_and_limit_have_distinct_remedies() {
        let busy = resolve_pairing_semantics(PairingState::Failed, Some(ErrorCode::RateLimited));
        let limit = resolve_pairing_semantics(PairingState::Failed, Some(ErrorCode::PairingLimit));
        assert_eq!(busy.message_id, PairingMessageId::Busy);
        assert!(!busy.retry);
        assert_eq!(limit.message_id, PairingMessageId::Limit);
        assert!(!limit.retry);
        assert_ne!(busy.message_id, limit.message_id);
    }

    #[test]
    fn failed_retry_is_exactly_the_typed_error_policy() {
        for code in [
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
            assert_eq!(
                resolve_pairing_semantics(PairingState::Failed, Some(code)).retry,
                code.retryable(),
                "{code:?}"
            );
            assert!(resolve_pairing_semantics(PairingState::Failed, Some(code)).terminal);
        }
        let none = resolve_pairing_semantics(PairingState::Failed, None);
        assert!(none.terminal);
        assert!(!none.retry);
    }
}
