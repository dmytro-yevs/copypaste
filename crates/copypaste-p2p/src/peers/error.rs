//! The paired-device store's error set.

/// Every failure this module can produce.
///
/// No variant can render a filesystem path (`CLAUDE.md` rule 4). The
/// `io::Error` is a `#[source]`, available to a log sink but never to this
/// type's own `Display`.
#[derive(Debug, thiserror::Error)]
pub enum PeerStoreError {
    #[error("the paired-devices file could not be read or written")]
    Io(#[source] std::io::Error),

    /// Not valid JSON, or tagged as this format but the wrong shape. Never
    /// repaired by overwriting: the file holds the only copy of every pairing,
    /// and discarding it silently costs the user a manual re-pair of every
    /// device.
    #[error("the paired-devices file is damaged")]
    Corrupt,

    /// A peer record failed validation.
    #[error("peer record is invalid: {0}")]
    Invalid(&'static str),

    /// This pairing was revoked, and a revocation is final. Saving it again
    /// would undo the one gesture that cuts off a lost device.
    #[error("that device was revoked and cannot be paired again")]
    Revoked,

    /// The list is at [`super::MAX_PAIRINGS`]. Refusing the new pairing is the
    /// decision: evicting to make room would cut off a device the user still
    /// owns, and only the user knows which one they are finished with.
    #[error("this device is already paired with as many devices as it can hold; unpair one first")]
    TooManyPairings,

    /// A thread panicked while holding the store's lock. Surfaced rather than
    /// swallowed: the map may have been observed mid-update, and its contents
    /// decide who is allowed to connect.
    #[error("the paired-devices store is no longer usable in this process")]
    Poisoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        let errors = [
            PeerStoreError::Io(std::io::Error::other("/home/someone/peers.json")),
            PeerStoreError::Corrupt,
            PeerStoreError::Invalid("pairing id is empty"),
            PeerStoreError::Revoked,
            PeerStoreError::TooManyPairings,
            PeerStoreError::Poisoned,
        ];
        for err in &errors {
            let text = err.to_string();
            assert!(!text.contains('/'), "path-like error text: {text}");
            assert!(!text.contains('\\'), "path-like error text: {text}");
            assert!(!text.contains("home"), "path-like error text: {text}");
        }
    }
}
