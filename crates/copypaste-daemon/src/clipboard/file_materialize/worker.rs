//! The thread that owns staged plaintext until it is gone.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use tracing::warn;

use super::cadence::SweepCadence;
use super::report::SweepReport;
use super::sweep::Sweeper;

/// Passes the dying thread runs. Each is bounded by the sweep budget, so a
/// staging tree bigger than one pass is still taken; the cap stops a directory
/// somebody else keeps writing into from holding the thread for ever.
const FINAL_PASSES: u32 = 16;

pub(super) struct CleanupWorker {
    stop: Arc<(Mutex<bool>, Condvar)>,
    alive: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CleanupWorker {
    /// `startup` is the pass that ran before the worker existed. Starting at
    /// `interval` regardless would give plaintext the startup sweep could not
    /// remove another whole `MAX_AGE` on disk.
    ///
    /// Returns only once the thread is running. `spawn` returning proves a
    /// thread was created, not that it reached its loop, and a worker that
    /// never ran is not an owner for the plaintext its caller is about to write.
    pub(super) fn start(
        sweeper: Sweeper,
        access: Arc<Mutex<()>>,
        max_age: Duration,
        interval: Duration,
        startup: SweepReport,
    ) -> io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_stop = Arc::clone(&stop);
        let alive = Arc::new(AtomicBool::new(true));
        let worker_alive = Arc::clone(&alive);
        let (running, started) = channel();
        let handle = spawn(move || {
            if running.send(()).is_err() {
                return;
            }
            let mut owned = Ownership {
                sweeper,
                access,
                alive: worker_alive,
                stopped: false,
            };
            let mut cadence = SweepCadence::new(interval);
            let mut wait = cadence.after(&startup);
            loop {
                let (lock, wake) = &*worker_stop;
                let stopped = lock.lock().unwrap_or_else(|held| held.into_inner());
                let (stopped, _) = wake
                    .wait_timeout_while(stopped, wait, |stopped| !*stopped)
                    .unwrap_or_else(|held| held.into_inner());
                if *stopped {
                    owned.stopped = true;
                    break;
                }
                drop(stopped);
                wait = cadence.after(&owned.sweep(max_age));
            }
        })?;
        Self::handshake(stop, alive, handle, &started)
    }

    fn handshake(
        stop: Arc<(Mutex<bool>, Condvar)>,
        alive: Arc<AtomicBool>,
        handle: JoinHandle<()>,
        started: &Receiver<()>,
    ) -> io::Result<Self> {
        if started.recv().is_err() {
            let _ = handle.join();
            return Err(io::Error::other(
                "the paste-file cleanup worker stopped before it started",
            ));
        }
        Ok(Self {
            stop,
            alive,
            handle: Some(handle),
        })
    }

    /// Occupying a slot is not ownership. A worker whose thread has panicked or
    /// returned will never sweep again, and the plaintext it was holding would
    /// otherwise stay on disk under a sweeper that no longer exists.
    ///
    /// The flag, not `is_finished`, is what closes the window: a dying thread
    /// is still unfinished for the whole of its final sweep, and plaintext
    /// staged in that window would be handed to an owner that is on its way out.
    pub(super) fn is_running(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
            && self
                .handle
                .as_ref()
                .is_some_and(|handle| !handle.is_finished())
    }
}

/// The staged plaintext a running worker owns.
///
/// A worker that stops without being asked is otherwise noticed only by the
/// next paste-back, which may never come, so the thread that owned the bytes
/// takes them on its way out. A paste-back racing this loses its staged copy,
/// which the next paste re-derives from the encrypted row; decrypted bytes left
/// with no owner are not recoverable that way.
struct Ownership {
    sweeper: Sweeper,
    access: Arc<Mutex<()>>,
    alive: Arc<AtomicBool>,
    stopped: bool,
}

impl Ownership {
    fn sweep(&mut self, max_age: Duration) -> SweepReport {
        let _access = self.access.lock().unwrap_or_else(|held| held.into_inner());
        self.sweeper.pass(SystemTime::now(), max_age)
    }
}

impl Drop for Ownership {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        if self.stopped {
            return;
        }
        // Deadline zero: nothing is waiting on these bytes any more.
        let mut report = self.sweep(Duration::ZERO);
        let mut passes = 1;
        while report.truncated && passes < FINAL_PASSES {
            report = self.sweep(Duration::ZERO);
            passes += 1;
        }
        warn!(
            passes,
            unfinished = report.unfinished(),
            "the paste-file cleanup worker stopped without being asked and took the plaintext it owned"
        );
    }
}

fn spawn(body: impl FnOnce() + Send + 'static) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("paste-file-cleanup".to_string())
        .spawn(body)
}

impl Drop for CleanupWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.stop;
        *lock.lock().unwrap_or_else(|held| held.into_inner()) = true;
        wake.notify_one();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// How a test worker dies once it is running. Death after the handshake is the
/// case `is_running` exists for, and neither a panic nor a plain return can be
/// provoked from outside the thread.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum Doom {
    Return,
    Panic,
}

#[cfg(test)]
impl CleanupWorker {
    pub(super) fn doomed(doom: Doom) -> io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let (running, started) = channel();
        let handle = spawn(move || {
            if running.send(()).is_err() {
                return;
            }
            if let Doom::Panic = doom {
                panic!("injected paste-file cleanup worker panic");
            }
        })?;
        Self::handshake(stop, Arc::new(AtomicBool::new(true)), handle, &started)
    }

    pub(super) fn wait_until_dead(&self, within: Duration) {
        let deadline = std::time::Instant::now() + within;
        while self.is_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!self.is_running(), "setup: the worker had to die");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_over(directory: &std::path::Path, interval: Duration) -> CleanupWorker {
        let fd = rustix::fs::open(
            directory,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap();
        CleanupWorker::start(
            Sweeper::new(fd),
            Arc::new(Mutex::new(())),
            Duration::from_secs(60),
            interval,
            SweepReport::default(),
        )
        .unwrap()
    }

    #[test]
    fn a_worker_is_running_when_start_returns() {
        let directory = tempfile::tempdir().unwrap();

        let worker = worker_over(directory.path(), Duration::from_secs(60));

        assert!(worker.is_running());
    }

    #[test]
    fn a_worker_that_returned_or_panicked_is_not_running() {
        for doom in [Doom::Return, Doom::Panic] {
            let worker = CleanupWorker::doomed(doom).unwrap();
            worker.wait_until_dead(Duration::from_secs(5));
            assert!(!worker.is_running());
        }
    }

    /// An ordinary stop is not a death: it leaves staged files inside their
    /// deadline for the next start-up sweep. Only a worker that stopped without
    /// being asked takes them, because only then is there nothing left to ask.
    #[test]
    fn stopping_a_worker_leaves_its_staged_files_for_the_next_start() {
        let directory = tempfile::tempdir().unwrap();
        let staged = directory.path().join("inside-the-deadline.bin");
        std::fs::write(&staged, b"decrypted").unwrap();

        let worker = worker_over(directory.path(), Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(20));
        drop(worker);

        assert!(
            staged.exists(),
            "an ordinary shutdown swept the staging area"
        );
    }

    #[test]
    fn stopping_a_worker_joins_its_thread_mid_cadence() {
        let directory = tempfile::tempdir().unwrap();

        let worker = worker_over(directory.path(), Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(20));

        assert!(worker.is_running());
        drop(worker);
    }
}
