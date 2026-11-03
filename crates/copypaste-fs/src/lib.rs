#![cfg_attr(not(windows), forbid(unsafe_code))]
#![cfg_attr(windows, deny(unsafe_code))]

//! Durable file replacement and owner-only file creation.
//!
//! Four call sites carried a copy of this and they had drifted. The peer sync
//! cursors were written with no `fsync` at all, so a power loss could publish a
//! rename to contents that had never reached the disk, and two of the four
//! wrote key material without narrowing the mode first.

use std::fs::File;
use std::io::{self, Write as _};
use std::path::Path;
use std::sync::Mutex;

use tempfile::NamedTempFile;

pub mod file_lock;
#[cfg(windows)]
mod owner_acl;
#[cfg(windows)]
pub use owner_acl::{owner_only_descriptor, OwnerScope};

static REPLACE: Mutex<()> = Mutex::new(());

/// Mode the owner-only helpers apply on unix.
#[cfg(unix)]
pub const OWNER_ONLY_MODE: u32 = 0o600;

/// Who may read the file once it is in place.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Visibility {
    /// The temporary file is created by [`create_private`], so key material
    /// never exists with wider access — not even for the span of one write. A
    /// fresh file replaces the target on every write, so a file someone widened
    /// by hand is narrowed again rather than left open.
    OwnerOnly,
    /// No access is set. On unix not a wider file: `tempfile` creates at
    /// `0600` and the rename carries that inode into place. On Windows the
    /// file inherits the directory's access list.
    Inherited,
}

/// Create a new file, failing if `path` exists, opened for writing and
/// readable only by its owner from the moment it exists.
///
/// Unix: created at [`OWNER_ONLY_MODE`]. Windows: created with a protected
/// access list that grants the current user full control and nobody else
/// anything, inheritance from the directory included.
///
/// # Errors
///
/// Any I/O failure; an existing `path` reports [`io::ErrorKind::AlreadyExists`].
pub fn create_private(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(OWNER_ONLY_MODE)
            .open(path)
    }
    #[cfg(windows)]
    {
        owner_acl::create_new(path)
    }
}

/// Narrow an existing file to its owner, with the same access
/// [`create_private`] creates. Handles opened before the call keep theirs.
///
/// # Errors
///
/// Any failure to read the account or to apply the access.
pub fn restrict_to_owner(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(OWNER_ONLY_MODE))
    }
    #[cfg(windows)]
    {
        owner_acl::restrict(path, OwnerScope::Object)
    }
}

/// Narrow an existing directory to its owner, with access inherited by
/// everything later created inside it.
///
/// # Errors
///
/// Any failure to read the account or to apply the access.
#[cfg(windows)]
pub fn restrict_tree_to_owner(path: &Path) -> io::Result<()> {
    owner_acl::restrict(path, OwnerScope::Tree)
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

    let mut tmp = match visibility {
        Visibility::OwnerOnly => tempfile::Builder::new().make_in(dir, create_private)?,
        Visibility::Inherited => NamedTempFile::new_in(dir)?,
    };
    tmp.write_all(bytes)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    persist(tmp, path)?;
    sync_dir(dir);
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

    #[test]
    fn a_private_file_is_owner_only_and_never_replaces_an_existing_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("export.json");
        create_private(&path)
            .expect("create")
            .write_all(b"history")
            .expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, OWNER_ONLY_MODE);
        }

        let error = create_private(&path).expect_err("must not clobber");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).expect("read"), b"history");
        restrict_to_owner(&path).expect("restrict");
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
