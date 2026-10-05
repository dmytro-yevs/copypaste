//! Main-thread AppKit lifetime and fresh capture-source identity requests.
//!
//! NSRunningApplication updates its properties at common-mode main-runloop
//! turns. The daemon's owned Tokio runtime therefore runs on a coordinator,
//! while the original main thread services Cocoa and the signaled request source.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, Weak};

use anyhow::{anyhow, Context};
use block2::RcBlock;
use core_foundation::base::{kCFAllocatorDefault, TCFType};
use core_foundation::runloop::{
    kCFRunLoopCommonModes, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext,
    CFRunLoopSourceCreate, CFRunLoopSourceInvalidate, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use futures_util::FutureExt;
use objc2::rc::{autoreleasepool, Retained};
use objc2::{msg_send, ClassType};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationLoad, NSRunningApplication,
    NSWorkspace, NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidDeactivateApplicationNotification, NSWorkspaceDidWakeNotification,
    NSWorkspaceSessionDidBecomeActiveNotification, NSWorkspaceSessionDidResignActiveNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{MainThreadMarker, NSNotification, NSNotificationCenter, NSObject};
use tokio::sync::watch;
use tracing::warn;

// Capture is serialized, but bound the service independently of its caller.
const MAX_REQUESTS: usize = 16;
const MAX_ACTIVATIONS: usize = 128;
static SERVICE: Mutex<Option<Weak<Shared>>> = Mutex::new(None);
static NEXT_SERVICE_ID: AtomicU64 = AtomicU64::new(1);

/// Only owned strings cross from the main thread to capture workers.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    // Unbundled applications may still have a localized display name.
    pub(crate) bundle_id: Option<String>,
    pub(crate) name: Option<String>,
}

/// A primitive history cursor sampled before a pasteboard generation read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observation {
    service_id: u64,
    epoch: u64,
}

struct Activation {
    epoch: u64,
    bundle_id: Option<String>,
}

struct ActivationHistory {
    service_id: u64,
    epoch: u64,
    events: VecDeque<Activation>,
    active: Option<String>,
}

impl ActivationHistory {
    fn new() -> Self {
        Self {
            // Exhausted identities are permanently unknown rather than reused.
            service_id: NEXT_SERVICE_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .unwrap_or(0),
            epoch: 0,
            events: VecDeque::new(),
            active: None,
        }
    }

    fn record(&mut self, bundle_id: Option<String>) -> u64 {
        self.active = bundle_id.clone();
        // Epoch exhaustion is unknown coverage forever, never a wrapped cursor.
        if let Some(epoch) = self.epoch.checked_add(1) {
            self.epoch = epoch;
            self.events.push_back(Activation { epoch, bundle_id });
            if self.events.len() > MAX_ACTIVATIONS {
                self.events.pop_front();
            }
        } else {
            self.events.clear();
        }
        self.epoch
    }

    fn deactivate(&mut self, bundle_id: Option<String>) -> u64 {
        let epoch = self.record(bundle_id);
        // The affected app is known, but the next active app is not yet known.
        // A matching activate notification can complete coverage; a getter
        // without that notification will instead record a gap in reconcile.
        self.active = None;
        epoch
    }

    fn reconcile(&mut self, bundle_id: Option<&str>) -> u64 {
        if self.active.as_deref() != bundle_id || self.events.is_empty() {
            // A getter disagreeing with notifications is a coverage gap. Keep
            // that gap and the fresh identity so a later clean observation can
            // recover without erasing the interval that was ambiguous.
            self.record(None);
            self.record(bundle_id.map(str::to_owned));
        }
        self.epoch
    }

    fn allows(
        &self,
        previous: Option<(i64, Observation)>,
        generation: i64,
        excluded: &[String],
    ) -> bool {
        if excluded.is_empty() {
            return true;
        }
        let Some((previous_generation, boundary)) = previous else {
            return false;
        };
        if self.service_id == 0
            || boundary.service_id != self.service_id
            || generation <= previous_generation
        {
            return false;
        }
        // Include the application already active at the previous observation,
        // plus every subsequent event. Records are never cleared on a reply.
        let Some(start) = self
            .events
            .iter()
            .position(|event| event.epoch == boundary.epoch)
        else {
            return false;
        };
        self.events.iter().skip(start).all(|event| {
            event
                .bundle_id
                .as_ref()
                .is_some_and(|bundle| !excluded.contains(bundle))
        })
    }
}

struct Request {
    previous: Option<(i64, Observation)>,
    generation: i64,
    excluded: Vec<String>,
    reply: mpsc::Sender<Option<SourceIdentity>>,
}

#[derive(Default)]
struct Queue {
    requests: VecDeque<Request>,
    completion: Option<anyhow::Result<()>>,
    closed: bool,
}

impl Queue {
    fn admit(&mut self, request: Request) -> bool {
        if self.closed || self.requests.len() >= MAX_REQUESTS {
            return false;
        }
        self.requests.push_back(request);
        true
    }

    fn close(&mut self) {
        self.closed = true;
        // Disconnect all admitted callers instead of leaving them waiting.
        self.requests.clear();
    }

    fn complete(&mut self, result: anyhow::Result<()>) {
        self.close();
        self.completion = Some(result);
    }
}

struct Signal {
    source: CFRunLoopSource,
    run_loop: CFRunLoop,
}

// CF runloop sources can be signaled from other threads. This wrapper shares
// only Signal/WakeUp under Shared::signal's mutex; registration, invalidation
// and the callback remain on main. Both CF references stay retained until the
// last in-progress signal has released that same mutex.
unsafe impl Send for Signal {}

struct Shared {
    queue: Mutex<Queue>,
    signal: Mutex<Option<Signal>>,
    failed: watch::Sender<bool>,
    history: Mutex<ActivationHistory>,
    observed_epoch: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A callback panic must still be able to close/disconnect a poisoned queue.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    fn record(&self, bundle_id: Option<String>) {
        let mut history = lock(&self.history);
        let epoch = history.record(bundle_id);
        // Publish while still serialized with every record and decision, so an
        // off-main recorder cannot publish an older epoch after a newer one.
        self.observed_epoch.store(epoch, Ordering::Release);
        drop(history);
        // Unknown notification state must recover on main even when there are
        // no changed-pasteboard requests. Known activation wakes are cheap.
        self.wake();
    }

    fn deactivate(&self, bundle_id: Option<String>) {
        let mut history = lock(&self.history);
        let epoch = history.deactivate(bundle_id);
        self.observed_epoch.store(epoch, Ordering::Release);
        drop(history);
        self.wake();
    }

    fn wake(&self) {
        if let Some(signal) = lock(&self.signal).as_ref() {
            unsafe {
                CFRunLoopSourceSignal(signal.source.as_concrete_TypeRef());
                CFRunLoopWakeUp(signal.run_loop.as_concrete_TypeRef());
            }
        }
    }

    fn fail(&self) {
        lock(&self.queue).close();
        self.failed.send_replace(true);
    }

    fn complete(&self, result: anyhow::Result<()>) {
        lock(&self.queue).complete(result);
        self.wake();
    }
}

struct MainContext {
    workspace: Retained<NSWorkspace>,
    run_loop: CFRunLoop,
    shared: Arc<Shared>,
    result: RefCell<Option<anyhow::Result<()>>>,
    observers: Vec<Retained<NSObject>>,
    notification_center: Retained<NSNotificationCenter>,
}

impl MainContext {
    fn perform(&self) {
        // This source is installed only on the original main runloop.
        let _main = MainThreadMarker::new().expect("workspace callback must run on main");
        let (completion, requests) = {
            let mut queue = lock(&self.shared.queue);
            (queue.completion.take(), std::mem::take(&mut queue.requests))
        };
        if let Some(result) = completion {
            *self.result.borrow_mut() = Some(result);
            self.run_loop.stop();
            return;
        }
        process_requests(requests, &self.shared, || {
            autoreleasepool(|_| unsafe {
                self.workspace.frontmostApplication().and_then(|app| {
                    let bundle_id = app.bundleIdentifier().map(|id| id.to_string());
                    let name = app.localizedName().map(|name| name.to_string());
                    (bundle_id.is_some() || name.is_some())
                        .then_some(SourceIdentity { bundle_id, name })
                })
            })
        });
    }
}

fn process_requests(
    requests: VecDeque<Request>,
    shared: &Shared,
    mut resolve: impl FnMut() -> Option<SourceIdentity>,
) {
    if requests.is_empty() {
        let sample_epoch = {
            let history = lock(&shared.history);
            if history.active.is_some() {
                return;
            }
            history.epoch
        };
        // Notified gaps/unmatched deactivations can recover without requiring
        // the next Copy; unchanged observations can then establish coverage.
        // Keep all old records; do not retroactively certify their interval.
        let identity = resolve();
        let mut history = lock(&shared.history);
        if history.epoch != sample_epoch {
            // A newer notified event owns coverage. Never publish an earlier
            // getter as Known after it and let an idle cursor trust stale state.
            return;
        }
        let epoch = history.reconcile(identity.as_ref().and_then(|app| app.bundle_id.as_deref()));
        shared.observed_epoch.store(epoch, Ordering::Release);
        return;
    }
    resolve_requests(requests, |request| {
        // All Cocoa work stays outside the mutex, including recovery getters.
        let sample_epoch = lock(&shared.history).epoch;
        let identity = resolve();
        let mut history = lock(&shared.history);
        if history.epoch != sample_epoch {
            return None;
        }
        let epoch = history.reconcile(identity.as_ref().and_then(|app| app.bundle_id.as_deref()));
        shared.observed_epoch.store(epoch, Ordering::Release);
        history
            .allows(request.previous, request.generation, &request.excluded)
            .then_some(identity)
            .flatten()
    });
}

impl Drop for MainContext {
    fn drop(&mut self) {
        // Also unregister on a control-source creation failure, before a
        // MainService exists. Observer blocks capture no context pointer.
        for observer in &self.observers {
            unsafe { self.notification_center.removeObserver(observer) };
        }
    }
}

fn resolve_requests(
    requests: VecDeque<Request>,
    mut resolve: impl FnMut(&Request) -> Option<SourceIdentity>,
) {
    for request in requests {
        // Every request gets a new workspace observation; no shared identity cache.
        let _ = request.reply.send(resolve(&request));
    }
}

extern "C" fn perform(info: *const c_void) {
    // MainService keeps this boxed context alive until the source is invalidated.
    let context = unsafe { &*info.cast::<MainContext>() };
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| context.perform())) {
        // Even an arbitrary panic payload's destructor must not unwind across C.
        std::mem::forget(payload);
        context.shared.fail();
        // Keep servicing completion: the coordinator must release its runtime
        // before this callback stops the main loop. Pending replies are gone.
    }
}

#[derive(Clone, Copy)]
enum NotificationKind {
    Activated,
    Deactivated,
    Gap,
}

fn notification_bundle(notification: NonNull<NSNotification>) -> Option<String> {
    // Apple's application notifications provide NSRunningApplication under
    // NSWorkspaceApplicationKey. Validate the class before using the typed view.
    unsafe {
        let object = notification
            .as_ref()
            .userInfo()?
            .objectForKey(NSWorkspaceApplicationKey)?;
        let valid: bool = msg_send![&*object, isKindOfClass: NSRunningApplication::class()];
        if !valid {
            return None;
        }
        let application = &*Retained::as_ptr(&object).cast::<NSRunningApplication>();
        application
            .bundleIdentifier()
            .map(|bundle| bundle.to_string())
    }
}

fn install_observers(
    center: &NSNotificationCenter,
    shared: &Arc<Shared>,
) -> Vec<Retained<NSObject>> {
    let notifications = unsafe {
        [
            (
                NSWorkspaceDidActivateApplicationNotification,
                NotificationKind::Activated,
            ),
            (
                NSWorkspaceDidDeactivateApplicationNotification,
                NotificationKind::Deactivated,
            ),
            (NSWorkspaceWillSleepNotification, NotificationKind::Gap),
            (NSWorkspaceDidWakeNotification, NotificationKind::Gap),
            (
                NSWorkspaceSessionDidResignActiveNotification,
                NotificationKind::Gap,
            ),
            (
                NSWorkspaceSessionDidBecomeActiveNotification,
                NotificationKind::Gap,
            ),
        ]
    };
    notifications
        .into_iter()
        .map(|(name, kind)| {
            let shared = Arc::clone(shared);
            let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    // Delivery with no operation queue is synchronous on the posting
                    // thread. Off-main delivery records unknown coverage directly
                    // under the decision's history mutex without accessing Cocoa.
                    if MainThreadMarker::new().is_none() {
                        shared.record(None);
                        return;
                    }
                    autoreleasepool(|_| match kind {
                        NotificationKind::Activated => {
                            shared.record(notification_bundle(notification))
                        }
                        NotificationKind::Deactivated => {
                            shared.deactivate(notification_bundle(notification))
                        }
                        NotificationKind::Gap => shared.record(None),
                    });
                }));
                if let Err(payload) = result {
                    std::mem::forget(payload);
                    shared.fail();
                }
            });
            // No raw MainContext pointer is captured: queued/in-progress observer
            // blocks may safely retain primitive Shared state after service teardown.
            unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            }
        })
        .collect()
}

struct MainService {
    context: Box<MainContext>,
}

impl MainService {
    fn install() -> anyhow::Result<Option<(Self, watch::Receiver<bool>)>> {
        let main = MainThreadMarker::new().context("initialize workspace on the main thread")?;
        let mut service = lock(&SERVICE);
        anyhow::ensure!(
            service.as_ref().and_then(Weak::upgrade).is_none(),
            "workspace service is already installed"
        );
        if !application_available(
            || unsafe { NSApplicationLoad() }.as_bool(),
            || {
                // The helper shares the app bundle and must never become a foreground app.
                NSApplication::sharedApplication(main)
                    .setActivationPolicy(NSApplicationActivationPolicy::Prohibited)
            },
        ) {
            // Preserve daemon availability when AppKit setup is unavailable.
            // No workspace or control source is created/published in this branch.
            return Ok(None);
        }
        let (failed, failure) = watch::channel(false);
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            signal: Mutex::new(None),
            failed,
            history: Mutex::new(ActivationHistory::new()),
            observed_epoch: AtomicU64::new(0),
        });
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let notification_center = unsafe { workspace.notificationCenter() };
        let observers = install_observers(&notification_center, &shared);
        // Register first, then seed the current app. Keep any earlier notification
        // records; the initial pasteboard generation has no previous coverage.
        shared.record(unsafe {
            workspace
                .frontmostApplication()
                .and_then(|app| app.bundleIdentifier().map(|bundle| bundle.to_string()))
        });
        let mut context = Box::new(MainContext {
            workspace,
            run_loop: CFRunLoop::get_main(),
            shared,
            result: RefCell::new(None),
            observers,
            notification_center,
        });
        let mut source_context = CFRunLoopSourceContext {
            version: 0,
            info: (&mut *context as *mut MainContext).cast(),
            retain: None,
            release: None,
            copyDescription: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform,
        };
        let source = unsafe { CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &mut source_context) };
        anyhow::ensure!(!source.is_null(), "create the workspace runloop source");
        let source = unsafe { CFRunLoopSource::wrap_under_create_rule(source) };
        // Common modes is a registration selector. CFRunLoopRun runs default mode.
        context
            .run_loop
            .add_source(&source, unsafe { kCFRunLoopCommonModes });
        *lock(&context.shared.signal) = Some(Signal {
            source,
            run_loop: context.run_loop.clone(),
        });
        // Seed/registration-time gaps may have arrived before signaling existed.
        // Queue initial recovery before the first turn; known state is a no-op.
        context.shared.wake();
        *service = Some(Arc::downgrade(&context.shared));
        Ok(Some((Self { context }, failure)))
    }

    fn wait(&self) -> anyhow::Result<()> {
        loop {
            // Completion is delivered by the source even if it was queued before
            // this first turn, so a pre-run CFRunLoopStop cannot be lost.
            CFRunLoop::run_current();
            if let Some(result) = self.context.result.borrow_mut().take() {
                return result;
            }
            // An unexpected external stop fails attribution. Only the terminal
            // coordinator event remains; keep its completion source serviced.
            self.context.shared.fail();
        }
    }
}

fn application_available(load: impl FnOnce() -> bool, prohibit: impl FnOnce() -> bool) -> bool {
    if !load() {
        warn!("macOS workspace services could not initialize; source application attribution is unavailable");
        return false;
    }
    if !prohibit() {
        warn!("the clipboard service could not enter background-only mode; source application attribution is unavailable");
        return false;
    }
    true
}

impl Drop for MainService {
    fn drop(&mut self) {
        *lock(&SERVICE) = None;
        self.context.shared.fail();
        // Late callers have closed replies. Serialize with any last signaler
        // before invalidating and releasing CF's reference to our boxed context.
        if let Some(signal) = lock(&self.context.shared.signal).take() {
            unsafe { CFRunLoopSourceInvalidate(signal.source.as_concrete_TypeRef()) };
        }
    }
}

/// Sample a primitive cursor before reading changeCount; this performs no Cocoa
/// query and never advances past an event that is recorded after the sample.
pub(crate) fn source_observation() -> Option<Observation> {
    let service = lock(&SERVICE).as_ref().and_then(Weak::upgrade)?;
    let epoch = service.observed_epoch.load(Ordering::Acquire);
    let service_id = lock(&service.history).service_id;
    Some(Observation { service_id, epoch })
}

/// Return fresh foreground metadata only if current exclusions also permit the
/// covered generation interval. Unknown/ambiguous history returns no identity.
/// Call only from workers; waiting on main would prevent its own reply.
pub(crate) fn source_identity(
    previous: Option<(i64, Observation)>,
    generation: i64,
    excluded: &[String],
) -> Option<SourceIdentity> {
    if MainThreadMarker::new().is_some() {
        return None;
    }
    let service = lock(&SERVICE).as_ref().and_then(Weak::upgrade)?;
    let (reply, response) = mpsc::channel();
    if !lock(&service.queue).admit(Request {
        previous,
        generation,
        excluded: excluded.to_vec(),
        reply,
    }) {
        return None;
    }
    service.wake();
    response.recv().ok().flatten()
}

fn panic_result(payload: Box<dyn std::any::Any + Send>) -> anyhow::Error {
    std::mem::forget(payload);
    anyhow!("the daemon coordinator panicked")
}

async fn coordinate<F>(future: F, mut failure: watch::Receiver<bool>) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    // Catch future panics before unwinding out of block_on so the existing
    // runtime still executes shutdown_timeout(Duration::ZERO).
    tokio::select! {
        biased;
        _ = failure.changed() => Err(anyhow!("the macOS workspace service failed")),
        result = AssertUnwindSafe(future).catch_unwind() => result.map_err(panic_result)?,
    }
}

/// Keep Cocoa on original main, and join only after terminal completion on main.
pub(crate) fn run<F>(future: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let Some((service, failure)) = autoreleasepool(|_| MainService::install())? else {
        // Only AppKit availability failures take this legacy runtime path.
        // With no installed service, attribution is unknown and exclusions fail closed.
        return crate::runtime::run_with_bounded_shutdown(future);
    };
    let shared = Arc::clone(&service.context.shared);
    let coordinator = std::thread::Builder::new()
        .name("daemon-coordinator".into())
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                crate::runtime::run_with_bounded_shutdown(coordinate(future, failure))
            }))
            .unwrap_or_else(|payload| Err(panic_result(payload)));
            shared.complete(result);
        })
        .context("start the daemon coordinator")?;
    let result = service.wait();
    coordinator.join().map_err(panic_result)?;
    // The main service outlives normal capture drain and every coordinator signal.
    drop(service);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_history() -> Shared {
        let (failed, _failure) = watch::channel(false);
        Shared {
            queue: Mutex::new(Queue::default()),
            signal: Mutex::new(None),
            failed,
            history: Mutex::new(ActivationHistory::new()),
            observed_epoch: AtomicU64::new(0),
        }
    }

    fn request(reply: mpsc::Sender<Option<SourceIdentity>>) -> Request {
        Request {
            previous: None,
            generation: 1,
            excluded: Vec::new(),
            reply,
        }
    }

    fn boundary(history: &ActivationHistory) -> Observation {
        Observation {
            service_id: history.service_id,
            epoch: history.epoch,
        }
    }

    fn excluded(bundle: &str) -> Vec<String> {
        vec![bundle.to_owned()]
    }

    fn identity(name: &str) -> Option<SourceIdentity> {
        Some(SourceIdentity {
            bundle_id: Some(format!("com.test.{name}")),
            name: Some(name.into()),
        })
    }

    #[test]
    fn unavailable_appkit_degrades_without_attempting_application_setup() {
        assert!(!application_available(
            || false,
            || panic!("application setup must not run when AppKit did not load")
        ));
    }

    #[test]
    fn unavailable_background_policy_degrades_after_successful_appkit_load() {
        let policy_attempted = std::cell::Cell::new(false);
        assert!(!application_available(
            || true,
            || {
                policy_attempted.set(true);
                false
            }
        ));
        assert!(policy_attempted.get());
    }

    #[test]
    fn available_appkit_and_background_policy_allow_service_installation() {
        assert!(application_available(|| true, || true));
    }

    #[test]
    fn each_request_resolves_fresh_owned_identity_and_disconnected_callers_are_safe() {
        let mut queue = Queue::default();
        let (first, first_reply) = mpsc::channel();
        let (gone, gone_reply) = mpsc::channel();
        let (last, last_reply) = mpsc::channel();
        assert!(queue.admit(request(first)));
        assert!(queue.admit(request(gone)));
        assert!(queue.admit(request(last)));
        drop(gone_reply);
        let mut names = ["TextEdit", "Safari", "Preview"].into_iter();
        resolve_requests(std::mem::take(&mut queue.requests), |_| {
            identity(names.next().unwrap())
        });
        assert_eq!(first_reply.try_recv().unwrap(), identity("TextEdit"));
        assert_eq!(last_reply.try_recv().unwrap(), identity("Preview"));
        assert!(names.next().is_none());
    }

    #[test]
    fn failure_disconnects_pending_and_late_requests_but_still_accepts_completion() {
        let mut queue = Queue::default();
        let (reply, response) = mpsc::channel();
        assert!(queue.admit(request(reply)));
        queue.close();
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        let (late, late_response) = mpsc::channel();
        assert!(!queue.admit(request(late)));
        assert!(matches!(
            late_response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        queue.complete(Err(anyhow!("service failed")));
        assert_eq!(
            queue.completion.take().unwrap().unwrap_err().to_string(),
            "service failed"
        );
    }

    #[test]
    fn completion_before_the_first_turn_closes_pending_and_preserves_startup_error() {
        let mut queue = Queue::default();
        let (reply, response) = mpsc::channel();
        assert!(queue.admit(request(reply)));
        queue.complete(Err(anyhow!("startup failed")));
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            queue.completion.take().unwrap().unwrap_err().to_string(),
            "startup failed"
        );
    }

    #[test]
    fn queue_overload_disconnects_the_unadmitted_caller() {
        let mut queue = Queue::default();
        for _ in 0..MAX_REQUESTS {
            assert!(queue.admit(request(mpsc::channel().0)));
        }
        let (reply, response) = mpsc::channel();
        assert!(!queue.admit(request(reply)));
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        queue.close();
    }

    #[test]
    fn startup_requires_a_previous_observation_only_when_exclusions_exist() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        assert!(!history.allows(None, 1, &excluded("Safari")));
        assert!(history.allows(None, 1, &[]));
    }

    #[test]
    fn excluded_to_allowed_transition_drops_until_a_clean_allowed_observation() {
        let mut history = ActivationHistory::new();
        history.record(Some("Safari".into()));
        let before_copy = boundary(&history);
        history.record(Some("TextEdit".into()));
        assert!(!history.allows(Some((10, before_copy)), 11, &excluded("Safari")));
        let after_discard = boundary(&history);
        assert!(history.allows(Some((11, after_discard)), 12, &excluded("Safari")));
    }

    #[test]
    fn pause_acknowledgement_does_not_make_an_excluded_to_allowed_resume_safe() {
        let mut history = ActivationHistory::new();
        history.record(Some("Safari".into()));
        // Private mode discards the generation and acknowledges its pre-read
        // observation without requesting identity or reading any representation.
        let paused_observation = boundary(&history);
        history.deactivate(Some("Safari".into()));
        history.record(Some("TextEdit".into()));
        assert!(!history.allows(Some((15, paused_observation)), 16, &excluded("Safari")));
        // An unchanged allowed observation after resume establishes a new
        // covered interval; a later allowed copy can then be captured normally.
        assert!(history.allows(Some((16, boundary(&history))), 17, &excluded("Safari")));
    }

    #[test]
    fn rapid_excluded_round_trip_keeps_every_intermediate_activation() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let before_copy = boundary(&history);
        history.record(Some("Safari".into()));
        history.record(Some("TextEdit".into()));
        history.record(Some("Safari".into()));
        assert!(!history.allows(Some((20, before_copy)), 21, &excluded("Safari")));
    }

    #[test]
    fn an_acknowledgement_never_clears_events_after_its_pre_read_sample() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let sampled_before_unchanged_read = boundary(&history);
        history.record(Some("Safari".into()));
        history.record(Some("TextEdit".into()));
        // The worker acknowledges the unchanged generation later, using the
        // earlier sample rather than the newer epoch present at acknowledgement.
        assert!(!history.allows(
            Some((30, sampled_before_unchanged_read)),
            31,
            &excluded("Safari")
        ));
        assert!(!history.allows(
            Some((30, sampled_before_unchanged_read)),
            31,
            &excluded("Safari")
        ));
    }

    #[test]
    fn unknown_gap_and_getter_disagreement_drop_but_clean_observations_recover() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let previous = boundary(&history);
        history.reconcile(Some("Preview"));
        assert!(!history.allows(Some((40, previous)), 41, &excluded("Safari")));
        let recovered = boundary(&history);
        assert!(history.allows(Some((41, recovered)), 42, &excluded("Safari")));
        history.record(None);
        history.reconcile(Some("Preview"));
        assert!(!history.allows(Some((41, recovered)), 42, &excluded("Safari")));
    }

    #[test]
    fn evicted_coverage_is_unknown_and_history_remains_bounded() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let evicted = boundary(&history);
        for _ in 0..MAX_ACTIVATIONS {
            history.record(Some("TextEdit".into()));
        }
        assert_eq!(history.events.len(), MAX_ACTIVATIONS);
        assert!(!history.allows(Some((50, evicted)), 51, &excluded("Safari")));
        assert!(history.allows(Some((50, boundary(&history))), 51, &excluded("Safari")));
    }

    #[test]
    fn current_exclusion_config_is_evaluated_against_retained_bundle_ids() {
        let mut history = ActivationHistory::new();
        history.record(Some("Safari".into()));
        let previous = boundary(&history);
        history.deactivate(Some("Safari".into()));
        history.record(Some("TextEdit".into()));
        assert!(history.allows(Some((60, previous)), 61, &[]));
        assert!(history.allows(Some((60, previous)), 61, &excluded("Preview")));
        assert!(!history.allows(Some((60, previous)), 61, &excluded("Safari")));
    }

    #[test]
    fn missing_activation_after_deactivation_is_unknown_coverage() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let previous = boundary(&history);
        history.deactivate(Some("TextEdit".into()));
        history.reconcile(Some("Preview"));
        assert!(!history.allows(Some((65, previous)), 66, &excluded("Safari")));
    }

    #[test]
    fn empty_request_recovery_allows_a_later_clean_copy_but_retains_gap_exclusion() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        let before_gap = boundary(&lock(&shared.history));
        shared.record(None);
        let unresolved = boundary(&lock(&shared.history));
        let mut getter_calls = 0;
        process_requests(VecDeque::new(), &shared, || {
            getter_calls += 1;
            identity("TextEdit")
        });
        assert_eq!(getter_calls, 1);
        let history = lock(&shared.history);
        let recovered = boundary(&history);
        assert_eq!(
            shared.observed_epoch.load(Ordering::Acquire),
            recovered.epoch
        );
        assert_eq!(history.events.len(), 4);
        assert!(!history.allows(Some((90, before_gap)), 91, &excluded("com.apple.Safari")));
        assert!(!history.allows(Some((90, unresolved)), 91, &excluded("com.apple.Safari")));
        drop(history);
        // The clipboard is observed unchanged after notified-state recovery.
        // Its pre-count sample can now be the boundary for the next allowed Copy.
        let (reply, response) = mpsc::channel();
        let request = Request {
            previous: Some((91, recovered)),
            generation: 92,
            excluded: excluded("com.apple.Safari"),
            reply,
        };
        process_requests(VecDeque::from([request]), &shared, || identity("TextEdit"));
        assert_eq!(response.try_recv().unwrap(), identity("TextEdit"));
    }

    #[test]
    fn initial_empty_control_turn_recovers_without_trusting_an_unobserved_startup_generation() {
        let shared = shared_history();
        process_requests(VecDeque::new(), &shared, || identity("TextEdit"));
        let history = lock(&shared.history);
        assert_eq!(history.active.as_deref(), Some("com.test.TextEdit"));
        assert!(!history.allows(None, 1, &excluded("com.apple.Safari")));
        // Only a clean unchanged generation sample after initial recovery can
        // establish coverage for a later newly copied allowed value.
        assert!(history.allows(
            Some((1, boundary(&history))),
            2,
            &excluded("com.apple.Safari")
        ));
    }

    #[test]
    fn empty_request_recovery_repairs_unmatched_deactivation_without_erasing_its_gap() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        let before_deactivation = boundary(&lock(&shared.history));
        shared.deactivate(Some("com.test.TextEdit".into()));
        process_requests(VecDeque::new(), &shared, || identity("TextEdit"));
        let history = lock(&shared.history);
        assert_eq!(history.active.as_deref(), Some("com.test.TextEdit"));
        assert!(!history.allows(
            Some((93, before_deactivation)),
            94,
            &excluded("com.apple.Safari")
        ));
        assert!(history.allows(
            Some((94, boundary(&history))),
            95,
            &excluded("com.apple.Safari")
        ));
    }

    #[test]
    fn empty_control_turn_with_known_identity_does_not_run_a_getter() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        process_requests(VecDeque::new(), &shared, || {
            panic!("known empty turn needs no getter")
        });
    }

    #[test]
    fn empty_recovery_getter_runs_outside_history_lock_and_preserves_a_concurrent_gap() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        let before_gap = boundary(&lock(&shared.history));
        shared.record(None);
        process_requests(VecDeque::new(), &shared, || {
            assert!(shared.history.try_lock().is_ok());
            shared.record(None);
            identity("TextEdit")
        });
        let history = lock(&shared.history);
        assert!(history.active.is_none());
        let unresolved = boundary(&history);
        assert!(!history.allows(Some((96, before_gap)), 97, &excluded("com.apple.Safari")));
        // Even a future unchanged cursor must not certify the earlier getter.
        assert!(!history.allows(Some((97, unresolved)), 98, &excluded("com.apple.Safari")));
        assert!(
            history
                .events
                .iter()
                .filter(|event| event.bundle_id.is_none())
                .count()
                >= 2
        );
    }

    #[test]
    fn newer_known_activation_during_recovery_is_not_overwritten_by_the_getter() {
        let shared = shared_history();
        shared.record(None);
        process_requests(VecDeque::new(), &shared, || {
            shared.record(Some("com.apple.Safari".into()));
            identity("TextEdit")
        });
        let history = lock(&shared.history);
        assert_eq!(history.active.as_deref(), Some("com.apple.Safari"));
        let newer = boundary(&history);
        assert!(!history.allows(Some((98, newer)), 99, &excluded("com.apple.Safari")));
    }

    #[test]
    fn off_main_gap_established_before_decision_lock_rejects_the_generation() {
        let shared = Arc::new(shared_history());
        shared.record(Some("com.test.TextEdit".into()));
        let previous = boundary(&lock(&shared.history));
        let (reply, response) = mpsc::channel();
        let request = Request {
            previous: Some((67, previous)),
            generation: 68,
            excluded: excluded("com.apple.Safari"),
            reply,
        };
        process_requests(VecDeque::from([request]), &shared, || {
            // Simulate already-copied metadata, then a concurrent delivered gap
            // before the production decision/reconciliation lock is acquired.
            let metadata = identity("TextEdit");
            let (established, gap_recorded) = mpsc::channel();
            let recorder = Arc::clone(&shared);
            let notification = std::thread::spawn(move || {
                recorder.record(None);
                established.send(()).unwrap();
            });
            gap_recorded.recv().unwrap();
            notification.join().unwrap();
            metadata
        });
        assert!(response.try_recv().unwrap().is_none());
        let history = lock(&shared.history);
        assert_eq!(shared.observed_epoch.load(Ordering::Acquire), history.epoch);
        assert!(history.active.is_none());
        assert!(!history.allows(
            Some((68, boundary(&history))),
            69,
            &excluded("com.apple.Safari")
        ));
    }

    #[test]
    fn generation_reset_or_foreign_service_observation_is_unknown() {
        let mut history = ActivationHistory::new();
        history.record(Some("TextEdit".into()));
        let mut previous = boundary(&history);
        assert!(!history.allows(Some((70, previous)), 70, &excluded("Safari")));
        assert!(!history.allows(Some((70, previous)), 69, &excluded("Safari")));
        previous.service_id += 1;
        assert!(!history.allows(Some((70, previous)), 71, &excluded("Safari")));
    }

    #[test]
    fn epoch_exhaustion_never_wraps_into_trusted_coverage() {
        let mut history = ActivationHistory::new();
        history.epoch = u64::MAX;
        history.record(Some("TextEdit".into()));
        assert!(history.events.is_empty());
        assert!(!history.allows(Some((80, boundary(&history))), 81, &excluded("Safari")));
    }

    #[tokio::test]
    async fn coordinator_panic_becomes_a_terminal_error_inside_the_runtime() {
        let (_failed, failure) = watch::channel(false);
        let result = coordinate(
            async {
                panic!("coordinator test");
            },
            failure,
        )
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "the daemon coordinator panicked"
        );
    }

    #[tokio::test]
    async fn already_failed_service_terminates_without_polling_the_daemon() {
        let (failed, failure) = watch::channel(false);
        failed.send_replace(true);
        let result = coordinate(
            async {
                panic!("daemon must not be polled");
            },
            failure,
        )
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "the macOS workspace service failed"
        );
    }
}
