//! The list of devices this one is paired with, and their pre-shared keys.
//!
//! # What this module is
//!
//! A small, file-backed, thread-safe map from [`crate::PairingToken::pairing_id`] to
//! [`Peer`]. The daemon reads it to decide who may connect ([`PeerStore::psks`]
//! feeds [`crate::Session::accept_any`]) and to remember where a peer was last
//! seen so a reconnect does not have to wait for mDNS.
//!
//! ```text
//! peers-v2.json  (0600)
//! {
//!   "format": "copypaste-peers-v2",
//!   "peers": [
//!     { "pairing_id": "…32 hex…", "name": "Laptop",
//!       "psk": "…64 hex…", "last_addr": "192.168.1.7:47654",
//!       "last_seen_ms": 1753900000000 }
//!   ]
//! }
//! ```
//!
//! # Layout
//!
//! Three responsibilities, and the middle one is where the security properties
//! live:
//!
//! * [`peer`] — one record: what is secret in it, what `Debug` may show, and
//!   what makes it valid.
//! * [`file`] — the bytes on disk: the format tag that separates "another
//!   version wrote this" from "damaged", the atomic replace, and the `0600`.
//!   This is the file that *is* a key store, and its module docs say why.
//! * [`store`] — the in-memory map, the lock, and the write-through.
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

pub use error::PeerStoreError;
pub use peer::Peer;
pub use store::PeerStore;

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
