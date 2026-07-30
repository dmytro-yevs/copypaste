//! Session recovery: the only place in this module that reacts to a 401 or a
//! 429, and the only place that decides *not* to react to a 5xx.
//!
//! Keeping all three rules in one file is the point. Each of them is
//! single-shot, and a single-shot guard that is written twice is a guard that
//! is eventually written once — which is how a refresh loop becomes an infinite
//! one.

use std::future::Future;
use std::time::Duration;

use super::driver::CloudSync;
use super::outcome::SyncError;
use super::transport::{AuthApi, AuthFault, RestApi, TransportFault};

/// What to wait when a 429 arrives with no `Retry-After` header.
pub(super) const RETRY_AFTER_FALLBACK: Duration = Duration::from_secs(1);

/// Ceiling applied to a server-supplied `Retry-After`.
///
/// The header is honoured, but a server that asks for an hour does not get to
/// pin the loop for an hour; manifest 05 §4.6.3 clamps it to the maximum
/// backoff, which is thirty seconds. The retry is single-shot regardless, so a
/// server stuck on 429 surfaces an error rather than blocking forever.
pub(super) const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Run one request with the current bearer, recovering from a 401 or a 429
    /// exactly once each.
    ///
    /// The single-shot guards are the point (manifest 05 §4.6.3):
    ///
    /// * **401** — refresh the session, retry once. A second 401 is a hard
    ///   error; without the guard, a refresh that hands back a token the server
    ///   still rejects loops forever.
    /// * **429** — sleep for `Retry-After` (clamped), retry once. A second 429
    ///   is a hard error, so a server stuck on 429 cannot pin the caller.
    /// * **5xx / network** — surfaced, **not** retried here. Manifest 05 §4.6.3
    ///   asks for a 1 s → 30 s ladder capped at four attempts, and `rest.rs`
    ///   and `auth.rs` already implement exactly that, once, behind
    ///   `transient_backoff()`. A transient fault reaching this function has
    ///   already spent that budget; retrying it again would be a second
    ///   scheduler for one condition — the shape of duplication `CLAUDE.md`
    ///   rule 1 exists to prevent — and would turn four attempts into sixteen.
    ///   The outer retry is the poll cadence.
    /// * **anything else** — give up. Retrying a 400 does not make it a 200.
    ///
    /// The loop therefore runs at most three times: the first attempt, one
    /// post-refresh retry, and one post-`Retry-After` retry. It cannot spin.
    pub(super) async fn execute<T, F, Fut>(&self, op: F) -> Result<T, SyncError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<T, TransportFault>>,
    {
        let mut refreshed = false;
        let mut waited_on_429 = false;

        loop {
            let token = self.lock_session().access_token.clone();

            match op(token).await {
                Ok(value) => return Ok(value),

                Err(TransportFault::Unauthorized) if !refreshed => {
                    refreshed = true;
                    tracing::debug!("bearer rejected; refreshing the session once");
                    self.refresh_session().await?;
                }
                Err(TransportFault::Unauthorized) => return Err(SyncError::Unauthorized),

                Err(TransportFault::RateLimited { retry_after }) if !waited_on_429 => {
                    waited_on_429 = true;
                    let delay = rate_limit_delay(retry_after);
                    tracing::debug!(
                        delay_ms = delay.as_millis(),
                        "backend asked us to slow down"
                    );
                    self.wait(delay).await;
                }
                Err(TransportFault::RateLimited { .. }) => return Err(SyncError::RateLimited),

                Err(TransportFault::Transient(reason) | TransportFault::Permanent(reason)) => {
                    return Err(SyncError::Transport(reason))
                }
            }
        }
    }

    /// Exchange the refresh token for a new session and install it.
    ///
    /// Prefers the refresh grant and never falls back to a password sign-in or
    /// to the anonymous key from inside a request: a silent downgrade to a
    /// lower-privilege scope masks credential rotation, a misconfiguration, or
    /// an attack (INV-N6). If the refresh token itself has aged out, that
    /// surfaces as [`SyncError::SessionExpired`] and the sign-in is the
    /// caller's decision to make.
    async fn refresh_session(&self) -> Result<(), SyncError> {
        let refresh_token = self.lock_session().refresh_token.clone();

        match self.auth.refresh(&refresh_token).await {
            Ok(session) => {
                // Keep the rotated refresh token: GoTrue rotates on every
                // refresh and the old one stops working.
                *self.lock_session() = session;
                Ok(())
            }
            Err(AuthFault::InvalidCredentials) => Err(SyncError::InvalidCredentials),
            Err(AuthFault::SessionExpired) => Err(SyncError::SessionExpired),
            Err(AuthFault::RateLimited { .. }) => Err(SyncError::RateLimited),
            Err(AuthFault::Unavailable(reason)) => Err(SyncError::Transport(reason)),
        }
    }
}

/// How long to honour a 429 for.
///
/// The server's `Retry-After` if it sent one, clamped to [`MAX_RETRY_AFTER`];
/// otherwise [`RETRY_AFTER_FALLBACK`], so a 429 without a header still slows
/// down rather than hammering. Pure, so the clamp can be asserted without
/// sleeping.
pub(super) fn rate_limit_delay(retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or(RETRY_AFTER_FALLBACK)
        .min(MAX_RETRY_AFTER)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::super::fakes::{cloud_row, driver, item, FakeAuth, FakeRest, FakeSource, Reply};
    use super::*;

    #[tokio::test]
    async fn a_401_refreshes_once_and_retries_once() {
        // AT-34. And the *refreshed* bearer must be the one used on the retry.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(sync.auth.refreshes.load(Ordering::SeqCst), 1);
        let tokens = sync.rest.tokens.lock().unwrap();
        assert_eq!(tokens.as_slice(), ["token-1", "token-refreshed"]);
    }

    #[tokio::test]
    async fn a_second_401_is_a_hard_error_not_a_loop() {
        // AT-36. A refresh that hands back a token the server still rejects
        // must not spin.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Unauthorized, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Unauthorized
        );
        assert_eq!(
            sync.auth.refreshes.load(Ordering::SeqCst),
            1,
            "refreshed more than once"
        );
    }

    #[tokio::test]
    async fn the_401_path_is_the_same_on_the_read_side() {
        // The read path was originally folded into a generic error bucket, so
        // an expired token stalled downloads while uploads kept working.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Ok]);
        rest.rows
            .lock()
            .unwrap()
            .insert("a".into(), cloud_row("a", 1_000, "x"));
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(stats.applied, 1);
        assert_eq!(sync.auth.refreshes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn bad_credentials_and_an_expired_session_are_distinguished() {
        // They need completely different recovery: one prompts a human, the
        // other re-authenticates silently. GoTrue's body cannot tell them
        // apart, which is why the classification comes from the grant kind.
        for (fault, expected) in [
            (AuthFault::InvalidCredentials, SyncError::InvalidCredentials),
            (AuthFault::SessionExpired, SyncError::SessionExpired),
        ] {
            let rest = FakeRest::scripted(vec![Reply::Unauthorized]);
            let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
            let sync = driver(rest, FakeAuth::failing(fault));

            assert_eq!(sync.push(&source).await.unwrap_err(), expected);
        }
    }

    #[tokio::test]
    async fn auth_failure_never_leaks_a_token_or_falls_back() {
        // INV-N6: no silent downgrade to a lower-privilege scope, and the anon
        // key never appears in an error.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::failing(AuthFault::SessionExpired));

        let err = sync.push(&source).await.unwrap_err();
        let rendered = err.to_string();
        assert!(!rendered.contains("anon"));
        assert!(!rendered.contains("token-1"));
        assert!(!rendered.contains("refresh-1"));
        // And nothing was uploaded under a degraded identity.
        assert!(sync.rest.rows.lock().unwrap().is_empty());
    }

    #[test]
    fn a_retry_after_is_honoured_and_clamped() {
        // AT-39, asserted on the value rather than on elapsed wall-clock time:
        // the workspace does not enable tokio's `test-util`, so there is no
        // paused clock, and a test that really slept for the server's hint
        // would be a test that really sleeps.
        assert_eq!(
            rate_limit_delay(Some(Duration::from_secs(7))),
            Duration::from_secs(7),
            "Retry-After was not honoured"
        );
        // A server asking for an hour does not get to hold the loop for one.
        assert_eq!(
            rate_limit_delay(Some(Duration::from_secs(3_600))),
            MAX_RETRY_AFTER
        );
        // No header: still slow down rather than retrying flat out.
        assert_eq!(rate_limit_delay(None), RETRY_AFTER_FALLBACK);
        assert_eq!(rate_limit_delay(Some(Duration::ZERO)), Duration::ZERO);
    }

    #[tokio::test]
    async fn a_429_is_retried_exactly_once() {
        let rest = FakeRest::scripted(vec![
            Reply::RateLimited(Some(Duration::from_secs(7))),
            Reply::Ok,
        ]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(sync.push(&source).await.unwrap().uploaded, 1);

        // Twice in a row is an error, not a second wait: a server stuck on 429
        // must not be able to pin the loop.
        let rest = FakeRest::scripted(vec![
            Reply::RateLimited(Some(Duration::from_secs(1))),
            Reply::RateLimited(Some(Duration::from_secs(1))),
            Reply::Ok,
        ]);
        let sync = driver(rest, FakeAuth::default());
        assert_eq!(
            sync.push(&source).await.unwrap_err(),
            SyncError::RateLimited
        );
    }

    #[tokio::test]
    async fn a_429_without_a_header_still_recovers() {
        let rest = FakeRest::scripted(vec![Reply::RateLimited(None), Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(sync.push(&source).await.unwrap().uploaded, 1);
    }

    #[tokio::test]
    async fn a_transient_failure_does_not_start_a_second_retry_ladder() {
        // The 1 s → 30 s × 4 ladder lives in `rest.rs` and `auth.rs`, once. A
        // transient fault arriving here has already spent it, so the driver
        // surfaces it instead of multiplying four attempts into sixteen. The
        // poll cadence is the outer retry.
        let rest = FakeRest::scripted(vec![Reply::Transient, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert!(matches!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Transport(_)
        ));
        assert_eq!(
            sync.rest.tokens.lock().unwrap().len(),
            1,
            "the request was attempted more than once"
        );
    }

    #[tokio::test]
    async fn no_recovery_path_can_spin() {
        // The whole recovery loop is bounded at three attempts: the first, one
        // post-refresh retry, and one post-Retry-After retry. Script every
        // recoverable fault back to back and assert the count.
        let rest = FakeRest::scripted(vec![
            Reply::Unauthorized,
            Reply::RateLimited(None),
            Reply::Unauthorized,
            Reply::Ok,
        ]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Unauthorized
        );
        assert_eq!(sync.rest.tokens.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_permanent_failure_is_not_retried() {
        let rest = FakeRest::scripted(vec![Reply::Permanent, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert!(matches!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Transport(_)
        ));
        assert_eq!(sync.rest.upserts.load(Ordering::SeqCst), 0);
    }
}
