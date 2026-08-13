//! Enforcement of the staging deadline.
//!
//! The directory holds *decrypted* payloads, so the deadline is a security
//! property rather than housekeeping. Every per-entry failure used to be
//! discarded — `let _ = unlinkat(..)`, `let Ok(stat) = .. else { continue }` —
//! so a sweep that left plaintext on disk was indistinguishable from one that
//! removed it. The walk is still best-effort across entries; what changed is
//! that it counts what it could not do and comes back sooner.

use std::ffi::CStr;
use std::io;
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{BackoffBuilder, ExponentialBuilder};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;
use tracing::warn;

use super::report::SweepReport;

/// Entries one sweep may classify. The sweep holds the staging lock, so an
/// unbounded walk parks the interactive paste-back path behind however many
/// files are in the directory. A truncated sweep asks to be resumed instead.
const SWEEP_BUDGET: u32 = 4096;

/// The first retry after a sweep that left work behind. `interval` is the whole
/// `MAX_AGE`, so waiting it out doubles the exposure of anything still on disk.
/// The ladder climbs back to `interval`, so a file nothing can ever delete
/// costs a short burst of wake-ups rather than a permanent 30-second timer.
const RETRY_MIN: Duration = Duration::from_secs(30);

/// Delete everything under `root` that is past `max_age`, and report what is
/// left. Infallible by design: a sweeper that refused to run would take file
/// paste-back down with it, and the caller cannot act on an error it is not
/// also told the shape of.
pub(super) fn sweep(root: &Path, now: SystemTime, max_age: Duration) -> SweepReport {
    let report = sweep_within(root, now, max_age, SWEEP_BUDGET);
    if report.unfinished() {
        warn!(
            examined = report.examined,
            removed = report.removed,
            retained = report.retained,
            unreadable = report.unreadable,
            truncated = report.truncated,
            error_kinds = ?report.failures,
            "paste-file staging cleanup left decrypted payloads past their deadline"
        );
    }
    report
}

fn sweep_within(root: &Path, now: SystemTime, max_age: Duration, budget: u32) -> SweepReport {
    let mut report = SweepReport::default();
    let root_fd = match rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return report,
        Err(error) => {
            report.unreadable_entry(error);
            return report;
        }
    };
    let mut root_dir = match Dir::new(root_fd) {
        Ok(dir) => dir,
        Err(error) => {
            report.unreadable_entry(error);
            return report;
        }
    };

    let mut budget = budget;
    while let Some(entry) = root_dir.read() {
        let entry = match entry {
            Ok(entry) => entry,
            // A failed `readdir` leaves the stream at an offset nothing can
            // reason about, so the rest of this directory is unvisited, not
            // absent.
            Err(error) => {
                report.unreadable_entry(error);
                report.truncated = true;
                break;
            }
        };
        let name = entry.file_name();
        if is_dot(name) {
            continue;
        }
        if budget == 0 {
            report.truncated = true;
            break;
        }
        budget -= 1;
        report.examined += 1;
        let root_fd = match root_dir.fd() {
            Ok(fd) => fd,
            Err(error) => {
                report.unreadable_entry(error);
                break;
            }
        };
        let Some(stat) = classify(root_fd, name, &mut report) else {
            continue;
        };
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile if is_expired(&stat, now, max_age) => {
                remove_file(root_fd, name, &mut report);
            }
            // Any real subdirectory, not only a well-formed content id. A
            // directory whose name does not parse still holds decrypted bytes,
            // and skipping it exempted them from the deadline for ever.
            FileType::Directory => {
                sweep_content_dir(root_fd, name, now, max_age, &mut budget, &mut report);
            }
            _ => {}
        }
    }
    report
}

fn sweep_content_dir(
    root_fd: BorrowedFd<'_>,
    name: &CStr,
    now: SystemTime,
    max_age: Duration,
    budget: &mut u32,
    report: &mut SweepReport,
) {
    let content_fd = match rustix::fs::openat(
        root_fd,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return,
        Err(error) => return report.unreadable_entry(error),
    };
    let mut content_dir = match Dir::new(content_fd) {
        Ok(dir) => dir,
        Err(error) => return report.unreadable_entry(error),
    };

    while let Some(entry) = content_dir.read() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.unreadable_entry(error);
                report.truncated = true;
                break;
            }
        };
        let filename = entry.file_name();
        if is_dot(filename) {
            continue;
        }
        if *budget == 0 {
            report.truncated = true;
            break;
        }
        *budget -= 1;
        report.examined += 1;
        let content_fd = match content_dir.fd() {
            Ok(fd) => fd,
            Err(error) => return report.unreadable_entry(error),
        };
        let Some(stat) = classify(content_fd, filename, report) else {
            continue;
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && is_expired(&stat, now, max_age)
        {
            remove_file(content_fd, filename, report);
        }
    }
    remove_emptied(root_fd, name, report);
}

fn classify(dir_fd: BorrowedFd<'_>, name: &CStr, report: &mut SweepReport) -> Option<Stat> {
    match rustix::fs::statat(dir_fd, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Some(stat),
        // Gone since `readdir` is the outcome the sweep wanted.
        Err(Errno::NOENT) => None,
        Err(error) => {
            report.unreadable_entry(error);
            None
        }
    }
}

fn remove_file(dir_fd: BorrowedFd<'_>, name: &CStr, report: &mut SweepReport) {
    match rustix::fs::unlinkat(dir_fd, name, AtFlags::empty()) {
        Ok(()) => report.removed += 1,
        Err(Errno::NOENT) => {}
        // Discarding this was the defect. A locked, immutable or
        // permission-denied file is decrypted plaintext living past its
        // deadline, and the sweep used to log the pass as clean.
        Err(error) => {
            report.retained += 1;
            report.note(error);
        }
    }
}

/// Without this the directory count only ever grows: every distinct payload
/// ever pasted back leaves a husk that each later sweep must walk again.
fn remove_emptied(root_fd: BorrowedFd<'_>, name: &CStr, report: &mut SweepReport) {
    match rustix::fs::unlinkat(root_fd, name, AtFlags::REMOVEDIR) {
        Ok(()) => report.directories_removed += 1,
        // Still holding live staging files, which is the ordinary case.
        Err(Errno::NOTEMPTY | Errno::EXIST | Errno::NOENT) => {}
        Err(error) => report.unreadable_entry(error),
    }
}

/// Past the deadline, or stamped with a time this daemon cannot have written.
///
/// `renew` stamps the mtime from the wall clock, so a time further ahead than
/// `max_age` came from a clock jump or another writer. Honouring it would
/// exempt decrypted bytes from the deadline until that time arrived.
fn is_expired(stat: &Stat, now: SystemTime, max_age: Duration) -> bool {
    let Ok(since_epoch) = now.duration_since(UNIX_EPOCH) else {
        return false;
    };
    let modified = (stat.st_mtime, modified_nanos(stat));
    if let Some(cutoff) = since_epoch.checked_sub(max_age) {
        if modified < epoch_parts(cutoff) {
            return true;
        }
    }
    modified > epoch_parts(since_epoch.saturating_add(max_age))
}

fn epoch_parts(offset: Duration) -> (i64, i64) {
    (
        i64::try_from(offset.as_secs()).unwrap_or(i64::MAX),
        i64::from(offset.subsec_nanos()),
    )
}

fn modified_nanos(stat: &Stat) -> i64 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let nanos = i64::try_from(stat.st_mtime_nsec).unwrap_or(i64::MAX);
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let nanos = stat.st_mtime_nsec;
    nanos
}

fn is_dot(name: &CStr) -> bool {
    name.to_bytes() == b"." || name.to_bytes() == b".."
}

/// When the next sweep runs. One schedule, from the workspace's only retry
/// crate; the worker's condvar stays the sole timer.
struct SweepCadence {
    interval: Duration,
    policy: ExponentialBuilder,
    schedule: <ExponentialBuilder as BackoffBuilder>::Backoff,
}

impl SweepCadence {
    fn new(interval: Duration) -> Self {
        let policy = ExponentialBuilder::new()
            .with_min_delay(RETRY_MIN.min(interval))
            .with_max_delay(interval)
            .without_max_times();
        Self {
            interval,
            policy,
            schedule: policy.build(),
        }
    }

    fn after(&mut self, report: &SweepReport) -> Duration {
        if !report.unfinished() {
            self.schedule = self.policy.build();
            return self.interval;
        }
        self.schedule
            .next()
            .unwrap_or(self.interval)
            .min(self.interval)
    }
}

pub(super) struct CleanupWorker {
    stop: Arc<(Mutex<bool>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl CleanupWorker {
    pub(super) fn start(
        root: PathBuf,
        access: Arc<Mutex<()>>,
        max_age: Duration,
        interval: Duration,
    ) -> io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_stop = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("paste-file-cleanup".to_string())
            .spawn(move || {
                let mut cadence = SweepCadence::new(interval);
                let mut wait = interval;
                loop {
                    let (lock, wake) = &*worker_stop;
                    let stopped = lock.lock().unwrap_or_else(|held| held.into_inner());
                    let (stopped, _) = wake
                        .wait_timeout_while(stopped, wait, |stopped| !*stopped)
                        .unwrap_or_else(|held| held.into_inner());
                    if *stopped {
                        break;
                    }
                    drop(stopped);
                    let _access = access.lock().unwrap_or_else(|held| held.into_inner());
                    wait = cadence.after(&sweep(&root, SystemTime::now(), max_age));
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

#[cfg(test)]
mod tests {
    use super::super::testutil::CapturedLog;
    use super::super::StagingArea;
    use super::*;
    use copypaste_core::FileMetadata;
    use std::fs;

    const MINUTE: Duration = Duration::from_secs(60);

    fn metadata(filename: impl Into<String>) -> FileMetadata {
        FileMetadata {
            filename: filename.into(),
            mime_type: "application/octet-stream".to_string(),
        }
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
    }

    /// A staging root with no `StagingArea` over it, so a sweep can be pointed
    /// at trees the staging path would never produce.
    fn staging_root() -> (tempfile::TempDir, PathBuf) {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("paste-files");
        fs::create_dir(&root).unwrap();
        (parent, root)
    }

    fn write_at(path: &Path, modified: SystemTime) {
        write_bytes_at(path, b"decrypted", modified);
    }

    fn write_bytes_at(path: &Path, bytes: &[u8], modified: SystemTime) {
        fs::write(path, bytes).unwrap();
        set_modified(path, modified);
    }

    fn content_dir(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn mode_bits_bind(dir: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o500)).unwrap();
        // Root ignores the mode bits, so the fault cannot be injected there.
        fs::write(dir.join(".probe"), b"").is_err()
    }

    /// Left read-only, the temporary directory cannot be cleaned up.
    #[cfg(unix)]
    fn unlock(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn an_undeletable_payload_is_counted_and_the_rest_of_the_sweep_still_runs() {
        let (_parent, root) = staging_root();
        let locked = content_dir(&root, "11111111-1111-1111-1111-111111111111");
        let open = content_dir(&root, "22222222-2222-2222-2222-222222222222");
        let now = SystemTime::now();
        write_at(&locked.join("a.txt"), now - Duration::from_secs(3600));
        write_at(&open.join("b.txt"), now - Duration::from_secs(3600));
        if !mode_bits_bind(&locked) {
            return;
        }

        let report = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);

        assert!(locked.join("a.txt").exists(), "setup: must be undeletable");
        assert_eq!(report.retained, 1, "{report:?}");
        assert_eq!(
            report.failures.count(io::ErrorKind::PermissionDenied),
            1,
            "{report:?}"
        );
        assert!(report.unfinished());
        assert!(
            !open.join("b.txt").exists(),
            "one blocked entry stopped the sweep reaching the next"
        );
        assert_eq!(report.removed, 1);
        unlock(&locked);
    }

    #[test]
    fn a_payload_stamped_in_the_future_does_not_outlive_the_deadline() {
        let (_parent, root) = staging_root();
        let dir = content_dir(&root, "33333333-3333-3333-3333-333333333333");
        let now = SystemTime::now();
        let skewed = dir.join("skewed.txt");
        write_at(&skewed, now + Duration::from_secs(86_400));

        let report = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);

        assert!(!skewed.exists(), "a future mtime exempted plaintext");
        assert_eq!(report.removed, 1);
        assert!(!report.unfinished());
    }

    #[test]
    fn a_payload_inside_the_deadline_survives_a_clock_that_ran_backwards() {
        let (_parent, root) = staging_root();
        let dir = content_dir(&root, "44444444-4444-4444-4444-444444444444");
        let now = SystemTime::now();
        let fresh = dir.join("fresh.txt");
        write_at(&fresh, now + Duration::from_secs(30));

        let report = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);

        assert!(fresh.exists(), "{report:?}");
    }

    #[test]
    fn a_directory_whose_name_is_not_a_content_id_is_still_swept() {
        let (_parent, root) = staging_root();
        let dir = content_dir(&root, "not-a-content-id");
        let now = SystemTime::now();
        let orphan = dir.join("orphan.txt");
        write_at(&orphan, now - Duration::from_secs(3600));

        let report = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);

        assert!(!report.unfinished(), "{report:?}");
        assert!(!orphan.exists(), "an unparsable name exempted plaintext");
        assert!(!dir.exists());
    }

    #[test]
    fn an_emptied_tree_costs_the_next_sweep_nothing() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..50u8 {
            let dir = content_dir(
                &root,
                &format!("00000000-0000-0000-0000-0000000000{index:02x}"),
            );
            write_at(&dir.join("payload.bin"), now - Duration::from_secs(3600));
        }

        let first = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);
        let second = sweep_within(&root, now, MINUTE, SWEEP_BUDGET);

        assert_eq!(first.removed, 50);
        assert_eq!(first.directories_removed, 50);
        assert_eq!(first.examined, 100);
        assert_eq!(
            second.examined, 0,
            "dead directories are re-walked by every later sweep"
        );
        assert!(!second.unfinished());
    }

    #[test]
    fn a_sweep_stops_at_its_budget_and_asks_to_be_resumed() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..5u8 {
            write_at(&root.join(format!("legacy-{index}.txt")), now - MINUTE * 2);
        }

        let first = sweep_within(&root, now, MINUTE, 2);
        assert_eq!(first.examined, 2);
        assert_eq!(first.removed, 2);
        assert!(first.truncated);
        assert!(first.unfinished());

        let mut sweeps = 1;
        while sweep_within(&root, now, MINUTE, 2).unfinished() {
            sweeps += 1;
            assert!(sweeps < 10, "the bounded sweep stopped making progress");
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }

    #[test]
    fn an_unfinished_sweep_is_retried_long_before_the_next_interval() {
        let interval = Duration::from_secs(600);
        let mut cadence = SweepCadence::new(interval);
        let clean = SweepReport::default();
        let blocked = SweepReport {
            retained: 1,
            ..SweepReport::default()
        };

        assert_eq!(cadence.after(&clean), interval);
        let first = cadence.after(&blocked);
        assert_eq!(first, RETRY_MIN);

        let mut previous = first;
        for _ in 0..12 {
            let next = cadence.after(&blocked);
            assert!(next >= previous, "the ladder went backwards");
            assert!(next <= interval, "a retry outran the ordinary interval");
            previous = next;
        }
        assert_eq!(previous, interval, "the ladder must settle at the interval");

        assert_eq!(cadence.after(&clean), interval);
        assert_eq!(
            cadence.after(&blocked),
            RETRY_MIN,
            "a clean sweep must reset the ladder"
        );
    }

    #[test]
    fn a_short_interval_never_produces_a_longer_retry() {
        let interval = Duration::from_millis(10);
        let mut cadence = SweepCadence::new(interval);
        let blocked = SweepReport {
            unreadable: 1,
            ..SweepReport::default()
        };

        for _ in 0..5 {
            assert!(cadence.after(&blocked) <= interval);
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_warning_names_no_path_filename_content_id_or_payload() {
        let (parent, root) = staging_root();
        let content_id = "66666666-6666-6666-6666-666666666666";
        let locked = content_dir(&root, content_id);
        let now = SystemTime::now();
        write_bytes_at(
            &locked.join("payslip.pdf"),
            b"sort-code-00-11-22",
            now - Duration::from_secs(3600),
        );
        if !mode_bits_bind(&locked) {
            return;
        }

        let capture = CapturedLog::default();
        let logged = capture.record(|| sweep(&root, now, MINUTE));
        unlock(&locked);

        assert!(
            logged.contains("paste-file staging cleanup left decrypted payloads"),
            "{logged}"
        );
        assert!(logged.contains("retained=1"), "{logged}");
        assert!(
            logged.contains("error_kinds={PermissionDenied: 1}"),
            "the per-kind counts did not reach the log: {logged}"
        );
        for secret in [
            root.to_str().unwrap(),
            parent.path().to_str().unwrap(),
            content_id,
            "payslip",
            ".pdf",
            "sort-code",
        ] {
            assert!(!logged.contains(secret), "the warning disclosed {secret}");
        }
    }

    #[test]
    fn cleanup_deletes_only_stale_regular_staging_files() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging =
            StagingArea::with_timing(data_dir.path(), MINUTE, Duration::from_secs(3600)).unwrap();
        let stale = staging
            .materialize(b"stale", &metadata("stale.txt"))
            .unwrap();
        let fresh = staging
            .materialize(b"fresh", &metadata("fresh.txt"))
            .unwrap();
        let boundary = staging
            .materialize(b"boundary", &metadata("boundary.txt"))
            .unwrap();
        let legacy_stale = staging.root().join("legacy-stale.txt");
        fs::write(&legacy_stale, b"old layout").unwrap();
        let now = SystemTime::now();
        set_modified(&stale, now - Duration::from_secs(61));
        set_modified(&fresh, now - Duration::from_secs(59));
        set_modified(&boundary, now - Duration::from_secs(60));
        set_modified(&legacy_stale, now - Duration::from_secs(61));

        let report = sweep(staging.root(), now, MINUTE);

        assert!(!stale.exists());
        assert!(fresh.exists());
        assert!(boundary.exists());
        assert!(!legacy_stale.exists());
        assert_eq!(report.removed, 2, "{report:?}");
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
        fs::write(&external, b"outside").unwrap();
        symlink(&external, content_dir.join("linked.txt")).unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(content_dir.join("receiver.fifo"))
            .status()
            .unwrap()
            .success());
        let nested = content_dir.join("nested");
        fs::create_dir(&nested).unwrap();
        let nested_file = nested.join("keep.txt");
        fs::write(&nested_file, b"keep").unwrap();
        let external_dir = data_dir.path().join("external-dir");
        fs::create_dir(&external_dir).unwrap();
        let external_child = external_dir.join("keep.txt");
        fs::write(&external_child, b"keep").unwrap();
        symlink(
            &external_dir,
            staging.root().join("00000000-0000-0000-0000-000000000000"),
        )
        .unwrap();

        let report = sweep(
            staging.root(),
            SystemTime::now() + Duration::from_secs(2),
            Duration::from_secs(1),
        );

        assert_eq!(report.retained, 0, "{report:?}");
        assert!(external.exists());
        assert!(content_dir.join("linked.txt").symlink_metadata().is_ok());
        assert!(content_dir.join("receiver.fifo").exists());
        assert!(nested_file.exists());
        assert!(external_child.exists());
    }

    #[test]
    fn asynchronous_receiver_can_open_before_cleanup_removes_the_file() {
        let data_dir = tempfile::tempdir().unwrap();
        let staging =
            StagingArea::with_timing(data_dir.path(), MINUTE, Duration::from_millis(10)).unwrap();
        let path = staging
            .materialize(b"received later", &metadata("later.txt"))
            .unwrap();
        let receiver_path = path.clone();
        let receiver = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            fs::read(receiver_path)
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
