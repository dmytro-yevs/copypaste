//! Private, time-bounded staging for native file paste-back.

mod cadence;
mod failure;
mod report;
mod sweep;
#[cfg(test)]
mod testutil;
mod worker;

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use copypaste_core::FileMetadata;
use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;
use tracing::warn;

use report::SweepReport;
use sweep::Sweeper;
use worker::CleanupWorker;

const DIRECTORY_MODE: Mode = Mode::RWXU;
const FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
const MAX_AGE: Duration = Duration::from_secs(10 * 60);
/// Matched to `MAX_AGE`: the sweep enforces a deadline nothing waits on, so
/// sampling it more often than it can expire is a resident timer for nothing.
/// A sweep that leaves work behind is retried far sooner — see `sweep::RETRY_MIN`.
const CLEANUP_INTERVAL: Duration = MAX_AGE;

pub(super) struct StagingArea {
    root: PathBuf,
    /// Opened once. Re-`open`ing and re-`fchmod`ing per paste-back put six
    /// syscalls on the interactive path to reassert a mode nothing changes, and
    /// it is the sweeper's only handle on the directory: a pathname can be
    /// renamed out from under it, this descriptor cannot.
    root_fd: std::os::fd::OwnedFd,
    access: Arc<Mutex<()>>,
    /// Started on the first `materialize` that put plaintext on disk, or at
    /// construction when the start-time sweep could not finish. On a machine
    /// that never pastes a file back and has nothing left over, the sweeper has
    /// nothing to sweep and was costing a resident OS thread for the life of
    /// the daemon.
    ///
    /// Replaceable, not write-once: a worker whose thread has died owns nothing,
    /// and the next paste-back has to be able to install a live one.
    cleanup: Mutex<Option<CleanupWorker>>,
    max_age: Duration,
    interval: Duration,
    #[cfg(test)]
    worker_starts: std::sync::atomic::AtomicU32,
    #[cfg(test)]
    worker_doom: Arc<std::sync::atomic::AtomicBool>,
}

impl StagingArea {
    pub(super) fn new(data_dir: &Path) -> io::Result<Self> {
        Self::with_timing(data_dir, MAX_AGE, CLEANUP_INTERVAL)
    }

    fn with_timing(data_dir: &Path, max_age: Duration, interval: Duration) -> io::Result<Self> {
        let root = std::path::absolute(data_dir)?.join("paste-files");
        let root_fd = open_or_create_root(&root)?;
        // Whatever the previous run left is already decrypted and already past
        // its deadline. If this pass could not finish the job the sweeper has
        // to start now — on a machine that never pastes a file back, nothing
        // else ever will — and it carries this sweeper, so the retry lands at
        // the floor and resumes where the pass stopped.
        let mut sweeper = Sweeper::new(root_fd.try_clone()?);
        let startup = sweeper.pass(SystemTime::now(), max_age);
        let area = Self {
            root,
            root_fd,
            access: Arc::new(Mutex::new(())),
            cleanup: Mutex::new(None),
            max_age,
            interval,
            #[cfg(test)]
            worker_starts: std::sync::atomic::AtomicU32::new(0),
            #[cfg(test)]
            worker_doom: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        if startup.unfinished() {
            // The plaintext this pass could not remove is already decrypted and
            // already past its deadline. Nothing but the worker will ever take
            // it, so failing here is better than handing back a staging area
            // that only looks supervised.
            area.install_worker(sweeper, startup)?;
        }
        Ok(area)
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(test)]
    fn sweep_now(&self, now: SystemTime, max_age: Duration) -> SweepReport {
        Sweeper::new(self.root_fd.try_clone().unwrap()).pass(now, max_age)
    }

    pub(super) fn materialize(&self, bytes: &[u8], metadata: &FileMetadata) -> io::Result<PathBuf> {
        let filename = safe_filename(metadata)?;
        let _access = self.access.lock().unwrap_or_else(|held| held.into_inner());
        self.ensure_cleanup()?;
        self.stage(bytes, filename)
    }

    /// A *running* cleanup worker must exist before any plaintext is created,
    /// and a worker that has died is not one. A descriptor-clone or
    /// thread-spawn failure here prevents staging so that decrypted bytes are
    /// never left with no timed cleanup owner.
    fn ensure_cleanup(&self) -> io::Result<()> {
        if is_running(&self.worker()) {
            return Ok(());
        }
        let sweeper = Sweeper::new(self.clone_root_fd()?);
        #[cfg(test)]
        let sweeper = sweeper.armed(Arc::clone(&self.worker_doom));
        self.install_worker(sweeper, SweepReport::default())
    }

    fn worker(&self) -> MutexGuard<'_, Option<CleanupWorker>> {
        self.cleanup.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn clone_root_fd(&self) -> io::Result<std::os::fd::OwnedFd> {
        #[cfg(test)]
        if let Some(error) = cleanup_fault::injected_clone_failure() {
            return Err(error);
        }
        self.root_fd.try_clone()
    }

    /// Installs a worker unless a running one is already in the slot. A failure
    /// leaves whatever was there, so the caller stays refused rather than
    /// proceeding under a worker that does not exist.
    fn install_worker(&self, sweeper: Sweeper, startup: SweepReport) -> io::Result<()> {
        let mut slot = self.worker();
        if is_running(&slot) {
            return Ok(());
        }
        #[cfg(test)]
        if let Some(error) = cleanup_fault::injected_spawn_failure() {
            return Err(error);
        }
        let worker = CleanupWorker::start(
            sweeper,
            Arc::clone(&self.access),
            self.max_age,
            self.interval,
            startup,
        )?;
        // Assigning drops a worker whose thread has already finished, so the
        // join in its `Drop` returns at once.
        *slot = Some(worker);
        #[cfg(test)]
        self.worker_starts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn stage(&self, bytes: &[u8], filename: &str) -> io::Result<PathBuf> {
        let content_id = copypaste_core::binary_item_id(bytes);
        let content_fd = open_or_create_content_dir(&self.root_fd, &content_id)?;

        match open_regular_at(&content_fd, filename) {
            Ok(file) => {
                renew(&file)?;
                return Ok(self.root.join(content_id).join(filename));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let temporary_name = format!(".{}.tmp", uuid::Uuid::new_v4());
        let temporary_fd = rustix::fs::openat(
            &content_fd,
            temporary_name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        )?;
        let mut temporary = File::from(temporary_fd);
        let write_result = (|| {
            temporary.write_all(bytes)?;
            if let Some(injected) = injected_failure() {
                return Err(injected);
            }
            // No `sync_all`: `write -> publish -> read` crosses a process
            // boundary, so the bytes must be visible to another process, which
            // `write(2)` alone makes them. They need never reach stable
            // storage — a staging file lost to a crash is re-derived from the
            // encrypted row on the next paste, and the fsync was tens of
            // milliseconds of interactive latency for a file `MAX_AGE` deletes
            // within ten minutes anyway.
            rustix::fs::linkat(
                &content_fd,
                temporary_name.as_str(),
                &content_fd,
                filename,
                AtFlags::empty(),
            )
            .map_err(io::Error::from)
        })();
        drop(temporary);
        let rollback = rollback_temporary(&content_fd, &temporary_name);

        match write_result {
            Ok(()) => {
                rollback?;
                Ok(self.root.join(content_id).join(filename))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                rollback?;
                let file = open_regular_at(&content_fd, filename)?;
                renew(&file)?;
                Ok(self.root.join(content_id).join(filename))
            }
            // The write failure is the more informative of the two, and the
            // rollback failure has already been reported.
            Err(error) => Err(error),
        }
    }
}

fn is_running(slot: &Option<CleanupWorker>) -> bool {
    slot.as_ref().is_some_and(CleanupWorker::is_running)
}

/// Removing the temporary is the only thing that unmakes the decrypted bytes
/// this function's caller just wrote. A silent failure here was how a partial
/// or complete plaintext payload could outlive a staging error unreported.
fn rollback_temporary(content_fd: &std::os::fd::OwnedFd, name: &str) -> io::Result<()> {
    let removed = match injected_failure() {
        Some(injected) => Err(injected),
        None => match rustix::fs::unlinkat(content_fd, name, AtFlags::empty()) {
            // Already gone is the outcome the rollback wanted.
            Ok(()) | Err(Errno::NOENT) => Ok(()),
            Err(error) => Err(io::Error::from(error)),
        },
    };
    if let Err(error) = &removed {
        warn!(
            error_kind = ?error.kind(),
            "could not remove the temporary paste-file; the decrypted bytes stay until the staging sweeper takes them"
        );
    }
    removed
}

#[cfg(not(test))]
fn injected_failure() -> Option<io::Error> {
    None
}

#[cfg(test)]
use fault::injected as injected_failure;

/// Failure injection for the window between creating the decrypted temporary
/// file and publishing it. Neither `linkat` nor `unlinkat` can be made to fail
/// on demand from a test — the staging directory is re-`fchmod`ed on every open
/// and the temporary name is a fresh uuid — and the branch they select decides
/// whether plaintext is left on disk with nothing to remove it.
#[cfg(test)]
mod fault {
    use std::cell::Cell;
    use std::io;

    #[derive(Clone, Copy)]
    enum Mode {
        Error,
        Panic,
    }

    thread_local! {
        static PUBLISH_FAILS: Cell<Option<Mode>> = const { Cell::new(None) };
    }

    pub(super) fn with_publish_failure<T>(body: impl FnOnce() -> T) -> T {
        armed(Mode::Error, body)
    }

    /// An unwind out of `stage` leaves the temporary behind with none of the
    /// rollback an `Err` return would have run.
    pub(super) fn with_publish_panic<T>(body: impl FnOnce() -> T) -> T {
        armed(Mode::Panic, body)
    }

    fn armed<T>(mode: Mode, body: impl FnOnce() -> T) -> T {
        struct Disarm;
        impl Drop for Disarm {
            fn drop(&mut self) {
                PUBLISH_FAILS.with(|armed| armed.set(None));
            }
        }
        PUBLISH_FAILS.with(|armed| armed.set(Some(mode)));
        let _disarm = Disarm;
        body()
    }

    pub(super) fn injected() -> Option<io::Error> {
        match PUBLISH_FAILS.with(Cell::get) {
            None => None,
            Some(Mode::Error) => Some(io::Error::from(io::ErrorKind::StorageFull)),
            Some(Mode::Panic) => panic!("injected paste-file staging panic"),
        }
    }
}

/// Failure injection for the cleanup-worker startup path: descriptor clone
/// and thread spawn. These are the two operations between `ensure_cleanup`
/// deciding to start and the `CleanupWorker` existing.
#[cfg(test)]
mod cleanup_fault {
    use std::cell::Cell;
    use std::io;

    thread_local! {
        static CLONE_FAILS: Cell<bool> = const { Cell::new(false) };
        static SPAWN_FAILS: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn with_clone_failure<T>(body: impl FnOnce() -> T) -> T {
        struct Disarm;
        impl Drop for Disarm {
            fn drop(&mut self) {
                CLONE_FAILS.with(|armed| armed.set(false));
            }
        }
        CLONE_FAILS.with(|armed| armed.set(true));
        let _disarm = Disarm;
        body()
    }

    pub(super) fn with_spawn_failure<T>(body: impl FnOnce() -> T) -> T {
        struct Disarm;
        impl Drop for Disarm {
            fn drop(&mut self) {
                SPAWN_FAILS.with(|armed| armed.set(false));
            }
        }
        SPAWN_FAILS.with(|armed| armed.set(true));
        let _disarm = Disarm;
        body()
    }

    pub(super) fn injected_clone_failure() -> Option<io::Error> {
        CLONE_FAILS
            .with(Cell::get)
            .then(|| io::Error::other("injected fd-clone failure"))
    }

    pub(super) fn injected_spawn_failure() -> Option<io::Error> {
        SPAWN_FAILS
            .with(Cell::get)
            .then(|| io::Error::other("injected thread-spawn failure"))
    }
}

fn safe_filename(metadata: &FileMetadata) -> io::Result<&str> {
    let filename = metadata.filename.as_str();
    if !metadata.is_valid() || filename.contains('/') || filename.as_bytes().contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file metadata did not contain a safe basename",
        ));
    }
    Ok(filename)
}

fn open_or_create_root(root: &Path) -> io::Result<std::os::fd::OwnedFd> {
    match std::fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let fd = rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    rustix::fs::fchmod(&fd, DIRECTORY_MODE)?;
    Ok(fd)
}

fn open_or_create_content_dir(
    root_fd: &std::os::fd::OwnedFd,
    content_id: &str,
) -> io::Result<std::os::fd::OwnedFd> {
    match rustix::fs::mkdirat(root_fd, content_id, DIRECTORY_MODE) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }
    let fd = rustix::fs::openat(
        root_fd,
        content_id,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    rustix::fs::fchmod(&fd, DIRECTORY_MODE)?;
    Ok(fd)
}

fn open_regular_at(dir_fd: &std::os::fd::OwnedFd, filename: &str) -> io::Result<File> {
    let fd = rustix::fs::openat(
        dir_fd,
        filename,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let file = File::from(fd);
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the staging target was not a regular file",
        ));
    }
    Ok(file)
}

fn renew(file: &File) -> io::Result<()> {
    file.set_times(std::fs::FileTimes::new().set_modified(SystemTime::now()))
}

#[cfg(test)]
mod tests {
    use super::testutil::CapturedLog;
    use super::worker::Doom;
    use super::*;

    impl StagingArea {
        fn has_live_cleanup(&self) -> bool {
            is_running(&self.worker())
        }

        fn has_cleanup(&self) -> bool {
            self.worker().is_some()
        }

        fn worker_starts(&self) -> u32 {
            self.worker_starts
                .load(std::sync::atomic::Ordering::Relaxed)
        }

        /// Puts a worker in the slot that reaches its loop and then dies, which
        /// is the state `ensure_cleanup` has to detect and replace.
        fn install_doomed_worker(&self, doom: Doom) {
            let worker = CleanupWorker::doomed(doom).unwrap();
            worker.wait_until_dead(Duration::from_secs(5));
            *self.worker() = Some(worker);
        }

        /// Panics the *running* worker on its next pass, which is how the owner
        /// of plaintext already on disk dies.
        fn doom_the_running_worker(&self) {
            self.worker_doom
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn metadata(filename: impl Into<String>) -> FileMetadata {
        FileMetadata {
            filename: filename.into(),
            mime_type: "application/octet-stream".to_string(),
        }
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
    }

    fn wait_until_gone(path: &Path, within: Duration) {
        let deadline = SystemTime::now() + within;
        while path.exists() && SystemTime::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn materialized_file_is_private_and_preserves_the_user_filename() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();
        let path = staging
            .materialize(b"bytes", &metadata("report.pdf"))
            .unwrap();
        assert_eq!(path.file_name().unwrap(), "report.pdf");
        let content_id = copypaste_core::binary_item_id(b"bytes");
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            std::ffi::OsStr::new(&content_id)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn a_255_byte_unicode_filename_fits_beneath_the_content_directory() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();
        let filename = format!("{}abc", "é".repeat(126));
        assert_eq!(filename.len(), 255);

        let path = staging
            .materialize(b"unicode", &metadata(filename.clone()))
            .unwrap();
        assert_eq!(path.file_name().unwrap(), filename.as_str());
    }

    #[test]
    fn traversal_and_separator_names_are_refused() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();

        for filename in ["../outside", "nested/report.pdf"] {
            let error = staging
                .materialize(b"bytes", &metadata(filename))
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{filename}");
        }
        assert!(!data_dir.path().join("outside").exists());
    }

    #[test]
    fn an_active_data_directory_owns_its_staged_plaintext() {
        let parent = tempfile::tempdir().unwrap();
        let active_data_dir = parent.path().join("custom-daemon-data");
        std::fs::create_dir(&active_data_dir).unwrap();
        let staging = StagingArea::new(&active_data_dir).unwrap();

        let path = staging
            .materialize(b"custom", &metadata("custom.txt"))
            .unwrap();
        assert!(path.starts_with(active_data_dir.join("paste-files")));
    }

    #[test]
    fn the_sweeper_starts_on_the_first_paste_back_and_not_before() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();
        assert!(
            !staging.has_cleanup(),
            "a daemon that never pasted a file back must not carry a sweeper thread"
        );

        staging.materialize(b"bytes", &metadata("a.txt")).unwrap();
        assert!(
            staging.has_live_cleanup(),
            "staged plaintext must be under a running TTL sweeper"
        );
        assert_eq!(staging.worker_starts(), 1);
    }

    #[test]
    fn a_paste_back_that_never_wrote_plaintext_starts_no_sweeper() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();

        staging
            .materialize(b"bytes", &metadata("../outside"))
            .unwrap_err();

        assert!(
            !staging.has_cleanup(),
            "a refused filename put nothing on disk and must not cost a thread"
        );
    }

    /// The failure this guards is silent by construction: a staging error after
    /// the temporary file exists returns `Err`, so the caller reports a failed
    /// paste — while the decrypted bytes stay on disk with no owner.
    #[test]
    fn a_publish_failure_reports_its_rollback_and_leaves_the_plaintext_owned() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_millis(1),
            Duration::from_millis(400),
        )
        .unwrap();
        let capture = CapturedLog::default();

        let logged = capture.record(|| {
            let error = fault::with_publish_failure(|| {
                staging.materialize(b"payload", &metadata("statement.pdf"))
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::StorageFull);
        });

        let content_dir = staging
            .root()
            .join(copypaste_core::binary_item_id(b"payload"));
        let leaked: Vec<PathBuf> = std::fs::read_dir(&content_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(
            leaked.len(),
            1,
            "setup: a temporary must be left: {leaked:?}"
        );
        assert!(
            logged.contains("could not remove the temporary paste-file"),
            "a failed rollback was not reported: {logged}"
        );
        assert!(logged.contains("StorageFull"), "{logged}");
        for secret in [
            "statement",
            ".pdf",
            "payload",
            content_dir.to_str().unwrap(),
        ] {
            assert!(!logged.contains(secret), "the report disclosed {secret}");
        }
        assert!(
            staging.has_live_cleanup(),
            "decrypted plaintext was left on disk with no sweeper"
        );
        wait_until_gone(&leaked[0], Duration::from_secs(5));
        assert!(
            !leaked[0].exists(),
            "the sweeper never removed the plaintext a failed publish left"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_startup_sweep_that_left_plaintext_retries_before_the_ordinary_interval() {
        use std::os::unix::fs::PermissionsExt;

        let data_dir = tempfile::tempdir().unwrap();
        let root = data_dir.path().join("paste-files");
        std::fs::create_dir(&root).unwrap();
        let left_behind = root.join("11111111-1111-1111-1111-111111111111");
        std::fs::create_dir(&left_behind).unwrap();
        let stale = left_behind.join("from-the-last-run.bin");
        std::fs::write(&stale, b"decrypted").unwrap();
        set_modified(&stale, SystemTime::now() - Duration::from_secs(3600));
        std::fs::set_permissions(&left_behind, std::fs::Permissions::from_mode(0o500)).unwrap();
        // Root ignores the mode bits, so the fault cannot be injected there.
        if std::fs::write(left_behind.join(".probe"), b"").is_ok() {
            return;
        }

        let interval = Duration::from_secs(2);
        let staging =
            StagingArea::with_timing(data_dir.path(), Duration::from_secs(60), interval).unwrap();
        assert!(
            staging.has_live_cleanup(),
            "plaintext the startup sweep could not remove must stay under a sweeper"
        );

        std::fs::set_permissions(&left_behind, std::fs::Permissions::from_mode(0o700)).unwrap();
        wait_until_gone(&stale, interval - Duration::from_millis(500));

        assert!(
            !stale.exists(),
            "an unfinished startup sweep waited the whole interval before retrying"
        );
    }

    #[test]
    fn reusing_a_staged_file_renews_the_receiver_window() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(60),
            Duration::from_secs(3600),
        )
        .unwrap();
        let file = staging.materialize(b"same", &metadata("same.txt")).unwrap();
        let now = SystemTime::now();
        set_modified(&file, now - Duration::from_secs(61));

        let reused = staging.materialize(b"same", &metadata("same.txt")).unwrap();
        let report = staging.sweep_now(now, Duration::from_secs(60));

        assert_eq!(reused, file);
        assert!(reused.exists());
        assert_eq!(report.removed, 0, "{report:?}");
    }

    fn staged_content_dirs(staging: &StagingArea) -> Vec<PathBuf> {
        std::fs::read_dir(staging.root())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }

    fn assert_refused_with_nothing_staged(staging: &StagingArea, error: io::Error) {
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!staging.has_live_cleanup());
        let staged = staged_content_dirs(staging);
        assert!(
            staged.is_empty(),
            "no content directory may exist when cleanup could not start: {staged:?}"
        );
    }

    /// B3: a descriptor-clone failure in `ensure_cleanup` must prevent any
    /// plaintext from being created, not just log and proceed.
    #[test]
    fn a_clone_failure_prevents_plaintext_creation() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();

        let error = cleanup_fault::with_clone_failure(|| {
            staging.materialize(b"secret", &metadata("secret.txt"))
        })
        .unwrap_err();

        assert_refused_with_nothing_staged(&staging, error);
    }

    /// B3: a thread-spawn failure in `ensure_cleanup` must prevent any
    /// plaintext from being created, not just log and proceed.
    #[test]
    fn a_spawn_failure_prevents_plaintext_creation() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();

        let error = cleanup_fault::with_spawn_failure(|| {
            staging.materialize(b"secret", &metadata("secret.txt"))
        })
        .unwrap_err();

        assert_refused_with_nothing_staged(&staging, error);
    }

    /// B3: once the cleanup worker is running, a subsequent clone or spawn
    /// failure is harmless — `ensure_cleanup` short-circuits and staging
    /// proceeds because the worker already owns the directory.
    #[test]
    fn cleanup_failure_after_worker_exists_does_not_block_staging() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();

        staging
            .materialize(b"first", &metadata("first.txt"))
            .unwrap();
        assert!(staging.has_live_cleanup());

        let path = cleanup_fault::with_clone_failure(|| {
            cleanup_fault::with_spawn_failure(|| {
                staging.materialize(b"second", &metadata("second.txt"))
            })
        })
        .unwrap();
        assert!(path.exists());
        assert_eq!(staging.worker_starts(), 1, "the worker was restarted");
    }

    /// B3: a worker that has exited or panicked owns nothing. Occupying the
    /// slot must not be mistaken for that — the defect was a `OnceLock` whose
    /// occupancy was read as liveness.
    #[test]
    fn a_dead_worker_is_replaced_before_the_next_plaintext() {
        for doom in [Doom::Return, Doom::Panic] {
            let data_dir = tempfile::tempdir().unwrap();
            let staging = StagingArea::with_timing(
                data_dir.path(),
                Duration::from_millis(1),
                Duration::from_millis(50),
            )
            .unwrap();
            staging.install_doomed_worker(doom);
            assert!(!staging.has_live_cleanup(), "setup: the worker had to die");

            let path = staging
                .materialize(b"after the death", &metadata("after.txt"))
                .unwrap();

            assert!(
                staging.has_live_cleanup(),
                "a dead worker was left in place"
            );
            assert_eq!(staging.worker_starts(), 1);
            wait_until_gone(&path, Duration::from_secs(5));
            assert!(
                !path.exists(),
                "the replacement worker never removed the plaintext"
            );
        }
    }

    /// B3/T-92: the worker that owns staged plaintext dies, and nothing pastes
    /// back afterwards. Waiting for a later `materialize` to notice is not
    /// ownership: on a machine that pastes a file back once, there is no later
    /// one, and the decrypted bytes stay for ever. The deadline here is ten
    /// minutes away and the file's mtime is now, so only the death can account
    /// for it.
    #[test]
    fn a_worker_that_dies_owning_plaintext_takes_it_with_no_second_paste_back() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(600),
            Duration::from_millis(20),
        )
        .unwrap();
        let path = staging
            .materialize(b"payload", &metadata("statement.pdf"))
            .unwrap();
        assert!(staging.has_live_cleanup(), "setup: an owner had to exist");

        staging.doom_the_running_worker();

        wait_until_gone(&path, Duration::from_secs(10));
        assert!(
            !path.exists(),
            "a dead worker left decrypted bytes that only a later paste-back would notice"
        );
        assert!(
            !staging.has_live_cleanup(),
            "the dead worker is still reported as an owner"
        );
        assert_eq!(
            staging.worker_starts(),
            1,
            "the plaintext was accounted for by a second paste-back, not by the death"
        );
        assert!(
            !path.parent().unwrap().exists(),
            "the emptied content directory outlived its owner"
        );
    }

    /// B3: and when the replacement cannot be started, the staging that would
    /// have created plaintext under the dead worker is refused.
    #[test]
    fn a_dead_worker_that_cannot_be_replaced_prevents_plaintext_creation() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::new(data_dir.path()).unwrap();
        staging.install_doomed_worker(Doom::Panic);

        let error = cleanup_fault::with_spawn_failure(|| {
            staging.materialize(b"secret", &metadata("secret.txt"))
        })
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!staging.has_live_cleanup());
        let staged = staged_content_dirs(&staging);
        assert!(staged.is_empty(), "{staged:?}");
    }

    /// B3: startup is where plaintext already exists. If the sweep could not
    /// take it and no worker can be started, construction fails rather than
    /// returning a staging area with unowned decrypted bytes under it.
    #[cfg(unix)]
    #[test]
    fn a_startup_sweep_that_left_plaintext_with_no_worker_fails_closed() {
        use std::os::unix::fs::PermissionsExt;

        let data_dir = tempfile::tempdir().unwrap();
        let root = data_dir.path().join("paste-files");
        std::fs::create_dir(&root).unwrap();
        let left_behind = root.join("11111111-1111-1111-1111-111111111111");
        std::fs::create_dir(&left_behind).unwrap();
        let stale = left_behind.join("from-the-last-run.bin");
        std::fs::write(&stale, b"decrypted").unwrap();
        set_modified(&stale, SystemTime::now() - Duration::from_secs(3600));
        std::fs::set_permissions(&left_behind, std::fs::Permissions::from_mode(0o500)).unwrap();
        // Root ignores the mode bits, so the fault cannot be injected there.
        if std::fs::write(left_behind.join(".probe"), b"").is_ok() {
            return;
        }

        let refused = cleanup_fault::with_spawn_failure(|| {
            StagingArea::with_timing(
                data_dir.path(),
                Duration::from_secs(60),
                Duration::from_secs(2),
            )
            .map(|_| PathBuf::new())
        });

        std::fs::set_permissions(&left_behind, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(refused.unwrap_err().kind(), io::ErrorKind::Other);
    }

    /// B3: the first `materialize` calls are serialized by `access`, so the
    /// race that would start two workers — or stage plaintext under neither —
    /// must not exist.
    #[test]
    fn concurrent_first_paste_backs_start_exactly_one_worker() {
        const WRITERS: usize = 8;
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(60),
            Duration::from_secs(3600),
        )
        .unwrap();

        std::thread::scope(|scope| {
            for index in 0..WRITERS {
                let staging = &staging;
                scope.spawn(move || {
                    let payload = format!("payload-{index}");
                    staging
                        .materialize(payload.as_bytes(), &metadata(format!("{index}.txt")))
                        .unwrap()
                });
            }
        });

        assert_eq!(staging.worker_starts(), 1);
        assert!(staging.has_live_cleanup());
        assert_eq!(staged_content_dirs(&staging).len(), WRITERS);
    }

    /// B3: a panic after the temporary exists is the one path that leaves
    /// plaintext without running the rollback. It happens after
    /// `ensure_cleanup`, so the bytes are already under a live worker — and the
    /// staging lock it poisons must not take the next paste-back down with it.
    #[test]
    fn a_panic_after_the_temporary_exists_leaves_it_under_a_live_worker() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_millis(1),
            Duration::from_millis(50),
        )
        .unwrap();

        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fault::with_publish_panic(|| {
                staging.materialize(b"payload", &metadata("statement.pdf"))
            })
        }));

        assert!(unwound.is_err(), "setup: the staging panic must escape");
        assert!(
            staging.has_live_cleanup(),
            "the panicking paste-back left plaintext with no owner"
        );
        let content_dir = staging
            .root()
            .join(copypaste_core::binary_item_id(b"payload"));
        let deadline = SystemTime::now() + Duration::from_secs(5);
        while content_dir.exists() && SystemTime::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !content_dir.exists(),
            "the sweeper never removed the plaintext a panic left"
        );
        assert!(
            staging
                .materialize(b"after", &metadata("after.txt"))
                .is_ok(),
            "the poisoned staging lock refused the next paste-back"
        );
    }
}
