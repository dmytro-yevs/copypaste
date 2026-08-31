use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::child::{ChildExitCode, ChildProcess, ChildState};
use super::quit::{ExitRequest, QuitGate, ReapCompletion, ReapReservation};

pub(super) struct OwnedChild {
    slot: Arc<Mutex<Option<Box<dyn ChildProcess>>>>,
    reap_pending: Arc<AtomicBool>,
    reap_finished: Arc<tokio::sync::Notify>,
    start_pending: Arc<AtomicBool>,
    terminal_failure: Arc<AtomicBool>,
    #[cfg(test)]
    reap_delivery_barrier: Mutex<Option<ReapDeliveryBarrier>>,
    #[cfg(test)]
    transfer_barrier: Mutex<Option<TransferBarrier>>,
}

#[cfg(test)]
struct ReapDeliveryBarrier {
    delivered: Arc<AtomicBool>,
    release: std::sync::mpsc::Receiver<()>,
    finished: Arc<AtomicBool>,
}

#[cfg(test)]
pub(super) struct ReapDeliveryHold {
    delivered: Arc<AtomicBool>,
    release: Option<std::sync::mpsc::Sender<()>>,
    finished: Arc<AtomicBool>,
}

#[cfg(test)]
impl ReapDeliveryHold {
    pub(super) fn delivered(&self) -> bool {
        self.delivered.load(Ordering::SeqCst)
    }

    pub(super) fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    pub(super) fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(test)]
impl Drop for ReapDeliveryHold {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
struct TransferBarrier {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
pub(super) struct TransferHold {
    pub(super) entered: std::sync::mpsc::Receiver<()>,
    release: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(test)]
impl TransferHold {
    pub(super) fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(test)]
impl Drop for TransferHold {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) struct StartReservation {
    slot: Arc<Mutex<Option<Box<dyn ChildProcess>>>>,
    pending: Arc<AtomicBool>,
}

pub(super) enum BeginReap {
    Started(tokio::sync::oneshot::Receiver<ReapCompletion>),
    AlreadyExited(Option<ReapReservation>),
}

impl Drop for StartReservation {
    fn drop(&mut self) {
        let _slot = self
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.pending.store(false, Ordering::SeqCst);
    }
}

impl Default for OwnedChild {
    fn default() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
            reap_pending: Arc::new(AtomicBool::new(false)),
            reap_finished: Arc::new(tokio::sync::Notify::new()),
            start_pending: Arc::new(AtomicBool::new(false)),
            terminal_failure: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            reap_delivery_barrier: Mutex::new(None),
            #[cfg(test)]
            transfer_barrier: Mutex::new(None),
        }
    }
}

impl OwnedChild {
    pub(super) fn is_reap_pending(&self) -> bool {
        self.reap_pending.load(Ordering::SeqCst)
    }

    pub(super) async fn wait_for_reap_completion(&self) {
        loop {
            let notified = self.reap_finished.notified();
            if !self.is_reap_pending() {
                return;
            }
            notified.await;
        }
    }

    pub(super) fn terminal_failure(&self) -> bool {
        self.terminal_failure.load(Ordering::SeqCst)
    }

    pub(super) fn clear_terminal_failure(&self) {
        self.terminal_failure.store(false, Ordering::SeqCst);
    }

    pub(super) fn terminal_failure_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.terminal_failure)
    }

    pub(super) fn service_is_owned(&self) -> bool {
        let mut slot = self.lock_slot();
        match slot.as_mut().map(|child| child.state()) {
            Some(Ok(ChildState::Running)) | Some(Err(_)) => true,
            Some(Ok(ChildState::Exited(code))) => {
                self.record_exit(code);
                *slot = None;
                self.reap_pending.store(false, Ordering::SeqCst);
                false
            }
            None => false,
        }
    }

    pub(super) fn child_exited(&self) -> bool {
        let mut slot = self.lock_slot();
        if let Some(Ok(ChildState::Exited(code))) = slot.as_mut().map(|child| child.state()) {
            self.record_exit(code);
            *slot = None;
            self.reap_pending.store(false, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub(super) fn request_quit(&self, gate: &QuitGate) -> ExitRequest {
        let mut slot = self.lock_slot();
        let pending =
            self.reap_pending.load(Ordering::SeqCst) || self.start_pending.load(Ordering::SeqCst);
        let owns = match slot.as_mut().map(|child| child.state()) {
            Some(Ok(ChildState::Running)) | Some(Err(_)) => true,
            Some(Ok(ChildState::Exited(code))) => {
                self.record_exit(code);
                *slot = None;
                self.reap_pending.store(false, Ordering::SeqCst);
                false
            }
            None => false,
        };
        if gate.is_reserved() {
            return gate.request(pending || owns);
        }
        if self.terminal_failure() {
            gate.reserve_failure();
            return ExitRequest::Failure;
        }
        gate.request(pending || owns)
    }

    pub(super) fn reserve_start(&self, gate: &QuitGate) -> Option<StartReservation> {
        let _slot = self.lock_slot();
        if gate.is_reserved() || self.reap_pending.load(Ordering::SeqCst) {
            return None;
        }
        self.start_pending.store(true, Ordering::SeqCst);
        Some(StartReservation {
            slot: Arc::clone(&self.slot),
            pending: Arc::clone(&self.start_pending),
        })
    }

    pub(super) fn spawn_if_start_admitted<S>(
        &self,
        gate: &QuitGate,
        spawn: &S,
        binary: &std::path::Path,
    ) -> crate::backend::Result<bool>
    where
        S: Fn(&std::path::Path) -> crate::backend::Result<Box<dyn ChildProcess>>,
    {
        let mut slot = self.lock_slot();
        if gate.is_reserved() {
            return Ok(false);
        }
        let child = spawn(binary)?;
        debug_assert!(slot.is_none(), "a live child cannot be replaced");
        *slot = Some(child);
        Ok(true)
    }

    pub(super) fn begin_reap(&self, reservation: Option<ReapReservation>) -> BeginReap {
        #[cfg(test)]
        let transfer = self
            .transfer_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(child) = ({
            let mut slot = self.lock_slot();
            let child = slot.take();
            self.reap_pending.store(child.is_some(), Ordering::SeqCst);
            #[cfg(test)]
            if let Some(barrier) = transfer {
                let _ = barrier.entered.send(());
                let _ = barrier.release.recv();
            }
            child
        }) else {
            return BeginReap::AlreadyExited(reservation);
        };
        let slot = Arc::clone(&self.slot);
        let pending = Arc::clone(&self.reap_pending);
        let reap_finished = Arc::clone(&self.reap_finished);
        let terminal_failure = Arc::clone(&self.terminal_failure);
        #[cfg(test)]
        let delivery = self
            .reap_delivery_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let (sent, received) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let mut child = child;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.reap()));
            let result = match result {
                Ok(Ok(code)) if code.is_success() => {
                    terminal_failure.store(false, Ordering::SeqCst);
                    Ok(code)
                }
                Ok(Ok(code)) => {
                    terminal_failure.store(true, Ordering::SeqCst);
                    Ok(code)
                }
                Ok(Err(error)) => {
                    let mut restored = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    if restored.is_none() {
                        *restored = Some(child);
                    } else {
                        std::mem::forget(child);
                    }
                    Err(error)
                }
                Err(_) => {
                    let mut restored = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    if restored.is_none() {
                        *restored = Some(child);
                    } else {
                        std::mem::forget(child);
                    }
                    terminal_failure.store(true, Ordering::SeqCst);
                    Err(std::io::Error::other("reap panicked"))
                }
            };
            pending.store(false, Ordering::SeqCst);
            reap_finished.notify_one();
            if sent
                .send(ReapCompletion {
                    result: Some(result),
                    reservation,
                })
                .is_ok()
            {
                #[cfg(test)]
                if let Some(barrier) = delivery {
                    barrier.delivered.store(true, Ordering::SeqCst);
                    let _ = barrier.release.recv();
                    barrier.finished.store(true, Ordering::SeqCst);
                }
            }
        });
        BeginReap::Started(received)
    }

    pub(super) fn forget_on_drop(&self) {
        if let Some(child) = self.lock_slot().take() {
            std::mem::forget(child);
        }
    }

    fn lock_slot(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn ChildProcess>>> {
        self.slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn record_exit(&self, code: ChildExitCode) {
        if !code.is_success() {
            self.terminal_failure.store(true, Ordering::SeqCst);
        }
        super::startup_diagnostics::child_exited(code);
    }

    #[cfg(test)]
    pub(super) fn test_slot(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn ChildProcess>>> {
        self.lock_slot()
    }

    #[cfg(test)]
    pub(super) fn is_reap_pending_for_test(&self) -> bool {
        self.is_reap_pending()
    }

    #[cfg(test)]
    pub(super) fn set_terminal_failure_for_test(&self) {
        self.terminal_failure.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn take_for_test(&self) -> Option<Box<dyn ChildProcess>> {
        let transfer = self
            .transfer_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let mut slot = self.lock_slot();
        let child = slot.take();
        self.reap_pending.store(child.is_some(), Ordering::SeqCst);
        if let Some(barrier) = transfer {
            let _ = barrier.entered.send(());
            let _ = barrier.release.recv();
        }
        child
    }

    #[cfg(test)]
    pub(super) fn hold_next_reap_delivery(&self) -> ReapDeliveryHold {
        let delivered = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let (release, receiver) = std::sync::mpsc::channel();
        *self
            .reap_delivery_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ReapDeliveryBarrier {
            delivered: Arc::clone(&delivered),
            release: receiver,
            finished: Arc::clone(&finished),
        });
        ReapDeliveryHold {
            delivered,
            release: Some(release),
            finished,
        }
    }

    #[cfg(test)]
    pub(super) fn hold_next_transfer(&self) -> TransferHold {
        let (entered, entered_at_transfer) = std::sync::mpsc::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        *self
            .transfer_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(TransferBarrier {
            entered,
            release: wait_for_release,
        });
        TransferHold {
            entered: entered_at_transfer,
            release: Some(release),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum ReapResult {
        Success,
        Nonzero,
        Unknown,
        Io,
        Panic,
    }

    struct TestChild {
        reap: ReapResult,
        exited: Option<ChildExitCode>,
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        release: Option<std::sync::mpsc::Receiver<()>>,
        finished: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl TestChild {
        fn running(reap: ReapResult) -> Self {
            Self {
                reap,
                exited: None,
                entered: None,
                release: None,
                finished: None,
            }
        }
    }

    impl ChildProcess for TestChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            Ok(self
                .exited
                .take()
                .map_or(ChildState::Running, ChildState::Exited))
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            if let Some(release) = self.release.take() {
                let _ = release.recv();
            }
            if let Some(finished) = self.finished.take() {
                let _ = finished.send(());
            }
            match self.reap {
                ReapResult::Success => Ok(ChildExitCode::from_test_code(0)),
                ReapResult::Nonzero => Ok(ChildExitCode::from_test_code(23)),
                ReapResult::Unknown => Ok(ChildExitCode::unavailable()),
                ReapResult::Io => Err(std::io::Error::other("controlled reap error")),
                ReapResult::Panic => panic!("controlled reap panic"),
            }
        }
    }

    #[test]
    fn quit_waits_for_a_child_transfer_and_then_drains_never_allows() {
        let owned = OwnedChild::default();
        let gate = QuitGate::default();
        *owned.test_slot() = Some(Box::new(TestChild::running(ReapResult::Success)));
        let mut transfer_hold = owned.hold_next_transfer();

        std::thread::scope(|scope| {
            let transfer_owned = &owned;
            let transfer = scope.spawn(move || transfer_owned.take_for_test());
            transfer_hold.entered.recv().expect("child was removed");
            let (sent, result) = std::sync::mpsc::channel();
            let quit_owned = &owned;
            scope.spawn(move || {
                sent.send(quit_owned.request_quit(&gate))
                    .expect("report quit")
            });

            assert!(
                matches!(result.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)),
                "quit observed a half-transferred child"
            );
            transfer_hold.release();
            assert_eq!(result.recv().expect("quit completed"), ExitRequest::Drain);
            drop(transfer.join().expect("transfer completed"));
        });
    }

    #[tokio::test]
    async fn direct_begin_reap_records_success_nonzero_unknown_io_and_panic_outcomes() {
        for reap in [
            ReapResult::Success,
            ReapResult::Nonzero,
            ReapResult::Unknown,
            ReapResult::Io,
            ReapResult::Panic,
        ] {
            let owned = OwnedChild::default();
            *owned.test_slot() = Some(Box::new(TestChild::running(reap)));
            let BeginReap::Started(received) = owned.begin_reap(None) else {
                panic!("inserted child begins reaping");
            };
            let (result, reservation) = received.await.expect("reap completion delivered").finish();
            assert!(reservation.is_none());
            if let Ok(code) = result {
                if code.is_success() {
                    assert!(!owned.terminal_failure());
                } else {
                    assert!(owned.terminal_failure());
                }
            }
        }
    }

    #[tokio::test]
    async fn cancelled_reap_delivery_finishes_or_restores_ownership() {
        let successful = OwnedChild::default();
        let (entered, reaper_entered) = tokio::sync::oneshot::channel();
        let (finished, reaper_finished) = tokio::sync::oneshot::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        *successful.test_slot() = Some(Box::new(TestChild {
            reap: ReapResult::Success,
            exited: None,
            entered: Some(entered),
            release: Some(wait_for_release),
            finished: Some(finished),
        }));
        let BeginReap::Started(received) = successful.begin_reap(None) else {
            panic!("inserted child begins reaping");
        };
        reaper_entered.await.expect("reaper started");
        drop(received);
        release.send(()).expect("release reaper");
        reaper_finished.await.expect("reaper finished");
        while successful.is_reap_pending_for_test() {
            tokio::task::yield_now().await;
        }
        assert!(!successful.service_is_owned());

        let restored = OwnedChild::default();
        *restored.test_slot() = Some(Box::new(TestChild::running(ReapResult::Io)));
        let BeginReap::Started(received) = restored.begin_reap(None) else {
            panic!("inserted child begins reaping");
        };
        drop(received);
        while restored.is_reap_pending_for_test() {
            tokio::task::yield_now().await;
        }
        assert!(restored.service_is_owned());
    }

    #[test]
    fn already_exited_child_failure_refuses_quit() {
        for exited in [
            ChildExitCode::from_test_code(23),
            ChildExitCode::unavailable(),
        ] {
            let owned = OwnedChild::default();
            let gate = QuitGate::default();
            *owned.test_slot() = Some(Box::new(TestChild {
                reap: ReapResult::Success,
                exited: Some(exited),
                entered: None,
                release: None,
                finished: None,
            }));
            assert_eq!(owned.request_quit(&gate), ExitRequest::Failure);
        }
    }
}
