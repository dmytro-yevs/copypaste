//! The paired-device store's error set.

/// Every failure this module can produce.
///
/// No variant can render a filesystem path (`CLAUDE.md` rule 4 — the daemon's
/// data directory discloses the local username). The `io::Error` is a
/// `#[source]`, so it is available to a log sink but never to this type's own
/// `Display`; `std`'s file errors do not embed the path either, but relying on
/// that would be relying on an implementation detail.
#[derive(Debug, thiserror::Error)]
pub enum PeerStoreError {
    /// The store could not be read, written or replaced.
    #[error("the paired-devices file could not be read or written")]
    Io(#[source] std::io::Error),

    /// The file is not valid JSON, or is tagged as this format but does not
    /// match its shape.
    ///
    /// Never repaired by overwriting. The file holds the only copy of every
    /// pairing; discarding it silently would cost the user a manual re-pair of
    /// every device.
    #[error("the paired-devices file is damaged")]
    Corrupt,

    /// The file was written by a different version of CopyPaste.
    ///
    /// v2 shares no formats with v0.4.x. The daemon should say exactly that:
    /// the old pairings are still on disk and still readable by the old build,
    /// and v2 needs the devices paired again.
    #[error("the paired-devices file was written by a different version of CopyPaste")]
    Legacy,

    /// A peer record failed validation.
    #[error("peer record is invalid: {0}")]
    Invalid(&'static str),

    /// A thread panicked while holding the store's lock.
    ///
    /// Surfaced rather than swallowed: the in-memory map may have been observed
    /// mid-update, and this store's contents decide who is allowed to connect.
    #[error("the paired-devices store is no longer usable in this process")]
    Poisoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        let errors = [
            PeerStoreError::Io(std::io::Error::other("/home/someone/peers-v2.json")),
            PeerStoreError::Corrupt,
            PeerStoreError::Legacy,
            PeerStoreError::Invalid("pairing id is empty"),
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
