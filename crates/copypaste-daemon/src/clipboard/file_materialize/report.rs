//! What one sweep achieved, in a shape that is safe to log.

use std::fmt;
use std::io;

use rustix::io::Errno;

/// How many distinct [`io::ErrorKind`]s one report keeps a count for. The
/// report is a fixed size because the number of failing entries is not: a
/// directory of a million unreadable files must not become a million-entry map.
const FAILURE_KINDS: usize = 6;

/// Per-kind failure counts. Kinds and counts only — never a path, a filename, a
/// content id or a byte of a payload, because this is logged and the sweeper's
/// inputs are user filenames and content digests.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct FailureCounts {
    kinds: [Option<(io::ErrorKind, u32)>; FAILURE_KINDS],
    /// Failures past the `FAILURE_KINDS` distinct kinds above.
    overflow: u32,
}

impl FailureCounts {
    fn note(&mut self, error: Errno) {
        let kind = io::Error::from(error).kind();
        for slot in &mut self.kinds {
            match slot {
                Some((seen, count)) if *seen == kind => {
                    *count = count.saturating_add(1);
                    return;
                }
                Some(_) => {}
                None => {
                    *slot = Some((kind, 1));
                    return;
                }
            }
        }
        self.overflow = self.overflow.saturating_add(1);
    }

    #[cfg(test)]
    pub(super) fn count(&self, kind: io::ErrorKind) -> u32 {
        self.kinds
            .iter()
            .flatten()
            .find_map(|(seen, count)| (*seen == kind).then_some(*count))
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(super) fn distinct(&self) -> usize {
        self.kinds.iter().flatten().count()
    }
}

impl fmt::Debug for FailureCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut kinds = formatter.debug_map();
        for (kind, count) in self.kinds.iter().flatten() {
            kinds.entry(kind, count);
        }
        if self.overflow > 0 {
            kinds.entry(&format_args!("other"), &self.overflow);
        }
        kinds.finish()
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SweepReport {
    pub(super) examined: u32,
    pub(super) removed: u32,
    pub(super) directories_removed: u32,
    /// Expired plaintext that was still on disk when the sweep ended.
    pub(super) retained: u32,
    /// Entries the sweep could not classify. Each one may be hiding expired
    /// plaintext, so it is unfinished work rather than nothing.
    pub(super) unreadable: u32,
    /// The entry budget ran out or a `readdir` failed, leaving part of the tree
    /// unvisited. The next pass continues from where this one stopped.
    pub(super) truncated: bool,
    pub(super) failures: FailureCounts,
}

impl SweepReport {
    pub(super) fn unfinished(&self) -> bool {
        self.retained > 0 || self.unreadable > 0 || self.truncated
    }

    pub(super) fn note(&mut self, error: Errno) {
        self.failures.note(error);
    }

    pub(super) fn unreadable_entry(&mut self, error: Errno) {
        self.unreadable += 1;
        self.note(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_counted_separately_until_the_bound() {
        let mut counts = FailureCounts::default();
        for _ in 0..3 {
            counts.note(Errno::ACCESS);
        }
        counts.note(Errno::IO);
        counts.note(Errno::NOTEMPTY);

        assert_eq!(counts.count(io::ErrorKind::PermissionDenied), 3);
        assert_eq!(counts.count(io::ErrorKind::NotFound), 0);
        assert_eq!(counts.distinct(), 3, "{counts:?}");
        let rendered = format!("{counts:?}");
        assert!(rendered.contains("PermissionDenied: 3"), "{rendered}");
        assert!(rendered.contains("DirectoryNotEmpty: 1"), "{rendered}");
    }

    #[test]
    fn kinds_past_the_bound_are_counted_rather_than_dropped_or_grown() {
        let mut counts = FailureCounts::default();
        // Seven distinct kinds against a six-slot report.
        for error in [
            Errno::ACCESS,
            Errno::NOENT,
            Errno::EXIST,
            Errno::NOTEMPTY,
            Errno::ISDIR,
            Errno::NOTDIR,
            Errno::NOSPC,
        ] {
            counts.note(error);
        }

        assert_eq!(counts.distinct(), FAILURE_KINDS);
        let rendered = format!("{counts:?}");
        assert!(rendered.contains("other: 1"), "{rendered}");
    }

    #[test]
    fn a_report_that_removed_nothing_it_should_have_is_unfinished() {
        assert!(!SweepReport::default().unfinished());
        for report in [
            SweepReport {
                retained: 1,
                ..SweepReport::default()
            },
            SweepReport {
                unreadable: 1,
                ..SweepReport::default()
            },
            SweepReport {
                truncated: true,
                ..SweepReport::default()
            },
        ] {
            assert!(report.unfinished(), "{report:?}");
        }
    }
}
