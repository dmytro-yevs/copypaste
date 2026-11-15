//! Bounded, canonical classification of a sweep failure.

use std::fmt;

use rustix::io::Errno;

/// Every failure the sweep can report, in the order a report renders them.
///
/// Canonical rather than first-seen: which causes a report keeps must not
/// depend on the order the filesystem happened to hand entries back. Each
/// variant names an errno `openat`, `readdir`, `statat`, `unlinkat` or `rmdir`
/// can return on this path; `Unclassified` is reserved for values none of them
/// is documented to return, so a future kind is visible rather than silently
/// merged into one that is already meaningful.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FailureKind {
    NotFound,
    PermissionDenied,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    AlreadyExists,
    SymlinkLoop,
    NameTooLong,
    ReadOnlyFilesystem,
    ResourceBusy,
    StorageFull,
    Interrupted,
    OutOfMemory,
    DescriptorLimit,
    BadDescriptor,
    InvalidArgument,
    ValueTooLarge,
    StaleHandle,
    Unsupported,
    InputOutput,
    Unclassified,
}

impl FailureKind {
    /// Indexed by `kind as usize`, so the order here is the storage order and
    /// the render order both. `every_kind_indexes_itself` holds the two together.
    const ALL: [Self; 21] = [
        Self::NotFound,
        Self::PermissionDenied,
        Self::NotADirectory,
        Self::IsADirectory,
        Self::DirectoryNotEmpty,
        Self::AlreadyExists,
        Self::SymlinkLoop,
        Self::NameTooLong,
        Self::ReadOnlyFilesystem,
        Self::ResourceBusy,
        Self::StorageFull,
        Self::Interrupted,
        Self::OutOfMemory,
        Self::DescriptorLimit,
        Self::BadDescriptor,
        Self::InvalidArgument,
        Self::ValueTooLarge,
        Self::StaleHandle,
        Self::Unsupported,
        Self::InputOutput,
        Self::Unclassified,
    ];

    /// A table rather than a `match`, because several of these errnos share a
    /// value on some targets — `NOTSUP` and `OPNOTSUPP` on Linux — and a `match`
    /// arm for the second would be an unreachable-pattern error there.
    fn of(error: Errno) -> Self {
        const TABLE: &[(Errno, FailureKind)] = &[
            (Errno::NOENT, FailureKind::NotFound),
            (Errno::ACCESS, FailureKind::PermissionDenied),
            (Errno::PERM, FailureKind::PermissionDenied),
            (Errno::NOTDIR, FailureKind::NotADirectory),
            (Errno::ISDIR, FailureKind::IsADirectory),
            (Errno::NOTEMPTY, FailureKind::DirectoryNotEmpty),
            (Errno::EXIST, FailureKind::AlreadyExists),
            (Errno::LOOP, FailureKind::SymlinkLoop),
            (Errno::NAMETOOLONG, FailureKind::NameTooLong),
            (Errno::ROFS, FailureKind::ReadOnlyFilesystem),
            (Errno::BUSY, FailureKind::ResourceBusy),
            (Errno::TXTBSY, FailureKind::ResourceBusy),
            (Errno::NOSPC, FailureKind::StorageFull),
            (Errno::DQUOT, FailureKind::StorageFull),
            (Errno::INTR, FailureKind::Interrupted),
            (Errno::NOMEM, FailureKind::OutOfMemory),
            (Errno::MFILE, FailureKind::DescriptorLimit),
            (Errno::NFILE, FailureKind::DescriptorLimit),
            (Errno::BADF, FailureKind::BadDescriptor),
            (Errno::INVAL, FailureKind::InvalidArgument),
            (Errno::OVERFLOW, FailureKind::ValueTooLarge),
            (Errno::STALE, FailureKind::StaleHandle),
            (Errno::NOSYS, FailureKind::Unsupported),
            (Errno::NOTSUP, FailureKind::Unsupported),
            (Errno::OPNOTSUPP, FailureKind::Unsupported),
            (Errno::IO, FailureKind::InputOutput),
        ];
        TABLE
            .iter()
            .find_map(|(errno, kind)| (*errno == error).then_some(*kind))
            .unwrap_or(Self::Unclassified)
    }
}

/// Per-kind failure counts. Kinds and counts only — never a path, a filename, a
/// content id or a byte of a payload, because this is logged and the sweeper's
/// inputs are user filenames and content digests.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct FailureCounts {
    counts: [u32; FailureKind::ALL.len()],
}

impl FailureCounts {
    pub(super) fn note(&mut self, error: Errno) {
        let slot = &mut self.counts[FailureKind::of(error) as usize];
        *slot = slot.saturating_add(1);
    }

    #[cfg(test)]
    pub(super) fn count(&self, kind: FailureKind) -> u32 {
        self.counts[kind as usize]
    }
}

impl fmt::Debug for FailureCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut kinds = formatter.debug_map();
        for (kind, count) in FailureKind::ALL.iter().zip(&self.counts) {
            if *count > 0 {
                kinds.entry(kind, count);
            }
        }
        kinds.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every errno the sweep's syscalls are documented to return, paired with
    /// the kind it must be counted as.
    const REACHABLE: &[(Errno, FailureKind)] = &[
        (Errno::NOENT, FailureKind::NotFound),
        (Errno::ACCESS, FailureKind::PermissionDenied),
        (Errno::PERM, FailureKind::PermissionDenied),
        (Errno::NOTDIR, FailureKind::NotADirectory),
        (Errno::ISDIR, FailureKind::IsADirectory),
        (Errno::NOTEMPTY, FailureKind::DirectoryNotEmpty),
        (Errno::EXIST, FailureKind::AlreadyExists),
        (Errno::LOOP, FailureKind::SymlinkLoop),
        (Errno::NAMETOOLONG, FailureKind::NameTooLong),
        (Errno::ROFS, FailureKind::ReadOnlyFilesystem),
        (Errno::BUSY, FailureKind::ResourceBusy),
        (Errno::TXTBSY, FailureKind::ResourceBusy),
        (Errno::NOSPC, FailureKind::StorageFull),
        (Errno::DQUOT, FailureKind::StorageFull),
        (Errno::INTR, FailureKind::Interrupted),
        (Errno::NOMEM, FailureKind::OutOfMemory),
        (Errno::MFILE, FailureKind::DescriptorLimit),
        (Errno::NFILE, FailureKind::DescriptorLimit),
        (Errno::BADF, FailureKind::BadDescriptor),
        (Errno::INVAL, FailureKind::InvalidArgument),
        (Errno::OVERFLOW, FailureKind::ValueTooLarge),
        (Errno::STALE, FailureKind::StaleHandle),
        (Errno::NOSYS, FailureKind::Unsupported),
        (Errno::IO, FailureKind::InputOutput),
    ];

    /// Values none of the sweep's syscalls returns here. They must land in
    /// `Unclassified` rather than in a kind that already means something.
    const UNNAMED: &[Errno] = &[Errno::FBIG, Errno::PIPE, Errno::RANGE, Errno::DOM];

    fn counted(errors: impl IntoIterator<Item = Errno>) -> FailureCounts {
        let mut counts = FailureCounts::default();
        for error in errors {
            counts.note(error);
        }
        counts
    }

    #[test]
    fn every_kind_indexes_itself() {
        for (index, kind) in FailureKind::ALL.iter().enumerate() {
            assert_eq!(index, *kind as usize, "{kind:?} is out of canonical order");
        }
    }

    #[test]
    fn every_reachable_errno_is_counted_as_its_own_kind() {
        for (error, expected) in REACHABLE {
            let counts = counted([*error]);
            assert_eq!(counts.count(*expected), 1, "{error:?} -> {counts:?}");
            assert_eq!(
                counts.count(FailureKind::Unclassified),
                0,
                "{error:?} was not classified"
            );
        }
    }

    #[test]
    fn an_errno_the_taxonomy_does_not_name_is_unclassified() {
        for error in UNNAMED {
            let counts = counted([*error]);
            assert_eq!(counts.count(FailureKind::Unclassified), 1, "{error:?}");
        }
    }

    /// The defect this replaces kept the first eight distinct kinds it met, so
    /// which causes survived — and the order they rendered in — depended on the
    /// order the filesystem walked the directory.
    #[test]
    fn encounter_order_changes_neither_retention_nor_rendered_output() {
        let every: Vec<Errno> = REACHABLE
            .iter()
            .map(|(error, _)| *error)
            .chain(UNNAMED.iter().copied())
            .collect();
        let forward = counted(every.iter().copied());
        let expected = format!("{forward:?}");

        let mut reversed: Vec<Errno> = every.iter().copied().rev().collect();
        assert_eq!(format!("{:?}", counted(reversed.iter().copied())), expected);
        for rotation in 1..every.len() {
            reversed.rotate_left(1);
            let rotated = counted(reversed.iter().copied());
            assert_eq!(
                format!("{rotated:?}"),
                expected,
                "rotation {rotation} changed the report"
            );
            assert_eq!(rotated, forward, "rotation {rotation} changed the counts");
        }
    }

    #[test]
    fn the_rendered_report_is_exactly_the_non_zero_kinds_in_canonical_order() {
        let counts = counted([
            Errno::IO,
            Errno::ACCESS,
            Errno::NOENT,
            Errno::ACCESS,
            Errno::FBIG,
        ]);

        assert_eq!(
            format!("{counts:?}"),
            "{NotFound: 1, PermissionDenied: 2, InputOutput: 1, Unclassified: 1}"
        );
        assert_eq!(format!("{:?}", FailureCounts::default()), "{}");
    }

    #[test]
    fn a_kind_that_never_stops_failing_saturates_rather_than_wrapping() {
        let mut counts = FailureCounts::default();
        counts.counts[FailureKind::PermissionDenied as usize] = u32::MAX;

        counts.note(Errno::ACCESS);

        assert_eq!(counts.count(FailureKind::PermissionDenied), u32::MAX);
    }
}
