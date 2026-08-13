//! Enforcement of the staging deadline.
//!
//! The directory holds *decrypted* payloads, so the deadline is a security
//! property rather than housekeeping. Every per-entry failure is counted and
//! retried sooner rather than discarded: plaintext the sweep could not delete
//! must never read as a clean pass.

use std::ffi::{CStr, CString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;
use tracing::warn;

use super::report::SweepReport;

/// Directory reads one sweep may perform, over the root and any content
/// directory it descends into. The sweep holds the staging lock, so an
/// unbounded walk parks the interactive paste-back path behind however many
/// files are in the directory.
///
/// One unit is charged before every read, including the `.` and `..` entries
/// and the read that reports end-of-directory. Nothing else in a pass touches a
/// directory stream, so a pass performs at most this many reads and, since each
/// charged read costs at most a `statat` and an `unlinkat`, O(budget) syscalls.
const SWEEP_BUDGET: u32 = 4096;

/// The streams of a cycle in progress, kept open across passes.
///
/// A `readdir` position is only meaningful while its stream is open: Apple and
/// POSIX both scope a `telldir` value to the lifetime of its `DIR*`, and leave
/// a value carried across `closedir` unspecified. Holding the stream is the
/// resume that *is* documented, and it is the one the cycle bound needs —
/// POSIX leaves only entries added or removed mid-cycle unspecified, so every
/// other entry present when the stream opened is returned exactly once.
struct Cycle {
    root: Dir,
    child: Option<ChildCycle>,
}

struct ChildCycle {
    name: CString,
    /// Kept beside the stream because `Dir::fd` borrows what `Dir::read` needs
    /// mutably, and the entries are unlinked relative to this descriptor.
    fd: OwnedFd,
    dir: Dir,
}

enum ChildOutcome {
    Suspended(ChildCycle),
    Finished(CString),
}

/// Deletes everything under the directory it was handed that is past `max_age`.
///
/// It holds the root as a *file descriptor*, never a path. A staging root that
/// is renamed away and replaced must not redirect the sweep into whatever now
/// answers to the same pathname, where it would unlink somebody else's files.
pub(super) struct Sweeper {
    root_fd: OwnedFd,
    budget: u32,
    cycle: Option<Cycle>,
    #[cfg(test)]
    fault: Option<std::sync::Arc<SweepFault>>,
}

impl Sweeper {
    pub(super) fn new(root_fd: OwnedFd) -> Self {
        Self {
            root_fd,
            budget: SWEEP_BUDGET,
            cycle: None,
            #[cfg(test)]
            fault: None,
        }
    }

    #[cfg(test)]
    fn with_budget(root_fd: OwnedFd, budget: u32) -> Self {
        Self {
            budget,
            ..Self::new(root_fd)
        }
    }

    #[cfg(test)]
    pub(super) fn armed(mut self, fault: std::sync::Arc<SweepFault>) -> Self {
        self.fault = Some(fault);
        self
    }

    /// Holds the dying thread before it claims the staging area, which is the
    /// only way a test can put a paste-back and a death in the interleaving
    /// that has to be proven.
    #[cfg(test)]
    pub(super) fn wait_while_held(&self) {
        if let Some(fault) = &self.fault {
            fault.wait_while_held();
        }
    }

    /// One pass, and what it left behind. Infallible by design: a sweeper that
    /// refused to run would take file paste-back down with it, and the caller
    /// cannot act on an error it is not also told the shape of.
    pub(super) fn pass(&mut self, now: SystemTime, max_age: Duration) -> SweepReport {
        #[cfg(test)]
        if self.fault.as_ref().is_some_and(|fault| fault.take_panic()) {
            panic!("injected paste-file sweep panic");
        }
        let report = self.walk(now, max_age);
        if report.unfinished() {
            warn!(
                operations = report.operations,
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
        let root_fd = self.root_fd.as_fd();
        let mut cycle = match self.cycle.take() {
            Some(cycle) => cycle,
            None => match Dir::read_from(root_fd) {
                Ok(root) => Cycle { root, child: None },
                Err(Errno::NOENT) => return report,
                Err(error) => {
                    report.unreadable_entry(error);
                    return report;
                }
            },
        };
        let mut budget = self.budget;

        // What is left in a suspended content directory is the oldest unvisited
        // plaintext of the cycle, so it comes before another root entry.
        if let Some(child) = cycle.child.take() {
            match sweep_content_dir(child, now, max_age, &mut budget, &mut report) {
                ChildOutcome::Suspended(child) => {
                    cycle.child = Some(child);
                    self.cycle = Some(cycle);
                    return report;
                }
                ChildOutcome::Finished(name) => remove_emptied(root_fd, &name, &mut report),
            }
        }

        while budget > 0 {
            budget -= 1;
            report.operations += 1;
            let Some(result) = cycle.root.read() else {
                // The cycle is complete. Dropping it starts the next pass on a
                // fresh stream, which is the only way entries added since this
                // one opened are ever reached.
                return report;
            };
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    // A stream that has failed reads no further, so this cycle
                    // cannot be resumed and the next pass opens a new one.
                    report.unreadable_entry(error);
                    report.truncated = true;
                    return report;
                }
            };
            let name = entry.file_name();
            if is_dot(name) {
                continue;
            }
            report.examined += 1;
            let Some(stat) = classify(root_fd, name, &mut report) else {
                continue;
            };
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::RegularFile if is_expired(&stat, now, max_age) => {
                    remove_file(root_fd, name, &mut report);
                }
                FileType::Directory => {
                    let Some(child) = open_content_dir(root_fd, name, &mut report) else {
                        continue;
                    };
                    match sweep_content_dir(child, now, max_age, &mut budget, &mut report) {
                        ChildOutcome::Suspended(child) => {
                            cycle.child = Some(child);
                            self.cycle = Some(cycle);
                            return report;
                        }
                        ChildOutcome::Finished(name) => remove_emptied(root_fd, &name, &mut report),
                    }
                }
                _ => {}
            }
        }
        report.truncated = true;
        self.cycle = Some(cycle);
        report
    }
}

/// Test-only control of the worker thread a sweeper belongs to: kill it on its
/// next pass, and hold it at the start of its final sweep.
///
/// The panic trips once. The death it provokes sweeps on this same sweeper, and
/// a second panic there would land in a `Drop` during an unwind and abort the
/// process.
#[cfg(test)]
#[derive(Default)]
pub(super) struct SweepFault {
    panic_next_pass: std::sync::atomic::AtomicBool,
    held: std::sync::Mutex<bool>,
    released: std::sync::Condvar,
}

#[cfg(test)]
impl SweepFault {
    pub(super) fn panic_next_pass(&self) {
        self.panic_next_pass
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(super) fn hold(&self) {
        *self.held.lock().unwrap_or_else(|held| held.into_inner()) = true;
    }

    pub(super) fn release(&self) {
        *self.held.lock().unwrap_or_else(|held| held.into_inner()) = false;
        self.released.notify_all();
    }

    fn take_panic(&self) -> bool {
        self.panic_next_pass
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    fn wait_while_held(&self) {
        let mut held = self.held.lock().unwrap_or_else(|held| held.into_inner());
        while *held {
            held = self
                .released
                .wait(held)
                .unwrap_or_else(|held| held.into_inner());
        }
    }
}

fn open_content_dir(
    root_fd: BorrowedFd<'_>,
    name: &CStr,
    report: &mut SweepReport,
) -> Option<ChildCycle> {
    let fd = match rustix::fs::openat(
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
    match Dir::read_from(&fd) {
        Ok(dir) => Some(ChildCycle {
            name: name.to_owned(),
            fd,
            dir,
        }),
        Err(error) => {
            report.unreadable_entry(error);
            None
        }
    }
}

fn sweep_content_dir(
    mut child: ChildCycle,
    now: SystemTime,
    max_age: Duration,
    budget: &mut u32,
    report: &mut SweepReport,
) -> ChildOutcome {
    while *budget > 0 {
        *budget -= 1;
        report.operations += 1;
        let Some(result) = child.dir.read() else {
            return ChildOutcome::Finished(child.name);
        };
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                // The stream is spent, and `rmdir` will refuse the directory
                // while anything the pass did not reach is still in it.
                report.unreadable_entry(error);
                report.truncated = true;
                return ChildOutcome::Finished(child.name);
            }
        };
        let name = entry.file_name();
        if is_dot(name) {
            continue;
        }
        report.examined += 1;
        let content_fd = child.fd.as_fd();
        let Some(stat) = classify(content_fd, name, report) else {
            continue;
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && is_expired(&stat, now, max_age)
        {
            remove_file(content_fd, name, report);
        }
    }
    report.truncated = true;
    ChildOutcome::Suspended(child)
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
    /// `.`, `..` and the read that reports end-of-directory. Every complete
    /// pass over a directory charges these on top of its entries.
    const FIXED_READS: u32 = 3;

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

    /// B1: the budget counts directory reads, and the count is exact. A pass
    /// that stops at the bound performs the bound, and a pass that completes
    /// charges every entry plus `.`, `..` and the end-of-directory read.
    #[test]
    fn a_complete_root_pass_charges_every_entry_and_the_dot_entries() {
        const ENTRIES: u32 = 6;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..ENTRIES {
            write_at(&root.join(format!("live-{index}.bin")), now);
        }

        let report = sweep_once(&root, now, MINUTE);

        assert_eq!(report.operations, ENTRIES + FIXED_READS, "{report:?}");
        assert_eq!(report.examined, ENTRIES, "{report:?}");
        assert!(!report.truncated, "{report:?}");
    }

    /// B1: a content directory is charged the same way, on the same budget.
    #[test]
    fn a_complete_pass_charges_the_content_directory_it_descends_into() {
        const CHILDREN: u32 = 5;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        let dir = content_dir(&root, "77777777-7777-7777-7777-777777777777");
        for index in 0..CHILDREN {
            write_at(&dir.join(format!("live-{index}.bin")), now);
        }

        let report = sweep_once(&root, now, MINUTE);

        // The root charges its one entry plus the fixed three; the child
        // charges its children plus its own fixed three.
        assert_eq!(
            report.operations,
            1 + FIXED_READS + CHILDREN + FIXED_READS,
            "{report:?}"
        );
        assert_eq!(report.examined, 1 + CHILDREN, "{report:?}");
        assert!(!report.truncated, "{report:?}");
    }

    /// B1: a truncated pass performs its budget and not one read more, and the
    /// pass that resumes it charges from the bound again rather than paying to
    /// skip back to where it stopped.
    #[test]
    fn a_truncated_pass_charges_exactly_its_budget_and_the_resume_charges_nothing() {
        const BUDGET: u32 = 4;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..8 {
            write_at(&root.join(format!("live-{index}.bin")), now);
        }

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        let first = sweeper.pass(now, MINUTE);
        let second = sweeper.pass(now, MINUTE);
        let third = sweeper.pass(now, MINUTE);

        assert_eq!(first.operations, BUDGET, "{first:?}");
        assert!(first.truncated);
        assert_eq!(second.operations, BUDGET, "{second:?}");
        assert!(second.truncated);
        // 8 entries + 2 dots + end-of-directory = 11 reads, of which 8 are
        // spent; the third pass completes the cycle in the remaining 3.
        assert_eq!(third.operations, 8 + FIXED_READS - 2 * BUDGET, "{third:?}");
        assert!(!third.truncated, "{third:?}");
        assert_eq!(
            first.examined + second.examined + third.examined,
            8,
            "the cycle examined an entry twice or not at all"
        );
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
        assert_eq!(first.operations, 2);
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
    /// one cycle of `ceil(reads / budget)` passes. Restarting `readdir` from the
    /// first entry has no such bound — a prefix of live files consumes every
    /// pass and everything behind it keeps its plaintext for ever.
    #[test]
    fn a_live_prefix_cannot_starve_the_expired_entries_behind_it() {
        const LIVE: u32 = 24;
        const EXPIRED: u32 = 4;
        const BUDGET: u32 = 4;

        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..LIVE {
            write_at(&root.join(format!("live-{index:02}.bin")), now);
        }
        let expired: Vec<PathBuf> = (0..EXPIRED)
            .map(|index| {
                let path = root.join(format!("expired-{index}.bin"));
                write_at(&path, now - HOUR);
                path
            })
            .collect();

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        let cycle = (LIVE + EXPIRED + FIXED_READS).div_ceil(BUDGET);
        for _ in 0..cycle {
            let _ = sweeper.pass(now, MINUTE);
        }

        assert!(
            expired.iter().all(|path| !path.exists()),
            "a live prefix starved the entries behind it past one cycle"
        );
        assert!(
            root.join("live-00.bin").exists(),
            "the sweep removed a payload inside its deadline"
        );
    }

    #[test]
    fn a_live_prefix_inside_a_content_directory_cannot_starve_it_either() {
        const LIVE: u32 = 8;
        const EXPIRED: u32 = 2;
        const BUDGET: u32 = 3;

        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        let dir = content_dir(&root, "55555555-5555-5555-5555-555555555555");
        for index in 0..LIVE {
            write_at(&dir.join(format!("live-{index}.bin")), now);
        }
        let expired: Vec<PathBuf> = (0..EXPIRED)
            .map(|index| {
                let path = dir.join(format!("expired-{index}.bin"));
                write_at(&path, now - HOUR);
                path
            })
            .collect();

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        // The root's one entry and fixed three, then the child's entries and
        // its own fixed three.
        let cycle = (1 + FIXED_READS + LIVE + EXPIRED + FIXED_READS).div_ceil(BUDGET);
        for _ in 0..cycle {
            let _ = sweeper.pass(now, MINUTE);
        }

        assert!(
            expired.iter().all(|path| !path.exists()),
            "a truncated content directory restarted instead of resuming"
        );
    }

    /// B1: a cycle is bounded over every directory stream it opens, not over
    /// the root's entries alone. Each stream charges its entries plus `.`, `..`
    /// and its own end-of-directory read, and a staging tree keeps nearly all
    /// of its payloads one level down — so counting only the root understates
    /// the retained-plaintext bound by three reads per content directory.
    #[test]
    fn one_cycle_charges_three_fixed_reads_for_every_stream_it_opens() {
        const DIRS: u32 = 3;
        const LIVE: u32 = 4;
        const EXPIRED: u32 = 2;
        const BUDGET: u32 = 5;

        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        let mut expired = Vec::new();
        for dir in 0..DIRS {
            let content = content_dir(&root, &format!("0000000{dir}-0000-0000-0000-000000000000"));
            for index in 0..LIVE {
                write_at(&content.join(format!("live-{index}.bin")), now);
            }
            for index in 0..EXPIRED {
                let path = content.join(format!("expired-{index}.bin"));
                write_at(&path, now - HOUR);
                expired.push(path);
            }
        }
        let reads = (DIRS + FIXED_READS) + DIRS * (LIVE + EXPIRED + FIXED_READS);

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        let mut charged = 0;
        for _ in 0..reads.div_ceil(BUDGET) {
            charged += sweeper.pass(now, MINUTE).operations;
        }

        assert_eq!(
            charged, reads,
            "the advertised cycle is not what a cycle costs"
        );
        assert!(
            sweeper.cycle.is_none(),
            "the cycle did not complete inside its advertised bound"
        );
        assert!(
            expired.iter().all(|path| !path.exists()),
            "expired plaintext survived one cycle"
        );
    }

    #[test]
    fn a_completed_pass_starts_the_next_one_at_the_first_entry() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..3 {
            write_at(&root.join(format!("live-{index}.bin")), now);
        }

        let mut sweeper = Sweeper::with_budget(open_root(&root), 4);
        assert!(sweeper.pass(now, MINUTE).truncated);
        assert!(sweeper.cycle.is_some(), "a truncated pass resumes");
        assert!(!sweeper.pass(now, MINUTE).truncated);
        assert!(
            sweeper.cycle.is_none(),
            "a completed cycle must start over, or new entries are never reached"
        );
    }

    /// The budget exists to bound how long the interactive paste-back path can
    /// be parked behind a sweep. Measured here rather than asserted from the
    /// constant alone.
    #[test]
    fn a_full_budget_sweep_holds_the_staging_lock_briefly() {
        let entries = SWEEP_BUDGET - FIXED_READS;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..entries {
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

        assert_eq!(report.operations, SWEEP_BUDGET, "{report:?}");
        assert_eq!(report.examined, entries, "{report:?}");
        assert_eq!(report.removed, entries, "{report:?}");
        assert!(!report.truncated, "{report:?}");
        println!("a {SWEEP_BUDGET}-read sweep held the staging lock for {held:?}");
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

    /// B1: deletions are the mutation an ordinal cursor could not survive —
    /// each one shifts an unvisited entry behind the cursor, and the cycle
    /// never finishes. On a stream that stays open, POSIX leaves only the
    /// removed entry unspecified, and it has already been visited.
    #[test]
    fn deleting_entries_does_not_break_the_one_cycle_claim() {
        const ENTRIES: u32 = 64;
        const BUDGET: u32 = 4;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..ENTRIES {
            write_at(&root.join(format!("expired-{index:03}.bin")), now - HOUR);
        }

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        for _ in 0..(ENTRIES + FIXED_READS).div_ceil(BUDGET) {
            let _ = sweeper.pass(now, MINUTE);
        }

        let remaining = fs::read_dir(&root).unwrap().count();
        assert_eq!(
            remaining, 0,
            "{remaining} entries survived one advertised cycle"
        );
    }

    /// B1: a resumed pass must hold the staging lock for O(budget), not
    /// O(saved position + budget). A retained stream skips nothing.
    #[test]
    fn a_resumed_pass_holds_the_lock_briefly() {
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..SWEEP_BUDGET {
            write_bytes_at(&root.join(format!("payload-{index:05}.bin")), b"x", now);
        }
        for index in 0..SWEEP_BUDGET {
            write_bytes_at(
                &root.join(format!("z-tail-{index:05}.bin")),
                b"x",
                now - HOUR,
            );
        }

        let mut sweeper = Sweeper::new(open_root(&root));
        let first = sweeper.pass(now, MINUTE);
        assert!(first.truncated, "setup: first pass must truncate");

        let started = std::time::Instant::now();
        let second = sweeper.pass(now, MINUTE);
        let held = started.elapsed();

        assert_eq!(second.operations, SWEEP_BUDGET, "{second:?}");
        println!("a resumed {SWEEP_BUDGET}-read pass held the staging lock for {held:?}");
        assert!(
            held < Duration::from_secs(2),
            "a resumed pass held the staging lock for {held:?}"
        );
    }

    /// B1: entries inserted mid-cycle are the half POSIX leaves unspecified.
    /// They cost at most the following cycle, and they must not extend the one
    /// the entries already on disk are being deleted in.
    #[test]
    fn entries_inserted_mid_cycle_do_not_extend_the_cycle_they_arrived_in() {
        const ORIGINAL: u32 = 32;
        const INSERTED: u32 = 8;
        const BUDGET: u32 = 8;
        let (_parent, root) = staging_root();
        let now = SystemTime::now();
        for index in 0..ORIGINAL {
            write_at(&root.join(format!("original-{index:02}.bin")), now - HOUR);
        }

        let mut sweeper = Sweeper::with_budget(open_root(&root), BUDGET);
        let cycle = (ORIGINAL + FIXED_READS).div_ceil(BUDGET);
        for pass in 0..cycle {
            let _ = sweeper.pass(now, MINUTE);
            if pass == 1 {
                for index in 0..INSERTED {
                    write_at(&root.join(format!("inserted-{index}.bin")), now - HOUR);
                }
            }
        }

        let survivors: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("original-"))
            .collect();
        assert!(
            survivors.is_empty(),
            "insertions extended the cycle: {survivors:?}"
        );

        // The inserted entries are bounded by the next cycle, not by this one.
        for _ in 0..(INSERTED + FIXED_READS).div_ceil(BUDGET) + 1 {
            let _ = sweeper.pass(now, MINUTE);
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}
