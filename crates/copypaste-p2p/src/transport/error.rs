//! The closed set of failures the transport can produce.

use super::session::MAX_MESSAGE_BYTES;

/// Every failure this module can produce.
///
/// No variant carries a `String`, a `PathBuf` or a foreign error's `Display`
/// output: `AGENTS.md` rule 4 forbids showing a user a filesystem path, and a
/// closed set of literals cannot leak one by accident. A cause worth keeping is
/// attached as a `#[source]`, which the daemon can log but which never appears
/// in this error's own `Display`.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The Noise handshake did not complete: wrong pairing token, a peer not
    /// speaking this protocol, a stall past
    /// [`HANDSHAKE_TIMEOUT`](super::HANDSHAKE_TIMEOUT), or a hang-up
    /// mid-handshake — **deliberately indistinguishable**, because saying
    /// which would tell an attacker probing the port whether a guessed token
    /// was structurally accepted (the oracle shape port manifest 02 I-15
    /// exists to prevent). Drop the connection.
    #[error("secure handshake failed")]
    Handshake,

    /// The underlying socket failed. The cause is a `#[source]`, never
    /// interpolated: this type must not be able to render a path, and an
    /// `io::Error` reaching here in a future refactor could hold one.
    #[error("network connection failed")]
    Io(#[source] std::io::Error),

    /// The message could not be serialised to, or deserialised from, JSON — a
    /// local encoding bug, or a peer sending a shape this build does not know.
    /// Distinct from [`TransportError::Malformed`] because the bytes did
    /// authenticate, so the caller may keep using the session.
    #[error("message payload could not be encoded or decoded")]
    Codec,

    /// A frame failed authentication, carried an unknown record marker, or the
    /// stream ended in the middle of a logical message. The session is poisoned
    /// once this is returned: an authentication failure desynchronises the
    /// Noise nonce sequence, and continuing would produce a cascade of failures
    /// that look like corruption. Drop the session and reconnect.
    #[error("received frame was malformed or failed authentication")]
    Malformed,

    /// A logical message exceeded [`MAX_MESSAGE_BYTES`], on send or on
    /// reassembly. Returned rather than truncating: silent truncation of
    /// clipboard content is data loss, which `AGENTS.md` rule 4 ranks as the
    /// worst outcome.
    #[error("message exceeds the {MAX_MESSAGE_BYTES}-byte limit")]
    TooLarge,

    /// A pairing code did not decode to a [`TOKEN_LEN`](super::TOKEN_LEN)-byte
    /// token: wrong length, a character outside the alphabet, or non-canonical
    /// trailing bits. Carries no detail about what the user typed — the input
    /// is a secret.
    #[error("pairing code is not valid")]
    InvalidCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        // `AGENTS.md` rule 4: no payload can hold a path, but the literals
        // themselves must stay clean too.
        let errors: Vec<TransportError> = vec![
            TransportError::Handshake,
            TransportError::Io(std::io::Error::other("socket")),
            TransportError::Codec,
            TransportError::Malformed,
            TransportError::TooLarge,
            TransportError::InvalidCode,
        ];
        for err in errors {
            let text = err.to_string();
            assert!(!text.contains('/'), "path-like error text: {text}");
            assert!(!text.contains('\\'), "path-like error text: {text}");
            assert!(!text.is_empty());
        }
        // The io cause is a source, not part of Display.
        let io = TransportError::Io(std::io::Error::other("/home/someone/socket"));
        assert!(!io.to_string().contains("/home"));
    }
}
