//! What one sweep achieved, in a shape that is safe to log.

use rustix::io::Errno;

use super::failure::FailureCounts;

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

    #[test]
    fn an_unreadable_entry_is_both_counted_and_classified() {
        let mut report = SweepReport::default();

        report.unreadable_entry(Errno::ACCESS);

        assert_eq!(report.unreadable, 1);
        assert_eq!(format!("{:?}", report.failures), "{PermissionDenied: 1}");
    }
}
