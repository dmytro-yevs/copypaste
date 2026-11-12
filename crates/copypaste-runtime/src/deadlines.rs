//! One wall-clock deadline task for the whole runtime, with no polling.
//!
//! Deadlines come from two places: one-shot timers that features schedule in
//! memory (a clipboard restore, an idle stop, an approval timeout), and rows
//! the store holds (clip expiry, the purge of sealed originals, 30-day
//! retention). The task sleeps until the earliest one, but only within the
//! short horizon of one hour. Anything later holds no timer at all: it is
//! picked up by a sweep at start, on system resume, on sync receipt and on the
//! first request after a quiet spell, each of which re-arms the task. With no
//! deadline the task holds no timer. Tokio sleeps on a clock that may stop
//! while the system sleeps, so hosts report resume; only when they cannot is
//! a short-horizon sleep capped at one minute.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use copypaste_core::now_ms;
use tokio::sync::{watch, Notify};

use crate::Runtime;

/// Deadlines further away than this hold no timer.
pub(crate) const SHORT_HORIZON_MS: i64 = 60 * 60 * 1000;
/// The sleep cap while no resume signal is available.
const FALLBACK_SLEEP: Duration = Duration::from_secs(60);
/// A request after this long without a sweep starts one.
const ACTIVITY_SWEEP_MS: i64 = 15 * 60 * 1000;
const UNARMED: i64 = i64::MIN;

/// Work run on the deadline task's blocking worker when its time comes.
pub(crate) type DeadlineAction = Box<dyn FnOnce(&Runtime) + Send>;

/// Names one in-memory deadline. Scheduling an existing key replaces it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DeadlineKey {
    owner: &'static str,
    id: String,
}

impl DeadlineKey {
    pub(crate) fn new(owner: &'static str, id: impl Into<String>) -> Self {
        Self {
            owner,
            id: id.into(),
        }
    }
}

struct Timer {
    at: i64,
    action: DeadlineAction,
}

#[derive(Default)]
struct State {
    timers: BTreeMap<DeadlineKey, Timer>,
    /// The earliest future store deadline found by the last sweep.
    store_next: Option<i64>,
    sweep_requested: bool,
    last_sweep: Option<i64>,
}

/// What is due at one instant.
struct Due {
    actions: Vec<DeadlineAction>,
    sweep: bool,
}

pub(crate) struct Deadlines {
    state: Mutex<State>,
    notify: Notify,
    resume_signal: AtomicBool,
    started: AtomicBool,
    /// The wall-clock instant the task sleeps until, or [`UNARMED`].
    armed: AtomicI64,
    /// Added to the wall clock, so tests can let time pass.
    skew: AtomicI64,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            notify: Notify::new(),
            resume_signal: AtomicBool::new(true),
            started: AtomicBool::new(false),
            armed: AtomicI64::new(UNARMED),
            skew: AtomicI64::new(0),
        }
    }
}

impl Deadlines {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn now(&self) -> i64 {
        now_ms().saturating_add(self.skew.load(Ordering::Relaxed))
    }

    /// Run `action` at wall-clock `at`, replacing any deadline under `key`.
    pub(crate) fn schedule(&self, key: DeadlineKey, at: i64, action: DeadlineAction) {
        self.lock().timers.insert(key, Timer { at, action });
        self.notify.notify_one();
    }

    pub(crate) fn cancel(&self, key: &DeadlineKey) -> bool {
        let removed = self.lock().timers.remove(key).is_some();
        if removed {
            self.notify.notify_one();
        }
        removed
    }

    /// A store deadline was written; the task re-arms if it is the earliest.
    pub(crate) fn note_store_deadline(&self, at: i64) {
        if at <= self.now() {
            self.request_sweep();
            return;
        }
        let mut state = self.lock();
        state.store_next = Some(state.store_next.map_or(at, |next| next.min(at)));
        drop(state);
        self.notify.notify_one();
    }

    /// Ask the task for a sweep. Cheap and callable from any thread.
    pub(crate) fn request_sweep(&self) {
        self.lock().sweep_requested = true;
        self.notify.notify_one();
    }

    /// How long the task sleeps from `now`, or `None` for no timer.
    pub(crate) fn next_wait(&self, now: i64) -> Option<Duration> {
        let state = self.lock();
        let earliest = state
            .timers
            .values()
            .map(|timer| timer.at)
            .chain(state.store_next)
            .min()?;
        let remaining = earliest.saturating_sub(now);
        if remaining > SHORT_HORIZON_MS {
            return None;
        }
        let wait = Duration::from_millis(u64::try_from(remaining).unwrap_or(0));
        Some(if self.resume_signal.load(Ordering::Acquire) {
            wait
        } else {
            wait.min(FALLBACK_SLEEP)
        })
    }

    fn take_due(&self, now: i64) -> Due {
        let mut state = self.lock();
        let due: Vec<DeadlineKey> = state
            .timers
            .iter()
            .filter(|(_, timer)| timer.at <= now)
            .map(|(key, _)| key.clone())
            .collect();
        let actions = due
            .iter()
            .filter_map(|key| state.timers.remove(key))
            .map(|timer| timer.action)
            .collect();
        let sweep = std::mem::take(&mut state.sweep_requested)
            || state.store_next.is_some_and(|next| next <= now);
        if sweep {
            state.store_next = None;
        }
        Due { actions, sweep }
    }

    /// Record a finished sweep. A deadline noted while it ran is kept.
    fn swept(&self, now: i64, next: Option<i64>) {
        let mut state = self.lock();
        state.last_sweep = Some(now);
        state.store_next = state
            .store_next
            .into_iter()
            .chain(next)
            .filter(|at| *at > now)
            .min();
    }

    fn sweep_is_stale(&self, now: i64) -> bool {
        self.lock()
            .last_sweep
            .is_none_or(|last| now.saturating_sub(last) >= ACTIVITY_SWEEP_MS)
    }

    #[cfg(test)]
    pub(crate) fn advance(&self, ms: i64) {
        self.skew.fetch_add(ms, Ordering::Relaxed);
    }

    #[cfg(test)]
    fn armed(&self) -> Option<i64> {
        Some(self.armed.load(Ordering::Acquire)).filter(|at| *at != UNARMED)
    }
}

/// Wakes the deadline task when the system resumes from sleep. A host keeps
/// one and calls it from its native resume notification, on any thread.
#[derive(Clone)]
pub struct ResumeWaker(Arc<Deadlines>);

impl ResumeWaker {
    pub fn resumed(&self) {
        tracing::debug!("system resumed; sweeping deadlines");
        self.0.request_sweep();
    }
}

impl Runtime {
    /// Start the deadline task until [`Runtime::shutdown`]. Returns the task
    /// the first time and `None` while it runs. It sweeps once on start.
    pub fn start_deadlines(self: &Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        if self.deadlines.started.swap(true, Ordering::AcqRel) {
            return None;
        }
        self.deadlines.request_sweep();
        let shutdown = self.shutdown.subscribe();
        Some(tokio::spawn(Arc::clone(self).run_deadlines(shutdown)))
    }

    /// The handle a host's resume notification calls.
    pub fn resume_waker(&self) -> ResumeWaker {
        ResumeWaker(Arc::clone(&self.deadlines))
    }

    /// Whether the host delivers resume notifications. Without them a
    /// short-horizon sleep is capped, so a late wake is caught within a minute.
    pub fn set_resume_signal(&self, available: bool) {
        self.deadlines
            .resume_signal
            .store(available, Ordering::Release);
        self.deadlines.notify.notify_one();
    }

    /// Sweep now on the calling thread: store housekeeping, due expiries and
    /// due timers, then re-arm the task.
    pub fn sweep_deadlines(&self) {
        self.deadlines.request_sweep();
        self.fire_deadlines();
    }

    /// A request arrived. After a quiet spell this starts a sweep, which is
    /// how an Android process that was frozen catches up on its next use.
    pub(crate) fn note_activity(&self) {
        if !self.deadlines.sweep_is_stale(self.deadlines.now()) {
            return;
        }
        if self.deadlines.started.load(Ordering::Acquire) {
            self.deadlines.request_sweep();
        } else {
            self.sweep_deadlines();
        }
    }

    fn fire_deadlines(&self) {
        let now = self.deadlines.now();
        let due = self.deadlines.take_due(now);
        if due.sweep {
            self.sweep_store(now);
        }
        for action in due.actions {
            action(self);
        }
    }

    fn sweep_store(&self, now: i64) {
        match self.store.sweep_local_state(now) {
            Ok(swept) if swept != Default::default() => tracing::debug!(
                expired_ids = swept.expired_ids,
                rule_conflicts = swept.rule_conflicts,
                rule_originals = swept.rule_originals,
                orphan_notes = swept.orphan_notes,
                "deadline housekeeping"
            ),
            Ok(_) => {}
            Err(error) => tracing::warn!(?error, "deadline housekeeping failed"),
        }
        self.expire_due_clips(now);
        self.purge_expired_snippets(now);
        let next = self.store.next_local_deadline(now).unwrap_or_else(|error| {
            tracing::warn!(?error, "could not read the next deadline");
            None
        });
        self.deadlines.swept(now, next);
    }

    async fn run_deadlines(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        let deadlines = Arc::clone(&self.deadlines);
        while !*shutdown.borrow() {
            let runtime = Arc::clone(&self);
            if tokio::task::spawn_blocking(move || runtime.fire_deadlines())
                .await
                .is_err()
            {
                tracing::error!("a deadline action panicked");
            }
            let now = deadlines.now();
            let wait = deadlines.next_wait(now);
            deadlines.armed.store(
                wait.map_or(UNARMED, |wait| {
                    now.saturating_add(i64::try_from(wait.as_millis()).unwrap_or(i64::MAX))
                }),
                Ordering::Release,
            );
            let timer = async {
                match wait {
                    Some(wait) => tokio::time::sleep(wait).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                changed = shutdown.changed() => if changed.is_err() { break },
                () = deadlines.notify.notified() => {}
                () = timer => {}
            }
        }
        deadlines.armed.store(UNARMED, Ordering::Release);
        deadlines.started.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use copypaste_core::storage::local_state::ClipLocalState;

    use super::*;
    use crate::tests::fixture;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    fn counter(runtime: &Runtime, key: &str, at: i64) -> Arc<AtomicUsize> {
        let fired = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&fired);
        runtime.deadlines.schedule(
            DeadlineKey::new("test", key),
            at,
            Box::new(move |_: &Runtime| {
                count.fetch_add(1, Ordering::SeqCst);
            }),
        );
        fired
    }

    fn expiring_clip(runtime: &Runtime, expires_at: i64) {
        let item = copypaste_core::ingest::ingest(
            &runtime.store,
            &runtime.keyring,
            "expiring",
            copypaste_ipc::content_type::TEXT,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();
        assert!(runtime
            .store
            .write_clip_local_state(&ClipLocalState {
                clip_id: item.id,
                expires_at: Some(expires_at),
                ..Default::default()
            })
            .unwrap());
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    #[tokio::test]
    async fn no_timer_is_armed_without_deadlines_or_for_a_long_horizon_one() {
        let (runtime, _dir) = fixture();
        let task = runtime.start_deadlines().unwrap();
        assert!(runtime.start_deadlines().is_none());
        settle().await;
        assert_eq!(runtime.deadlines.armed(), None, "no deadline, no timer");

        expiring_clip(&runtime, runtime.deadlines.now() + 30 * DAY_MS);
        runtime.resume_waker().resumed();
        settle().await;
        assert_eq!(
            runtime.deadlines.armed(),
            None,
            "30 days away holds no timer"
        );
        runtime.shutdown();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn the_earliest_short_deadline_fires_once_and_a_new_one_rearms_the_task() {
        let (runtime, _dir) = fixture();
        let task = runtime.start_deadlines().unwrap();
        settle().await;
        let now = runtime.deadlines.now();
        let soon = counter(&runtime, "soon", now + 40);
        let later = counter(&runtime, "later", now + 10 * 60 * 1000);
        settle().await;
        settle().await;
        assert_eq!(soon.load(Ordering::SeqCst), 1);
        assert_eq!(later.load(Ordering::SeqCst), 0);
        let armed = runtime
            .deadlines
            .armed()
            .expect("the later deadline is armed");
        assert!(armed > now + 60_000);

        let next = counter(&runtime, "next", runtime.deadlines.now() + 40);
        settle().await;
        settle().await;
        assert_eq!(next.load(Ordering::SeqCst), 1, "notify re-armed the task");
        assert_eq!(soon.load(Ordering::SeqCst), 1);
        runtime.shutdown();
        task.await.unwrap();
    }

    #[test]
    fn a_sweep_moves_a_long_horizon_deadline_into_the_short_horizon_when_it_nears() {
        let (runtime, _dir) = fixture();
        let deadline = runtime.deadlines.now() + 2 * SHORT_HORIZON_MS;
        expiring_clip(&runtime, deadline);
        runtime.sweep_deadlines();
        let now = runtime.deadlines.now();
        assert_eq!(runtime.deadlines.next_wait(now), None);

        runtime
            .deadlines
            .advance(SHORT_HORIZON_MS + SHORT_HORIZON_MS / 2);
        runtime.sweep_deadlines();
        let now = runtime.deadlines.now();
        let wait = runtime
            .deadlines
            .next_wait(now)
            .expect("now within an hour");
        assert!(wait <= Duration::from_millis(u64::try_from(deadline - now).unwrap()));

        runtime.set_resume_signal(false);
        assert_eq!(runtime.deadlines.next_wait(now), Some(FALLBACK_SLEEP));
    }
}
