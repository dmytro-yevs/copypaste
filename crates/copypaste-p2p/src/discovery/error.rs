//! Discovery's error set.

/// Discovery never surfaces a filesystem path or any secret in these messages;
/// the daemon socket path in particular would disclose the local username.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// The device name is empty, or is nothing but characters we must strip.
    #[error("device name is empty or unusable")]
    InvalidDeviceName,

    /// A pairing id was not an identifier we are willing to put on the wire.
    #[error("pairing id is not a valid identifier")]
    InvalidPairingId,

    /// The mDNS library refused a command. Its messages carry socket and
    /// interface details only — no paths, no key material.
    #[error("mdns is unavailable: {0}")]
    Mdns(String),
}

impl From<mdns_sd::Error> for DiscoveryError {
    fn from(e: mdns_sd::Error) -> Self {
        Self::Mdns(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_disclose_no_paths() {
        for e in [
            DiscoveryError::InvalidDeviceName,
            DiscoveryError::InvalidPairingId,
            DiscoveryError::Mdns("socket bind refused".to_string()),
        ] {
            let text = e.to_string();
            assert!(!text.contains('/'), "error discloses a path: {text}");
            assert!(!text.contains('\\'), "error discloses a path: {text}");
        }
    }
}
