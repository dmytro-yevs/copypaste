//! Creating the daemon child, and binding its lifetime to this process.
//!
//! Separate from the supervisor because the supervisor's concern is *when* to
//! start and stop the service, and this module's is what a started child is on
//! each platform. Only Windows needs anything beyond `Command::spawn`.

use std::path::Path;
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use super::child::{ChildExitCode, ChildState};
use super::startup_diagnostics;
#[cfg(windows)]
use super::startup_diagnostics::JobStage;
use super::{ChildProcess, MSG_START_FAILED};
use crate::backend::{BackendError, Result};

pub(super) fn spawn_process(binary: &Path) -> Result<Box<dyn ChildProcess>> {
    let mut command = Command::new(binary);
    command
        .arg("--foreground")
        // A child holding the app's descriptors keeps them open past a crash.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    platform(&mut command);

    let child = command.spawn().map_err(|error| {
        startup_diagnostics::child_spawn_failed(&error);
        // The OS error can contain the binary path and local username.
        BackendError::Internal(MSG_START_FAILED.into())
    })?;
    startup_diagnostics::child_started(binary, child.id());
    adopt(child)
}

#[cfg(not(windows))]
fn platform(_command: &mut Command) {}

/// The daemon is a console-subsystem binary, so a windowed process starting it
/// gets a console on screen — over the app, in the taskbar, and closable by a
/// user who then loses their clipboard history. Its handles are already null,
/// so suppressing the console costs nothing.
#[cfg(windows)]
fn platform(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn adopt(child: Child) -> Result<Box<dyn ChildProcess>> {
    Ok(Box::new(child))
}

/// Bind the child to a job object that dies with this process, or refuse to
/// start.
///
/// `Supervisor::drop` and the `RunEvent::Exit` hook both stop the daemon, and
/// **neither runs when the app is killed outright** — Task Manager, a crash, an
/// installer closing it. `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is the only thing
/// that survives that. An unbound child is therefore exactly the orphan this
/// exists to prevent, so it is terminated, reaped, and reported as a failed
/// start. A user told the service did not start can press the button again; a
/// user with an orphan holding the pipe cannot, because the next launch adopts
/// it and — not having started it (ADR-0004) — will never kill it.
#[cfg(windows)]
fn adopt(child: Child) -> Result<Box<dyn ChildProcess>> {
    adopt_with(child, bind)
}

#[cfg(windows)]
fn bind(child: &Child) -> std::result::Result<win32job::Job, win32job::JobError> {
    use std::os::windows::io::AsRawHandle;

    let job = win32job::Job::create()?;
    let mut info = job.query_extended_limit_info()?;
    info.limit_kill_on_job_close();
    job.set_extended_limit_info(&info)?;
    job.assign_process(child.as_raw_handle() as isize)?;
    Ok(job)
}

#[cfg(windows)]
fn adopt_with<B>(mut child: Child, bind: B) -> Result<Box<dyn ChildProcess>>
where
    B: Fn(&Child) -> std::result::Result<win32job::Job, win32job::JobError>,
{
    match bind(&child) {
        Ok(job) => {
            startup_diagnostics::job_adopted();
            Ok(Box::new(JobBoundChild::new(child, job)))
        }
        Err(error) => {
            let (stage, code) = job_error_identity(&error);
            startup_diagnostics::job_adoption_failed(stage, code);
            tracing::warn!(%error, "the background service could not be bound to this process");
            let _ = child.kill();
            if let Ok(status) = child.wait() {
                startup_diagnostics::child_exited(ChildExitCode::from_status(status));
            }
            Err(BackendError::Internal(MSG_START_FAILED.into()))
        }
    }
}

#[cfg(windows)]
fn job_error_identity(error: &win32job::JobError) -> (JobStage, Option<i32>) {
    match error {
        win32job::JobError::CreateFailed(error) => (JobStage::Create, error.raw_os_error()),
        win32job::JobError::GetInfoFailed(error) => (JobStage::Query, error.raw_os_error()),
        win32job::JobError::SetInfoFailed(error) => (JobStage::Configure, error.raw_os_error()),
        win32job::JobError::AssignFailed(error) => (JobStage::Assign, error.raw_os_error()),
        _ => (JobStage::Unknown, None),
    }
}

/// A child whose job handle lives exactly as long as it does.
///
/// The job is dropped with this value, which is what makes `Supervisor::stop`'s
/// take-and-drop a second, kernel-enforced kill rather than only a
/// `TerminateProcess` that a wedged child could outlive. `kill` therefore drops
/// the job *before* waiting: an unreachable `TerminateProcess` must not leave
/// the blocking wait ahead of the fallback that would end the child.
#[cfg(windows)]
struct JobBoundChild {
    child: Child,
    job: Option<win32job::Job>,
}

#[cfg(windows)]
impl ChildProcess for JobBoundChild {
    fn state(&mut self) -> std::io::Result<ChildState> {
        self.child.try_wait().map(|status| match status {
            Some(status) => ChildState::Exited(ChildExitCode::from_status(status)),
            None => ChildState::Running,
        })
    }

    fn kill(&mut self) -> std::io::Result<()> {
        let killed = self.child.kill();
        // Closing the job is `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so it ends
        // the child even when the handle-based terminate did not.
        self.job.take();
        killed
    }

    fn reap(&mut self) -> std::io::Result<()> {
        self.child.wait().map(|_| ())
    }
}

#[cfg(windows)]
impl JobBoundChild {
    fn new(child: Child, job: win32job::Job) -> Self {
        Self {
            child,
            job: Some(job),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_errors_do_not_expose_the_binary_path() {
        let path = Path::new("C:/Users/a-person/private/copypaste-daemon");
        let error = spawn_process(path).err().expect("the path must not exist");
        assert_eq!(error.to_string(), MSG_START_FAILED);
        assert!(!error.to_string().contains("a-person"));
    }

    /// The flag is the whole of the console fix, so it is worth pinning by
    /// value: `CREATE_NO_WINDOW` is `0x0800_0000`, and a build that picked up a
    /// different constant would suppress nothing and show a console window on
    /// every launch.
    #[cfg(windows)]
    #[test]
    fn the_console_suppression_flag_is_the_documented_one() {
        assert_eq!(
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW,
            0x0800_0000
        );
    }

    #[cfg(windows)]
    #[test]
    fn every_job_error_keeps_its_stage_and_numeric_code() {
        let errors = [
            (
                win32job::JobError::CreateFailed(std::io::Error::from_raw_os_error(5)),
                "create",
            ),
            (
                win32job::JobError::GetInfoFailed(std::io::Error::from_raw_os_error(5)),
                "query",
            ),
            (
                win32job::JobError::SetInfoFailed(std::io::Error::from_raw_os_error(5)),
                "configure",
            ),
            (
                win32job::JobError::AssignFailed(std::io::Error::from_raw_os_error(5)),
                "assign",
            ),
        ];
        for (error, expected_stage) in errors {
            let (stage, code) = job_error_identity(&error);
            assert_eq!(startup_diagnostics::job_stage_name(stage), expected_stage);
            assert_eq!(code, Some(5));
        }
    }

    #[cfg(windows)]
    fn sleeping_child(output: Stdio) -> Child {
        Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(output)
            .stderr(Stdio::null())
            .spawn()
            .expect("ping.exe")
    }

    /// A real child through the production binding, not a job the test built
    /// for itself. What has to be true is not that a job exists but that
    /// closing it takes the child with it — which is the only thing standing
    /// between a Task-Manager kill of the app and an orphaned daemon.
    #[cfg(windows)]
    #[test]
    fn the_production_binding_makes_a_child_die_with_its_job() {
        let mut child = sleeping_child(Stdio::null());
        let job = bind(&child).expect("the production binding");
        drop(job);

        // Timed, not code-checked. The exit code a terminated process reports
        // is the kernel's business and varies with what killed it.
        let started = std::time::Instant::now();
        let status = child.wait().expect("the child is reapable");
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "the job did not end the child: waited {elapsed:?}, status {status:?}"
        );
    }

    /// Binding fails closed. Before this, a job that could not be created —
    /// already inside a job that forbids nesting, a policy-restricted desktop —
    /// left a live, unbound daemon behind and called the start a success.
    ///
    /// The child is proved dead through its own inherited handle: an exclusive
    /// open of the file it writes to is refused while it lives and granted once
    /// it is gone.
    #[cfg(windows)]
    #[test]
    fn a_child_that_cannot_be_bound_is_ended_and_the_start_fails() {
        use std::os::windows::fs::OpenOptionsExt;

        fn opened_exclusively(path: &Path) -> std::io::Result<std::fs::File> {
            std::fs::OpenOptions::new()
                .write(true)
                .share_mode(0)
                .open(path)
        }

        let directory = tempfile::tempdir().expect("tempdir");
        let held = directory.path().join("held-open.txt");
        let child = sleeping_child(Stdio::from(
            std::fs::File::create(&held).expect("create the held file"),
        ));
        assert!(
            opened_exclusively(&held).is_err(),
            "the child is not holding the file, so this proves nothing"
        );

        let error = adopt_with(child, |_| {
            Err(win32job::JobError::CreateFailed(std::io::Error::other(
                "a job object could not be created",
            )))
        })
        .err()
        .expect("an unbindable child must not be handed back");

        assert_eq!(error.to_string(), MSG_START_FAILED);
        opened_exclusively(&held).expect("the unbound child outlived the failed start");
    }

    /// `kill` releases the job before the blocking wait that follows it. A
    /// `TerminateProcess` that does not land must not leave `reap` waiting in
    /// front of the fallback that would have ended the child.
    #[cfg(windows)]
    #[test]
    fn killing_releases_the_job_before_anything_waits() {
        let mut bound = JobBoundChild::new(
            sleeping_child(Stdio::null()),
            win32job::Job::create().expect("a job object"),
        );

        bound.kill().expect("terminate");

        assert!(bound.job.is_none(), "the job outlived the kill");
        bound.reap().expect("the child is reapable");
    }
}
