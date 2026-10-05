//! Runtime-owned capture scopes. No metadata lock survives a caller callback.
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};

use copypaste_ipc::ConfigData;

type Cancellation = Arc<dyn Fn() + Send + Sync>;
const MAX_OPERATIONS: usize = 32;
const MAX_HOSTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureKind {
    Implicit,
    Explicit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureScope {
    Read,
    Commit,
    Completion,
}
#[derive(Clone)]
struct Operation {
    host: u64,
    generation: u64,
    committed: bool,
    cancel: Cancellation,
}
struct State {
    config: ConfigData,
    generation: u64,
    next_id: u64,
    transitioning: bool,
    closed: bool,
    hosts: HashMap<u64, CaptureKind>,
    operations: HashMap<u64, Operation>,
    active: HashMap<u64, u64>,
}
/// IDs are process-local opaque capabilities in the positive JNI long range.
pub struct CaptureAdmission {
    state: Mutex<State>,
    drained: Condvar,
    #[cfg(test)]
    before_commit: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl std::fmt::Debug for CaptureAdmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CaptureAdmission")
    }
}
impl CaptureAdmission {
    pub(crate) fn new(config: ConfigData) -> Self {
        Self {
            state: Mutex::new(State {
                config,
                generation: 1,
                next_id: 1,
                transitioning: false,
                closed: false,
                hosts: HashMap::new(),
                operations: HashMap::new(),
                active: HashMap::new(),
            }),
            drained: Condvar::new(),
            #[cfg(test)]
            before_commit: Mutex::new(None),
        }
    }
    fn try_state(&self) -> Option<MutexGuard<'_, State>> {
        match self.state.try_lock() {
            Ok(state) => Some(state),
            Err(TryLockError::WouldBlock) => None,
            Err(TryLockError::Poisoned(poison)) => {
                drop(poison);
                self.fail_closed();
                None
            }
        }
    }
    fn id(state: &mut State) -> Option<u64> {
        if state.next_id >= i64::MAX as u64 {
            state.closed = true;
            return None;
        }
        let id = state.next_id;
        state.next_id += 1;
        Some(id)
    }
    fn cancel(callbacks: Vec<Cancellation>) {
        for cancel in callbacks {
            cancel();
        }
    }
    /// Poison recovery is used only to close admission and release references.
    /// It never constitutes successful drain or a usable policy snapshot.
    pub(crate) fn fail_closed(&self) {
        let callbacks = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.closed = true;
            state.hosts.clear();
            state.operations.drain().map(|(_, op)| op.cancel).collect()
        };
        Self::cancel(callbacks);
    }
    /// Busy, poisoned, closed and unknown admission fail closed without waiting.
    pub fn open_host(&self, kind: CaptureKind) -> Option<u64> {
        let mut state = self.try_state()?;
        if state.closed || state.transitioning || state.hosts.len() >= MAX_HOSTS {
            return None;
        }
        let Some(id) = Self::id(&mut state) else {
            drop(state);
            self.fail_closed();
            return None;
        };
        state.hosts.insert(id, kind);
        Some(id)
    }
    pub fn begin(&self, host: u64, cancel: Cancellation) -> Option<u64> {
        let mut state = self.try_state()?;
        let kind = *state.hosts.get(&host)?;
        if state.closed
            || state.transitioning
            || state.operations.len() >= MAX_OPERATIONS
            || state.config.private_mode
            || (kind == CaptureKind::Implicit && !state.config.excluded_app_bundle_ids.is_empty())
        {
            return None;
        }
        let Some(id) = Self::id(&mut state) else {
            drop(state);
            self.fail_closed();
            return None;
        };
        let generation = state.generation;
        state.operations.insert(
            id,
            Operation {
                host,
                generation,
                committed: false,
                cancel,
            },
        );
        Some(id)
    }
    pub fn acquire(&self, token: u64, scope: CaptureScope) -> Option<Permit<'_>> {
        let mut state = self.try_state()?;
        if state.closed || state.transitioning {
            return None;
        }
        let operation = state.operations.get(&token)?.clone();
        if operation.generation != state.generation
            || !state.hosts.contains_key(&operation.host)
            || operation.committed != (scope == CaptureScope::Completion)
            || state.active.contains_key(&token)
        {
            return None;
        }
        if scope == CaptureScope::Completion {
            state.operations.remove(&token);
        }
        state.active.insert(token, operation.host);
        let permit = Permit {
            owner: self,
            token,
            scope,
            committed: false,
            config: state.config.clone(),
        };
        drop(state);
        #[cfg(test)]
        if scope == CaptureScope::Commit {
            let hook = self.before_commit.lock().unwrap().take();
            if let Some(hook) = hook {
                hook();
            }
        }
        Some(permit)
    }
    #[cfg(test)]
    pub(crate) fn before_next_commit(&self, hook: impl FnOnce() + Send + 'static) {
        *self.before_commit.lock().unwrap() = Some(Box::new(hook));
    }
    pub fn abandon(&self, token: u64) {
        let callback = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.operations.remove(&token).map(|op| op.cancel)
        };
        if let Some(cancel) = callback {
            cancel();
        }
    }
    /// Revocation linearizes synchronously, without waiting for active callbacks.
    pub fn revoke_host(&self, host: u64) -> Result<(), ()> {
        let callbacks = {
            let mut state = self.state.lock().map_err(|_| ())?;
            state.hosts.remove(&host);
            let tokens: Vec<_> = state
                .operations
                .iter()
                .filter(|(_, op)| op.host == host)
                .map(|(id, _)| *id)
                .collect();
            tokens
                .into_iter()
                .filter_map(|id| state.operations.remove(&id).map(|op| op.cancel))
                .collect()
        };
        Self::cancel(callbacks);
        Ok(())
    }
    /// Worker-only proof of drain. Call only after synchronous host revocation.
    pub fn drain_host(&self, host: u64) -> Result<(), ()> {
        let mut state = self.state.lock().map_err(|_| ())?;
        if state.hosts.contains_key(&host) {
            return Err(());
        }
        while state
            .active
            .values()
            .any(|active_host| *active_host == host)
        {
            state = self.drained.wait(state).map_err(|_| ())?;
        }
        Ok(())
    }
    pub(crate) fn transition(&self) -> Result<Transition<'_>, ()> {
        let callbacks = {
            let mut state = self.state.lock().map_err(|_| ())?;
            if state.closed || state.transitioning {
                return Err(());
            }
            let Some(generation) = state.generation.checked_add(1) else {
                drop(state);
                self.fail_closed();
                return Err(());
            };
            state.transitioning = true;
            state.generation = generation;
            state
                .operations
                .drain()
                .map(|(_, operation)| operation.cancel)
                .collect()
        };
        Self::cancel(callbacks);
        let mut state = self.state.lock().map_err(|_| ())?;
        while !state.active.is_empty() {
            state = self.drained.wait(state).map_err(|_| ())?;
        }
        Ok(Transition { owner: self })
    }
    /// Worker-only permanent close; process-global reuse cannot reopen admission.
    pub fn shutdown(&self) -> Result<(), ()> {
        self.fail_closed();
        let mut state = self.state.lock().map_err(|_| ())?;
        while !state.active.is_empty() {
            state = self.drained.wait(state).map_err(|_| ())?;
        }
        Ok(())
    }
}
pub struct Permit<'a> {
    owner: &'a CaptureAdmission,
    token: u64,
    scope: CaptureScope,
    committed: bool,
    pub config: ConfigData,
}
impl Permit<'_> {
    /// Only the successful encrypted ingest may enable terminal publication.
    pub fn committed(&mut self) {
        if self.scope != CaptureScope::Commit {
            return;
        }
        self.committed = true;
        if let Ok(mut state) = self.owner.state.lock() {
            if let Some(operation) = state.operations.get_mut(&self.token) {
                operation.committed = true;
            }
        }
    }
}
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let callback = {
            let mut state = self
                .owner
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.active.remove(&self.token);
            self.owner.drained.notify_all();
            if self.scope == CaptureScope::Commit && !self.committed {
                state.operations.remove(&self.token).map(|op| op.cancel)
            } else {
                None
            }
        };
        if let Some(cancel) = callback {
            cancel();
        }
    }
}
pub(crate) struct Transition<'a> {
    owner: &'a CaptureAdmission,
}
impl Transition<'_> {
    pub(crate) fn publish(&self, config: ConfigData) -> Result<(), ()> {
        let mut state = self.owner.state.lock().map_err(|_| ())?;
        if state.closed || !state.transitioning {
            return Err(());
        }
        state.config = config;
        Ok(())
    }
}
impl Drop for Transition<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.owner.state.lock() {
            state.transitioning = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Barrier,
    };

    fn token(gate: &CaptureAdmission, kind: CaptureKind) -> (u64, u64) {
        let host = gate.open_host(kind).unwrap();
        (host, gate.begin(host, Arc::new(|| {})).unwrap())
    }

    #[test]
    fn counting_clipboard_and_provider_reads_obey_both_transition_orderings() {
        for provider in [false, true] {
            let gate = Arc::new(CaptureAdmission::new(ConfigData::default()));
            let (host, token) = token(&gate, CaptureKind::Explicit);
            let clipboard_reads = Arc::new(AtomicUsize::new(0));
            let provider_reads = Arc::new(AtomicUsize::new(0));
            let read_entered = Arc::new(Barrier::new(2));
            let read_release = Arc::new(Barrier::new(2));
            let worker_gate = Arc::clone(&gate);
            let entered = Arc::clone(&read_entered);
            let release = Arc::clone(&read_release);
            let clipboard = Arc::clone(&clipboard_reads);
            let content_provider = Arc::clone(&provider_reads);
            let worker = std::thread::spawn(move || {
                let _read = worker_gate.acquire(token, CaptureScope::Read).unwrap();
                // These counting fakes stand in for the synchronous JNI callback.
                if provider {
                    content_provider.fetch_add(1, Ordering::SeqCst);
                } else {
                    clipboard.fetch_add(1, Ordering::SeqCst);
                }
                entered.wait();
                release.wait();
            });
            read_entered.wait();
            let invalidated = Arc::new(Barrier::new(2));
            let cancelled = Arc::clone(&invalidated);
            gate.begin(
                host,
                Arc::new(move || {
                    cancelled.wait();
                }),
            )
            .unwrap();
            let transition_gate = Arc::clone(&gate);
            let (done_tx, done_rx) = mpsc::channel();
            let transition = std::thread::spawn(move || {
                drop(transition_gate.transition().unwrap());
                done_tx.send(()).unwrap();
            });
            invalidated.wait();
            assert!(done_rx.try_recv().is_err());
            read_release.wait();
            worker.join().unwrap();
            done_rx.recv().unwrap();
            transition.join().unwrap();
            assert_eq!(
                clipboard_reads.load(Ordering::SeqCst),
                usize::from(!provider)
            );
            assert_eq!(provider_reads.load(Ordering::SeqCst), usize::from(provider));
            // Transition-first refuses before the fake OS access can occur.
            if let Some(_permit) = gate.acquire(token, CaptureScope::Read) {
                clipboard_reads.fetch_add(1, Ordering::SeqCst);
                provider_reads.fetch_add(1, Ordering::SeqCst);
            }
            assert_eq!(
                clipboard_reads.load(Ordering::SeqCst),
                usize::from(!provider)
            );
            assert_eq!(provider_reads.load(Ordering::SeqCst), usize::from(provider));
        }
    }

    #[test]
    fn failed_commit_cannot_publish_or_retry_and_success_is_single_use() {
        let gate = CaptureAdmission::new(ConfigData::default());
        let (_, failed) = token(&gate, CaptureKind::Explicit);
        drop(gate.acquire(failed, CaptureScope::Commit).unwrap());
        assert!(gate.acquire(failed, CaptureScope::Completion).is_none());
        assert!(gate.acquire(failed, CaptureScope::Commit).is_none());
        let (_, saved) = token(&gate, CaptureKind::Explicit);
        let mut commit = gate.acquire(saved, CaptureScope::Commit).unwrap();
        assert!(gate.acquire(saved, CaptureScope::Completion).is_none());
        commit.committed();
        drop(commit);
        assert!(gate.acquire(saved, CaptureScope::Read).is_none());
        drop(gate.acquire(saved, CaptureScope::Completion).unwrap());
        assert!(gate.acquire(saved, CaptureScope::Completion).is_none());
    }

    #[test]
    fn read_commit_and_completion_drain_before_policy_publication() {
        for scope in [
            CaptureScope::Read,
            CaptureScope::Commit,
            CaptureScope::Completion,
        ] {
            let gate = Arc::new(CaptureAdmission::new(ConfigData::default()));
            let host = gate.open_host(CaptureKind::Explicit).unwrap();
            let invalidated = Arc::new(Barrier::new(2));
            let cancellation = Arc::clone(&invalidated);
            // Completion consumes its operation. A second queued operation provides
            // a deterministic observation of transition invalidation in every phase.
            let op = gate.begin(host, Arc::new(|| {})).unwrap();
            gate.begin(
                host,
                Arc::new(move || {
                    cancellation.wait();
                }),
            )
            .unwrap();
            if scope == CaptureScope::Completion {
                let mut commit = gate.acquire(op, CaptureScope::Commit).unwrap();
                commit.committed();
            }
            let permit = gate.acquire(op, scope).unwrap();
            let worker_gate = Arc::clone(&gate);
            let (done_tx, done_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let transition = worker_gate.transition().unwrap();
                let mut next = ConfigData::default();
                next.private_mode = true;
                transition.publish(next).unwrap();
                done_tx.send(()).unwrap();
            });
            invalidated.wait();
            assert!(done_rx.try_recv().is_err());
            assert!(gate.acquire(op, CaptureScope::Read).is_none());
            assert!(gate.begin(host, Arc::new(|| {})).is_none());
            drop(permit);
            done_rx.recv().unwrap();
            worker.join().unwrap();
            assert!(gate.begin(host, Arc::new(|| {})).is_none());
        }
    }

    #[test]
    fn transition_first_never_reads_or_commits_and_aba_does_not_revive_tokens() {
        for scope in [CaptureScope::Read, CaptureScope::Commit] {
            let gate = CaptureAdmission::new(ConfigData::default());
            let (host, old) = token(&gate, CaptureKind::Implicit);
            let transition = gate.transition().unwrap();
            assert!(gate.acquire(old, scope).is_none());
            let mut paused = ConfigData::default();
            paused.private_mode = true;
            transition.publish(paused).unwrap();
            drop(transition);
            let transition = gate.transition().unwrap();
            transition.publish(ConfigData::default()).unwrap();
            drop(transition);
            assert!(gate.acquire(old, scope).is_none());
            assert!(gate.begin(host, Arc::new(|| {})).is_some());
        }
    }

    #[test]
    fn exclusions_and_limits_preserve_explicit_exception_but_pause_blocks_both() {
        let mut excluded = ConfigData::default();
        excluded.excluded_app_bundle_ids = vec!["com.example.editor".into()];
        let gate = CaptureAdmission::new(excluded.clone());
        let implicit = gate.open_host(CaptureKind::Implicit).unwrap();
        let explicit = gate.open_host(CaptureKind::Explicit).unwrap();
        assert!(gate.begin(implicit, Arc::new(|| {})).is_none());
        let old = gate.begin(explicit, Arc::new(|| {})).unwrap();
        let transition = gate.transition().unwrap();
        transition.publish(ConfigData::default()).unwrap();
        drop(transition);
        assert!(gate.acquire(old, CaptureScope::Commit).is_none());
        let new = gate.begin(explicit, Arc::new(|| {})).unwrap();
        let permit = gate.acquire(new, CaptureScope::Read).unwrap();
        assert_eq!(
            permit.config.capture_limit_bytes("text/plain"),
            ConfigData::default().capture_limit_bytes("text/plain")
        );
        drop(permit);
        let transition = gate.transition().unwrap();
        excluded.private_mode = true;
        transition.publish(excluded).unwrap();
        drop(transition);
        assert!(gate.begin(explicit, Arc::new(|| {})).is_none());
        assert!(gate.acquire(new, CaptureScope::Commit).is_none());
    }

    #[test]
    fn host_revocation_is_immediate_and_drain_requires_actual_scope_exit() {
        let gate = Arc::new(CaptureAdmission::new(ConfigData::default()));
        let (host, old) = token(&gate, CaptureKind::Implicit);
        let permit = gate.acquire(old, CaptureScope::Read).unwrap();
        gate.revoke_host(host).unwrap();
        assert!(gate.begin(host, Arc::new(|| {})).is_none());
        assert!(gate.acquire(old, CaptureScope::Commit).is_none());
        let worker_gate = Arc::clone(&gate);
        let entered = Arc::new(Barrier::new(2));
        let worker_entered = Arc::clone(&entered);
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_entered.wait();
            worker_gate.drain_host(host).unwrap();
            done_tx.send(()).unwrap();
        });
        entered.wait();
        assert!(done_rx.try_recv().is_err());
        drop(permit);
        done_rx.recv().unwrap();
        worker.join().unwrap();
        let (fresh_host, fresh) = token(&gate, CaptureKind::Implicit);
        assert_ne!(host, fresh_host);
        assert!(gate.acquire(fresh, CaptureScope::Read).is_some());
        assert!(gate.acquire(old, CaptureScope::Read).is_none());
    }

    #[test]
    fn shutdown_overflow_and_poison_cancel_without_claiming_failed_drain() {
        for overflow in [false, true] {
            let gate = CaptureAdmission::new(ConfigData::default());
            let host = gate.open_host(CaptureKind::Explicit).unwrap();
            let cancelled = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&cancelled);
            let op = gate
                .begin(
                    host,
                    Arc::new(move || {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }),
                )
                .unwrap();
            if overflow {
                gate.state.lock().unwrap().generation = u64::MAX;
                assert!(gate.transition().is_err());
            } else {
                gate.shutdown().unwrap();
            }
            assert_eq!(cancelled.load(Ordering::SeqCst), 1);
            assert!(gate.acquire(op, CaptureScope::Read).is_none());
            assert!(gate.open_host(CaptureKind::Explicit).is_none());
        }
        let gate = CaptureAdmission::new(ConfigData::default());
        let (host, op) = token(&gate, CaptureKind::Implicit);
        let permit = gate.acquire(op, CaptureScope::Read).unwrap();
        let _ = std::panic::catch_unwind(|| {
            let _lock = gate.state.lock().unwrap();
            panic!("poison");
        });
        assert!(gate.revoke_host(host).is_err());
        assert!(gate.drain_host(host).is_err());
        assert!(gate.shutdown().is_err());
        drop(permit);
        assert!(gate.open_host(CaptureKind::Explicit).is_none());
        let gate = CaptureAdmission::new(ConfigData::default());
        let (host, old) = token(&gate, CaptureKind::Implicit);
        gate.state.lock().unwrap().next_id = i64::MAX as u64;
        assert!(gate.begin(host, Arc::new(|| {})).is_none());
        assert!(gate.acquire(old, CaptureScope::Read).is_none());
    }

    #[test]
    fn cancelled_queue_is_bounded_and_transition_failure_never_revives_it() {
        let gate = CaptureAdmission::new(ConfigData::default());
        let host = gate.open_host(CaptureKind::Explicit).unwrap();
        let ops: Vec<_> = (0..MAX_OPERATIONS)
            .map(|_| gate.begin(host, Arc::new(|| {})).unwrap())
            .collect();
        assert!(gate.begin(host, Arc::new(|| {})).is_none());
        // Dropping an unpublished transition represents persistence refusal.
        drop(gate.transition().unwrap());
        for op in ops {
            assert!(gate.acquire(op, CaptureScope::Read).is_none());
        }
        assert!(gate.begin(host, Arc::new(|| {})).is_some());
    }
}
