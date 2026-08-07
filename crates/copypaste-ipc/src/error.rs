//! Stable error codes are the UI contract; clients never branch on error text.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No such item; paired devices use `PeerNotFound` (review finding 4).
    NotFound,
    InvalidRequest,
    ProtocolMismatch,
    NotReady,
    AuthFailed,
    /// The keystore is temporarily inaccessible.
    KeyLocked,
    /// The stored key cannot decrypt this history; retrying cannot repair it.
    KeyUnusable,

    PairingCode,
    PairingAddress,
    /// The paired device may reappear, so this failure is retryable.
    PeerUnreachable,
    PairingLimit,
    PeerFailed,
    PeerVersion,
    /// No such paired device; items use `NotFound`.
    PeerNotFound,

    Internal,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::InvalidRequest => "invalid_request",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::NotReady => "not_ready",
            Self::AuthFailed => "auth_failed",
            Self::KeyLocked => "key_locked",
            Self::KeyUnusable => "key_unusable",
            Self::PairingCode => "pairing_code",
            Self::PairingAddress => "pairing_address",
            Self::PeerUnreachable => "peer_unreachable",
            Self::PairingLimit => "pairing_limit",
            Self::PeerFailed => "peer_failed",
            Self::PeerVersion => "peer_version",
            Self::PeerNotFound => "peer_not_found",
            Self::Internal => "internal",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "not_found" => Some(Self::NotFound),
            "invalid_request" => Some(Self::InvalidRequest),
            "protocol_mismatch" => Some(Self::ProtocolMismatch),
            "not_ready" => Some(Self::NotReady),
            "auth_failed" => Some(Self::AuthFailed),
            "key_locked" => Some(Self::KeyLocked),
            "key_unusable" => Some(Self::KeyUnusable),
            "pairing_code" => Some(Self::PairingCode),
            "pairing_address" => Some(Self::PairingAddress),
            "peer_unreachable" => Some(Self::PeerUnreachable),
            "pairing_limit" => Some(Self::PairingLimit),
            "peer_failed" => Some(Self::PeerFailed),
            "peer_version" => Some(Self::PeerVersion),
            "peer_not_found" => Some(Self::PeerNotFound),
            "internal" => Some(Self::Internal),
            _ => None,
        }
    }

    /// Shared retry policy; exhaustive matching forces every new code to choose.
    #[must_use]
    pub fn retryable(self) -> bool {
        match self {
            Self::NotReady
            | Self::KeyLocked
            | Self::PeerUnreachable
            | Self::PeerFailed
            | Self::Internal => true,
            Self::NotFound
            | Self::InvalidRequest
            | Self::ProtocolMismatch
            | Self::AuthFailed
            | Self::KeyUnusable
            | Self::PairingCode
            | Self::PairingAddress
            | Self::PairingLimit
            | Self::PeerVersion
            | Self::PeerNotFound => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_keep_their_wire_spelling() {
        for (code, wire) in [
            (ErrorCode::NotFound, "\"not_found\""),
            (ErrorCode::InvalidRequest, "\"invalid_request\""),
            (ErrorCode::ProtocolMismatch, "\"protocol_mismatch\""),
            (ErrorCode::NotReady, "\"not_ready\""),
            (ErrorCode::AuthFailed, "\"auth_failed\""),
            (ErrorCode::KeyLocked, "\"key_locked\""),
            (ErrorCode::KeyUnusable, "\"key_unusable\""),
            (ErrorCode::PairingCode, "\"pairing_code\""),
            (ErrorCode::PairingAddress, "\"pairing_address\""),
            (ErrorCode::PeerUnreachable, "\"peer_unreachable\""),
            (ErrorCode::PairingLimit, "\"pairing_limit\""),
            (ErrorCode::PeerFailed, "\"peer_failed\""),
            (ErrorCode::PeerVersion, "\"peer_version\""),
            (ErrorCode::PeerNotFound, "\"peer_not_found\""),
            (ErrorCode::Internal, "\"internal\""),
        ] {
            assert_eq!(serde_json::to_string(&code).unwrap(), wire);
            assert_eq!(code.as_str(), wire.trim_matches('"'));
            assert_eq!(ErrorCode::parse(wire.trim_matches('"')), Some(code));
        }
    }

    #[test]
    fn unknown_error_codes_are_not_known_codes() {
        assert_eq!(ErrorCode::parse("future_daemon_state"), None);
        assert_eq!(ErrorCode::parse("NOT_FOUND"), None);
    }

    #[test]
    fn a_locked_key_store_is_retryable_and_an_unusable_key_is_not() {
        assert!(ErrorCode::KeyLocked.retryable());
        assert!(!ErrorCode::KeyUnusable.retryable());
    }

    #[test]
    fn only_the_pairing_failures_a_repeat_could_answer_are_retryable() {
        assert!(ErrorCode::PeerUnreachable.retryable());
        assert!(ErrorCode::PeerFailed.retryable());
        for code in [
            ErrorCode::PairingCode,
            ErrorCode::PairingAddress,
            ErrorCode::PairingLimit,
            ErrorCode::PeerVersion,
            ErrorCode::PeerNotFound,
        ] {
            assert!(!code.retryable(), "{code:?}");
        }
    }

    #[test]
    fn a_missing_device_is_not_a_missing_item() {
        assert_ne!(ErrorCode::PeerNotFound, ErrorCode::NotFound);
    }
}
