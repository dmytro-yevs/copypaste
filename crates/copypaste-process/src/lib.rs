//! Bounded waits for short-lived helper processes.
#![forbid(unsafe_code)]

#[cfg(unix)]
pub use unix::wait_or_kill;

#[cfg(unix)]
mod unix {
    use std::io;
    use std::process::{Child, ExitStatus};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    use rustix::io::Errno;
    use rustix::process::{waitid, Pid, WaitId, WaitIdOptions};

    /// Blocks until `child` exits or `timeout` elapses, without polling.
    ///
    /// Returns `None` when the deadline passed; the child is then killed and
    /// reaped. A waiter thread observes the exit with `WNOWAIT`, so the child
    /// stays unreaped and its pid cannot be reused before `child` reaps it.
    pub fn wait_or_kill(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        let pid = Pid::from_child(child);
        let (exited, observed) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("copypaste-child-wait".into())
            .spawn(move || {
                while let Err(Errno::INTR) = waitid(
                    WaitId::Pid(pid),
                    WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
                ) {}
                let _ = exited.send(());
            })?;
        match observed.recv_timeout(timeout) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => child.wait().map(Some),
            Err(RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                child.wait()?;
                Ok(None)
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::wait_or_kill;

    #[test]
    fn returns_the_exit_status_of_a_prompt_child() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 3"])
            .spawn()
            .unwrap();
        let status = wait_or_kill(&mut child, Duration::from_secs(5)).unwrap();
        assert_eq!(status.and_then(|status| status.code()), Some(3));
    }

    #[test]
    fn kills_and_reaps_a_child_past_its_deadline() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 5"])
            .spawn()
            .unwrap();
        let started = Instant::now();
        assert!(wait_or_kill(&mut child, Duration::from_millis(10))
            .unwrap()
            .is_none());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(child.try_wait().unwrap().is_some());
    }
}
