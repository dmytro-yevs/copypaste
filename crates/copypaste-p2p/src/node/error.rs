//! What a node operation can fail with.
//!
//! Every variant's `Display` is a fixed sentence with no interpolation. These
//! reach a peer's log and a user's screen, so none of them may carry a
//! filesystem path (CLAUDE.md rule 4) and none may describe *how* a pairing
//! code was wrong — the input is a secret, and saying which part failed is a
//! hint about what a valid one looks like.

/// A pairing or sync operation that did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NodeError {
    #[error("that pairing code is not valid")]
    BadCode,
    #[error("that address could not be resolved; expected host:port")]
    BadAddress,
    #[error("the other device did not accept this pairing code")]
    Handshake,
    #[error("a device cannot pair with itself")]
    SelfPairing,
    #[error("no such paired device")]
    NoPeer,
    #[error("this peer has never been reached and is not visible on the network; sync from the other device, or re-pair with an address")]
    NoAddress,
    #[error("the sync session with the other device failed")]
    Session,
    #[error("the two devices are running versions of CopyPaste that cannot sync with each other; update both and try again")]
    PeerVersion,
    #[error("the other device stopped responding")]
    Timeout,
    #[error("the paired-device list could not be updated")]
    PeerStore,
    /// [`crate::peers::MAX_PAIRINGS`] reached. The user has to decide which
    /// device to let go; nothing here will decide for them.
    #[error("this device is already paired with as many devices as it can hold; unpair one first")]
    TooManyPairings,
}

impl NodeError {
    /// Whether the caller can act on it — bad input, or a precondition they
    /// control — as opposed to something having gone wrong here. Callers map
    /// this onto their own error taxonomy.
    ///
    /// `TooManyPairings` is here rather than with the internal failures on
    /// purpose: it is a refusal with a remedy the user can carry out, and
    /// reporting it as an internal error would hide the remedy.
    #[must_use]
    pub fn is_client_error(self) -> bool {
        matches!(
            self,
            NodeError::BadCode
                | NodeError::BadAddress
                | NodeError::Handshake
                | NodeError::SelfPairing
                | NodeError::TooManyPairings
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every client-visible string this module can produce, asserted pathless.
    const ALL: &[NodeError] = &[
        NodeError::BadCode,
        NodeError::BadAddress,
        NodeError::Handshake,
        NodeError::SelfPairing,
        NodeError::NoPeer,
        NodeError::NoAddress,
        NodeError::Session,
        NodeError::PeerVersion,
        NodeError::Timeout,
        NodeError::PeerStore,
        NodeError::TooManyPairings,
    ];

    #[test]
    fn client_visible_messages_disclose_no_path() {
        for err in ALL {
            let message = err.to_string();
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }

    #[test]
    fn only_the_callers_own_mistakes_are_client_errors() {
        assert!(NodeError::BadCode.is_client_error());
        assert!(!NodeError::PeerStore.is_client_error());
        assert!(!NodeError::NoPeer.is_client_error());
    }
}
