use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use super::cadence::AdaptiveCadence;
use super::outcome::{SyncError, SyncStats};
use super::source::{CloudSource, SensitiveGuard};
use super::transport::{AuthApi, RestApi};
use crate::auth::Session;
use crate::crypto::SyncKey;
use crate::CloudConfig;
use zeroize::Zeroizing;

pub struct CloudSync<R: RestApi, A: AuthApi> {
    pub(super) rest: R,
    pub(super) auth: A,
    pub(super) key: SyncKey,
    config: CloudConfig,
    // A 401 rotates this for every later request; never hold it across an await.
    session: Mutex<SessionState>,
    pub(super) refresh_lock: tokio::sync::Mutex<()>,
    pub(super) sensitive: SensitiveGuard,
    pub(super) cadence: AdaptiveCadence,
    pub(super) push_channel: AtomicBool,
    // Tests set this to zero; retry duration math is exercised separately.
    delay_scale: f64,
}

struct SessionState {
    session: Session,
    revision: u64,
    fenced: bool,
}

pub(super) enum SessionRevision {
    Current,
    Superseded,
    Fenced,
}

impl<R: RestApi, A: AuthApi> std::fmt::Debug for CloudSync<R, A> {
    /// Redacted in full: this type holds a session and a key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudSync").finish_non_exhaustive()
    }
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Requires a sensitivity gate: live sensitive payloads must never sync
    /// (manifest 05 T-7).
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
            session: Mutex::new(SessionState {
                session,
                revision: 0,
                fenced: false,
            }),
            refresh_lock: tokio::sync::Mutex::new(()),
            sensitive,
            cadence: AdaptiveCadence::default(),
            push_channel: AtomicBool::new(false),
            delay_scale: 1.0,
        }
    }

    #[cfg(test)]
    pub(super) fn without_retry_delays(mut self) -> Self {
        self.delay_scale = 0.0;
        self
    }

    pub fn config(&self) -> &CloudConfig {
        &self.config
    }

    /// Borrows the rotated session so callers do not retain another bearer
    /// copy.
    pub fn inspect_session<T>(&self, f: impl FnOnce(&Session) -> T) -> T {
        f(&self.lock_session().session)
    }

    /// Identifies the session generation an auth outcome belongs to.
    pub fn session_revision(&self) -> u64 {
        self.lock_session().revision
    }

    pub(super) fn refresh_snapshot(&self) -> Option<(u64, Zeroizing<String>)> {
        let state = self.lock_session();
        (!state.fenced).then(|| {
            (
                state.revision,
                Zeroizing::new(state.session.refresh_token.clone()),
            )
        })
    }

    /// Stops refresh results from being installed, optionally only while the
    /// named generation is current. Sign-out uses an unconditional fence.
    pub fn fence_session(&self, expected_revision: Option<u64>) -> bool {
        let mut state = self.lock_session();
        if expected_revision.is_some_and(|expected| state.revision != expected) {
            return false;
        }
        state.fenced = true;
        true
    }

    pub(super) fn revision_state(&self, expected: u64) -> SessionRevision {
        let state = self.lock_session();
        if state.fenced {
            SessionRevision::Fenced
        } else if state.revision == expected {
            SessionRevision::Current
        } else {
            SessionRevision::Superseded
        }
    }

    pub(super) fn install_refreshed_session(
        &self,
        expected_revision: u64,
        session: Session,
    ) -> SessionRevision {
        let mut state = self.lock_session();
        if state.fenced {
            return SessionRevision::Fenced;
        }
        if state.revision != expected_revision {
            return SessionRevision::Superseded;
        }
        state.session = session;
        state.revision = state.revision.wrapping_add(1);
        SessionRevision::Current
    }

    /// Pushes first so a new tombstone cannot be briefly shadowed by a stale
    /// live row (INV-N2). Retained local LWW winners are then republished
    /// because backend upserts resolve same-item writes by arrival.
    pub async fn sync(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        let pushed = self.push(source).await?;
        let (pulled, republish) = self.pull_with_republish(source).await?;
        let stats = if republish {
            pushed.merge(pulled).merge(self.push(source).await?)
        } else {
            pushed.merge(pulled)
        };

        self.note_activity(stats.changed());
        Ok(stats)
    }

    pub(super) async fn wait(&self, delay: Duration) {
        let scaled = delay.mul_f64(self.delay_scale);
        if !scaled.is_zero() {
            tokio::time::sleep(scaled).await;
        }
    }

    // Panics cannot tear these values; poison must not disable sync permanently.
    fn lock_session(&self) -> MutexGuard<'_, SessionState> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::super::fakes::{
        allow_everything, cloud_row, driver, item, FakeAuth, FakeRest, FakeSource,
    };

    #[test]
    fn the_driver_and_the_guard_redact_their_debug_output() {
        let sync = driver(FakeRest::default(), FakeAuth::default());
        assert_eq!(format!("{sync:?}"), "CloudSync { .. }");
        assert_eq!(format!("{:?}", allow_everything()), "SensitiveGuard { .. }");
    }

    #[tokio::test]
    async fn a_local_lww_winner_is_republished_after_pull() {
        let local = item("shared", 5, "local winner");
        let source = FakeSource::with_local(local);
        source.set_upload_floor(6);
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("shared", 4, "stale remote")]),
            FakeAuth::default(),
        );

        let stats = sync.sync(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(sync.rest.rows.lock().unwrap()["shared"].created_at, 5);
    }

    #[tokio::test]
    async fn an_ordinary_round_does_not_push_its_rows_twice() {
        let source = FakeSource::with_outgoing(vec![item("a", 1, "ordinary")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.sync(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(
            sync.rest.upserts.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
