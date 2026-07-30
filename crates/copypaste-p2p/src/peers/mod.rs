//! The list of devices this one is paired with, and their pre-shared keys.
//!
//! A file-backed, thread-safe map from [`crate::PairingToken::pairing_id`] to
//! [`Peer`]. [`PeerStore::psks`] feeds [`crate::Session::accept_any`], so this
//! decides who may connect; it also remembers where a peer was last seen. It
//! holds the PSKs, so the file is itself a key store — see [`file`].
//!
//! Two answers it gives that are not "is this device paired?": whether an
//! unredeemed pairing code has aged out ([`PAIRING_CODE_TTL`], see [`store`])
//! and whether a pairing was revoked ([`PeerStore::revoke`]). Both are refusals,
//! and both are enforced on every read path rather than at one gate.
//!
//! # No backward compatibility
//!
//! `CLAUDE.md` rule 3: v2 must not open, or appear to open, anything v1 wrote.
//! Two guards, both cheap:
//!
//! * [`DEFAULT_FILE_NAME`] is `peers-v2.json`, so v1's `peers.json` is never
//!   touched and a user who downgrades finds it intact.
//! * The envelope carries a `format` tag. A file that is valid JSON but not
//!   tagged [`FORMAT_TAG`] is [`PeerStoreError::Legacy`], a plain "this was
//!   written by a different version" — not a decryption error, and not silently
//!   overwritten.

mod error;
mod file;
mod peer;
mod store;

#[cfg(test)]
mod testutil;

use std::time::Duration;

pub use error::PeerStoreError;
pub use peer::Peer;
pub use store::PeerStore;

/// How long a freshly minted pairing code stays redeemable.
///
/// v1's `QR_PAIRING_TTL_SECS` was 120 s, for a QR code the screen regenerated on
/// a timer. This code is 52 characters read off one device and typed into
/// another, sometimes in a different room, and a deadline that expires mid-typing
/// teaches people to distrust the feature rather than to hurry. Five minutes is
/// the shortest window that does not do that.
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(300);

/// A pairing that was cut off, and when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevokedDevice {
    pub pairing_id: String,
    /// Unix milliseconds.
    pub revoked_at_ms: i64,
}

/// Filename the daemon should use.
///
/// Deliberately not `peers.json`: that is v1's name, and v2 must never open a
/// v1 file (`CLAUDE.md` rule 3).
pub const DEFAULT_FILE_NAME: &str = "peers-v2.json";

/// Envelope tag. Its only job is to make "written by another version" a
/// distinguishable answer from "corrupt".
pub const FORMAT_TAG: &str = "copypaste-peers-v2";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_name_is_not_v1s() {
        // `CLAUDE.md` rule 3: a v1 install's file must never be opened, or
        // appear to be opened, by v2.
        assert_eq!(DEFAULT_FILE_NAME, "peers-v2.json");
        assert_ne!(DEFAULT_FILE_NAME, "peers.json");
    }
}
