//! Reap invocation/install/download leftovers without touching live module data.

use copypaste_fs::file_lock::{lock_shared, try_lock_exclusive, unlock, TryLock};
use std::{
    fs, io,
    path::Path,
    time::{Duration, SystemTime},
};

/// Managers retain a shared OS lease through inference shutdown. Cleanup needs
/// exclusive ownership, so reopening a manager cannot remove another one's work.
pub(crate) fn open(root: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".staging-owner"))?;
    match try_lock_exclusive(&file)? {
        TryLock::Acquired => {
            cleanup(root, SystemTime::now());
            unlock(&file)?;
            lock_shared(&file)?;
        }
        TryLock::WouldBlock => lock_shared(&file)?,
    }
    Ok(file)
}

fn cleanup(root: &Path, now: SystemTime) {
    for directory in [
        root.to_path_buf(),
        root.join("inputs"),
        root.join("packages"),
    ] {
        sweep(&directory, now);
    }
    if let Ok(entries) = fs::read_dir(root.join("updates")) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| semver::Version::parse(name).is_ok())
            {
                // Completed pending versions live under module/version names;
                // only interrupted tempfile extraction directories are eligible.
                sweep(&entry.path(), now);
            }
        }
    }
}

fn sweep(directory: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.len() != 10
            || !name.starts_with(".tmp")
            || !name[4..].bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.is_dir()
            && metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age >= Duration::from_secs(24 * 60 * 60))
        {
            // remove_dir_all removes internal symlinks, never their targets.
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_temporary_inputs_and_extracts_are_removed_but_models_and_pending_updates_survive() {
        let root = tempfile::tempdir().unwrap();
        let paths = [
            ".tmpAbc123",
            "inputs/.tmpAbc123",
            "packages/.tmpAbc123",
            "updates/2.0.0/.tmpAbc123",
        ];
        for path in paths {
            fs::create_dir_all(root.path().join(path)).unwrap();
        }
        let keep = [
            "data/semantic/models/model",
            "packages/ocr/1.0.0",
            "updates/2.0.0/ocr/1.1.0/package",
            "inputs/not-ours",
        ];
        for path in keep {
            fs::create_dir_all(root.path().join(path)).unwrap();
        }
        cleanup(
            root.path(),
            SystemTime::now() + Duration::from_secs(2 * 24 * 60 * 60),
        );
        for path in paths {
            assert!(!root.path().join(path).exists(), "{path}");
        }
        for path in keep {
            assert!(root.path().join(path).exists(), "{path}");
        }
    }

    #[test]
    fn live_manager_lease_prevents_cleanup_and_recent_staging_is_retained() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("inputs/.tmpAbc123")).unwrap();
        let first = open(root.path()).unwrap();
        let second = open(root.path()).unwrap();
        assert!(root.path().join("inputs/.tmpAbc123").exists());
        let probe = fs::File::options()
            .read(true)
            .write(true)
            .open(root.path().join(".staging-owner"))
            .unwrap();
        assert!(matches!(
            try_lock_exclusive(&probe).unwrap(),
            TryLock::WouldBlock
        ));
        drop(first);
        drop(second);
        assert!(matches!(
            try_lock_exclusive(&probe).unwrap(),
            TryLock::Acquired
        ));
        cleanup(
            root.path(),
            SystemTime::now() + Duration::from_secs(2 * 24 * 60 * 60),
        );
        assert!(!root.path().join("inputs/.tmpAbc123").exists());
    }
}
