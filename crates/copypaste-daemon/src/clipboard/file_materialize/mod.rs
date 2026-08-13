//! Private, time-bounded staging for native file paste-back.

mod sweep;

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use copypaste_core::FileMetadata;
use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;
use tracing::warn;

use sweep::{sweep, CleanupWorker};

const DIRECTORY_MODE: Mode = Mode::RWXU;
const FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
const MAX_AGE: Duration = Duration::from_secs(10 * 60);
/// Matched to `MAX_AGE`: the sweep enforces a deadline nothing waits on, so
/// sampling it more often than it can expire is a resident timer for nothing.
const CLEANUP_INTERVAL: Duration = MAX_AGE;

pub(super) struct StagingArea {
    root: PathBuf,
    /// Opened once. Re-`open`ing and re-`fchmod`ing per paste-back put six
    /// syscalls on the interactive path to reassert a mode nothing changes.
    root_fd: std::os::fd::OwnedFd,
    access: Arc<Mutex<()>>,
    /// Started on the first successful `materialize`, or at construction when
    /// the start-time sweep could not finish. On a machine that never pastes a
    /// file back and has nothing left over, the sweeper has nothing to sweep and
    /// was costing a resident OS thread for the life of the daemon.
    cleanup: OnceLock<CleanupWorker>,
    max_age: Duration,
    interval: Duration,
}

impl StagingArea {
    pub(super) fn new(data_dir: &Path) -> io::Result<Self> {
        Self::with_timing(data_dir, MAX_AGE, CLEANUP_INTERVAL)
    }

    fn with_timing(data_dir: &Path, max_age: Duration, interval: Duration) -> io::Result<Self> {
        let root = std::path::absolute(data_dir)?.join("paste-files");
        let root_fd = open_or_create_root(&root)?;
        let area = Self {
            root,
            root_fd,
            access: Arc::new(Mutex::new(())),
            cleanup: OnceLock::new(),
            max_age,
            interval,
        };
        // Whatever the previous run left is already decrypted and already past
        // its deadline. If this pass could not finish the job the sweeper has to
        // start now: on a machine that never pastes a file back, nothing else
        // ever will.
        if sweep(&area.root, SystemTime::now(), max_age).unfinished() {
            area.start_cleanup();
        }
        Ok(area)
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn materialize(&self, bytes: &[u8], metadata: &FileMetadata) -> io::Result<PathBuf> {
        let filename = safe_filename(metadata)?;
        let _access = self.access.lock().unwrap_or_else(|held| held.into_inner());
        let path = self.stage(bytes, filename)?;
        self.start_cleanup();
        Ok(path)
    }

    /// The TTL sweeper exists because this directory holds *decrypted* payloads.
    /// Retried on the next paste-back if the thread could not be spawned.
    fn start_cleanup(&self) {
        if self.cleanup.get().is_some() {
            return;
        }
        match CleanupWorker::start(
            self.root.clone(),
            Arc::clone(&self.access),
            self.max_age,
            self.interval,
        ) {
            Ok(worker) => {
                let _ = self.cleanup.set(worker);
            }
            Err(error) => warn!(
                error_kind = ?error.kind(),
                "could not start the paste-file staging sweeper"
            ),
        }
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
            // `flush`, never `sync_all`: `write -> publish -> read` crosses a
            // process boundary, so the bytes must be visible before the
            // `linkat`, but they need never reach stable storage. A staging file
            // lost to a crash is re-derived from the encrypted row on the next
            // paste, and the fsync was tens of milliseconds of interactive
            // latency for a file `MAX_AGE` deletes within ten minutes anyway.
            temporary.flush()?;
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
        let unlink_result =
            rustix::fs::unlinkat(&content_fd, temporary_name.as_str(), AtFlags::empty());

        match write_result {
            Ok(()) => {
                unlink_result?;
                Ok(self.root.join(content_id).join(filename))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                unlink_result?;
                let file = open_regular_at(&content_fd, filename)?;
                renew(&file)?;
                Ok(self.root.join(content_id).join(filename))
            }
            Err(error) => {
                let _ = unlink_result;
                Err(error)
            }
        }
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
    use super::*;

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
            staging.cleanup.get().is_none(),
            "a daemon that never pasted a file back must not carry a sweeper thread"
        );

        staging.materialize(b"bytes", &metadata("a.txt")).unwrap();
        assert!(
            staging.cleanup.get().is_some(),
            "staged plaintext must be under the TTL sweeper"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_restart_that_cannot_finish_its_sweep_starts_the_sweeper_anyway() {
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

        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(60),
            Duration::from_secs(3600),
        )
        .unwrap();

        assert!(
            staging.cleanup.get().is_some(),
            "plaintext the startup sweep could not remove must stay under a sweeper"
        );
        std::fs::set_permissions(&left_behind, std::fs::Permissions::from_mode(0o700)).unwrap();
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
        let report = sweep(&staging.root, now, Duration::from_secs(60));

        assert_eq!(reused, file);
        assert!(reused.exists());
        assert_eq!(report.removed, 0, "{report:?}");
    }
}
