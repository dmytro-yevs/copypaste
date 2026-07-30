//! Fixtures shared by this module's tests.

use std::time::Duration;

use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};

use super::stub::Stub;
use super::SupabaseAuth;
use crate::CloudConfig;

pub(super) const ANON: &str = "anon-key-abc";

/// The body GoTrue returns for a wrong password *and* for a dead refresh
/// token. Byte-identical on purpose: it is the whole point.
pub(super) const INVALID_GRANT: &str =
    r#"{"error":"invalid_grant","error_description":"Invalid login credentials"}"#;

/// A retry policy that finishes in milliseconds, so a test that exercises
/// the transient path does not sleep for seconds.
pub(super) fn fast_retry() -> ExponentialBackoff {
    ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(1))
        .with_max_interval(Duration::from_millis(2))
        .with_max_elapsed_time(Some(Duration::from_millis(10)))
        .build()
}

pub(super) fn client(stub: &Stub) -> SupabaseAuth {
    SupabaseAuth::new(CloudConfig {
        url: stub.base_url.clone(),
        anon_key: ANON.to_string(),
    })
    .with_retry_policy(fast_retry())
}

pub(super) fn session_body(access: &str, refresh: &str, expires_in: i64) -> String {
    format!(
        r#"{{"access_token":"{access}","refresh_token":"{refresh}",
             "token_type":"bearer","expires_in":{expires_in},
             "user":{{"id":"user-uuid-1","email":"a@example.com"}}}}"#
    )
}
