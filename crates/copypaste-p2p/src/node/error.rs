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
    #[error("no such paired device")]
    NoPeer,
    #[error("this peer has never been reached and is not visible on the network; sync from the other device, or re-pair with an address")]
    NoAddress,
    #[error("the sync session with the other device failed")]
    Session,
    #[error("the other device stopped responding")]
    Timeout,
    #[error("the paired-device list could not be updated")]
    PeerStore,
}

impl NodeError {
    /// Whether the caller supplied something invalid, as opposed to something
    /// having gone wrong here. Callers map this onto their own error taxonomy.
    #[must_use]
    pub fn is_client_error(self) -> bool {
        matches!(
            self,
            NodeError::BadCode | NodeError::BadAddress | NodeError::Handshake
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
        NodeError::NoPeer,
        NodeError::NoAddress,
        NodeError::Session,
        NodeError::Timeout,
        NodeError::PeerStore,
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
