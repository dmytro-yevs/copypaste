//! The production transports.
//!
//! Two adapters, each a `match` from a sibling's error enum onto the four things
//! this driver knows how to do about a failure. All the behaviour is in the
//! other files; this is the translation.

use std::time::Duration;

use super::transport::{AuthApi, AuthFault, RestApi, TransportFault};
use crate::auth::Session;
use crate::rest::CloudItem;

/// `RestError` -> [`TransportFault`].
///
/// The interesting arm is the transient one. `SupabaseRest` retries `Network`
/// and `Server` under the crate's single `backon` policy *before* surfacing
/// them, so by the time one arrives here the 1 s → 30 s × 4 budget that
/// manifest 05 §4.6.3 asks for has already been spent. It is still classified
/// as [`TransportFault::Transient`] — that is what it is, and it is what a log
/// or a status UI wants to say — and
/// [`CloudSync::execute`](super::CloudSync::execute) deliberately does not
/// retry it, so there is exactly one transient ladder in the crate.
fn classify_rest(err: crate::rest::RestError) -> TransportFault {
    use crate::rest::RestError as E;
    match err {
        E::Unauthorized => TransportFault::Unauthorized,
        E::RateLimited { retry_after_secs } => TransportFault::RateLimited {
            retry_after: retry_after_secs.map(Duration::from_secs),
        },
        E::Network(_) => TransportFault::Transient("could not reach the sync backend"),
        E::Server { .. } => TransportFault::Transient("the sync backend faulted"),
        // Refreshing the token cannot fix RLS refusing the row, a missing
        // unique index behind the upsert's conflict target, a malformed
        // response, or an item this client refused to send.
        E::Forbidden => TransportFault::Permanent("the account may not touch these rows"),
        E::Rejected { .. } => TransportFault::Permanent("the sync backend rejected the request"),
        E::Malformed => {
            TransportFault::Permanent("the sync backend returned an unexpected response")
        }
        E::InvalidItem { reason } => TransportFault::Permanent(reason),
    }
}

impl RestApi for crate::rest::SupabaseRest {
    async fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        after_item_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CloudItem>, TransportFault> {
        crate::rest::SupabaseRest::fetch_since(self, token, since_ms, after_item_id, limit)
            .await
            .map_err(classify_rest)
    }

    async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
        crate::rest::SupabaseRest::upsert(self, token, items)
            .await
            .map(|_| ())
            .map_err(classify_rest)
    }

    async fn tombstone(&self, token: &str, item_ids: &[String]) -> Result<(), TransportFault> {
        crate::rest::SupabaseRest::tombstone(self, token, item_ids)
            .await
            .map(|_| ())
            .map_err(classify_rest)
    }
}

/// `AuthError` -> [`AuthFault`].
///
/// The two credential cases stay apart: GoTrue answers `invalid_grant` for
/// both a bad password and a dead refresh token, and `auth.rs` has already
/// disambiguated them by grant kind (manifest 05 §4.6.1). Collapsing them here
/// would throw that away and tell a user to retype a password that was fine.
fn classify_auth(err: crate::auth::AuthError) -> AuthFault {
    use crate::auth::AuthError as E;
    match err {
        E::InvalidCredentials => AuthFault::InvalidCredentials,
        // All four mean "this session cannot be revived by refreshing": the
        // token is dead, the account is unconfirmed, or the service answered
        // something a refresh cannot use. Sign-in is the recovery, and it is
        // the caller's decision to make, never a silent downgrade (INV-N6).
        E::SessionExpired | E::EmailConfirmationRequired => AuthFault::SessionExpired,
        E::Rejected { .. } | E::Malformed => AuthFault::SessionExpired,
        E::RateLimited { retry_after_secs } => AuthFault::RateLimited {
            retry_after: retry_after_secs.map(Duration::from_secs),
        },
        E::Network(_) => AuthFault::Unavailable("could not reach the account service"),
        E::Server { .. } => AuthFault::Unavailable("the account service faulted"),
    }
}

impl AuthApi for crate::auth::SupabaseAuth {
    async fn refresh(&self, refresh_token: &str) -> Result<Session, AuthFault> {
        crate::auth::SupabaseAuth::refresh(self, refresh_token)
            .await
            .map_err(classify_auth)
    }
}
