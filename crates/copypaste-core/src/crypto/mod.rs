//! Crypto: one device secret, one derivation path, one AEAD path.
//!
//! # What this module is
//!
//! A single 32-byte device secret lives in the OS keystore. Two keys are
//! derived from it with HKDF-SHA256, domain-separated by `info` string:
//!
//! ```text
//! device secret (32 B, OS keystore)
//!   └─ HKDF-SHA256(salt = HKDF_SALT, ikm = device secret)
//!        ├─ expand(info = b"copypaste/v2/sqlcipher-db-key")   -> db_key   (32 B)
//!        └─ expand(info = b"copypaste/v2/item-content-key")   -> item_key (32 B)
//! ```
//!
//! Item content is sealed with XChaCha20-Poly1305. The AAD binds the item's
//! logical id, so a ciphertext lifted out of one row and pasted into another
//! fails authentication instead of decrypting.
//!
//! # What this module deliberately is not
//!
//! v2 drops backward compatibility with v0.4.x (`CLAUDE.md` rule 3 and
//! `docs/rewrite/port-manifest/README.md`). The v1 manifest's HKDF strings, AAD
//! byte layouts, `CHUNK_FORMAT_V1` framing and `key_version` dispatch are
//! **reference only** and are not reproduced here. Concretely, this module has:
//!
//! * no `key_version` column, no version dispatch, no trial decryption;
//! * no rotation sweep and no repair sweep;
//! * no second hash function, no second salt, no second AAD schema.
//!
//! That is the simplification dropping compatibility bought. v1 carried two key
//! generations forever because it could not stop reading old rows; v2 can, and
//! every branch that existed only to choose between them is gone. If a future
//! version needs a second generation, add it then — a version field that only
//! ever holds one value is the beginning of the v1 problem, not a defence
//! against it.
//!
//! # Security properties carried over from v1 (port manifest 02, §2)
//!
//! * **Fail closed.** A wrong key, a wrong AAD, or a flipped bit anywhere in
//!   nonce/ciphertext/tag produces [`CryptoError::AuthFailed`]. There is no
//!   fallback read path and no way to ask *why* authentication failed — that
//!   distinction is an oracle (I-15).
//! * **AAD binds item identity.** `item_id` is the cross-device logical id, not
//!   a row primary key (I-6 / §3.3).
//! * **Fresh OS-CSPRNG nonce per message.** `OsRng`, never a counter, never
//!   reused (I-11).
//! * **Zeroize on drop** for every value holding key material (I-12).
//! * **Constant-time comparison** of secrets via `subtle` (I-13).
//! * **No panics on caller-supplied data.** Every failure is a typed `Result`
//!   (I-14).
//! * **Errors never contain a filesystem path** (`CLAUDE.md` rule 4). This is
//!   enforced structurally: [`CryptoError`]'s payloads are `&'static str`, so
//!   there is no way to interpolate a path into one even by accident.
//! * **Keystore naming is frozen** at `com.copypaste.daemon` /
//!   `device-secret-key` (I-10).
//!
//! # Dependencies
//!
//! Per `CLAUDE.md` rule 1, nothing here is hand-rolled: `chacha20poly1305` for
//! the AEAD, `hkdf` + `sha2` for derivation, `rand` for nonces, `zeroize` for
//! erasure, `subtle` for comparison, `directories` for the data dir. The only
//! code in this file is glue and policy.
//!
//! # Layout
//!
//! * [`keys`] — the device secret, HKDF derivation and the two key types.
//! * [`aead`] — the item envelope: AAD construction, [`encrypt`], [`decrypt`].
//! * [`keystore`] — the OS-keystore backends that hold the device secret.
//!
//! [`CryptoError`] stays here because all three share it.

// The macOS Keychain backend is behind the `macos-keychain` cargo feature,
// which `crates/copypaste-core/Cargo.toml` does not declare yet (it needs
// `security-framework = { workspace = true, optional = true }`). Until it does,
// `cfg(feature = "macos-keychain")` is an unknown feature name; allow the
// check-cfg lint so the rest of the crate compiles cleanly meanwhile.
#![allow(unexpected_cfgs)]

mod aead;
mod keys;
mod keystore;

pub use aead::{decrypt, encrypt, NONCE_LEN, TAG_LEN};
pub use keys::{ItemKey, Keyring, KEY_LEN};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Every failure this module can produce.
///
/// Payloads are `&'static str` on purpose. `CLAUDE.md` rule 4 forbids showing
/// users a filesystem path (the daemon's data directory discloses the local
/// username), and a `String` payload is an open invitation to `format!` one in.
/// A closed set of literals cannot leak.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Wrong key, wrong AAD (wrong `item_id`), or tampered
    /// nonce/ciphertext/tag.
    ///
    /// These are **deliberately indistinguishable**: telling a caller which one
    /// happened turns decryption into an oracle (port manifest 02, I-15). The
    /// only correct handling is skip-and-count; never treat it as success.
    #[error("authentication failed")]
    AuthFailed,

    /// The nonce handed to [`decrypt`] is not [`NONCE_LEN`] bytes.
    ///
    /// Structural, not secret-bearing: it says nothing about the key or the
    /// plaintext, so unlike [`CryptoError::AuthFailed`] it is safe to
    /// distinguish. It means the storage row is corrupt or a caller passed the
    /// wrong column.
    #[error("nonce must be {NONCE_LEN} bytes")]
    InvalidNonce,

    /// The OS keystore could not be read or written, and its contents are
    /// therefore *unknown*.
    ///
    /// This is never a licence to mint a fresh secret: doing so would open the
    /// existing SQLCipher database with the wrong key. Only an unambiguous
    /// "no entry exists" answer authorises creation, and that path returns
    /// `Ok` rather than this error (port manifest 02, I-20). Callers should
    /// degrade — report locked, leave the encrypted database untouched.
    #[error("key store unavailable: {0}")]
    KeystoreUnavailable(&'static str),

    /// A cryptographic primitive failed for a reason that is not attacker
    /// controlled — e.g. an HKDF output length the implementation rejects, or
    /// an AEAD input past the ~256 GiB per-message limit.
    #[error("internal crypto error: {0}")]
    Internal(&'static str),
}

/// Fixtures shared by the submodule test suites, so the same secret and item id
/// mean the same thing in every one of them.
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
        // CLAUDE.md rule 4. The type makes this structural — payloads are
        // &'static str — but pin the rendered strings anyway.
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
