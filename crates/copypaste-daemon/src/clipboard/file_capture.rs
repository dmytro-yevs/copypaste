//! Descriptor-bounded local file capture. Errors never carry source data.

use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileReadError {
    OpenPermissionDenied,
    OpenNotFound,
    OpenFailed,
    StatFailed,
    NonRegular,
    TooLarge,
    UnsupportedEmptyFile,
    AllocationFailed,
    ReadPermissionDenied,
    ReadFailed,
    SourceChanged,
}

impl FileReadError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::OpenPermissionDenied => "file capture input rejected: open permission denied",
            Self::OpenNotFound => "file capture input rejected: source not found",
            Self::OpenFailed => "file capture input rejected: open failed",
            Self::StatFailed => "file capture input rejected: descriptor metadata unavailable",
            Self::NonRegular => "file capture input rejected: source is not a regular file",
            Self::TooLarge => "file capture input rejected: size limit exceeded",
            Self::UnsupportedEmptyFile => "file capture input rejected: empty file unsupported",
            Self::AllocationFailed => "file capture input rejected: allocation failed",
            Self::ReadPermissionDenied => "file capture input rejected: read permission denied",
            Self::ReadFailed => "file capture input rejected: read failed",
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    regular: bool,
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64, i64, i64),
    #[cfg(not(unix))]
    created: Option<SystemTime>,
}
impl From<Metadata> for Snapshot {
    fn from(metadata: Metadata) -> Self {
        Self {
            regular: metadata.is_file(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            identity: {
                use std::os::unix::fs::MetadataExt;
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            },
            #[cfg(not(unix))]
            created: metadata.created().ok(),
        }
    }
}

fn open_error(error: io::Error) -> FileReadError {
    match error.kind() {
        io::ErrorKind::PermissionDenied => FileReadError::OpenPermissionDenied,
        io::ErrorKind::NotFound => FileReadError::OpenNotFound,
        _ => FileReadError::OpenFailed,
    }
}

pub(crate) fn read(path: &Path, cap: u64) -> Result<Vec<u8>, FileReadError> {
    #[cfg(test)]
    TEST_OPEN_COUNT.with(|count| count.set(count.get() + 1));
    #[cfg(unix)]
    let mut file = {
        use rustix::fs::{open, Mode, OFlags};
        let descriptor = open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|error| open_error(error.into()))?;
        File::from(descriptor)
    };
    #[cfg(not(unix))]
    let mut file = File::open(path).map_err(open_error)?;
    let before = file
        .metadata()
        .map(Snapshot::from)
        .map_err(|_| FileReadError::StatFailed)?;
    read_bounded(&mut file, before, cap, |file| {
        file.metadata()
            .map(Snapshot::from)
            .map_err(|_| FileReadError::StatFailed)
    })
}

fn read_bounded<R: Read>(
    reader: &mut R,
    before: Snapshot,
    cap: u64,
    after: impl FnOnce(&R) -> Result<Snapshot, FileReadError>,
) -> Result<Vec<u8>, FileReadError> {
    let cap = cap.min(copypaste_ipc::MAX_CONTENT_BYTES as u64);
    if !before.regular {
        return Err(FileReadError::NonRegular);
    }
    if before.len == 0 {
        return Err(FileReadError::UnsupportedEmptyFile);
    }
    if before.len > cap {
        return Err(FileReadError::TooLarge);
    }
    let len = usize::try_from(before.len).map_err(|_| FileReadError::TooLarge)?;
    let capacity = len.checked_add(1).ok_or(FileReadError::TooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| FileReadError::AllocationFailed)?;
    bytes.resize(capacity, 0);
    let mut used = 0;
    while used < capacity {
        match reader.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => used += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(match error.kind() {
                    io::ErrorKind::PermissionDenied => FileReadError::ReadPermissionDenied,
                    _ => FileReadError::ReadFailed,
                })
            }
        }
    }
    let after = after(reader)?;
    if used as u64 > cap || after.len > cap {
        return Err(FileReadError::TooLarge);
    }
    if used != len || before != after {
        return Err(FileReadError::SourceChanged);
    }
    bytes.truncate(used);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn snapshot(len: u64) -> Snapshot {
        Snapshot {
            regular: true,
            len,
            modified: None,
            #[cfg(unix)]
            identity: (1, 1, 1, 1, 1, 1),
            #[cfg(not(unix))]
            created: None,
        }
    }
    #[test]
    fn boundary_and_growth_shrink_mutation_are_distinct() {
        let before = snapshot(4);
        assert_eq!(
            read_bounded(&mut Cursor::new(b"1234"), before.clone(), 4, |_| Ok(
                before.clone()
            )),
            Ok(b"1234".to_vec())
        );
        assert_eq!(
            read_bounded(&mut Cursor::new(b"12345"), before.clone(), 4, |_| Ok(
                snapshot(5)
            )),
            Err(FileReadError::TooLarge)
        );
        assert_eq!(
            read_bounded(&mut Cursor::new(b"12345"), before.clone(), 8, |_| Ok(
                snapshot(5)
            )),
            Err(FileReadError::SourceChanged)
        );
        assert_eq!(
            read_bounded(&mut Cursor::new(b"123"), before.clone(), 8, |_| Ok(
                snapshot(3)
            )),
            Err(FileReadError::SourceChanged)
        );
        let mut mutated = before.clone();
        mutated.modified = Some(SystemTime::UNIX_EPOCH);
        assert_eq!(
            read_bounded(&mut Cursor::new(b"1234"), before, 8, |_| Ok(mutated)),
            Err(FileReadError::SourceChanged)
        );
    }
    #[test]
    fn unsupported_inputs_do_not_read() {
        struct NeverRead;
        impl Read for NeverRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("must not read rejected file");
            }
        }
        let mut nonregular = snapshot(4);
        nonregular.regular = false;
        for (before, cap, expected) in [
            (snapshot(0), 4, FileReadError::UnsupportedEmptyFile),
            (snapshot(5), 4, FileReadError::TooLarge),
            (nonregular, 4, FileReadError::NonRegular),
        ] {
            assert_eq!(
                read_bounded(&mut NeverRead, before, cap, |_| panic!(
                    "must not stat again"
                )),
                Err(expected)
            );
        }
    }
    #[test]
    fn io_errors_are_fixed_and_partial_bytes_are_discarded() {
        struct Denied;
        impl Read for Denied {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "private path must not escape",
                ))
            }
        }
        assert_eq!(
            read_bounded(&mut Denied, snapshot(4), 4, |_| Ok(snapshot(4))),
            Err(FileReadError::ReadPermissionDenied)
        );
        assert_eq!(
            open_error(io::Error::from(io::ErrorKind::NotFound)),
            FileReadError::OpenNotFound
        );
        assert_eq!(
            open_error(io::Error::from(io::ErrorKind::PermissionDenied)),
            FileReadError::OpenPermissionDenied
        );
        assert_eq!(
            open_error(io::Error::from(io::ErrorKind::Other)),
            FileReadError::OpenFailed
        );
    }
    #[test]
    fn descriptor_reader_handles_missing_empty_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read(&dir.path().join("absent"), 4),
            Err(FileReadError::OpenNotFound)
        );
        #[cfg(unix)]
        assert_eq!(read(dir.path(), 4), Err(FileReadError::NonRegular));
        let path = dir.path().join("empty");
        File::create(&path).unwrap();
        assert_eq!(read(&path, 4), Err(FileReadError::UnsupportedEmptyFile));
        std::fs::write(&path, b"1234").unwrap();
        assert_eq!(read(&path, 4), Ok(b"1234".to_vec()));
    }
    #[cfg(unix)]
    #[test]
    fn nonblocking_descriptor_rejects_fifo_socket_and_preserves_regular_symlink() {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        use rustix::fs::{mkfifoat, Mode, CWD};
        let dir = tempfile::tempdir().unwrap();
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let fifo = dir.path().join("fifo");
            mkfifoat(CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();
            assert_eq!(read(&fifo, 4), Err(FileReadError::NonRegular));
        }
        let socket = dir.path().join("socket");
        let _socket = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(matches!(
            read(&socket, 4),
            Err(FileReadError::OpenFailed) | Err(FileReadError::NonRegular)
        ));
        let regular = dir.path().join("regular");
        std::fs::write(&regular, b"1234").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&regular, &link).unwrap();
        assert_eq!(read(&link, 4), Ok(b"1234".to_vec()));
    }
    #[test]
    fn allocation_and_read_slice_are_bounded_by_observed_length_plus_one() {
        struct Inspect {
            maximum: usize,
        }
        impl Read for Inspect {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.maximum = self.maximum.max(bytes.len());
                bytes.fill(1);
                Ok(bytes.len())
            }
        }
        let mut reader = Inspect { maximum: 0 };
        assert_eq!(
            read_bounded(&mut reader, snapshot(4), 100, |_| Ok(snapshot(4))),
            Err(FileReadError::SourceChanged)
        );
        assert_eq!(reader.maximum, 5);
    }
}
