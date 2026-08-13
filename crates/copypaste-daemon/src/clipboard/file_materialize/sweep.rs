//! Enforcement of the staging deadline.
//!
//! The directory holds *decrypted* payloads, so the deadline is a security
//! property rather than housekeeping. Every per-entry failure is counted and
//! retried sooner rather than discarded: plaintext the sweep could not delete
//! must never read as a clean pass.

use std::ffi::CStr;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;
use tracing::warn;

use super::report::SweepReport;

/// Entries one sweep may classify. The sweep holds the staging lock, so an
/// unbounded walk parks the interactive paste-back path behind however many
/// files are in the directory. A sweep that hits the bound records where it
/// stopped and the next pass continues from there.
const SWEEP_BUDGET: u32 = 4096;

/// Where the previous pass ran out of budget, as a position in the root's
/// `readdir` order and a position within that entry if it is a directory.
///
/// A pass that restarted from the first entry could be consumed for ever by a
/// prefix of live or undeletable files, and everything behind that prefix would
/// keep its plaintext indefinitely. Continuing bounds the retained lifetime at
/// `max_age` plus the passes one full cycle of `ceil(entries / SWEEP_BUDGET)`
/// takes, whatever the prefix does.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Cursor {
    root: u32,
    child: u32,
}

/// Deletes everything under the directory it was handed that is past `max_age`.
///
/// It holds the root as a *file descriptor*, never a path. A staging root that
/// is renamed away and replaced must not redirect the sweep into whatever now
/// answers to the same pathname, where it would unlink somebody else's files.
pub(super) struct Sweeper {
    root_fd: OwnedFd,
    budget: u32,
    resume: Cursor,
}

impl Sweeper {
    pub(super) fn new(root_fd: OwnedFd) -> Self {
        Self {
            root_fd,
            budget: SWEEP_BUDGET,
            resume: Cursor::default(),
        }
    }

    #[cfg(test)]
    fn with_budget(root_fd: OwnedFd, budget: u32) -> Self {
        Self {
            budget,
            ..Self::new(root_fd)
        }
    }

    /// One pass, and what it left behind. Infallible by design: a sweeper that
    /// refused to run would take file paste-back down with it, and the caller
    /// cannot act on an error it is not also told the shape of.
    pub(super) fn pass(&mut self, now: SystemTime, max_age: Duration) -> SweepReport {
        let report = self.walk(now, max_age);
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

    fn walk(&mut self, now: SystemTime, max_age: Duration) -> SweepReport {
        let mut report = SweepReport::default();
        let resume = std::mem::take(&mut self.resume);
        // `read_from` opens `.` relative to the retained descriptor, so the
        // stream is this directory's however the pathname is repointed.
        let mut root_dir = match Dir::read_from(&self.root_fd) {
            Ok(dir) => dir,
            Err(Errno::NOENT) => return report,
            Err(error) => {
                report.unreadable_entry(error);
                return report;
            }
        };

        let mut budget = self.budget;
        let mut position = 0;
        while let Some(entry) = root_dir.read() {
            let entry = match entry {
                Ok(entry) => entry,
                // A failed `readdir` leaves the stream at an offset nothing can
                // reason about, so the rest of this directory is unvisited, not
                // absent.
                Err(error) => {
                    report.unreadable_entry(error);
                    report.truncated = true;
                    self.resume = Cursor {
                        root: position,
                        child: 0,
                    };
                    break;
                }
            };
            let here = position;
            position += 1;
            let name = entry.file_name();
            if is_dot(name) {
                continue;
            }
            if here < resume.root {
                continue;
            }
            if budget == 0 {
                report.truncated = true;
                self.resume = Cursor {
                    root: here,
                    child: 0,
                };
                break;
            }
            budget -= 1;
            report.examined += 1;
            let root_fd = match root_dir.fd() {
                Ok(fd) => fd,
                Err(error) => {
                    report.unreadable_entry(error);
                    report.truncated = true;
                    self.resume = Cursor {
                        root: here,
                        child: 0,
                    };
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
                // directory whose name does not parse still holds decrypted
                // bytes, and skipping it exempted them from the deadline for
                // ever.
                FileType::Directory => {
                    let skip = if here == resume.root { resume.child } else { 0 };
                    let suspended = sweep_content_dir(
                        root_fd,
                        name,
                        now,
                        max_age,
                        skip,
                        &mut budget,
                        &mut report,
                    );
                    if let Some(child) = suspended {
                        self.resume = Cursor { root: here, child };
                        break;
                    }
                }
                _ => {}
            }
        }
        report
    }
}

/// `Some(position)` when the sweep stopped part-way through this directory, so
/// the next pass resumes there rather than re-walking what it already did.
fn sweep_content_dir(
    root_fd: BorrowedFd<'_>,
    name: &CStr,
    now: SystemTime,
    max_age: Duration,
    skip: u32,
    budget: &mut u32,
    report: &mut SweepReport,
) -> Option<u32> {
    let content_fd = match rustix::fs::openat(
        root_fd,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return None,
        Err(error) => {
            report.unreadable_entry(error);
            return None;
        }
    };
    let mut content_dir = match Dir::new(content_fd) {
        Ok(dir) => dir,
        Err(error) => {
            report.unreadable_entry(error);
            return None;
        }
    };

    let mut position = 0;
    let mut suspended = None;
    while let Some(entry) = content_dir.read() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.unreadable_entry(error);
                report.truncated = true;
                suspended = Some(position);
                break;
            }
        };
        let here = position;
        position += 1;
        let filename = entry.file_name();
        if is_dot(filename) {
            continue;
        }
        if here < skip {
            continue;
        }
        if *budget == 0 {
            report.truncated = true;
            suspended = Some(here);
            break;
        }
        *budget -= 1;
        report.examined += 1;
        let content_fd = match content_dir.fd() {
            Ok(fd) => fd,
            Err(error) => {
                report.unreadable_entry(error);
                report.truncated = true;
                suspended = Some(here);
                break;
            }
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
    if suspended.is_none() {
        remove_emptied(root_fd, name, report);
    }
    suspended
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

#[cfg(test)]
mod tests {
    use super::super::failure::FailureKind;
    use super::super::testutil::CapturedLog;
    use super::super::StagingArea;
    use super::*;
    use copypaste_core::FileMetadata;
    use std::fs;
    use std::path::{Path, PathBuf};

    const MINUTE: Duration = Duration::from_secs(60);
    const HOUR: Duration = Duration::from_secs(3600);

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

    fn open_root(root: &Path) -> OwnedFd {
        rustix::fs::open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap()
    }

    fn sweep_once(root: &Path, now: SystemTime, max_age: Duration) -> SweepReport {
        Sweeper::new(open_root(root)).pass(now, max_age)
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
        write_at(&locked.join("a.txt"), now - HOUR);
        write_at(&open.join("b.txt"), now - HOUR);
        if !mode_bits_bind(&locked) {
            return;
        }

        let report = sweep_once(&root, now, MINUTE);

        assert!(locked.join("a.txt").exists(), "setup: must be undeletable");
        assert_eq!(report.retained, 1, "{report:?}");
        assert_eq!(
            report.failures.count(FailureKind::PermissionDenied),
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

        let report = sweep_once(&root, now, MINUTE);

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

        let report = sweep_once(&root, now, MINUTE);

        assert!(fresh.exists(), "{report:?}");
    }

    #[test]
    fn a_directory_whose_name_is_not_a_content_id_is_still_swept() {
        let (_parent, root) = staging_root();
        let dir = content_dir(&root, "not-a-content-id");
        let now = SystemTime::now();
        let orphan = dir.join("orphan.txt");
        write_at(&orphan, now - HOUR);

        let report = sweep_once(&root, now, MINUTE);

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
            write_at(&dir.join("payload.bin"), now - HOUR);
        }

        let mut sweeper = Sweeper::new(open_root(&root));
        let first = sweeper.pass(now, MINUTE);
        let second = sweeper.pass(now, MINUTE);

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

        let mut sweeper = Sweeper::with_budget(open_root(&root), 2);
        let first = sweeper.pass(now, MINUTE);
        assert_eq!(first.examined, 2);
        assert_eq!(first.removed, 2);
        assert!(first.truncated);
        assert!(first.unfinished());

        let mut sweeps = 1;
        while fs::read_dir(&root).unwrap().count() > 0 {
            let _ = sweeper.pass(now, MINUTE);
            sweeps += 1;
            assert!(sweeps < 10, "the bounded sweep stopped making progress");
        }
    }

    /// The bound this proves is the retained-plaintext lifetime: `max_age` plus
    /// one full cycle of `ceil(entries / budget)` passes. Restarting `readdir`
    /// from the first entry has no such bound — a prefix of live files consumes
    /// every pass and everything behind it keeps its plaintext for ever.
    #[test]
    fn a_live_prefix_cannot_starve_the_expired_entries_behind_it() {
        const LIVE: usize = 24;
        const BUDGET: u32 = 4;

        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..LIVE {
            write_at(&root.join(format!("live-{index:02}.bin")), now);
        }
        let expired: Vec<PathBuf> = (0..4)
            .map(|index| {
                let path = root.join(format!("expired-{index}.bin"));
                write_at(&path, now - HOUR);
                path
            })
            .collect();

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        // `.` and `..` hold two positions of their own, and a removal can shift
        // an unvisited entry behind the cursor into the cycle after this one.
        let cycle = (LIVE + expired.len() + 2).div_ceil(BUDGET as usize);
        let mut passes = 0;
        while expired.iter().any(|path| path.exists()) {
            let _ = sweeper.pass(now, MINUTE);
            passes += 1;
            assert!(
                passes <= 2 * cycle,
                "a live prefix starved the entries behind it for {passes} passes"
            );
        }
        assert!(
            root.join("live-00.bin").exists(),
            "the sweep removed a payload inside its deadline"
        );
    }

    #[test]
    fn a_live_prefix_inside_a_content_directory_cannot_starve_it_either() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        let dir = content_dir(&root, "55555555-5555-5555-5555-555555555555");
        for index in 0..8 {
            write_at(&dir.join(format!("live-{index}.bin")), now);
        }
        let expired: Vec<PathBuf> = (0..2)
            .map(|index| {
                let path = dir.join(format!("expired-{index}.bin"));
                write_at(&path, now - HOUR);
                path
            })
            .collect();

        // Three units a pass: the content directory itself plus two children.
        let mut sweeper = Sweeper::with_budget(open_root(&root), 3);
        let mut passes = 0;
        while expired.iter().any(|path| path.exists()) {
            let _ = sweeper.pass(now, MINUTE);
            passes += 1;
            assert!(
                passes <= 16,
                "a truncated content directory restarted instead of resuming"
            );
        }
    }

    #[test]
    fn a_completed_pass_starts_the_next_one_at_the_first_entry() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..3 {
            write_at(&root.join(format!("live-{index}.bin")), now);
        }

        let mut sweeper = Sweeper::with_budget(open_root(&root), 2);
        assert!(sweeper.pass(now, MINUTE).truncated);
        assert_ne!(
            sweeper.resume,
            Cursor::default(),
            "a truncated pass resumes"
        );
        assert!(!sweeper.pass(now, MINUTE).truncated);
        assert_eq!(
            sweeper.resume,
            Cursor::default(),
            "a completed cycle must start over, or new entries are never reached"
        );
    }

    /// The budget exists to bound how long the interactive paste-back path can
    /// be parked behind a sweep. Measured here rather than asserted from the
    /// constant alone.
    #[test]
    fn a_full_budget_sweep_holds_the_staging_lock_briefly() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..SWEEP_BUDGET {
            write_bytes_at(
                &root.join(format!("payload-{index:05}.bin")),
                b"x",
                now - HOUR,
            );
        }

        let mut sweeper = Sweeper::new(open_root(&root));
        let started = std::time::Instant::now();
        let report = sweeper.pass(now, MINUTE);
        let held = started.elapsed();

        assert_eq!(report.examined, SWEEP_BUDGET, "{report:?}");
        assert_eq!(report.removed, SWEEP_BUDGET, "{report:?}");
        assert!(!report.truncated, "{report:?}");
        println!("a {SWEEP_BUDGET}-entry sweep held the staging lock for {held:?}");
        assert!(
            held < Duration::from_secs(2),
            "a full-budget sweep held the staging lock for {held:?}"
        );
    }

    /// A pathname can be renamed away and replaced between two sweeps; a
    /// descriptor cannot. Following the pathname would let whoever wins that
    /// race point the sweeper at a directory of somebody else's files.
    #[cfg(unix)]
    #[test]
    fn a_root_replaced_by_rename_cannot_redirect_the_sweep() {
        let (parent, root) = staging_root();
        let now = SystemTime::now();
        let ours = root.join("expired.bin");
        write_at(&ours, now - HOUR);
        let mut sweeper = Sweeper::new(open_root(&root));

        let moved = parent.path().join("moved-away");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        let theirs = root.join("someone-elses.bin");
        write_at(&theirs, now - HOUR);

        let report = sweeper.pass(now, MINUTE);

        assert!(
            theirs.exists(),
            "the sweep followed the pathname into a replaced directory"
        );
        assert!(
            !moved.join("expired.bin").exists(),
            "the sweep lost the directory it was given"
        );
        assert_eq!(report.removed, 1, "{report:?}");
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
            now - HOUR,
        );
        if !mode_bits_bind(&locked) {
            return;
        }

        let capture = CapturedLog::default();
        let logged = capture.record(|| sweep_once(&root, now, MINUTE));
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

        let report = staging.sweep_now(now, MINUTE);

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

        let report = staging.sweep_now(
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
