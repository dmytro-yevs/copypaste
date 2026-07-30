//! The network seam: the two traits the driver talks through, and the four
//! things it knows how to do about a failure.
//!
//! Both traits exist so that the recovery rules in [`super::retry`] can be
//! exercised against fakes with no HTTP anywhere; every 401, 429 and tombstone
//! assertion in this module's tests would otherwise need a live backend or a
//! stub server, and a rule that is only asserted end to end tends not to be
//! asserted at all. The production implementations are in [`super::adapters`].

use std::future::Future;
use std::time::Duration;

use crate::auth::Session;
use crate::rest::CloudItem;

/// What the driver needs to know about a failed request.
///
/// The REST and auth layers classify their own HTTP statuses; the driver acts
/// on the classification. Keeping the two apart is what lets this module be
/// tested against a fake transport with no HTTP anywhere, and it is the reason
/// the recovery rules below are asserted rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportFault {
    /// 401. The bearer is stale or wrong.
    Unauthorized,
    /// 429, with the server's `Retry-After` if it sent one.
    ///
    /// Delta-seconds only. The HTTP-date form is deliberately unsupported:
    /// Supabase emits integer seconds, and a date parser buys nothing but a
    /// dependency and a clock-skew bug (manifest 05 §4.6.3).
    RateLimited { retry_after: Option<Duration> },
    /// 5xx, a timeout, or a socket error. Worth retrying.
    Transient(&'static str),
    /// Any other 4xx, or a malformed response. Retrying will not help.
    Permanent(&'static str),
}

/// The REST surface this driver uses.
///
/// `SupabaseRest` is the production implementation — the adapter is in
/// [`super::adapters`]. The trait exists so that the recovery rules above can
/// be exercised against a fake with no HTTP anywhere; every 401, 429 and
/// tombstone assertion in this module's tests would otherwise need a live
/// backend or a stub server, and a rule that is only asserted end to end tends
/// not to be asserted at all.
pub trait RestApi: Send + Sync {
    /// Rows with `created_at >= since_ms`, **oldest first**, at most `limit` of
    /// them.
    ///
    /// Inclusive, for the reason on
    /// [`CloudSource::local_changes_since`](super::CloudSource::local_changes_since).
    /// The ordering must be total and tie-free — `(created_at, item_id)`, not
    /// `created_at` alone — or a burst sharing one millisecond is silently lost
    /// (manifest 05 INV-N1).
    ///
    /// **Oldest first is a hard requirement, not a preference.** The cursor is
    /// a lower bound that moves forward, so a newest-first page cannot be
    /// drained: taking the newest `limit` rows and advancing past them skips
    /// everything older that has not been seen yet, and *not* advancing means
    /// the same page comes back forever. That is exactly the shape of the
    /// original watermark bug in manifest 05 §4.4 —
    /// `order=wall_time.desc&limit=20` re-fetched the same newest twenty rows
    /// on every tick and older history never downloaded at all. The fix that
    /// manifest records is `order=…asc` with a compound keyset.
    ///
    /// [`CloudSync::pull`](super::CloudSync::pull) sorts each page defensively
    /// and warns if one arrives out of order, so a violation is loud rather
    /// than silent — but it cannot repair it.
    fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<CloudItem>, TransportFault>> + Send;

    /// Upsert on `item_id`, so re-sending a row is a no-op rather than a
    /// conflict. Every column that has a server-side default must be sent
    /// explicitly, `deleted` above all: omitting it lets the column default
    /// fire and resurrects a tombstoned item (manifest 05 T-5,
    /// `CopyPaste-kgs7`).
    fn upsert(
        &self,
        token: &str,
        items: &[CloudItem],
    ) -> impl Future<Output = Result<(), TransportFault>> + Send;

    /// Mark rows deleted. A *tombstone*, not a row removal, and it must clear
    /// the ciphertext rather than leave a stale one behind (manifest 05 T-4).
    fn tombstone(
        &self,
        token: &str,
        item_ids: &[String],
    ) -> impl Future<Output = Result<(), TransportFault>> + Send;
}

/// Why a token refresh failed.
///
/// The two credential cases are kept apart because their recovery differs
/// completely, and GoTrue's error *body* cannot tell them apart — it returns
/// `invalid_grant` for both. The grant kind is what disambiguates, which is
/// `auth.rs`'s job (manifest 05 §4.6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFault {
    /// The stored password is wrong. A human has to fix this.
    InvalidCredentials,
    /// The refresh token expired or was revoked. A full sign-in can recover.
    SessionExpired,
    /// GoTrue is throttling.
    RateLimited { retry_after: Option<Duration> },
    /// Network or 5xx.
    Unavailable(&'static str),
}

/// The auth surface this driver uses. See [`RestApi`] for why it is a trait.
pub trait AuthApi: Send + Sync {
    /// Exchange a refresh token for a new session.
    ///
    /// The rotated refresh token in the returned [`Session`] must be kept —
    /// GoTrue rotates it on every refresh, and reusing the old one fails.
    fn refresh(
        &self,
        refresh_token: &str,
    ) -> impl Future<Output = Result<Session, AuthFault>> + Send;
}
