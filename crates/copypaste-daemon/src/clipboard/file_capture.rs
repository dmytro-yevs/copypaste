//! Fixed rejection reasons for local file capture. They never carry source data.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileReadError {
    OpenPermissionDenied,
    OpenNotFound,
    OpenFailed,
    #[cfg(test)]
    StatFailed,
    NonRegular,
    TooLarge,
    #[cfg(test)]
    UnsupportedEmptyFile,
    #[cfg(test)]
    AllocationFailed,
    #[cfg(test)]
    ReadPermissionDenied,
    ReadFailed,
    #[cfg(test)]
    SourceChanged,
}

impl FileReadError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::OpenPermissionDenied => "file capture input rejected: open permission denied",
            Self::OpenNotFound => "file capture input rejected: source not found",
            Self::OpenFailed => "file capture input rejected: open failed",
            #[cfg(test)]
            Self::StatFailed => "file capture input rejected: descriptor metadata unavailable",
            Self::NonRegular => "file capture input rejected: source is not a regular file",
            Self::TooLarge => "file capture input rejected: size limit exceeded",
            #[cfg(test)]
            Self::UnsupportedEmptyFile => "file capture input rejected: empty file unsupported",
            #[cfg(test)]
            Self::AllocationFailed => "file capture input rejected: allocation failed",
            #[cfg(test)]
            Self::ReadPermissionDenied => "file capture input rejected: read permission denied",
            Self::ReadFailed => "file capture input rejected: read failed",
            #[cfg(test)]
            Self::SourceChanged => "file capture input rejected: source changed during read",
        }
    }
}

#[cfg(test)]
thread_local! { static TEST_OPEN_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn reset_test_open_count() {
    TEST_OPEN_COUNT.with(|count| count.set(0));
}
#[cfg(test)]
pub(crate) fn test_open_count() -> usize {
    TEST_OPEN_COUNT.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(crate) fn note_test_open() {
    TEST_OPEN_COUNT.with(|count| count.set(count.get() + 1));
}
