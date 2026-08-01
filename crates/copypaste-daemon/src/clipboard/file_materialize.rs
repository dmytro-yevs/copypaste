//! Private, time-bounded staging for native file paste-back.

use std::ffi::CStr;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use copypaste_core::FileMetadata;
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};
use rustix::io::Errno;
use tracing::warn;

const DIRECTORY_MODE: Mode = Mode::RWXU;
const FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
const MAX_AGE: Duration = Duration::from_secs(10 * 60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct StagingArea {
    root: PathBuf,
    access: Arc<Mutex<()>>,
    _cleanup: CleanupWorker,
}

impl StagingArea {
    pub(super) fn new(data_dir: &Path) -> io::Result<Self> {
        Self::with_timing(data_dir, MAX_AGE, CLEANUP_INTERVAL)
    }

    fn with_timing(data_dir: &Path, max_age: Duration, interval: Duration) -> io::Result<Self> {
        let root = std::path::absolute(data_dir)?.join("paste-files");
        let root_fd = open_or_create_root(&root)?;
        drop(root_fd);
        cleanup_stale_at(&root, SystemTime::now(), max_age)?;
        let access = Arc::new(Mutex::new(()));
        let cleanup = CleanupWorker::start(root.clone(), Arc::clone(&access), max_age, interval)?;
        Ok(Self {
            root,
            access,
            _cleanup: cleanup,
        })
    }

    pub(super) fn materialize(&self, bytes: &[u8], metadata: &FileMetadata) -> io::Result<PathBuf> {
        let filename = safe_filename(metadata)?;
        let _access = self.access.lock().unwrap_or_else(|held| held.into_inner());
        let content_id = copypaste_core::binary_item_id(bytes);
        let root_fd = open_or_create_root(&self.root)?;
        let content_fd = open_or_create_content_dir(&root_fd, &content_id)?;

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
            temporary.sync_all()?;
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

struct CleanupWorker {
    stop: Arc<(Mutex<bool>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl CleanupWorker {
    fn start(
        root: PathBuf,
        access: Arc<Mutex<()>>,
        max_age: Duration,
        interval: Duration,
    ) -> io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_stop = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("paste-file-cleanup".to_string())
            .spawn(move || loop {
                let (lock, wake) = &*worker_stop;
                let stopped = lock.lock().unwrap_or_else(|held| held.into_inner());
                let (stopped, _) = wake
                    .wait_timeout_while(stopped, interval, |stopped| !*stopped)
                    .unwrap_or_else(|held| held.into_inner());
                if *stopped {
                    break;
                }
                drop(stopped);
                let _access = access.lock().unwrap_or_else(|held| held.into_inner());
                if let Err(error) = cleanup_stale_at(&root, SystemTime::now(), max_age) {
                    warn!(
                        error_kind = ?error.kind(),
                        "paste-file staging cleanup did not finish"
                    );
                }
            })?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }
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

fn cleanup_stale_at(root: &Path, now: SystemTime, max_age: Duration) -> io::Result<()> {
    let root_fd = match rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut root_dir = Dir::new(root_fd)?;
    while let Some(entry) = root_dir.read() {
        let entry = entry?;
        let name = entry.file_name();
        if is_dot(name) {
            continue;
        }
        let root_fd = root_dir.fd()?;
        let Ok(stat) = rustix::fs::statat(root_fd, name, AtFlags::SYMLINK_NOFOLLOW) else {
            continue;
        };
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile if is_older_than(&stat, now, max_age) => {
                let _ = rustix::fs::unlinkat(root_fd, name, AtFlags::empty());
            }
            FileType::Directory if is_content_id(name) => {
                cleanup_content_dir(root_fd, name, now, max_age);
            }
            _ => {}
        }
    }
    Ok(())
}

fn cleanup_content_dir(
    root_fd: std::os::fd::BorrowedFd<'_>,
    name: &CStr,
    now: SystemTime,
    max_age: Duration,
) {
    let Ok(content_fd) = rustix::fs::openat(
        root_fd,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return;
    };
    let Ok(mut content_dir) = Dir::new(content_fd) else {
        return;
    };
    while let Some(Ok(entry)) = content_dir.read() {
        let filename = entry.file_name();
        if is_dot(filename) {
            continue;
        }
        let Ok(content_fd) = content_dir.fd() else {
            return;
        };
        let Ok(stat) = rustix::fs::statat(content_fd, filename, AtFlags::SYMLINK_NOFOLLOW) else {
            continue;
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && is_older_than(&stat, now, max_age)
        {
            let _ = rustix::fs::unlinkat(content_fd, filename, AtFlags::empty());
        }
    }
}

fn is_older_than(stat: &rustix::fs::Stat, now: SystemTime, max_age: Duration) -> bool {
    let Ok(now) = now.duration_since(UNIX_EPOCH) else {
        return false;
    };
    let Some(cutoff) = now.checked_sub(max_age) else {
        return false;
    };
    let modified_secs = stat.st_mtime;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let modified_nanos = i64::try_from(stat.st_mtime_nsec).unwrap_or(i64::MAX);
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let modified_nanos = stat.st_mtime_nsec;
    let cutoff_secs = i64::try_from(cutoff.as_secs()).unwrap_or(i64::MAX);
    let cutoff_nanos = i64::from(cutoff.subsec_nanos());
    modified_secs < cutoff_secs || (modified_secs == cutoff_secs && modified_nanos < cutoff_nanos)
}

fn is_content_id(name: &CStr) -> bool {
    name.to_str()
        .ok()
        .is_some_and(|name| name.len() == 36 && uuid::Uuid::parse_str(name).is_ok())
}

fn is_dot(name: &CStr) -> bool {
    name.to_bytes() == b"." || name.to_bytes() == b".."
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
    fn cleanup_deletes_only_stale_regular_staging_files() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(60),
            Duration::from_secs(3600),
        )
        .unwrap();
        let stale = staging
            .materialize(b"stale", &metadata("stale.txt"))
            .unwrap();
        let fresh = staging
            .materialize(b"fresh", &metadata("fresh.txt"))
            .unwrap();
        let boundary = staging
            .materialize(b"boundary", &metadata("boundary.txt"))
            .unwrap();
        let legacy_stale = staging.root.join("legacy-stale.txt");
        std::fs::write(&legacy_stale, b"old layout").unwrap();
        let now = SystemTime::now();
        set_modified(&stale, now - Duration::from_secs(61));
        set_modified(&fresh, now - Duration::from_secs(59));
        set_modified(&boundary, now - Duration::from_secs(60));
        set_modified(&legacy_stale, now - Duration::from_secs(61));

        cleanup_stale_at(&staging.root, now, Duration::from_secs(60)).unwrap();
        assert!(!stale.exists());
        assert!(fresh.exists());
        assert!(boundary.exists());
        assert!(!legacy_stale.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_never_follows_symlinks_or_removes_fifos() {
        use std::os::unix::fs::symlink;

        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(1),
            Duration::from_secs(3600),
        )
        .unwrap();
        let content_dir = staging
            .materialize(b"seed", &metadata("seed.txt"))
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let external = data_dir.path().join("external.txt");
        std::fs::write(&external, b"outside").unwrap();
        symlink(&external, content_dir.join("linked.txt")).unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(content_dir.join("receiver.fifo"))
            .status()
            .unwrap()
            .success());
        let nested = content_dir.join("nested");
        std::fs::create_dir(&nested).unwrap();
        let nested_file = nested.join("keep.txt");
        std::fs::write(&nested_file, b"keep").unwrap();
        let external_dir = data_dir.path().join("external-dir");
        std::fs::create_dir(&external_dir).unwrap();
        let external_child = external_dir.join("keep.txt");
        std::fs::write(&external_child, b"keep").unwrap();
        symlink(
            &external_dir,
            staging.root.join("00000000-0000-0000-0000-000000000000"),
        )
        .unwrap();

        cleanup_stale_at(
            &staging.root,
            SystemTime::now() + Duration::from_secs(2),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(external.exists());
        assert!(content_dir.join("linked.txt").symlink_metadata().is_ok());
        assert!(content_dir.join("receiver.fifo").exists());
        assert!(nested_file.exists());
        assert!(external_child.exists());
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
        cleanup_stale_at(&staging.root, now, Duration::from_secs(60)).unwrap();
        assert_eq!(reused, file);
        assert!(reused.exists());
    }

    #[test]
    fn asynchronous_receiver_can_open_before_cleanup_removes_the_file() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging = StagingArea::with_timing(
            data_dir.path(),
            Duration::from_secs(60),
            Duration::from_millis(10),
        )
        .unwrap();
        let path = staging
            .materialize(b"received later", &metadata("later.txt"))
            .unwrap();
        let receiver_path = path.clone();
        let receiver = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            std::fs::read(receiver_path)
        });

        assert_eq!(receiver.join().unwrap().unwrap(), b"received later");
        assert!(path.exists());
        set_modified(&path, SystemTime::now() - Duration::from_secs(61));
        let deadline = SystemTime::now() + Duration::from_secs(2);
        while path.exists() && SystemTime::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!path.exists(), "the cleanup worker left stale plaintext");
    }
}
