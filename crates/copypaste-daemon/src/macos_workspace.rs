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
static SERVICE: Mutex<Option<Weak<Shared>>> = Mutex::new(None);
static NEXT_SERVICE_ID: AtomicU64 = AtomicU64::new(1);

use crate::clipboard::source_coverage::{ActivationHistory, Decision};
pub(crate) use crate::clipboard::source_coverage::{Observation, SourceIdentity};

fn new_history() -> ActivationHistory {
    let id = NEXT_SERVICE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .unwrap_or(0);
    ActivationHistory::new(id)
}

struct Request {
    previous: Option<(i64, Observation)>,
    generation: i64,
    sample: Observation,
    reply: mpsc::Sender<Option<Decision>>,
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
        history.record(bundle_id);
        drop(history);
        // Unknown notification state must recover on main even when there are
        // no changed-pasteboard requests. Known activation wakes are cheap.
        self.wake();
    }

    fn deactivate(&self, bundle_id: Option<String>) {
        let mut history = lock(&self.history);
        history.deactivate(bundle_id);
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
            if history.active_is_known() {
                return;
            }
            history.observation().epoch
        };
        // Notified gaps/unmatched deactivations can recover without requiring
        // the next Copy; unchanged observations can then establish coverage.
        // Keep all old records; do not retroactively certify their interval.
        let identity = resolve();
        let mut history = lock(&shared.history);
        if history.observation().epoch != sample_epoch {
            // A newer notified event owns coverage. Never publish an earlier
            // getter as Known after it and let an idle cursor trust stale state.
            return;
        }
        history.reconcile(identity.as_ref().and_then(|app| app.bundle_id.as_deref()));
        return;
    }
    resolve_requests(requests, |request| {
        // All Cocoa work stays outside the mutex, including recovery getters.
        let sample_epoch = lock(&shared.history).observation().epoch;
        let identity = resolve();
        let mut history = lock(&shared.history);
        if history.observation().epoch != sample_epoch {
            return None;
        }
        history.reconcile(identity.as_ref().and_then(|app| app.bundle_id.as_deref()));
        Some(history.decide(
            request.previous,
            request.generation,
            request.sample,
            identity,
        ))
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
    mut resolve: impl FnMut(&Request) -> Option<Decision>,
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
            history: Mutex::new(new_history()),
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
    let observation = lock(&service.history).observation();
    Some(observation)
}

/// Resolve a generation's retained coverage and optional foreground metadata.
/// Fresh display identity is independent of the retained admission evidence.
/// Call only from workers; waiting on main would prevent its own reply.
pub(crate) fn source_decision(
    previous: Option<(i64, Observation)>,
    generation: i64,
    sample: Observation,
) -> Option<Decision> {
    if MainThreadMarker::new().is_some() {
        return None;
    }
    let service = lock(&SERVICE).as_ref().and_then(Weak::upgrade)?;
    let (reply, response) = mpsc::channel();
    if !lock(&service.queue).admit(Request {
        previous,
        generation,
        sample,
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
            history: Mutex::new(new_history()),
        }
    }
    fn identity(name: &str) -> Option<SourceIdentity> {
        Some(SourceIdentity {
            bundle_id: Some(format!("com.test.{name}")),
            name: Some(name.into()),
        })
    }
    fn request(shared: &Shared, reply: mpsc::Sender<Option<Decision>>) -> Request {
        let sample = lock(&shared.history).observation();
        Request {
            previous: Some((1, sample)),
            generation: 2,
            sample,
            reply,
        }
    }
    #[test]
    fn unavailable_appkit_does_not_attempt_setup() {
        assert!(!application_available(
            || false,
            || panic!("unexpected setup")
        ));
        assert!(!application_available(|| true, || false));
        assert!(application_available(|| true, || true));
    }
    #[test]
    fn failed_queue_disconnects_pending_and_late_callers() {
        let shared = shared_history();
        let (reply, response) = mpsc::channel();
        let mut queue = Queue::default();
        assert!(queue.admit(request(&shared, reply)));
        queue.close();
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        let (reply, response) = mpsc::channel();
        assert!(!queue.admit(request(&shared, reply)));
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        queue.complete(Ok(()));
        assert!(queue.completion.take().unwrap().is_ok());
    }
    #[test]
    fn queue_is_bounded() {
        let shared = shared_history();
        let mut queue = Queue::default();
        for _ in 0..MAX_REQUESTS {
            let (reply, _) = mpsc::channel();
            assert!(queue.admit(request(&shared, reply)));
        }
        let (reply, _) = mpsc::channel();
        assert!(!queue.admit(request(&shared, reply)));
    }
    #[test]
    fn each_request_resolves_owned_metadata_without_holding_history_lock() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        let (reply, response) = mpsc::channel();
        process_requests(VecDeque::from([request(&shared, reply)]), &shared, || {
            assert!(shared.history.try_lock().is_ok());
            identity("TextEdit")
        });
        assert_eq!(
            response.recv().unwrap().unwrap().identity,
            identity("TextEdit")
        );
    }
    #[test]
    fn newer_notification_during_getter_cannot_publish_stale_known_state() {
        let shared = shared_history();
        shared.record(None);
        process_requests(VecDeque::new(), &shared, || {
            shared.record(Some("com.test.Safari".into()));
            identity("TextEdit")
        });
        let sample = lock(&shared.history).observation();
        let (reply, response) = mpsc::channel();
        process_requests(VecDeque::from([request(&shared, reply)]), &shared, || {
            identity("Safari")
        });
        let decision = response.recv().unwrap().unwrap();
        assert_eq!(decision.identity, identity("Safari"));
        assert!(decision.fence(Some(sample), 2));
        assert!(!decision.coverage.allows(&["com.test.Safari".into()]));
    }
    #[test]
    fn request_getter_with_concurrent_gap_fails_closed() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        let (reply, response) = mpsc::channel();
        process_requests(VecDeque::from([request(&shared, reply)]), &shared, || {
            shared.record(None);
            identity("TextEdit")
        });
        assert!(response.recv().unwrap().is_none());
    }
    #[test]
    fn known_empty_control_turn_does_not_run_getter() {
        let shared = shared_history();
        shared.record(Some("com.test.TextEdit".into()));
        process_requests(VecDeque::new(), &shared, || panic!("unexpected getter"));
    }
    #[tokio::test]
    async fn coordinator_panic_becomes_terminal_error() {
        let (_failed, failure) = watch::channel(false);
        let result = coordinate(async { panic!("coordinator test") }, failure).await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "the daemon coordinator panicked"
        );
    }
    #[tokio::test]
    async fn failed_service_does_not_poll_daemon() {
        let (failed, failure) = watch::channel(false);
        failed.send_replace(true);
        let result = coordinate(async { panic!("must not be polled") }, failure).await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "the macOS workspace service failed"
        );
    }
}
