//! A key with the two row operations bound to it.

use super::error::CloudCryptoError;
use super::key::{derive_sync_key, SyncKey};
use super::row::{decrypt_row, encrypt_row};

/// A [`SyncKey`] with the two row operations bound to it.
///
/// Purely a convenience for callers that would otherwise thread the key through
/// every call site; it holds no state beyond the key and adds no behaviour.
/// [`derive_sync_key`], [`encrypt_row`] and [`decrypt_row`] remain the API —
/// this is one indirection over them, not a second implementation.
pub struct CloudCrypto {
    key: SyncKey,
}

impl CloudCrypto {
    /// Derive the key and wrap it. See [`derive_sync_key`] for the cost and for
    /// what `account_id` must be.
    ///
    /// # Errors
    ///
    /// As [`derive_sync_key`].
    pub fn derive(passphrase: &str, account_id: &str) -> Result<Self, CloudCryptoError> {
        Ok(Self {
            key: derive_sync_key(passphrase, account_id)?,
        })
    }

    /// Wrap a key that was derived earlier — typically read back from the OS
    /// keystore, so that a restart does not re-prompt for the passphrase.
    pub fn new(key: SyncKey) -> Self {
        Self { key }
    }

    /// The wrapped key, for handing to [`crate::sync::CloudSync`].
    pub fn key(&self) -> &SyncKey {
        &self.key
    }

    /// See [`encrypt_row`].
    ///
    /// # Errors
    ///
    /// As [`encrypt_row`].
    pub fn seal(
        &self,
        plaintext: &[u8],
        item_id: &str,
    ) -> Result<(String, String), CloudCryptoError> {
        encrypt_row(plaintext, &self.key, item_id)
    }

    /// See [`decrypt_row`].
    ///
    /// # Errors
    ///
    /// As [`decrypt_row`].
    pub fn open(
        &self,
        ciphertext_b64: &str,
        nonce_b64: &str,
        item_id: &str,
    ) -> Result<Vec<u8>, CloudCryptoError> {
        decrypt_row(ciphertext_b64, nonce_b64, &self.key, item_id)
    }
}

/// Redacted: a `Debug` that prints key material ends up in a log file.
impl std::fmt::Debug for CloudCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudCrypto").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{ACCOUNT, ITEM, PASS};
    use super::*;

    #[test]
    fn the_handle_agrees_with_the_free_functions() {
        let crypto = CloudCrypto::derive(PASS, ACCOUNT).unwrap();
        let (nonce, ct) = crypto.seal(b"through the handle", ITEM).unwrap();

        assert_eq!(
            crypto.open(&ct, &nonce, ITEM).unwrap(),
            b"through the handle"
        );
        // And the free function opens what the handle sealed, with the key the
        // handle exposes.
        assert_eq!(
            decrypt_row(&ct, &nonce, crypto.key(), ITEM).unwrap(),
            b"through the handle"
        );
    }

    #[test]
    fn debug_output_never_contains_key_material() {
        let crypto = CloudCrypto::derive(PASS, ACCOUNT).unwrap();
        assert_eq!(format!("{crypto:?}"), "CloudCrypto { .. }");
    }
}
