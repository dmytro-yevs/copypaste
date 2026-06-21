//! [`PairingToken`] — the security-critical PAKE shared secret transported by the QR code.
//!
//! # Security invariants (MUST NOT be weakened by any refactor)
//!
//! * **Constant-time equality** via [`subtle::ConstantTimeEq`]. The `PartialEq` impl
//!   never short-circuits on a differing byte and must never be replaced with `==` on
//!   the inner byte array.
//! * **Zeroization on drop** via [`zeroize::ZeroizeOnDrop`]. The 32 secret bytes are
//!   scrubbed from memory when the token is dropped.
//! * **No `Debug` / `Display` / `Clone`** to prevent accidental logging or silent
//!   duplication of the secret.
//! * **OS CSPRNG** — [`PairingToken::generate`] draws exclusively from [`rand::rngs::OsRng`].

use base64::Engine as _;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{b64, PairingQrError, PAIRING_TOKEN_LEN};

/// A short-lived, high-entropy secret transported by the QR code and fed into
/// the PAKE handshake as the shared "password".
///
/// # Security
/// * `ZeroizeOnDrop` scrubs the bytes when dropped.
/// * Does NOT implement `Debug` / `Display` / `Clone` to avoid accidental
///   logging or silent duplication.
/// * Equality is constant-time via [`ConstantTimeEq`].
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PairingToken([u8; PAIRING_TOKEN_LEN]);

impl PairingToken {
    /// Generate a fresh 256-bit pairing token from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; PAIRING_TOKEN_LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Borrow the raw token bytes.
    pub fn as_bytes(&self) -> &[u8; PAIRING_TOKEN_LEN] {
        &self.0
    }

    /// Construct a token from exactly [`PAIRING_TOKEN_LEN`] bytes.
    ///
    /// # Errors
    /// Returns [`PairingQrError::TokenLength`] if `bytes` is the wrong length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PairingQrError> {
        let arr: [u8; PAIRING_TOKEN_LEN] = bytes
            .try_into()
            .map_err(|_| PairingQrError::TokenLength(bytes.len()))?;
        Ok(Self(arr))
    }

    /// Encode the token as the PAKE "password" string.
    ///
    /// Renders the raw token bytes as base64url so the full 256 bits of entropy
    /// survive the byte→str conversion losslessly.
    pub fn to_pake_password(&self) -> String {
        b64().encode(self.0)
    }

    /// Access the raw inner bytes (used by `PairingPayload::encode`).
    pub(super) fn inner(&self) -> [u8; PAIRING_TOKEN_LEN] {
        self.0
    }
}

impl PartialEq for PairingToken {
    /// Constant-time comparison — never short-circuit on the first differing byte.
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for PairingToken {}
