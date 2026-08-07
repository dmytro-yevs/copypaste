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
mod cursor;
mod error;
mod expiry;
mod file;
mod peer;
mod store;

#[cfg(test)]
mod testutil;

use std::time::Duration;

pub use cursor::{CursorStore, DEFAULT_CURSOR_FILE_NAME};
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

/// The most pairings one device will hold.
///
/// **A refusal, never an eviction.** Making room by dropping the oldest pairing
/// would cut off a device the user still owns, and nothing on this end can put
/// it back — the other half's key is gone and the two devices have to be paired
/// again by hand. That is the data-loss-shaped outcome `CLAUDE.md` rule 4 rules
/// out, so a new pairing at the cap fails with
/// [`PeerStoreError::TooManyPairings`] and the user unpairs something. An
/// *existing* pairing is never refused: a rename, a new address and the
/// end-of-session timestamp all still write, or every session would fail once
/// the list filled up.
///
/// Sixteen because that is what one mDNS record advertises
/// (`discovery::record::MAX_ADVERTISED_PAIRING_IDS`, pinned equal by a test):
/// past it a pairing is reachable only from an explicit address, so a larger
/// cap would hand out devices discovery cannot find. It also bounds the work an
/// unauthenticated dialler can cause, because
/// [`crate::Session::accept_any`] tries one pre-shared key per stored pairing
/// per inbound connection (security review F-13).
pub const MAX_PAIRINGS: usize = 16;

/// A pairing that was cut off, and when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevokedDevice {
    pub pairing_id: String,
    /// Unix milliseconds.
    pub revoked_at_ms: i64,
}

/// Filename the daemon should use.
///
pub const DEFAULT_FILE_NAME: &str = "peers.json";

#[cfg(test)]
mod tests {
    use super::*;

    /// The cap's stated reason is that every pairing this device holds fits in
    /// one advertisement. A build failure if that stops being true, rather than
    /// a comment that quietly goes stale.
    const _: () = assert!(MAX_PAIRINGS <= crate::discovery::MAX_ADVERTISED_PAIRING_IDS);
}
