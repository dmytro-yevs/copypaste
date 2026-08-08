use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;

use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt as _, FilePath};
use tauri_plugin_fs::{FsExt as _, OpenOptions};

use crate::backend::BackendError;

type Result<T> = std::result::Result<T, BackendError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WriteContract {
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

const REPLACE_OR_CREATE: WriteContract = WriteContract {
    write: true,
    append: false,
    truncate: true,
    create: true,
    create_new: false,
};

impl WriteContract {
    fn plugin_options(self) -> OpenOptions {
        let mut options = OpenOptions::new();
        options
            .write(self.write)
            .append(self.append)
            .truncate(self.truncate)
            .create(self.create)
            .create_new(self.create_new);
        // Unix paths are owner-only from creation; document providers own URI access.
        #[cfg(unix)]
        options.mode(0o600);
        options
    }

    #[cfg(test)]
    fn android_mode(self) -> String {
        let mut mode = String::new();
        if self.write {
            mode.push('w');
        }
        if self.truncate {
            mode.push('t');
        }
        if self.append {
            mode.push('a');
        }
        mode
    }
}

pub(super) async fn save_panel_file<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
) -> Option<FilePath> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(name)
        .save_file(move |chosen| {
            let _ = tx.send(chosen);
        });
    rx.await.unwrap_or(None)
}

pub(super) async fn open_panel_file<R: Runtime>(app: &AppHandle<R>) -> Option<FilePath> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_file(move |chosen| {
        let _ = tx.send(chosen);
    });
    rx.await.unwrap_or(None)
}

pub(super) fn picked_to_temp<R: Runtime>(
    app: &AppHandle<R>,
    src: FilePath,
    limit: u64,
    too_large_message: &'static str,
    message: &'static str,
) -> Result<tempfile::NamedTempFile> {
    let mut options = OpenOptions::new();
    options.read(true);
    let input = app
        .fs()
        .open(src, options)
        .map_err(|_| BackendError::Invalid(message))?;
    copy_to_temp(input, limit, too_large_message, message)
}

fn copy_to_temp(
    mut input: impl Read,
    limit: u64,
    too_large_message: &'static str,
    message: &'static str,
) -> Result<tempfile::NamedTempFile> {
    let mut output = tempfile::NamedTempFile::new().map_err(|_| BackendError::Invalid(message))?;
    let copied = std::io::copy(
        &mut input.by_ref().take(limit.saturating_add(1)),
        &mut output,
    )
    .map_err(|_| BackendError::Invalid(message))?;
    if copied > limit {
        return Err(BackendError::Invalid(too_large_message));
    }
    output.flush().map_err(|_| BackendError::Invalid(message))?;
    Ok(output)
}

pub(super) fn write_picked<R: Runtime>(
    app: &AppHandle<R>,
    dest: FilePath,
    bytes: &[u8],
    message: &'static str,
) -> Result<()> {
    copy_to_picked(app, dest, &mut std::io::Cursor::new(bytes), message)
}

pub(super) fn copy_to_picked<R: Runtime>(
    app: &AppHandle<R>,
    dest: FilePath,
    input: &mut impl Read,
    message: &'static str,
) -> Result<()> {
    copy_with(
        input,
        |contract| app.fs().open(dest, contract.plugin_options()),
        message,
    )
}

fn copy_with<W: Write>(
    input: &mut impl Read,
    open: impl FnOnce(WriteContract) -> std::io::Result<W>,
    message: &'static str,
) -> Result<()> {
    let mut output = open(REPLACE_OR_CREATE).map_err(|_| BackendError::Internal(message.into()))?;
    std::io::copy(input, &mut output)
        .and_then(|_| output.flush())
        .map_err(|_| BackendError::Internal(message.into()))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io;

    use super::*;

    const ERROR: &str = "The document couldn't be written.";

    struct FakeDestination {
        bytes: RefCell<Vec<u8>>,
        requests: RefCell<Vec<WriteContract>>,
        fail_open: bool,
    }

    impl FakeDestination {
        fn open(&self, contract: WriteContract) -> io::Result<FakeWriter<'_>> {
            self.requests.borrow_mut().push(contract);
            if self.fail_open {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "C:\\private",
                ));
            }
            if contract.truncate {
                self.bytes.borrow_mut().clear();
            }
            Ok(FakeWriter(&self.bytes))
        }
    }

    struct FakeWriter<'a>(&'a RefCell<Vec<u8>>);

    impl Write for FakeWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn write_path(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
        copy_with(
            &mut io::Cursor::new(bytes),
            |contract| std::fs::OpenOptions::from(contract.plugin_options()).open(path),
            ERROR,
        )
    }

    #[test]
    fn replacing_a_longer_destination_leaves_no_old_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        std::fs::write(&path, b"new value plus stale tail").unwrap();

        write_path(&path, b"new value").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new value");
    }

    #[test]
    fn a_missing_destination_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new-export.json");

        write_path(&path, b"new value").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new value");
    }

    #[test]
    fn an_open_failure_preserves_the_destination_and_maps_the_error() {
        let destination = FakeDestination {
            bytes: RefCell::new(b"original".to_vec()),
            requests: RefCell::default(),
            fail_open: true,
        };
        let mut input = io::Cursor::new(b"replacement");
        let result = copy_with(&mut input, |contract| destination.open(contract), ERROR);

        assert_eq!(&*destination.bytes.borrow(), b"original");
        assert_eq!(&*destination.requests.borrow(), &[REPLACE_OR_CREATE]);
        assert!(matches!(result, Err(BackendError::Internal(message)) if message == ERROR));
    }

    #[test]
    fn replace_contract_maps_to_the_plugin_android_wt_arguments() {
        assert_eq!(REPLACE_OR_CREATE.android_mode(), "wt");
    }
}
