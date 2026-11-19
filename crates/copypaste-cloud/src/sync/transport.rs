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
use crate::rest::{CloudItem, RowListing, RowPage};

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

/// The REST surface this driver uses. `SupabaseRest` is the production
/// implementation, via the adapter in [`super::adapters`].
pub trait RestApi: Send + Sync {
    fn fetch_group_catalog(
        &self,
        _token: &str,
        _after_device: Option<&str>,
    ) -> impl Future<Output = Result<Vec<crate::rest::group::GroupVersion>, TransportFault>> + Send
    {
        async {
            Err(TransportFault::Permanent(
                "device group transport is unavailable",
            ))
        }
    }
    fn fetch_groups(
        &self,
        _token: &str,
        _device_ids: &[String],
    ) -> impl Future<Output = Result<Vec<crate::rest::GroupRow>, TransportFault>> + Send {
        async {
            Err(TransportFault::Permanent(
                "device group transport is unavailable",
            ))
        }
    }
    fn upsert_group(
        &self,
        _token: &str,
        _row: &crate::rest::GroupRow,
    ) -> impl Future<Output = Result<(), TransportFault>> + Send {
        async {
            Err(TransportFault::Permanent(
                "device group transport is unavailable",
            ))
        }
    }
    /// `None` when the backend has no space schema yet; the passphrase key
    /// then stays the only key.
    fn fetch_space_envelopes(
        &self,
        _token: &str,
        _device_id: &str,
    ) -> impl Future<
        Output = Result<Option<Vec<copypaste_sync::space::SpaceEnvelope>>, TransportFault>,
    > + Send {
        async { Ok(None) }
    }
    fn fetch_space_index(
        &self,
        _token: &str,
        _epoch: u64,
    ) -> impl Future<Output = Result<Vec<crate::rest::space::SpaceIndexEntry>, TransportFault>> + Send
    {
        async { Ok(Vec::new()) }
    }
    fn insert_space_envelopes(
        &self,
        _token: &str,
        _rows: &[copypaste_sync::space::SpaceEnvelope],
    ) -> impl Future<Output = Result<(), TransportFault>> + Send {
        async { Ok(()) }
    }
    fn delete_space_envelopes(
        &self,
        _token: &str,
        _sender: &str,
        _before_epoch: u64,
    ) -> impl Future<Output = Result<(), TransportFault>> + Send {
        async { Ok(()) }
    }
    /// One page of a listing in the order the backend placed the rows,
    /// **oldest first**, at most `limit` of them, after a keyset cursor. A
    /// listing carries no payload.
    ///
    /// `position` is the backend's own, never a device's stamp: a row written
    /// late or under a wrong clock still lands after everything accepted
    /// before it, so a cursor that only moves forward cannot leave it behind.
    ///
    /// `after_item_id` is the tie-break half of the cursor. With it, the page
    /// starts strictly after the pair `(position, after_item_id)`; without it,
    /// the bound is inclusive on `position` alone.
    ///
    /// The ordering must be total and tie-free — `(position, item_id)` — and
    /// the cursor expressed in the same pair, or a batch that shares one
    /// position and outgrows a page can never be paged past (INV-N1).
    ///
    /// **Oldest first is a hard requirement, not a preference.** A forward
    /// cursor cannot drain a newest-first page: advancing past it skips
    /// everything older, and not advancing returns it forever (manifest 05
    /// §4.4).
    fn list_since(
        &self,
        token: &str,
        position: i64,
        after_item_id: Option<&str>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<RowListing>, TransportFault>> + Send;

    /// The rows with these ids, whole. A page may end early at a byte budget,
    /// and [`RowPage::cut`] says so: an id missing from a page that was not
    /// cut no longer exists, one missing from a cut page was not reached.
    fn fetch_rows(
        &self,
        token: &str,
        item_ids: &[String],
    ) -> impl Future<Output = Result<RowPage, TransportFault>> + Send;

    /// Upsert on `item_id`, so re-sending a row is a no-op rather than a
    /// conflict. Every column that has a server-side default must be sent
    /// explicitly, `deleted` above all: omitting it lets the column default
    /// fire and resurrects a tombstoned item (manifest 05 T-5,
    /// `CopyPaste-kgs7`).
    ///
    /// **The only write path.** A tombstone is an ordinary row on it — one with
    /// `deleted = true` and no payload (T-4) — rather than a partial update of
    /// an existing row. Two reasons, either sufficient: manifest 05 §7.5 asked
    /// for one upsert and one code path, and a partial write cannot carry a
    /// metadata signature that covers the columns it does not send.
    fn upsert(
        &self,
        token: &str,
        items: &[CloudItem],
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
