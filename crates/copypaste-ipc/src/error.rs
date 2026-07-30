//! The failure taxonomy, and the single answer to "would asking again help".
//!
//! Split from the envelope in `lib.rs` for size. A client branches on these and
//! never on the `error` string (manifest 04, I9), so a condition that a client
//! must *render differently* needs a code of its own — text alone is not a
//! contract, and the sentence is free to be reworded or translated.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No such **item**.
    ///
    /// Devices are [`ErrorCode::PeerNotFound`], and keeping the two apart is
    /// not pedantry: every client turns this one into a fixed sentence about
    /// the clipboard, so a missing *device* answered under this code told the
    /// user their clipboard item was gone (post-merge review, finding 4).
    NotFound,
    InvalidRequest,
    ProtocolMismatch,
    NotReady,
    /// Credentials were rejected, or the operation needs credentials the daemon
    /// does not hold. Its own code because the recovery is a human action —
    /// sign in, retype the passphrase — and never a retry (manifest 04's
    /// `auth_failed`).
    AuthFailed,
    /// The file is a CopyPaste 0.4 history. v2 shares no formats with v0.4.x
    /// (CLAUDE.md rule 3) and has neither opened nor altered it.
    ///
    /// Its own code because every other reading of the same failure sends the
    /// user somewhere useless: `Internal` reads as a bug, and the restore
    /// path's "not a backup, or damaged" reads as data loss when the data is
    /// intact. The decision is a human one — keep the old history and run an
    /// older build, or start fresh — so a client must not offer a retry.
    LegacyDatabase,
    /// The key store could not be read, so what it holds is *unknown* — locked
    /// keychain, unreadable directory, wrong data directory
    /// (`CryptoError::KeystoreUnavailable`).
    ///
    /// Transient by nature: unlocking the keychain turns this into success,
    /// which is the entire reason it is not [`ErrorCode::KeyUnusable`].
    KeyLocked,
    /// The device key is present and cannot be used
    /// (`CryptoError::KeystoreEntryUnusable`), so the history encrypted under
    /// it cannot be decrypted by anything.
    ///
    /// The counterpart to [`ErrorCode::KeyLocked`] and the reason both exist:
    /// collapsing the two tells a user to retry against a condition where no
    /// number of retries produces a different answer.
    KeyUnusable,

    // ---- pairing and peer sync ---------------------------------------------
    // `copypaste_p2p::NodeError` authors nine sentences and these six carry
    // them. Grouped by **what the user does next**, not one code per variant: a
    // mistyped code and a rejected handshake end at the same screen, and so do
    // a peer with no address and one that stopped answering. Before them all
    // nine arrived as `invalid_request`, `internal` or — worse — `not_found`,
    // and a client had nothing to branch on but text it does not author.
    /// The pairing code was malformed, or the other device refused it. A fresh
    /// code is the only way forward.
    PairingCode,
    /// The address is not `host:port`, or resolves to nothing.
    PairingAddress,
    /// The device is not visible on the network and did not answer. The one
    /// pairing condition worth a **Try again**, because the device may come
    /// back without the user doing anything.
    PeerUnreachable,
    /// `copypaste_p2p::peers::MAX_PAIRINGS` reached. The remedy — unpair a
    /// device — is the whole content of the refusal, so it must not collapse
    /// into a generic failure.
    PairingLimit,
    /// The sync session, or the write to the paired-device list, failed. Not
    /// the user's doing and worth repeating.
    PeerFailed,
    /// No such **paired device**. See [`ErrorCode::NotFound`].
    PeerNotFound,

    Internal,
}

impl ErrorCode {
    /// Whether repeating the same request could plausibly succeed.
    ///
    /// One answer, next to the taxonomy, because the alternative is each
    /// surface deciding again — and the defect this guards is a client
    /// offering **Try again** against a condition that can never change.
    /// Anything a human must act on is `false`, whether the action is a
    /// sign-in, a downgrade, or an admission that the data is gone.
    ///
    /// Total, with no `_` arm: a new code does not compile until somebody has
    /// decided which of the two it is.
    #[must_use]
    pub fn retryable(self) -> bool {
        match self {
            Self::NotReady
            | Self::KeyLocked
            | Self::PeerUnreachable
            | Self::PeerFailed
            | Self::Internal => true,
            Self::NotFound
            | Self::InvalidRequest
            | Self::ProtocolMismatch
            | Self::AuthFailed
            | Self::LegacyDatabase
            | Self::KeyUnusable
            | Self::PairingCode
            | Self::PairingAddress
            | Self::PairingLimit
            | Self::PeerNotFound => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The codes are the contract; a client branches on them and never on the
    /// text (manifest 04, I9). Renaming one silently is a client that stops
    /// recognising a state and falls back to its generic one.
    #[test]
    fn error_codes_keep_their_wire_spelling() {
        for (code, wire) in [
            (ErrorCode::NotFound, "\"not_found\""),
            (ErrorCode::InvalidRequest, "\"invalid_request\""),
            (ErrorCode::ProtocolMismatch, "\"protocol_mismatch\""),
            (ErrorCode::NotReady, "\"not_ready\""),
            (ErrorCode::AuthFailed, "\"auth_failed\""),
            (ErrorCode::LegacyDatabase, "\"legacy_database\""),
            (ErrorCode::KeyLocked, "\"key_locked\""),
            (ErrorCode::KeyUnusable, "\"key_unusable\""),
            (ErrorCode::PairingCode, "\"pairing_code\""),
            (ErrorCode::PairingAddress, "\"pairing_address\""),
            (ErrorCode::PeerUnreachable, "\"peer_unreachable\""),
            (ErrorCode::PairingLimit, "\"pairing_limit\""),
            (ErrorCode::PeerFailed, "\"peer_failed\""),
            (ErrorCode::PeerNotFound, "\"peer_not_found\""),
            (ErrorCode::Internal, "\"internal\""),
        ] {
            assert_eq!(serde_json::to_string(&code).unwrap(), wire);
        }
    }

    /// The pair the distinction exists for: the same subsystem, one worth
    /// waiting on and one that no amount of waiting changes.
    #[test]
    fn a_locked_key_store_is_retryable_and_an_unusable_key_is_not() {
        assert!(ErrorCode::KeyLocked.retryable());
        assert!(!ErrorCode::KeyUnusable.retryable());
        assert!(!ErrorCode::LegacyDatabase.retryable());
    }

    /// A device that may come back is worth a retry; a code, an address and a
    /// full pairing list each need the user to do something first.
    #[test]
    fn only_the_pairing_failures_a_repeat_could_answer_are_retryable() {
        assert!(ErrorCode::PeerUnreachable.retryable());
        assert!(ErrorCode::PeerFailed.retryable());
        for code in [
            ErrorCode::PairingCode,
            ErrorCode::PairingAddress,
            ErrorCode::PairingLimit,
            ErrorCode::PeerNotFound,
        ] {
            assert!(!code.retryable(), "{code:?}");
        }
    }

    /// A missing device and a missing item are different sentences on every
    /// surface, so they must be different codes on the wire.
    #[test]
    fn a_missing_device_is_not_a_missing_item() {
        assert_ne!(ErrorCode::PeerNotFound, ErrorCode::NotFound);
    }
}
