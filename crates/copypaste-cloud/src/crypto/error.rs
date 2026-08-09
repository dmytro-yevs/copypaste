//! The closed set of failures, and the fail-closed policy each variant
//! encodes.

use super::key::MIN_PASSPHRASE_CHARS;

/// Every failure this module can produce.
///
/// Payloads are `&'static str` on purpose. `AGENTS.md` rule 4 forbids showing
/// users a filesystem path, and the sync path additionally must never render a
/// token or a passphrase. A closed set of literals cannot leak one even by
/// accident — there is no `String` to `format!` into.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CloudCryptoError {
    /// Wrong key (wrong passphrase or wrong account), wrong `item_id`, or a
    /// tampered nonce/ciphertext/tag.
    ///
    /// These are **deliberately indistinguishable**. Telling the caller which
    /// one happened turns decryption into an oracle (manifest 02, I-15). The
    /// only correct handling is skip-and-count; never treat it as success, and
    /// never let it delete or overwrite a local row (manifest 05, INV-N3).
    #[error("authentication failed")]
    AuthFailed,

    /// The passphrase is shorter than [`MIN_PASSPHRASE_CHARS`] characters.
    ///
    /// Carries no length and no content: it is a user-facing validation error,
    /// and the value being validated is a secret.
    #[error("the sync passphrase must be at least {MIN_PASSPHRASE_CHARS} characters")]
    PassphraseTooShort,

    /// `account_id` was empty.
    ///
    /// Rejected loudly rather than allowed to collapse the per-account salt
    /// toward a shared constant, which would reintroduce the cross-account
    /// Argon2id precompute weakness the salt exists to prevent (manifest 02,
    /// I-19).
    #[error("an account id is required to derive the sync key")]
    EmptyAccountId,

    /// Structural: the base64 did not decode, or the nonce is not
    /// [`NONCE_LEN`](super::NONCE_LEN) bytes.
    ///
    /// Safe to distinguish from [`CloudCryptoError::AuthFailed`] — it says
    /// nothing about the key. It means the row is corrupt or a caller passed
    /// the wrong column.
    #[error("the stored row is not well formed")]
    Malformed,

    /// The row's metadata was not signed by a holder of the sync key: the
    /// signature is absent, unparseable, or simply wrong.
    ///
    /// Not distinguished from one another, for the same reason
    /// [`CloudCryptoError::AuthFailed`] is not: the only correct response to any
    /// of them is to refuse the row before it participates in the merge
    /// (manifest 05 §5.3), and a caller that could tell "unsigned" from "wrongly
    /// signed" would be an oracle for whichever of the two an attacker was
    /// probing.
    #[error("the row's metadata signature did not verify")]
    SignatureInvalid,

    /// A primitive failed for a reason that is not attacker-controlled — an
    /// Argon2 parameter the implementation rejects, or an AEAD input past the
    /// ~256 GiB per-message limit.
    #[error("internal crypto error: {0}")]
    Internal(&'static str),
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{ACCOUNT, PASS};
    use super::*;

    #[test]
    fn error_messages_contain_no_paths_and_no_secrets() {
        // AGENTS.md rule 4, plus the sync rule that a token or a passphrase
        // must never be rendered. The type makes this structural — every
        // payload is a `&'static str` — but pin the rendered strings too.
        let errors = [
            CloudCryptoError::AuthFailed,
            CloudCryptoError::PassphraseTooShort,
            CloudCryptoError::EmptyAccountId,
            CloudCryptoError::Malformed,
            CloudCryptoError::SignatureInvalid,
            CloudCryptoError::Internal("AEAD rejected the plaintext (too large)"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains(PASS), "passphrase in {msg:?}");
            assert!(!msg.contains(ACCOUNT), "account id in {msg:?}");
        }
    }
}
