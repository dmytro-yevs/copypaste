//! The closed set of failures the transport can produce.
//!
//! Kept in its own file because the *shape* of this enum is a security
//! property, not an implementation detail: see the type docs.

use super::session::MAX_MESSAGE_BYTES;

/// Every failure this module can produce.
///
/// No variant carries a `String`, a `PathBuf` or a foreign error's `Display`
/// output. That is deliberate on two counts: `CLAUDE.md` rule 4 forbids showing
/// a user a filesystem path (the daemon's socket path discloses the local
/// username), and a closed set of literal messages cannot leak one by accident.
/// Where a cause is worth keeping it is attached as a `#[source]`, which the
/// daemon can log but which never appears in this error's own `Display`.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The Noise handshake did not complete: wrong pairing token, a peer that
    /// is not speaking this protocol, a peer that stalled past
    /// [`HANDSHAKE_TIMEOUT`](super::HANDSHAKE_TIMEOUT), or a peer that hung up
    /// mid-handshake.
    ///
    /// These are **deliberately indistinguishable**. Reporting *which* one
    /// happened would tell an attacker probing the port whether a guessed token
    /// was structurally accepted, which is the decryption-oracle shape port
    /// manifest 02 I-15 exists to prevent. The only correct handling is to drop
    /// the connection.
    #[error("secure handshake failed")]
    Handshake,

    /// The underlying socket failed.
    ///
    /// The cause is a `#[source]`, not interpolated into the message: this type
    /// must never be able to render a path, and while this module only ever
    /// touches sockets, an `io::Error` from elsewhere in a future refactor
    /// could.
    #[error("network connection failed")]
    Io(#[source] std::io::Error),

    /// The message could not be serialised to, or deserialised from, JSON.
    ///
    /// A local encoding bug or a peer sending a shape this build does not know.
    /// Distinct from [`TransportError::Malformed`] because the channel is still
    /// intact — the bytes authenticated, they just did not parse — so the
    /// caller may keep using the session.
    #[error("message payload could not be encoded or decoded")]
    Codec,

    /// A frame failed authentication, carried an unknown record marker, or the
    /// stream ended in the middle of a logical message.
    ///
    /// The session is poisoned once this is returned: an authentication failure
    /// desynchronises the Noise nonce sequence, and continuing would produce a
    /// cascade of failures that look like corruption. Drop the session and
    /// reconnect.
    #[error("received frame was malformed or failed authentication")]
    Malformed,

    /// A logical message exceeded [`MAX_MESSAGE_BYTES`], on send or on
    /// reassembly.
    ///
    /// Returned rather than truncating. Silent truncation of clipboard content
    /// is data loss, which `CLAUDE.md` rule 4 ranks as the worst outcome.
    #[error("message exceeds the {MAX_MESSAGE_BYTES}-byte limit")]
    TooLarge,

    /// A pairing code did not decode to a [`TOKEN_LEN`](super::TOKEN_LEN)-byte
    /// token.
    ///
    /// Wrong length, a character outside the alphabet, or non-canonical
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
        // `CLAUDE.md` rule 4. The variants have no payload that could hold one,
        // but the literals themselves must also stay clean.
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
