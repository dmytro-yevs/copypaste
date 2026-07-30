//! The driver value itself: what it holds, how it is assembled, and the one
//! method that needs both paths at once.
//!
//! The push and pull paths, the recovery rules and the cadence are separate
//! files; each is an `impl` block on this type. What lives here is the state
//! they share — the session behind its mutex, the key, the guard — and the
//! ordering constraint between them ([`CloudSync::sync`]).

use std::sync::Mutex;
use std::time::Duration;

use super::cadence::MIN_POLL_INTERVAL;
use super::outcome::{SyncError, SyncStats};
use super::source::{CloudSource, SensitiveGuard};
use super::transport::{AuthApi, RestApi};
use crate::auth::Session;
use crate::crypto::SyncKey;
use crate::CloudConfig;

/// Push, pull, and the cadence in between.
///
/// Generic over the two transports so the recovery rules can be tested against
/// fakes with no HTTP in the picture; the production instantiation is
/// `CloudSync<SupabaseRest, SupabaseAuth>`.
pub struct CloudSync<R: RestApi, A: AuthApi> {
    pub(super) rest: R,
    pub(super) auth: A,
    pub(super) key: SyncKey,
    config: CloudConfig,
    /// The live session. Behind a mutex because a 401 on any request rotates it
    /// and every subsequent request must see the new bearer. Never held across
    /// an await.
    pub(super) session: Mutex<Session>,
    pub(super) sensitive: SensitiveGuard,
    pub(super) idle: Mutex<Duration>,
    /// Scale applied to every retry sleep. `1.0` in production.
    ///
    /// The tests set it to zero so the recovery rules can be asserted without
    /// wall-clock sleeps: the workspace does not enable tokio's `test-util`, so
    /// the clock cannot be paused. The *duration* the 429 rule computes is
    /// asserted separately and directly, against
    /// [`rate_limit_delay`](super::retry::rate_limit_delay), which is pure.
    delay_scale: f64,
}

impl<R: RestApi, A: AuthApi> std::fmt::Debug for CloudSync<R, A> {
    /// Redacted in full: this type holds a session and a key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudSync").finish_non_exhaustive()
    }
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Assemble a driver.
    ///
    /// `sensitive` is not optional; see [`SensitiveGuard`].
    pub fn new(
        rest: R,
        auth: A,
        key: SyncKey,
        config: CloudConfig,
        session: Session,
        sensitive: SensitiveGuard,
    ) -> Self {
        Self {
            rest,
            auth,
            key,
            config,
            session: Mutex::new(session),
            sensitive,
            idle: Mutex::new(MIN_POLL_INTERVAL),
            delay_scale: 1.0,
        }
    }

    /// Retry immediately instead of sleeping. Tests only — see `delay_scale`.
    #[cfg(test)]
    pub(super) fn without_retry_delays(mut self) -> Self {
        self.delay_scale = 0.0;
        self
    }

    /// The deployment this driver talks to.
    pub fn config(&self) -> &CloudConfig {
        &self.config
    }

    /// Read the live session — the access token, and the refresh token as
    /// rotated by the last refresh.
    ///
    /// A borrow rather than a clone, so that persisting the rotated refresh
    /// token does not require [`Session`] to be `Clone` and does not leave a
    /// second copy of a bearer lying around for the caller to forget about.
    pub fn inspect_session<T>(&self, f: impl FnOnce(&Session) -> T) -> T {
        f(&self.lock_session())
    }

    /// Push, then pull, then adjust the idle cadence.
    ///
    /// Push first so that a local delete reaches the backend before this device
    /// asks for rows — otherwise a tombstone written a moment ago can be
    /// shadowed by the live version another device is still serving, and the
    /// user watches a deleted item come back for one tick.
    ///
    /// # Errors
    ///
    /// As [`CloudSync::push`] and [`CloudSync::pull`]. A push failure aborts
    /// before the pull: if the session is dead, the pull would only fail the
    /// same way.
    pub async fn sync(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        let pushed = self.push(source).await?;
        let pulled = self.pull(source).await?;
        let stats = pushed.merge(pulled);

        self.note_activity(stats.changed());
        Ok(stats)
    }

    // -- lock and sleep helpers ---------------------------------------------
    //
    // A poisoned mutex means some other task panicked while holding it. The
    // data behind it is a session and a duration; neither can be left in a
    // torn state by a panic, and refusing to sync from then on would be a worse
    // outcome than continuing.

    /// Sleep for a scaled retry delay. Zero-length sleeps are skipped entirely
    /// so a test does not pay a scheduler round trip per retry.
    pub(super) async fn wait(&self, delay: Duration) {
        let scaled = delay.mul_f64(self.delay_scale);
        if !scaled.is_zero() {
            tokio::time::sleep(scaled).await;
        }
    }

    pub(super) fn lock_session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn lock<'a, T>(&self, m: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::super::fakes::{allow_everything, driver, FakeAuth, FakeRest};

    #[test]
    fn the_driver_and_the_guard_redact_their_debug_output() {
        let sync = driver(FakeRest::default(), FakeAuth::default());
        assert_eq!(format!("{sync:?}"), "CloudSync { .. }");
        assert_eq!(format!("{:?}", allow_everything()), "SensitiveGuard { .. }");
    }
}
