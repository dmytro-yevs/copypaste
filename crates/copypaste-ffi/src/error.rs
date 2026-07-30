//! The one error type that crosses the FFI boundary.
//!
//! # Why every variant is field-less
//!
//! `CLAUDE.md` rule 4 and manifest 06 INV-12 say the same thing twice: no
//! user-visible error text may contain a filesystem path, because the path
//! discloses the local username, and on Android it also discloses the package's
//! private data directory. v1 leaked the daemon socket path into the DOM,
//! screenshots and the accessibility tree that way (`CopyPaste-tzzu`,
//! `CopyPaste-j5qg`), and the fix — `friendlyIpcError()` — was a mapping the
//! caller had to remember to apply.
//!
//! So this enum removes the possibility rather than documenting it. **No
//! variant carries a payload of any kind.** There is no `String` field to
//! `format!` a path into, and therefore no code path — present or future, ours
//! or a caller's — that can put one in front of a user. What Kotlin receives is
//! a *code*, and the friendly sentence is chosen on the Kotlin side, which is
//! exactly the "code → copy mapping" INV-12 asks for and is also where
//! localisation belongs.
//!
//! The `#[error(...)]` sentences below are for the Rust log only. They are
//! pathless too (asserted in the tests) so that a `tracing` line is safe even
//! when it is captured into a bug report.
//!
//! # Why the variants are the shape they are
//!
//! They name *what the app should do next*, not what failed internally:
//! `Locked` means "ask the user to unlock the device"; `Storage` means "this is
//! ours, not yours". Distinctions that would tell an attacker something are
//! deliberately absent — [`CopyPasteError::Crypto`] covers a wrong key, a wrong
//! AAD and a flipped ciphertext bit alike, because `copypaste-core` refuses to
//! separate them (manifest 02 I-15: separating them is a decryption oracle).

use copypaste_core::{CryptoError, StoreError};
use copypaste_p2p::{PeerStoreError, TransportError};

/// Everything a native app can be told went wrong.
///
/// Maps to a sealed set of Kotlin exceptions. Field-less by design — see the
/// module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum CopyPasteError {
    /// The device secret handed to `CopyPaste::open` was not 32 bytes.
    ///
    /// A caller bug, not a user one: the app's keystore wrapper returned
    /// something the wrong size.
    #[error("the device secret must be 32 bytes")]
    BadDeviceSecret,

    /// The database exists but this device's key does not open it.
    ///
    /// Fail closed (`CLAUDE.md` rule 4): never a fallback to an unkeyed read.
    /// On Android the usual cause is that the keystore-wrapped secret was lost
    /// — a backup restore onto a new device, or the user's screen lock being
    /// removed and re-added, which invalidates the wrapping key.
    #[error("the history database did not open with this device's key")]
    Locked,

    /// The database could not be read or written.
    #[error("the history database could not be read or written")]
    Storage,

    /// An item could not be sealed, or could not be opened.
    ///
    /// Covers a wrong key, a wrong AAD and a tampered ciphertext without
    /// distinguishing them (manifest 02 I-15).
    #[error("the item could not be encrypted or decrypted")]
    Crypto,

    /// No live item has that id. It may have been deleted or evicted.
    #[error("no such item")]
    ItemNotFound,

    /// The text handed to `add` was empty or entirely whitespace.
    ///
    /// Not a failure the user needs to see — an empty clipboard is a normal
    /// state — but the caller must not treat it as "stored".
    #[error("there was nothing to store")]
    EmptyContent,

    /// The pairing code did not parse.
    ///
    /// Deliberately says nothing about *how* it was wrong: the input is a
    /// secret and the shape of a valid one is not something to hint at.
    #[error("that pairing code is not valid")]
    InvalidPairingCode,

    /// The address could not be resolved. Expected `host:port`.
    #[error("that address could not be resolved")]
    InvalidAddress,

    /// The other device did not accept this pairing code.
    #[error("the other device did not accept this pairing code")]
    PairingRefused,

    /// No paired device has that id.
    #[error("no such paired device")]
    PeerNotFound,

    /// The paired-device list could not be read or written.
    #[error("the paired-device list could not be read or written")]
    PeerStore,

    /// Data written by CopyPaste v0.4.x was found.
    ///
    /// `CLAUDE.md` rule 3's one obligation: v2 shares no formats with v1, and a
    /// v2 build that meets v1 data must **say so plainly** rather than failing
    /// with something that reads like corruption. The old data is untouched and
    /// still readable by the old build; v2 needs the devices paired again.
    #[error("this data was written by an older version of CopyPaste")]
    LegacyData,

    /// This peer has no address on file, so there is nothing to dial.
    ///
    /// Happens when the pairing was minted here and the other device has never
    /// connected: it knows how to reach us, we have never seen it.
    #[error("this device has never been reached at a known address")]
    PeerAddressUnknown,

    /// The other device did not answer, or stopped answering.
    #[error("the other device did not respond")]
    PeerUnreachable,

    /// Clipboard sync between devices is not available in this build.
    ///
    /// This is not a runtime fault; it is a missing capability, reported
    /// honestly rather than as a spurious network failure. See
    /// [`crate::pairing`] for exactly what is missing and what would close it.
    #[error("syncing clipboard history between devices is not available yet")]
    SyncUnavailable,
}

impl From<StoreError> for CopyPasteError {
    fn from(e: StoreError) -> Self {
        // The source error is logged here and discarded, so nothing it might
        // carry can reach the boundary. `StoreError` is pathless today; this
        // does not depend on it staying that way.
        match e {
            StoreError::InvalidKey => Self::Locked,
            StoreError::NotFound => Self::ItemNotFound,
            other => {
                tracing::warn!(error = ?other, "storage failure");
                Self::Storage
            }
        }
    }
}

impl From<CryptoError> for CopyPasteError {
    fn from(e: CryptoError) -> Self {
        tracing::warn!(error = ?e, "crypto failure");
        Self::Crypto
    }
}

impl From<PeerStoreError> for CopyPasteError {
    fn from(e: PeerStoreError) -> Self {
        match e {
            // Rule 3: this one is reported as itself, not folded into a generic
            // failure, so the app can tell the user their old pairings are
            // intact rather than implying the file is damaged.
            PeerStoreError::Legacy => Self::LegacyData,
            other => {
                tracing::warn!(error = ?other, "paired-device list failure");
                Self::PeerStore
            }
        }
    }
}

impl From<TransportError> for CopyPasteError {
    fn from(e: TransportError) -> Self {
        // `TransportError::InvalidCode` is the one case a user can act on. Every
        // other transport failure is "the other end did not play along", and
        // saying which is a hint to whoever is holding the socket.
        match e {
            TransportError::InvalidCode => Self::InvalidPairingCode,
            other => {
                tracing::debug!(error = %other, "peer transport failure");
                Self::PairingRefused
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant, so a new one cannot be added without appearing here.
    const ALL: &[CopyPasteError] = &[
        CopyPasteError::BadDeviceSecret,
        CopyPasteError::Locked,
        CopyPasteError::Storage,
        CopyPasteError::Crypto,
        CopyPasteError::ItemNotFound,
        CopyPasteError::EmptyContent,
        CopyPasteError::InvalidPairingCode,
        CopyPasteError::InvalidAddress,
        CopyPasteError::PairingRefused,
        CopyPasteError::PeerNotFound,
        CopyPasteError::PeerStore,
        CopyPasteError::LegacyData,
        CopyPasteError::PeerAddressUnknown,
        CopyPasteError::PeerUnreachable,
        CopyPasteError::SyncUnavailable,
    ];

    #[test]
    fn no_variant_renders_anything_path_like() {
        // CLAUDE.md rule 4 / manifest 06 INV-12. The type makes a path
        // structurally unreachable (no variant has a field), but the log
        // sentences are pinned too.
        for e in ALL {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path separator in {msg:?}");
            assert!(!msg.contains('\\'), "path separator in {msg:?}");
            assert!(!msg.contains("home"), "path fragment in {msg:?}");
            assert!(!msg.contains("data/user"), "path fragment in {msg:?}");
            assert!(!msg.contains(".db"), "file name in {msg:?}");
        }
    }

    #[test]
    fn every_variant_is_field_less() {
        // The guarantee above only holds while this is true, and `Copy` is the
        // cheapest way to say it: an enum carrying a `String` cannot be `Copy`.
        // If someone adds a payload, this file stops compiling.
        fn assert_copy<T: Copy>() {}
        assert_copy::<CopyPasteError>();
    }

    #[test]
    fn a_wrong_key_is_locked_not_storage() {
        // Fail closed, and say so specifically: the app shows "unlock and try
        // again", not "something went wrong".
        assert_eq!(
            CopyPasteError::from(StoreError::InvalidKey),
            CopyPasteError::Locked
        );
    }

    #[test]
    fn a_bad_pairing_code_is_distinguishable_but_nothing_else_is() {
        assert_eq!(
            CopyPasteError::from(TransportError::InvalidCode),
            CopyPasteError::InvalidPairingCode
        );
        assert_eq!(
            CopyPasteError::from(TransportError::Handshake),
            CopyPasteError::PairingRefused
        );
        assert_eq!(
            CopyPasteError::from(TransportError::Malformed),
            CopyPasteError::PairingRefused
        );
    }

    #[test]
    fn v1_data_is_reported_as_itself() {
        // CLAUDE.md rule 3: "say so plainly rather than failing with a
        // decryption error that reads like corruption."
        assert_eq!(
            CopyPasteError::from(PeerStoreError::Legacy),
            CopyPasteError::LegacyData
        );
        assert_ne!(
            CopyPasteError::from(PeerStoreError::Corrupt),
            CopyPasteError::LegacyData
        );
    }

    #[test]
    fn authentication_failures_are_not_distinguishable_from_each_other() {
        // Manifest 02 I-15: a caller must not be able to tell a wrong key from a
        // wrong AAD from a flipped bit.
        assert_eq!(
            CopyPasteError::from(CryptoError::AuthFailed),
            CopyPasteError::from(CryptoError::InvalidNonce)
        );
    }
}
