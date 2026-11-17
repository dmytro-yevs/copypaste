//! Loaded in-process module instances. An instance unused for the idle
//! period, held by no lease or endpoint session and not running a sync
//! provider is unloaded; the reaper sleeps while nothing can expire. A new
//! instance loads only after the previous one of its module is torn down.
use crate::native::ModuleInstance;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Bounds the cost of a growing module collection before idle expiry.
const MAX_LOADED: usize = 4;
/// A module request already gives up after this long; so does a wait for a
/// stuck teardown, instead of blocking every later call of that module.
const TEARDOWN_LIMIT: Duration = crate::inference::REQUEST_LIMIT;
/// Outlasts every time-windowed state a module keeps (import review, hourly rate limits).
const IDLE_PERIOD: Duration = Duration::from_secs(60 * 60);

struct Loaded {
    instance: Arc<ModuleInstance>,
    last_used: Instant,
    /// A running sync provider keeps working between commands.
    retained: bool,
}

#[derive(Default)]
struct Map {
    loaded: BTreeMap<String, Loaded>,
    /// Instances taken out of `loaded` whose teardown has not finished.
    retiring: BTreeMap<String, usize>,
}

impl Map {
    fn retire(&mut self, shared: &Arc<Shared>, id: &str) -> Option<Retiring> {
        let entry = self.loaded.remove(id)?;
        *self.retiring.entry(id.into()).or_default() += 1;
        Some(Retiring {
            instance: entry.instance,
            _slot: RetiringSlot {
                id: id.into(),
                shared: Arc::clone(shared),
            },
        })
    }
}

struct Shared {
    idle_period: Duration,
    map: Mutex<Map>,
    wake: Condvar,
    retired: Condvar,
    stopped: AtomicBool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Map> {
        self.map.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// False when an instance of `id` is still tearing down after the limit.
    fn torn_down(&self, id: &str) -> bool {
        let map = self.lock();
        !self
            .retired
            .wait_timeout_while(map, TEARDOWN_LIMIT, |map| map.retiring.contains_key(id))
            .unwrap_or_else(|error| error.into_inner())
            .1
            .timed_out()
    }

    fn release(self: &Arc<Self>, id: &str) {
        let evicted = {
            let mut map = self.lock();
            if let Some(entry) = map.loaded.get_mut(id) {
                entry.last_used = Instant::now();
            }
            trim(&mut map, self)
        };
        drop(evicted);
    }
}

/// Restarts the idle period of its module when an endpoint session ends.
pub(crate) struct SessionUse {
    id: String,
    shared: Weak<Shared>,
}

impl Drop for SessionUse {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.release(&self.id);
        }
    }
}

/// An instance taken out of use. Dropping it stops the instance, waiting for
/// running work, and then admits a new instance of the same module.
pub(crate) struct Retiring {
    instance: Arc<ModuleInstance>,
    // Dropped after the instance, also when its teardown panics.
    _slot: RetiringSlot,
}

impl Drop for Retiring {
    fn drop(&mut self) {
        self.instance.stop();
    }
}

struct RetiringSlot {
    id: String,
    shared: Arc<Shared>,
}

impl Drop for RetiringSlot {
    fn drop(&mut self) {
        let mut map = self.shared.lock();
        if let Some(count) = map.retiring.get_mut(&self.id) {
            *count -= 1;
            if *count == 0 {
                map.retiring.remove(&self.id);
            }
        }
        drop(map);
        self.shared.retired.notify_all();
    }
}

/// A leased instance that may not be used before [`Lease::ready`].
pub(crate) struct Lease {
    id: String,
    instance: Arc<ModuleInstance>,
    evicted: Vec<Retiring>,
    shared: Arc<Shared>,
}

impl Lease {
    /// Tears down evicted instances and waits, within the teardown limit,
    /// until no instance of this module is still tearing down. Call it
    /// without holding manager locks.
    pub(crate) fn ready(self) -> Result<Arc<ModuleInstance>, crate::ModuleError> {
        drop(self.evicted);
        if !self.shared.torn_down(&self.id) {
            return Err(crate::ModuleError::Load);
        }
        Ok(self.instance)
    }
}

pub(crate) struct Instances {
    shared: Arc<Shared>,
    reaper: OnceLock<Option<JoinHandle<()>>>,
}

impl Instances {
    pub(crate) fn new() -> Self {
        Self::with_idle_period(IDLE_PERIOD)
    }

    fn with_idle_period(idle_period: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                idle_period,
                map: Mutex::default(),
                wake: Condvar::new(),
                retired: Condvar::new(),
                stopped: AtomicBool::new(false),
            }),
            reaper: OnceLock::new(),
        }
    }

    /// The loaded instance of `id`, marked as just used. Creating one only
    /// constructs it; the module code loads on first use. Instances held by a
    /// lease are never evicted.
    pub(crate) fn lease(
        &self,
        id: &str,
        retained: bool,
        create: impl FnOnce() -> ModuleInstance,
    ) -> Lease {
        let mut map = self.shared.lock();
        let entry = map.loaded.entry(id.into()).or_insert_with(|| {
            self.shared.wake.notify_all();
            Loaded {
                instance: Arc::new(create()),
                last_used: Instant::now(),
                retained,
            }
        });
        entry.last_used = Instant::now();
        if entry.retained != retained {
            entry.retained = retained;
            self.shared.wake.notify_all();
        }
        let instance = Arc::clone(&entry.instance);
        let evicted = trim(&mut map, &self.shared);
        drop(map);
        self.start_reaper();
        Lease {
            id: id.into(),
            instance,
            evicted,
            shared: Arc::clone(&self.shared),
        }
    }

    /// Restarts the idle period of `id` after its work ended.
    pub(crate) fn release(&self, id: &str) {
        self.shared.release(id);
    }

    /// Held by an endpoint session; see [`SessionUse`].
    pub(crate) fn session_use(&self, id: &str) -> SessionUse {
        SessionUse {
            id: id.into(),
            shared: Arc::downgrade(&self.shared),
        }
    }

    /// Waits, within the teardown limit, until no instance of `id` is still
    /// tearing down, so its files can be deleted.
    pub(crate) fn torn_down(&self, id: &str) -> bool {
        self.shared.torn_down(id)
    }

    pub(crate) fn tearing_down(&self, id: &str) -> bool {
        self.shared.lock().retiring.contains_key(id)
    }

    /// Takes `id` out of use; drop the result outside other locks to stop it.
    pub(crate) fn remove(&self, id: &str) -> Option<Retiring> {
        self.shared.lock().retire(&self.shared, id)
    }

    pub(crate) fn set_preferences(&self, id: &str, values: BTreeMap<String, serde_json::Value>) {
        if let Some(entry) = self.shared.lock().loaded.get(id) {
            entry.instance.set_preferences(values);
        }
    }

    fn start_reaper(&self) {
        self.reaper.get_or_init(|| {
            let shared = Arc::clone(&self.shared);
            std::thread::Builder::new()
                .name("module-idle".into())
                .spawn(move || reap_idle(shared))
                .ok()
        });
    }
}

impl Drop for Instances {
    fn drop(&mut self) {
        // Under the lock, so the reaper cannot miss it between check and wait.
        let map = self.shared.lock();
        self.shared.stopped.store(true, Ordering::Release);
        drop(map);
        self.shared.wake.notify_all();
        if let Some(reaper) = self.reaper.take().flatten() {
            let _ = reaper.join();
        }
    }
}

fn trim(map: &mut Map, shared: &Arc<Shared>) -> Vec<Retiring> {
    let mut evicted = Vec::new();
    while map.loaded.len() > MAX_LOADED {
        let oldest = map
            .loaded
            .iter()
            .filter(|(_, entry)| !entry.retained && Arc::strong_count(&entry.instance) == 1)
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(id, _)| id.clone());
        let Some(id) = oldest else {
            break;
        };
        evicted.extend(map.retire(shared, &id));
    }
    evicted
}

/// Unloading happens under the map lock only when the map holds the sole
/// reference, so a concurrent lease either keeps the instance or loads anew.
fn reap_idle(shared: Arc<Shared>) {
    let mut map = shared.lock();
    loop {
        if shared.stopped.load(Ordering::Acquire) {
            return;
        }
        let now = Instant::now();
        let mut next: Option<Duration> = None;
        let mut expired = Vec::new();
        for (id, entry) in &mut map.loaded {
            if entry.retained {
                continue;
            }
            let mut remaining = shared
                .idle_period
                .saturating_sub(now.saturating_duration_since(entry.last_used));
            if remaining.is_zero() {
                if Arc::strong_count(&entry.instance) == 1 {
                    expired.push(id.clone());
                    continue;
                }
                // An open session or a running command counts as use.
                entry.last_used = now;
                remaining = shared.idle_period;
            }
            next = Some(next.map_or(remaining, |next: Duration| next.min(remaining)));
        }
        if !expired.is_empty() {
            let expired: Vec<_> = expired
                .iter()
                .filter_map(|id| map.retire(&shared, id))
                .collect();
            drop(map);
            drop(expired);
            map = shared.lock();
            continue;
        }
        map = match next {
            Some(timeout) => {
                shared
                    .wake
                    .wait_timeout(map, timeout)
                    .unwrap_or_else(|error| error.into_inner())
                    .0
            }
            None => shared
                .wake
                .wait(map)
                .unwrap_or_else(|error| error.into_inner()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_instances_unload_while_leased_and_sync_provider_instances_stay() {
        let instances = Instances::with_idle_period(Duration::from_millis(50));
        let shared = Arc::<crate::bridge::HostShared>::default();
        let create = |id: &'static str| {
            let shared = Arc::clone(&shared);
            move || ModuleInstance::new(id, shared)
        };
        let idle = instances
            .lease("idle", false, create("idle"))
            .ready()
            .unwrap();
        let leased = instances
            .lease("leased", false, create("leased"))
            .ready()
            .unwrap();
        let provider = instances
            .lease("provider", true, create("provider"))
            .ready()
            .unwrap();
        drop((idle, provider));
        let started = Instant::now();
        while instances.shared.lock().loaded.contains_key("idle") {
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::yield_now();
        }
        let map = instances.shared.lock();
        assert!(map.loaded.contains_key("leased") && map.loaded.contains_key("provider"));
        assert!(Arc::ptr_eq(&map.loaded["leased"].instance, &leased));
    }

    #[test]
    fn the_cap_keeps_retained_instances_and_dropping_stops_a_waiting_reaper() {
        let instances = Instances::new();
        let shared = Arc::<crate::bridge::HostShared>::default();
        for (index, id) in ["provider", "a", "b", "c", "d"].into_iter().enumerate() {
            let lease = instances.lease(id, index == 0, || {
                ModuleInstance::new(id, Arc::clone(&shared))
            });
            drop(lease.ready().unwrap());
        }
        {
            let map = instances.shared.lock();
            assert!(map.loaded.contains_key("provider") && !map.loaded.contains_key("a"));
        }
        let (dropped, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drop(instances);
            let _ = dropped.send(());
        });
        assert!(done.recv_timeout(Duration::from_secs(2)).is_ok());
    }

    #[test]
    fn a_closed_endpoint_session_restarts_the_idle_period() {
        let instances = Instances::new();
        let shared = Arc::<crate::bridge::HostShared>::default();
        let instance = instances
            .lease("module", false, || ModuleInstance::new("module", shared))
            .ready()
            .unwrap();
        let session = instances.session_use("module");
        let opened = instances.shared.lock().loaded["module"].last_used;
        std::thread::sleep(Duration::from_millis(5));
        drop((session, instance));
        assert!(instances.shared.lock().loaded["module"].last_used > opened);
    }

    #[test]
    fn a_new_instance_waits_for_the_previous_one_to_finish_teardown() {
        let instances = Instances::new();
        let shared = Arc::<crate::bridge::HostShared>::default();
        let first = instances
            .lease("module", false, || {
                ModuleInstance::new("module", Arc::clone(&shared))
            })
            .ready()
            .unwrap();
        let retiring = instances.remove("module").unwrap();
        let lease = instances.lease("module", false, || {
            ModuleInstance::new("module", Arc::clone(&shared))
        });
        let other = instances
            .lease("other", false, || {
                ModuleInstance::new("other", Arc::clone(&shared))
            })
            .ready()
            .unwrap();
        let waiting = std::thread::spawn(move || lease.ready().unwrap());
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waiting.is_finished());
        drop(retiring);
        let second = waiting.join().unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        drop(other);
    }
}
