//! The tokens a signed-in user holds, and the arithmetic on their expiry.

use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// How long before expiry a session should be refreshed.
///
/// Exported for the refresh loop that lives outside this module; the manifest's
/// `REFRESH_MARGIN_SECS = 60`.
pub const REFRESH_MARGIN_MS: i64 = 60_000;

/// A signed-in user's tokens.
///
/// `Clone` is derived because the daemon hands copies to the poll, push and
/// Realtime paths. `Debug` is hand-written and redacts both tokens. Dropping a
/// `Session` — including every clone — zeroizes them.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Session {
    /// The user JWT. Presented as `Authorization: Bearer` to PostgREST and as
    /// `config.access_token` on the Realtime join.
    pub access_token: String,
    /// Rotated by GoTrue on every refresh — the value returned by
    /// [`SupabaseAuth::refresh`](super::SupabaseAuth::refresh) must replace the
    /// one that was sent, or the next refresh presents a token the server has
    /// already retired.
    pub refresh_token: String,
    /// `auth.uid()` — the RLS pivot, and the Realtime filter value.
    pub user_id: String,
    /// Absolute expiry, ms since epoch, computed with a saturating add so a
    /// hostile `expires_in` cannot wrap into "already expired" (AT-43).
    pub expires_at_ms: i64,
}

impl Session {
    /// Whether the access token is past its expiry at `now_ms`.
    pub fn is_expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Whether the access token expires within [`REFRESH_MARGIN_MS`] — the
    /// condition a proactive refresh loop waits on.
    pub fn needs_refresh(&self, now_ms: i64) -> bool {
        self.expires_at_ms.saturating_sub(now_ms) <= REFRESH_MARGIN_MS
    }
}

impl fmt::Debug for Session {
    /// Redacts both tokens. `expires_at_ms` and `user_id` stay visible because
    /// they are what a bug report actually needs (AT-42).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("user_id", &self.user_id)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_for_a_session_shows_no_token() {
        let session = Session {
            access_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.secret".into(),
            refresh_token: "refresh-secret".into(),
            user_id: "user-uuid-1".into(),
            expires_at_ms: 1_700_000_000_000,
        };
        let rendered = format!("{session:?}");
        assert!(
            !rendered.contains("eyJhbGciOi"),
            "access token leaked: {rendered}"
        );
        assert!(
            !rendered.contains("refresh-secret"),
            "refresh token leaked: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("user-uuid-1"));
        assert!(
            rendered.contains("1700000000000"),
            "expiry should stay visible"
        );
    }

    #[test]
    fn a_session_zeroizes_on_drop() {
        fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Session>();
    }
}
