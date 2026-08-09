//! Client-side encryption for cloud rows. This module is what makes the
//! claim in the crate docs — *the server never sees plaintext* — true.
//!
//! # The shape of it
//!
//! ```text
//! passphrase (never leaves the device)
//!   │
//!   │  salt = HKDF-SHA256(salt = SALT_HKDF_SALT,
//!   │                     ikm  = account_id,
//!   │                     info = INFO_PER_ACCOUNT_SALT) -> 16 B
//!   ▼
//! Argon2id(m = 19456 KiB, t = 2, p = 1, v = 0x13, out = 32 B)
//!   ▼
//! SyncKey ─┬► XChaCha20-Poly1305, AAD = "copypaste/v2/cloud-row-aead|1|<len>:<item_id>"
//!          │    ▼
//!          │  (nonce_b64, ciphertext_b64)  ──►  Supabase
//!          │
//!          └► HKDF-Expand(info = ".../cloud-row-signature/hmac-sha256")
//!               ▼
//!             HMAC-SHA256 over every column the client writes
//!               ▼
//!             signature_b64  ──►  Supabase
//! ```
//!
//! Supabase holds the two base64 strings, the `item_id`, and metadata. It holds
//! nothing from which the key can be derived: the passphrase is never sent, and
//! the per-account salt is derived from the account id, which the server already
//! knows — a salt is not a secret, it exists so that two users who pick the same
//! passphrase do not get the same key, and so that one Argon2id table cannot be
//! precomputed against every account at once.
//!
//! The AEAD protects content; [`sign`] protects the metadata the merge orders
//! on, which travels in the clear because the backend pages on it. Read that
//! module's header for the attack it closes — encryption cannot fix an ordering
//! attack.
//!
//! The two halves of that diagram are the two files below: [`key`] turns a
//! passphrase into a key, [`row`] turns a key and a row into ciphertext. They
//! are separable because nothing in the AEAD path may depend on *how* the key
//! was obtained — the daemon's keystore path hands over 32 bytes with no
//! passphrase in sight, and that must be indistinguishable from a fresh
//! derivation.
//!
//! # Fail closed
//!
//! A wrong passphrase, a wrong `account_id`, a wrong `item_id`, a flipped bit
//! anywhere in nonce or ciphertext or tag: all of them produce
//! [`CloudCryptoError::AuthFailed`], with no detail about which. Distinguishing
//! them is an oracle (port manifest 02, I-15). There is no fallback read path,
//! no "try the other key", no version byte to negotiate down.
//!
//! Structural framing failures — base64 that does not decode, a nonce that is
//! not 24 bytes — are reported separately as [`CloudCryptoError::Malformed`].
//! They carry no information about the key and callers need them to tell "this
//! row is corrupt" from "this row is not mine" (I-15 again: framing errors are
//! *deliberately* distinguishable).
//!
//! # Dependencies
//!
//! Per `AGENTS.md` rule 1 nothing here is hand-rolled: `argon2` for the
//! passphrase KDF, `hkdf` + `sha2` for the salt, `chacha20poly1305` for the
//! AEAD, `rand` for nonces, `zeroize` for erasure, `base64` for the wire
//! encoding. The code in this module is glue and policy.
//!
//! [`key`] and [`row`] are separable because nothing in the AEAD path may depend
//! on *how* the key was obtained — the daemon's keystore path hands over 32
//! bytes with no passphrase in sight, and that must be indistinguishable from a
//! fresh derivation.

pub mod error;
pub mod handle;
pub mod key;
pub mod row;
pub mod sign;

#[cfg(test)]
mod testkit;

pub use error::CloudCryptoError;
pub use handle::CloudCrypto;
pub use key::{
    derive_sync_key, SyncKey, ARGON2_M_COST_KIB, ARGON2_P_COST, ARGON2_T_COST, KEY_LEN,
    MIN_PASSPHRASE_CHARS,
};
pub use row::{decrypt_row, encrypt_row, NONCE_LEN, TAG_LEN};
pub use sign::{sign_metadata, verify_metadata, RowMetadata, SIGNATURE_LEN};
