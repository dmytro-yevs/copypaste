//! Windows process-exit confirmation after an acknowledged IPC shutdown.

use crate::error::CliError;
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Completion {
    AlreadyStopped,
    ExitedZero,
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitResult {
    Object,
    Timeout,
    Abandoned,
    Failed,
}

#[cfg(any(windows, test))]
pub(crate) trait ProcessWait {
    fn wait(&mut self, milliseconds: u32) -> Result<WaitResult, ()>;
    fn exit_code(&self) -> Result<u32, ()>;
}

#[cfg(any(windows, test))]
fn remaining_milliseconds_at(
    deadline: Instant,
    now: impl FnOnce() -> Instant,
) -> Result<u32, CliError> {
    let milliseconds = deadline
        .checked_duration_since(now())
        .and_then(|remaining| u32::try_from(remaining.as_millis()).ok())
        .filter(|milliseconds| *milliseconds > 0)
        .ok_or(CliError::DaemonTimeout)?;
    Ok(milliseconds)
}

#[cfg(any(windows, test))]
pub(crate) fn wait_for_exit_with<P: ProcessWait>(
    process: P,
    deadline: Instant,
) -> Result<Completion, CliError> {
    wait_for_exit_with_clock(process, deadline, Instant::now)
}

#[cfg(any(windows, test))]
fn wait_for_exit_with_clock<P: ProcessWait>(
    mut process: P,
    deadline: Instant,
    now: impl Fn() -> Instant,
) -> Result<Completion, CliError> {
    tokio::task::block_in_place(move || {
        let milliseconds = remaining_milliseconds_at(deadline, &now)?;
        let wait = process.wait(milliseconds);
        remaining_milliseconds_at(deadline, &now)?;
        match wait {
            Ok(WaitResult::Object) => {
                let exit_code = process.exit_code();
                remaining_milliseconds_at(deadline, &now)?;
                if exit_code == Ok(0) {
                    Ok(Completion::ExitedZero)
                } else {
                    Err(completion_failure())
                }
            }
            Ok(WaitResult::Timeout) => Err(CliError::DaemonTimeout),
            Ok(WaitResult::Abandoned | WaitResult::Failed) | Err(()) => Err(completion_failure()),
        }
    })
}

pub(crate) fn completion_failure() -> CliError {
    CliError::local("the CopyPaste daemon could not be confirmed stopped")
}

#[cfg(windows)]
pub(crate) struct NativeProcess(winsafe::guard::CloseHandleGuard<winsafe::HPROCESS>);

#[cfg(windows)]
impl ProcessWait for NativeProcess {
    fn wait(&mut self, milliseconds: u32) -> Result<WaitResult, ()> {
        use winsafe::co;

        match self.0.WaitForSingleObject(Some(milliseconds)) {
            Ok(co::WAIT::OBJECT_0) => Ok(WaitResult::Object),
            Ok(co::WAIT::TIMEOUT) => Ok(WaitResult::Timeout),
            Ok(co::WAIT::ABANDONED) => Ok(WaitResult::Abandoned),
            Ok(co::WAIT::FAILED) | Err(_) => Ok(WaitResult::Failed),
            Ok(_) => Ok(WaitResult::Failed),
        }
    }

    fn exit_code(&self) -> Result<u32, ()> {
        self.0.GetExitCodeProcess().map_err(|_| ())
    }
}

#[cfg(windows)]
pub(crate) fn open_for_stream(
    stream: &copypaste_ipc::transport::Stream,
    deadline: Instant,
) -> Result<NativeProcess, CliError> {
    use winsafe::co;

    remaining_milliseconds_at(deadline, Instant::now)?;
    let pid = stream.peer_process_id().map_err(|_| completion_failure())?;
    remaining_milliseconds_at(deadline, Instant::now)?;
    let process = winsafe::HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
        false,
        pid,
    )
    .map_err(|_| completion_failure())?;
    remaining_milliseconds_at(deadline, Instant::now)?;
    Ok(NativeProcess(process))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;

    struct FakeProcess {
        wait: Result<WaitResult, ()>,
        exit: Result<u32, ()>,
        milliseconds: Arc<AtomicU32>,
    }

    impl ProcessWait for FakeProcess {
        fn wait(&mut self, milliseconds: u32) -> Result<WaitResult, ()> {
            self.milliseconds.store(milliseconds, Ordering::SeqCst);
            self.wait
        }

        fn exit_code(&self) -> Result<u32, ()> {
            self.exit
        }
    }

    fn fake(wait: Result<WaitResult, ()>, exit: Result<u32, ()>) -> (FakeProcess, Arc<AtomicU32>) {
        let milliseconds = Arc::new(AtomicU32::new(0));
        (
            FakeProcess {
                wait,
                exit,
                milliseconds: Arc::clone(&milliseconds),
            },
            milliseconds,
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn waits_once_for_a_zero_exit_and_keeps_the_remaining_deadline() {
        let (process, milliseconds) = fake(Ok(WaitResult::Object), Ok(0));
        let deadline = Instant::now() + Duration::from_secs(5);

        assert!(matches!(
            wait_for_exit_with(process, deadline),
            Ok(Completion::ExitedZero)
        ));
        assert!((1..=5_000).contains(&milliseconds.load(Ordering::SeqCst)));
    }

    #[test]
    fn already_stopped_is_distinct_from_a_confirmed_zero_exit() {
        assert_ne!(Completion::AlreadyStopped, Completion::ExitedZero);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_wait_timeout_uses_the_cli_timeout_exit_code() {
        let (process, _) = fake(Ok(WaitResult::Timeout), Ok(0));
        let err = wait_for_exit_with(process, Instant::now() + Duration::from_secs(1)).unwrap_err();

        assert_eq!(err.exit_code(), crate::error::EXIT_TIMEOUT);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn terminal_wait_errors_and_nonzero_exit_refuse_completion() {
        for (wait, exit) in [
            (Ok(WaitResult::Abandoned), Ok(0)),
            (Ok(WaitResult::Failed), Ok(0)),
            (Err(()), Ok(0)),
            (Ok(WaitResult::Object), Err(())),
            (Ok(WaitResult::Object), Ok(1)),
        ] {
            let (process, _) = fake(wait, exit);
            let err =
                wait_for_exit_with(process, Instant::now() + Duration::from_secs(1)).unwrap_err();
            assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_expired_deadline_never_starts_a_wait() {
        let (process, milliseconds) = fake(Ok(WaitResult::Object), Ok(0));
        let err = wait_for_exit_with(process, Instant::now()).unwrap_err();

        assert_eq!(err.exit_code(), crate::error::EXIT_TIMEOUT);
        assert_eq!(milliseconds.load(Ordering::SeqCst), 0);
    }

    struct DeadlineConsumingProcess {
        clock: Arc<std::sync::Mutex<Instant>>,
    }

    impl ProcessWait for DeadlineConsumingProcess {
        fn wait(&mut self, _milliseconds: u32) -> Result<WaitResult, ()> {
            *self.clock.lock().unwrap() += Duration::from_secs(1);
            Ok(WaitResult::Object)
        }

        fn exit_code(&self) -> Result<u32, ()> {
            Ok(0)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_wait_that_consumes_the_deadline_cannot_report_a_clean_exit() {
        let now = Instant::now();
        let clock = Arc::new(std::sync::Mutex::new(now));
        let process = DeadlineConsumingProcess {
            clock: Arc::clone(&clock),
        };

        let err = wait_for_exit_with_clock(process, now + Duration::from_secs(1), move || {
            *clock.lock().unwrap()
        })
        .unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_TIMEOUT);
    }

    struct HeldProcess {
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        release: mpsc::Receiver<()>,
        dropped: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl ProcessWait for HeldProcess {
        fn wait(&mut self, _milliseconds: u32) -> Result<WaitResult, ()> {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            let _ = self.release.recv();
            Ok(WaitResult::Object)
        }

        fn exit_code(&self) -> Result<u32, ()> {
            Ok(0)
        }
    }

    impl Drop for HeldProcess {
        fn drop(&mut self) {
            if let Some(dropped) = self.dropped.take() {
                let _ = dropped.send(());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_does_not_orphan_the_process_handle_during_the_wait() {
        let (entered, entered_wait) = tokio::sync::oneshot::channel();
        let (release, release_wait) = mpsc::channel();
        let (dropped, mut dropped_wait) = tokio::sync::oneshot::channel();
        let process = HeldProcess {
            entered: Some(entered),
            release: release_wait,
            dropped: Some(dropped),
        };
        let task = tokio::spawn(async move {
            wait_for_exit_with(process, Instant::now() + Duration::from_secs(1))
        });

        entered_wait.await.expect("wait began");
        task.abort();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut dropped_wait)
                .await
                .is_err(),
            "the handle must remain owned until the in-place wait returns"
        );
        release.send(()).expect("wait remains connected");
        tokio::time::timeout(Duration::from_secs(1), &mut dropped_wait)
            .await
            .expect("the handle is released when the wait completes")
            .expect("the process handle dropped");
    }
}
