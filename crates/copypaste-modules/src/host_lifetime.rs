//! Revocation callbacks are drained before their module-owned contexts are freed.
use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{Arc, Condvar, Mutex},
};

type Callback = unsafe extern "C" fn(*mut c_void);
const MAX_CALLBACKS: usize = 32;

#[derive(Default)]
pub(crate) struct HostLifetime {
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    revoked: bool,
    next: u64,
    callbacks: BTreeMap<u64, Arc<Entry>>,
}
struct Entry {
    callback: Callback,
    context: usize,
    state: Mutex<EntryState>,
    drained: Condvar,
}
struct EntryState {
    active: bool,
    running: usize,
}
impl Entry {
    fn call(&self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if !state.active {
                return;
            }
            state.running += 1;
        }
        // SAFETY: the module keeps its context alive until unregister returns.
        unsafe { (self.callback)(self.context as *mut c_void) };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.running -= 1;
        self.drained.notify_all();
    }
    fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active = false;
        while state.running != 0 {
            state = self.drained.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
}
impl HostLifetime {
    pub(crate) fn subscribe(&self, callback: Callback, context: *mut c_void) -> u64 {
        let entry = Arc::new(Entry {
            callback,
            context: context as usize,
            state: Mutex::new(EntryState {
                active: true,
                running: 0,
            }),
            drained: Condvar::new(),
        });
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.revoked || state.callbacks.len() >= MAX_CALLBACKS {
            drop(state);
            entry.call();
            return 0;
        }
        let Some(next) = state.next.checked_add(1) else {
            drop(state);
            entry.call();
            return 0;
        };
        state.next = next;
        let id = state.next;
        state.callbacks.insert(id, entry);
        id
    }
    pub(crate) fn unsubscribe(&self, id: u64) {
        let entry = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .callbacks
            .remove(&id);
        if let Some(entry) = entry {
            entry.stop();
        }
    }
    pub(crate) fn revoke(&self) {
        let callbacks = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.revoked {
                return;
            }
            state.revoked = true;
            state.callbacks.values().cloned().collect::<Vec<_>>()
        };
        for entry in callbacks {
            entry.call();
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

    unsafe extern "C" fn increment(context: *mut c_void) {
        unsafe { &*context.cast::<AtomicUsize>() }.fetch_add(1, Ordering::SeqCst);
    }
    #[test]
    fn revoked_registration_is_not_lost_and_unsubscribed_callbacks_do_not_run() {
        let lifetime = HostLifetime::default();
        let calls = AtomicUsize::new(0);
        let ptr = (&calls as *const AtomicUsize).cast_mut().cast();
        let id = lifetime.subscribe(increment, ptr);
        lifetime.unsubscribe(id);
        lifetime.revoke();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(lifetime.subscribe(increment, ptr), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        lifetime.revoke();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn registration_racing_revocation_is_not_lost_or_duplicated() {
        for _ in 0..100 {
            let lifetime = Arc::new(HostLifetime::default());
            let calls = Arc::new(AtomicUsize::new(0));
            let barrier = Arc::new(Barrier::new(2));
            let worker = {
                let lifetime = Arc::clone(&lifetime);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    lifetime.revoke();
                })
            };
            barrier.wait();
            let id = lifetime.subscribe(increment, Arc::as_ptr(&calls).cast_mut().cast());
            worker.join().unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            if id != 0 {
                lifetime.unsubscribe(id);
            }
        }
    }

    #[test]
    fn subscription_limit_fails_closed() {
        let lifetime = HostLifetime::default();
        let calls = AtomicUsize::new(0);
        let ptr = (&calls as *const AtomicUsize).cast_mut().cast();
        let ids: Vec<_> = (0..MAX_CALLBACKS)
            .map(|_| lifetime.subscribe(increment, ptr))
            .collect();
        assert!(ids.iter().all(|id| *id != 0));
        assert_eq!(lifetime.subscribe(increment, ptr), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        for id in ids {
            lifetime.unsubscribe(id);
        }
    }

    struct Blocked {
        entered: Barrier,
        release: Barrier,
    }
    unsafe extern "C" fn block(context: *mut c_void) {
        let state = unsafe { &*context.cast::<Blocked>() };
        state.entered.wait();
        state.release.wait();
    }
    #[test]
    fn unsubscribe_waits_for_inflight_callback_before_context_can_be_freed() {
        let lifetime = Arc::new(HostLifetime::default());
        let blocked = Box::new(Blocked {
            entered: Barrier::new(2),
            release: Barrier::new(2),
        });
        let id = lifetime.subscribe(block, (&*blocked as *const Blocked).cast_mut().cast());
        let revoker = {
            let lifetime = Arc::clone(&lifetime);
            std::thread::spawn(move || lifetime.revoke())
        };
        blocked.entered.wait();
        let (done, received) = mpsc::channel();
        let (started, start_received) = mpsc::channel();
        let unsubscribe = {
            let lifetime = Arc::clone(&lifetime);
            std::thread::spawn(move || {
                started.send(()).unwrap();
                lifetime.unsubscribe(id);
                done.send(()).unwrap();
            })
        };
        start_received.recv().unwrap();
        assert!(received.try_recv().is_err());
        blocked.release.wait();
        received.recv().unwrap();
        revoker.join().unwrap();
        unsubscribe.join().unwrap();
    }
}
