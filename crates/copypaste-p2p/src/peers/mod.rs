//! The list of devices this one is paired with, and their pre-shared keys.
//!
//! A file-backed, thread-safe map from [`crate::PairingToken::pairing_id`] to
//! [`Peer`]. [`PeerStore::psks`] feeds [`crate::Session::accept_any`], so this
//! decides who may connect; it also remembers where a peer was last seen. It
//! holds the PSKs, so the file is itself a key store — see [`file`].
//!
//! A revocation is kept after the peer record is gone, so a credential someone
//! retained cannot recreate a device the user cut off ([`PeerStore::revoke`]).
//!
mod cursor;
mod error;
mod file;
mod peer;
mod revocation;
mod store;
mod tentative;

#[cfg(test)]
mod testutil;

pub use cursor::{CursorStore, DEFAULT_CURSOR_FILE_NAME};
pub use error::PeerStoreError;
pub use peer::Peer;
pub use store::PeerStore;
pub use tentative::{PeerSnapshot, Rollback};

/// The most pairings one device will hold.
///
/// **A refusal, never an eviction.** Making room by dropping the oldest pairing
/// would cut off a device the user still owns, and nothing on this end can put
/// it back — the other half's key is gone and the two devices have to be paired
/// again by hand. That is the data-loss-shaped outcome `AGENTS.md` rule 4 rules
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

/// The cap on the revocation list, which unlike the pairings only ever grows —
/// a revocation is never evicted, because evicting one is exactly what lets a
/// device someone still holds a code for be re-added (`CopyPaste-gbo`).
///
/// So the refusal is the same shape as [`MAX_PAIRINGS`] and for the same
/// reason: at the cap a *new* revocation fails rather than displacing an old
/// one. Deliberately far above [`MAX_PAIRINGS`] — every pairing this device can
/// hold could be revoked and re-made many times over before it binds.
pub const MAX_REVOCATIONS: usize = 4096;

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
