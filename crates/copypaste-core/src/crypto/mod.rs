//! Crypto: one device secret, one derivation path, one AEAD path.
//!
//! A 32-byte device secret in the OS keystore, HKDF-SHA256 into a db key and an
//! item key ([`keys`]); item content sealed with XChaCha20-Poly1305 whose AAD
//! binds the item's logical id, so a ciphertext lifted into another row fails
//! authentication instead of decrypting.
//!
//! No `key_version` dispatch, no trial decryption, no rotation or repair sweep:
//! v2 does not read v0.4.x data (`CLAUDE.md` rule 3). A version field that only
//! ever holds one value is the beginning of the v1 problem, not a defence.
//!
//! # Security properties (port manifest 02, §2)
//!
//! * **Fail closed.** A wrong key, a wrong AAD, or a flipped bit anywhere in
//!   nonce/ciphertext/tag produces [`CryptoError::AuthFailed`]. There is no
//!   fallback read path and no way to ask *why* — that distinction is an
//!   oracle (I-15).
//! * **AAD binds item identity.** `item_id` is the cross-device logical id, not
//!   a row primary key (I-6 / §3.3).
//! * **Fresh OS-CSPRNG nonce per message.** `OsRng`, never a counter, never
//!   reused (I-11).
//! * **Zeroize on drop** for every value holding key material (I-12).
//! * **Constant-time comparison** of secrets via `subtle` (I-13).
//! * **No panics on caller-supplied data.** Every failure is a typed `Result`
//!   (I-14).
//! * **Errors never contain a filesystem path** (`CLAUDE.md` rule 4), enforced
//!   structurally: [`CryptoError`]'s payloads are `&'static str`.
//! * **Keystore naming is frozen** at `com.copypaste.daemon` /
//!   `device-secret-key` (I-10).

mod aead;
mod keys;
mod keystore;

pub use aead::{decrypt, encrypt, NONCE_LEN, TAG_LEN};
pub use keys::{ItemKey, Keyring, KEY_LEN};

/// Every failure this module can produce.
///
/// Payloads are `&'static str` on purpose: `CLAUDE.md` rule 4 forbids showing
/// users a filesystem path, and a `String` payload is an invitation to
/// `format!` one in. A closed set of literals cannot leak.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Wrong key, wrong AAD (wrong `item_id`), or tampered
    /// nonce/ciphertext/tag — **deliberately indistinguishable**, because
    /// telling a caller which one happened turns decryption into an oracle
    /// (port manifest 02, I-15). Correct handling is skip-and-count; never
    /// treat it as success.
    #[error("authentication failed")]
    AuthFailed,

    /// The nonce handed to [`decrypt`] is not [`NONCE_LEN`] bytes. Structural,
    /// not secret-bearing, so unlike [`CryptoError::AuthFailed`] it is safe to
    /// distinguish: the row is corrupt or a caller passed the wrong column.
    #[error("nonce must be {NONCE_LEN} bytes")]
    InvalidNonce,

    /// The OS keystore could not be read or written, so its contents are
    /// *unknown*.
    ///
    /// Never a licence to mint a fresh secret: that would open the existing
    /// SQLCipher database with the wrong key. Only an unambiguous "no entry
    /// exists" answer authorises creation, and that path returns `Ok` rather
    /// than this error (port manifest 02, I-20). Callers degrade — report
    /// locked, leave the encrypted database untouched.
    #[error("key store unavailable: {0}")]
    KeystoreUnavailable(&'static str),

    /// A primitive failed for a reason that is not attacker controlled — an
    /// HKDF output length the implementation rejects, or an AEAD input past
    /// the ~256 GiB per-message limit.
    #[error("internal crypto error: {0}")]
    Internal(&'static str),
}

#[cfg(test)]
pub(super) mod test_support {
    use super::{ItemKey, Keyring, KEY_LEN};

    pub(super) const SECRET_A: [u8; KEY_LEN] = [7u8; KEY_LEN];
    pub(super) const SECRET_B: [u8; KEY_LEN] = [9u8; KEY_LEN];
    pub(super) const ITEM: &str = "1f3c9a4e-0000-4000-8000-000000000001";

    pub(super) fn key_a() -> ItemKey {
        Keyring::from_secret(&SECRET_A).item_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        // CLAUDE.md rule 4: the type makes this structural, but pin the
        // rendered strings anyway.
        let errors = [
            CryptoError::AuthFailed,
            CryptoError::InvalidNonce,
            CryptoError::KeystoreUnavailable("the keychain is locked or access was denied"),
            CryptoError::Internal("AEAD rejected the plaintext (too large)"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains("home"), "path-like fragment in {msg:?}");
        }
    }
}
