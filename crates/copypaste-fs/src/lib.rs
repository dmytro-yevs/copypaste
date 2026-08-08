#![forbid(unsafe_code)]

//! Durable file replacement.
//!
//! Four call sites carried a copy of this and they had drifted. The peer sync
//! cursors were written with no `fsync` at all, so a power loss could publish a
//! rename to contents that had never reached the disk, and two of the four
//! wrote key material without narrowing the mode first.

use std::io::{self, Write as _};
use std::path::Path;
use std::sync::Mutex;

use tempfile::NamedTempFile;

static REPLACE: Mutex<()> = Mutex::new(());

/// Mode [`Visibility::OwnerOnly`] applies on unix.
#[cfg(unix)]
pub const OWNER_ONLY_MODE: u32 = 0o600;

/// Who may read the file once it is in place.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Visibility {
    /// [`OWNER_ONLY_MODE`], applied to the temporary file *before* any bytes go
    /// into it, so key material never exists at a wider mode — not even for the
    /// span of one write. It is applied on every write, so a file someone
    /// widened by hand is narrowed again rather than left open.
    OwnerOnly,
    /// No mode is set. Not a wider file: `tempfile` creates at `0600` and the
    /// rename carries that inode into place, so this differs from
    /// [`Visibility::OwnerOnly`] only in promising nothing.
    Inherited,
}

/// Replace `path` with `bytes` in one observable step, creating the parent
/// directory if it is missing.
///
/// Readers see the previous contents or the new ones, never a mixture or gap.
/// The same-directory temporary is synced before rename and the directory after.
///
/// Concurrent calls in this process are serialized. Multiple processes writing
/// the same target are outside this API's ownership contract.
///
/// # Errors
///
/// Any I/O failure. The previous contents survive all of them.
pub fn write_atomically(path: &Path, bytes: &[u8], visibility: Visibility) -> io::Result<()> {
    write_with(path, bytes, visibility, |tmp, target| {
        let _replace = REPLACE.lock().unwrap_or_else(|error| error.into_inner());
        tmp.persist(target).map(|_| ()).map_err(|err| err.error)
    })
}

fn write_with(
    path: &Path,
    bytes: &[u8],
    visibility: Visibility,
    persist: impl FnOnce(NamedTempFile, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    if let Some(dir) = parent {
        std::fs::create_dir_all(dir)?;
    }
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut tmp = NamedTempFile::new_in(dir)?;
    if visibility == Visibility::OwnerOnly {
        restrict(tmp.as_file())?;
    }
    tmp.write_all(bytes)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    persist(tmp, path)?;
    sync_dir(dir);
    Ok(())
}

#[cfg(unix)]
fn restrict(file: &std::fs::File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(OWNER_ONLY_MODE))
}

/// Scope is macOS and Android, both unix. This exists so the workspace still
/// builds on a Windows CI runner, not as a supported configuration.
#[cfg(not(unix))]
fn restrict(_file: &std::fs::File) -> io::Result<()> {
    Ok(())
}

/// Best effort: some filesystems refuse to open a directory for sync, and
/// failing the whole write for that would be worse than the residual risk of
/// losing the rename to a power cut.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn temp_entries(dir: &Path) -> usize {
        std::fs::read_dir(dir).expect("read_dir").count()
    }

    #[test]
    fn a_replaced_file_never_reads_as_a_mixture_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");

        for round in 0..8u8 {
            let bytes = vec![round; 128 * 1024];
            write_atomically(&path, &bytes, Visibility::Inherited).expect("write");
            assert_eq!(std::fs::read(&path).expect("read"), bytes);
            // The temporary file is gone, so the target is the only entry.
            assert_eq!(temp_entries(dir.path()), 1);
        }
    }

    #[test]
    fn concurrent_writers_publish_one_complete_value_without_collisions() {
        const WRITERS: usize = 16;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Arc::new(dir.path().join("state.json"));
        let barrier = Arc::new(Barrier::new(WRITERS));
        let values: Vec<Vec<u8>> = (0..WRITERS)
            .map(|writer| vec![writer as u8; 256 * 1024])
            .collect();

        std::thread::scope(|scope| {
            let handles: Vec<_> = values
                .iter()
                .map(|value| {
                    let path = Arc::clone(&path);
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        write_atomically(&path, value, Visibility::Inherited)
                    })
                })
                .collect();
            for handle in handles {
                handle.join().expect("writer panicked").expect("write");
            }
        });

        let stored = std::fs::read(path.as_path()).expect("read");
        assert!(values.contains(&stored));
        assert_eq!(temp_entries(dir.path()), 1);
    }

    #[test]
    fn a_missing_parent_directory_is_created() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("deeper").join("state.json");

        write_atomically(&path, b"hello", Visibility::Inherited).expect("write");

        assert_eq!(std::fs::read(&path).expect("read"), b"hello");
    }

    #[test]
    fn a_failed_publish_keeps_the_previous_contents_and_removes_the_temporary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.json");
        write_atomically(&path, b"before", Visibility::Inherited).expect("first write");

        let error = write_with(&path, b"after", Visibility::Inherited, |_, _| {
            Err(io::Error::other("publish refused"))
        })
        .expect_err("must fail");

        assert_eq!(error.to_string(), "publish refused");
        assert_eq!(std::fs::read(&path).expect("read"), b"before");
        assert_eq!(temp_entries(dir.path()), 1);
    }

    #[cfg(unix)]
    #[test]
    fn owner_only_narrows_a_file_that_was_widened_by_hand() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("keys.json");
        write_atomically(&path, b"secret", Visibility::OwnerOnly).expect("write");
        let mode = |path: &Path| {
            std::fs::metadata(path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(&path), OWNER_ONLY_MODE);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("widen");
        write_atomically(&path, b"secret again", Visibility::OwnerOnly).expect("rewrite");

        assert_eq!(mode(&path), OWNER_ONLY_MODE);
    }
}
