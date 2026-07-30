//! What a session can fail with.

use crate::protocol::ProtocolError;

/// Session failures.
///
/// No variant carries a path or any item content: these are logged and shown to
/// users (CLAUDE.md rule 4). Implementations of [`SyncSource`](super::SyncSource)
/// must hold to the same rule when they build [`SyncError::Source`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// The channel underneath failed — closed, timed out, or refused a frame.
    #[error("sync channel error: {0}")]
    Channel(String),

    #[error("peer sent a {got} message where a {expected} was expected")]
    Unexpected {
        expected: &'static str,
        got: &'static str,
    },

    /// The local store failed. The message must already be safe to display.
    #[error("sync source error: {0}")]
    Source(String),

    /// The peer claims our own device id. Either the pairing is pointed at this
    /// device, or something is reflecting our traffic; neither is a session.
    #[error("peer reports this device's own id")]
    SelfSync,

    /// The peer kept sending after it had answered everything it was asked for.
    /// Bounding this is what stops a peer from holding the session open forever.
    #[error("peer sent more than it was asked for")]
    PeerOverran,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_disclose_nothing_sensitive() {
        for err in [
            SyncError::SelfSync,
            SyncError::PeerOverran,
            SyncError::Unexpected {
                expected: "hello",
                got: "done",
            },
            SyncError::Channel("peer went away".into()),
        ] {
            let text = err.to_string();
            assert!(!text.contains('/'), "error names a path: {text}");
        }
    }
}
