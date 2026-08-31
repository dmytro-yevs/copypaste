use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use super::{child::ChildExitCode, Supervisor};
use tauri::{AppHandle, Runtime};

pub(super) const MSG_QUIT_FAILED: &str = "CopyPaste could not safely stop the background service.";

pub(super) struct ReapCompletion {
    pub(super) result: Option<std::io::Result<ChildExitCode>>,
    pub(super) reservation: Option<ReapReservation>,
}
impl ReapCompletion {
    pub(super) fn finish(mut self) -> (std::io::Result<ChildExitCode>, Option<ReapReservation>) {
        (
            self.result.take().expect("completion has one result"),
            self.reservation.take(),
        )
    }
}
impl Drop for ReapCompletion {
    fn drop(&mut self) {
        drop(self.reservation.take());
    }
}

pub(super) enum ReapReservation {
    Ordinary(ShutdownPermit),
    Update(UpdateDrainPermit),
}

impl ReapReservation {
    pub(super) fn ordinary(gate: QuitGate) -> Self {
        Self::Ordinary(ShutdownPermit(Some(gate)))
    }

    pub(super) fn into_update(self) -> Option<UpdateDrainPermit> {
        match self {
            Self::Update(permit) => Some(permit),
            Self::Ordinary(_) => None,
        }
    }

    pub(super) fn into_ordinary(self) -> Option<ShutdownPermit> {
        match self {
            Self::Ordinary(permit) => Some(permit),
            Self::Update(_) => None,
        }
    }

    pub(super) fn retain_ordinary_failure(self) {
        if let Self::Ordinary(mut permit) = self {
            let _ = permit.0.take();
        }
    }
}

pub(crate) struct UpdateDrainPermit(Option<QuitGate>);

impl UpdateDrainPermit {
    pub(super) fn reserve(gate: QuitGate) -> Option<Self> {
        if gate.reserve() {
            Some(Self(Some(gate)))
        } else {
            None
        }
    }
}

impl Drop for UpdateDrainPermit {
    fn drop(&mut self) {
        if let Some(gate) = self.0.take() {
            gate.failed();
        }
    }
}

pub(crate) struct ShutdownPermit(pub(super) Option<QuitGate>);
impl ShutdownPermit {
    pub(crate) fn allow_exit(mut self) {
        if let Some(gate) = self.0.take() {
            gate.allow_exit();
        }
    }
}
impl Drop for ShutdownPermit {
    fn drop(&mut self) {
        if let Some(gate) = self.0.take() {
            gate.failed();
        }
    }
}

pub(crate) fn show_failure<R: Runtime>(app: &AppHandle<R>, mut presentation: FailurePresentation) {
    use tauri_plugin_dialog::DialogExt as _;

    app.dialog()
        .message(MSG_QUIT_FAILED)
        .title("CopyPaste")
        .show(move |_| presentation.ack());
}

pub(crate) fn finish_failure(supervisor: &Supervisor, show: impl FnOnce(FailurePresentation)) {
    show(supervisor.failure_presentation());
}

pub(crate) struct FailurePresentation {
    terminal_failure: Arc<AtomicBool>,
    gate: QuitGate,
    acknowledged: bool,
}

impl FailurePresentation {
    pub(super) fn new(terminal_failure: Arc<AtomicBool>, gate: QuitGate) -> Self {
        Self {
            terminal_failure,
            gate,
            acknowledged: false,
        }
    }

    pub(crate) fn ack(&mut self) {
        self.terminal_failure.store(false, Ordering::SeqCst);
        self.gate.failed();
        self.acknowledged = true;
    }
}

impl Drop for FailurePresentation {
    fn drop(&mut self) {
        if !self.acknowledged {
            self.gate.failed();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitRequest {
    Allow,
    Drain,
    AlreadyDraining,
    Failure,
}

const IDLE: u8 = 0;
const DRAINING: u8 = 1;
const ALLOW_EXIT: u8 = 2;

/// Serializes ordinary app quit without making a second exit request recursive.
#[derive(Clone)]
pub(super) struct QuitGate(std::sync::Arc<AtomicU8>);

impl Default for QuitGate {
    fn default() -> Self {
        Self(std::sync::Arc::new(AtomicU8::new(IDLE)))
    }
}

impl QuitGate {
    pub(super) fn request(&self, owns_or_is_draining: bool) -> ExitRequest {
        match self.0.load(Ordering::SeqCst) {
            ALLOW_EXIT => ExitRequest::Allow,
            DRAINING => ExitRequest::AlreadyDraining,
            IDLE if owns_or_is_draining => {
                if self
                    .0
                    .compare_exchange(IDLE, DRAINING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    ExitRequest::Drain
                } else {
                    self.request(owns_or_is_draining)
                }
            }
            IDLE => {
                self.0.store(ALLOW_EXIT, Ordering::SeqCst);
                ExitRequest::Allow
            }
            _ => unreachable!("quit gate has a known state"),
        }
    }

    pub(super) fn failed(&self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }

    pub(super) fn allow_exit(&self) {
        self.0.store(ALLOW_EXIT, Ordering::SeqCst);
    }

    pub(super) fn is_reserved(&self) -> bool {
        self.0.load(Ordering::SeqCst) != IDLE
    }

    fn reserve(&self) -> bool {
        self.0
            .compare_exchange(IDLE, DRAINING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub(super) fn reserve_failure(&self) {
        debug_assert!(self.reserve());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_requests_start_one_drain_and_final_exit_does_not_recurse() {
        let gate = QuitGate::default();

        assert_eq!(gate.request(true), ExitRequest::Drain);
        assert_eq!(gate.request(true), ExitRequest::AlreadyDraining);
        gate.allow_exit();
        assert_eq!(gate.request(true), ExitRequest::Allow);
        assert_eq!(gate.request(true), ExitRequest::Allow);
    }

    #[test]
    fn failed_drain_can_be_requested_again() {
        let gate = QuitGate::default();
        assert_eq!(gate.request(true), ExitRequest::Drain);
        gate.failed();
        assert_eq!(gate.request(true), ExitRequest::Drain);
    }
}
