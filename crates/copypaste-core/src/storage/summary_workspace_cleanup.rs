//! Leases and cleanup for app-owned encrypted sync workspaces.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use copypaste_fs::file_lock::{lock_exclusive, try_lock_exclusive, TryLock};

const ROOT: &str = ".copypaste-sync-workspaces";
const PREFIX: &str = "session-";
const LOCK: &str = ".active";
const OWNER: &str = ".owner";
static COORDINATOR: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) struct WorkspaceDirectory {
    // Close the session lease before removing the directory on Windows.
    _lease: File,
    directory: tempfile::TempDir,
}

impl WorkspaceDirectory {
    pub(super) fn path(&self) -> &Path {
        self.directory.path()
    }
}

pub(super) fn create(parent: Option<&Path>) -> io::Result<WorkspaceDirectory> {
    let Some(parent) = parent else {
        // In-memory stores have no app-owned data directory. Their unique
        // owner-only temporary directory is never swept as a shared root.
        let directory = tempfile::Builder::new().prefix(PREFIX).tempdir()?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join(LOCK))?;
        lock_exclusive(&lease)?;
        return Ok(WorkspaceDirectory {
            _lease: lease,
            directory,
        });
    };
    let root = parent.join(ROOT);
    let _process_guard = COORDINATOR
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    fs::create_dir_all(&root)?;
    if !fs::symlink_metadata(&root)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid sync workspace root",
        ));
    }
    let coordination = root.join(".coordination");
    if let Ok(meta) = fs::symlink_metadata(&coordination) {
        if !meta.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid sync workspace lock",
            ));
        }
    }
    let guard = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(coordination)?;
    lock_exclusive(&guard)?;
    sweep(&root)?;
    let directory = tempfile::Builder::new().prefix(PREFIX).tempdir_in(&root)?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.path().join(LOCK))?;
    lock_exclusive(&lease)?;
    let mut owner = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.path().join(OWNER))?;
    write!(owner, "{}", std::process::id())?;
    drop(owner);
    drop(guard);
    Ok(WorkspaceDirectory {
        _lease: lease,
        directory,
    })
}

fn sweep(root: &Path) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with(PREFIX) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_dir() {
            continue;
        }
        // Same-process advisory locks are platform dependent; never open a
        // lease owned by this process, including another runtime engine.
        let owner = path.join(OWNER);
        if let Ok(meta) = fs::symlink_metadata(&owner) {
            if !meta.file_type().is_file() || meta.len() > 32 {
                continue;
            }
            if fs::read_to_string(&owner)
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                == Some(std::process::id())
            {
                continue;
            }
        }
        let lock_path = path.join(LOCK);
        match fs::symlink_metadata(&lock_path) {
            Ok(meta) if meta.file_type().is_file() => {
                let Ok(file) = OpenOptions::new().read(true).write(true).open(&lock_path) else {
                    continue;
                };
                match try_lock_exclusive(&file) {
                    Ok(TryLock::Acquired) => {}
                    Ok(TryLock::WouldBlock) | Err(_) => continue,
                }
                drop(file);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => continue,
        }
        // Names are unique and new sessions can only be created while this
        // sweep holds the coordination lock.
        let _ = fs::remove_dir_all(path);
    }
    Ok(())
}

/// Clean abandoned encrypted sessions when the owning store is opened.
pub(super) fn cleanup(parent: &Path) -> io::Result<()> {
    // OS locks may be process-scoped. Serialize engines within this process
    // as well as coordinating with other processes via the file lock.
    let _process_guard = match COORDINATOR.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
    };
    let root = parent.join(ROOT);
    let metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid sync workspace root",
        ));
    }
    let coordination = root.join(".coordination");
    if let Ok(metadata) = fs::symlink_metadata(&coordination) {
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid sync workspace lock",
            ));
        }
    }
    let guard = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(coordination)?;
    match try_lock_exclusive(&guard) {
        Ok(TryLock::Acquired) => sweep(&root),
        Ok(TryLock::WouldBlock) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_holds_workspace_lease() {
        let Some(path) = std::env::var_os("COPYPASTE_TEST_WORKSPACE_LEASE") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join(LOCK))
            .unwrap();
        lock_exclusive(&lease).unwrap();
        fs::write(path.join("ready"), b"").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !path.join("release").exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn sweep_preserves_a_lease_held_by_another_process() {
        let parent = tempfile::tempdir().unwrap();
        let live = create(Some(parent.path())).unwrap();
        let path = live.path().to_owned();
        // Move ownership of this fixture to the child without deleting it.
        let WorkspaceDirectory {
            _lease: lease,
            directory,
        } = live;
        drop(lease);
        fs::write(path.join(OWNER), b"0").unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::summary_workspace_cleanup::tests::child_holds_workspace_lease",
            ])
            .env("COPYPASTE_TEST_WORKSPACE_LEASE", &path)
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !path.join("ready").exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cleanup(parent.path()).unwrap();
        assert!(path.exists());
        fs::write(path.join("release"), b"").unwrap();
        assert!(child.wait().unwrap().success());
        cleanup(parent.path()).unwrap();
        assert!(!path.exists());
        drop(directory);
    }

    #[test]
    fn sweep_removes_abandoned_and_preserves_live_and_unrelated_paths() {
        let parent = tempfile::tempdir().unwrap();
        let live = create(Some(parent.path())).unwrap();
        let root = parent.path().join(ROOT);
        let stale = root.join("session-abandoned");
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join(LOCK), b"").unwrap();
        fs::write(stale.join(OWNER), b"0").unwrap();
        let unrelated = root.join("user-folder");
        fs::create_dir(&unrelated).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&unrelated, root.join("session-link")).unwrap();
        cleanup(parent.path()).unwrap();
        let second = create(Some(parent.path())).unwrap();
        assert!(live.path().exists());
        assert!(second.path().exists());
        assert!(!stale.exists());
        assert!(unrelated.exists());
        #[cfg(unix)]
        assert!(fs::symlink_metadata(root.join("session-link"))
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
