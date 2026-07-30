//! What one round reports: the counters on the way out, and the closed set of
//! failures on the way back.
//!
//! The two live together because they are the same thing seen from either side
//! — a caller matches on one or reads the other, and neither is useful without
//! knowing what the other can say.

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// What one push, pull or sync did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// Live rows sealed and upserted.
    pub uploaded: usize,
    /// Tombstones propagated.
    pub tombstoned: usize,
    /// Rows fetched, including ones that were skipped.
    pub downloaded: usize,
    /// Rows the store actually took.
    pub applied: usize,
    /// Local items withheld by the [`SensitiveGuard`](super::SensitiveGuard).
    pub skipped_sensitive: usize,
    /// Remote rows that would not open. Never treated as a delete (INV-N3).
    pub skipped_undecryptable: usize,
    /// Remote rows whose metadata was unsigned or wrongly signed, and which
    /// therefore never reached the comparator (manifest 05 §5.3).
    ///
    /// A non-zero count here is not routine. It means something wrote a row
    /// into the account that does not hold the sync passphrase.
    pub skipped_forged: usize,
    /// Remote rows stamped implausibly far in the future.
    pub skipped_future: usize,
    /// Local items over the per-item upload cap. Withheld, never deleted.
    pub skipped_too_large: usize,
}

impl SyncStats {
    /// Did this round change anything? Drives the idle backoff.
    pub fn changed(&self) -> bool {
        self.uploaded > 0 || self.tombstoned > 0 || self.applied > 0
    }

    pub(super) fn merge(self, other: Self) -> Self {
        Self {
            uploaded: self.uploaded + other.uploaded,
            tombstoned: self.tombstoned + other.tombstoned,
            downloaded: self.downloaded + other.downloaded,
            applied: self.applied + other.applied,
            skipped_sensitive: self.skipped_sensitive + other.skipped_sensitive,
            skipped_undecryptable: self.skipped_undecryptable + other.skipped_undecryptable,
            skipped_forged: self.skipped_forged + other.skipped_forged,
            skipped_future: self.skipped_future + other.skipped_future,
            skipped_too_large: self.skipped_too_large + other.skipped_too_large,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Sync failures.
///
/// Every payload is a `&'static str`, so no variant can carry a filesystem
/// path, an access token, a refresh token, a passphrase or row content. That is
/// `CLAUDE.md` rule 4 enforced by the type rather than by review. Implementors
/// of [`CloudSource`](super::CloudSource) must hold to the same rule when they
/// build [`SyncError::Source`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    /// The store failed.
    #[error("local store error: {0}")]
    Source(&'static str),

    /// A row could not be sealed for upload. Never a reason to upload it in the
    /// clear.
    #[error("could not encrypt an item for upload")]
    Encrypt,

    /// The bearer was rejected, a refresh was attempted exactly once, and the
    /// retry was rejected too.
    ///
    /// One refresh, one retry, then stop. A refresh that returns a token which
    /// is itself rejected would otherwise spin forever (manifest 05 AT-36).
    #[error("the backend rejected this session even after a refresh")]
    Unauthorized,

    /// The stored credentials are wrong. Distinct from
    /// [`SyncError::SessionExpired`] because only a human can fix it: prompt,
    /// do not retry, and never fall back to a lower-privilege scope (INV-N6).
    #[error("the stored account credentials were rejected")]
    InvalidCredentials,

    /// The refresh token expired or was revoked. A full sign-in recovers.
    #[error("the session has expired and must be renewed by signing in")]
    SessionExpired,

    /// The backend asked us to slow down, we waited as asked, and it asked
    /// again. Single-shot, so a server stuck on 429 cannot pin the loop.
    #[error("the backend is rate limiting this account")]
    RateLimited,

    /// Network or backend failure that outlived the retry budget.
    #[error("cloud transport error: {0}")]
    Transport(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_no_paths_tokens_or_passphrases() {
        let errors = [
            SyncError::Source("could not read the history"),
            SyncError::Encrypt,
            SyncError::Unauthorized,
            SyncError::InvalidCredentials,
            SyncError::SessionExpired,
            SyncError::RateLimited,
            SyncError::Transport("backend unavailable"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains("home"), "path-like fragment in {msg:?}");
            assert!(
                !msg.contains(crate::sync::fakes::PASS),
                "passphrase in {msg:?}"
            );
            assert!(!msg.contains("eyJ"), "jwt-like text in {msg:?}");
        }
    }
}
