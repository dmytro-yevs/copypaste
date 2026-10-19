//! Owner-only, short-lived plaintext files for Windows file paste-back.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use copypaste_core::FileMetadata;
use widestring::U16CString;
use win_security_identifier::{GetCurrentSid, SecurityIdentifier};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
};

const MAX_AGE: Duration = Duration::from_secs(10 * 60);
const RECOVERY_RETRY: Duration = Duration::from_secs(60);

pub(super) struct StagingArea {
    root: PathBuf,
    state: Arc<(Mutex<State>, Condvar)>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

struct State {
    entries: Vec<Entry>,
    stopped: bool,
    recovering: bool,
}

struct Entry {
    directory: PathBuf,
    expires: Instant,
}

impl StagingArea {
    pub(super) fn new(data_dir: &Path) -> io::Result<Self> {
        Self::with_recovery_retry(data_dir, RECOVERY_RETRY)
    }

    fn with_recovery_retry(data_dir: &Path, recovery_retry: Duration) -> io::Result<Self> {
        let root = std::path::absolute(data_dir)?.join("paste-files");
        let state = Arc::new((
            Mutex::new(State {
                entries: Vec::new(),
                stopped: false,
                recovering: false,
            }),
            Condvar::new(),
        ));
        let area = Self {
            root,
            state,
            worker: Mutex::new(None),
        };
        if let Err(error) = prepare_root(&area.root) {
            // Existing owner-only plaintext may still be in use by the target
            // application. Keep retrying cleanup, but do not prevent text,
            // image, or capture service from starting while file paste-back is
            // safely unavailable.
            tracing::warn!(
                error_kind = ?error.kind(),
                "Windows paste-file staging cleanup is deferred; native file paste-back is unavailable"
            );
            area.start_recovery(recovery_retry)?;
        }
        Ok(area)
    }

    pub(super) fn materialize(&self, bytes: &[u8], metadata: &FileMetadata) -> io::Result<PathBuf> {
        if !metadata.is_valid() || metadata.filename.contains(['/', '\\', ':']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid file metadata",
            ));
        }
        let filename = metadata.filename.as_str();
        let (lock, wake) = &*self.state;
        let mut state = lock.lock().unwrap_or_else(|held| held.into_inner());
        if state.recovering {
            return Err(io::Error::from(io::ErrorKind::WouldBlock));
        }
        prune_locked(&mut state.entries);
        let directory = tempfile::Builder::new()
            .prefix("file-")
            .tempdir_in(&self.root)?;
        let directory = directory.keep();
        let path = directory.join(filename);
        state.entries.push(Entry {
            directory,
            expires: Instant::now() + MAX_AGE,
        });
        if let Err(error) = self.ensure_worker() {
            let entry = state
                .entries
                .pop()
                .expect("the staging entry was just inserted");
            let _ = fs::remove_dir_all(entry.directory);
            return Err(error);
        }
        wake.notify_one();
        drop(state);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        if metadata.transfer.is_some() {
            let mut source = fs::File::open(
                metadata
                    .source_reference
                    .as_deref()
                    .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?,
            )?;
            io::copy(&mut source, &mut file)?;
        } else {
            file.write_all(bytes)?;
        }
        Ok(path)
    }

    fn ensure_worker(&self) -> io::Result<()> {
        let mut worker = self.worker.lock().unwrap_or_else(|held| held.into_inner());
        if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
            return Ok(());
        }
        if let Some(worker) = worker.take() {
            let _ = worker.join();
        }
        let state = Arc::clone(&self.state);
        *worker = Some(
            std::thread::Builder::new()
                .name("copypaste-file-sweeper".into())
                .spawn(move || sweep(state))?,
        );
        Ok(())
    }

    fn start_recovery(&self, retry: Duration) -> io::Result<()> {
        {
            let (lock, _) = &*self.state;
            lock.lock()
                .unwrap_or_else(|held| held.into_inner())
                .recovering = true;
        }
        let root = self.root.clone();
        let state = Arc::clone(&self.state);
        *self.worker.lock().unwrap_or_else(|held| held.into_inner()) = Some(
            std::thread::Builder::new()
                .name("copypaste-file-staging-recovery".into())
                .spawn(move || recover(root, state, retry))?,
        );
        Ok(())
    }
}

impl Drop for StagingArea {
    fn drop(&mut self) {
        let (state, wake) = &*self.state;
        state
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .stopped = true;
        wake.notify_one();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .take()
        {
            let _ = worker.join();
        }
        prune_locked(
            &mut state
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .entries,
        );
        if let Err(error) = fs::remove_dir_all(&self.root) {
            tracing::warn!(error_kind = ?error.kind(), "could not remove Windows paste-file staging");
        }
    }
}

fn sweep(state: Arc<(Mutex<State>, Condvar)>) {
    loop {
        let (lock, wake) = &*state;
        let mut state = lock.lock().unwrap_or_else(|held| held.into_inner());
        prune_locked(&mut state.entries);
        if state.stopped {
            return;
        }
        let Some(deadline) = state.entries.iter().map(|entry| entry.expires).min() else {
            let _state = wake.wait(state).unwrap_or_else(|held| held.into_inner());
            continue;
        };
        let wait = deadline.saturating_duration_since(Instant::now());
        let _state = wake
            .wait_timeout(state, wait)
            .unwrap_or_else(|held| held.into_inner());
    }
}

fn recover(root: PathBuf, state: Arc<(Mutex<State>, Condvar)>, retry: Duration) {
    loop {
        if prepare_root(&root).is_ok() {
            let (lock, wake) = &*state;
            let mut guarded = lock.lock().unwrap_or_else(|held| held.into_inner());
            if guarded.stopped {
                return;
            }
            guarded.recovering = false;
            wake.notify_all();
            drop(guarded);
            sweep(state);
            return;
        }

        let (lock, wake) = &*state;
        let guarded = lock.lock().unwrap_or_else(|held| held.into_inner());
        if guarded.stopped {
            return;
        }
        let (guarded, _) = wake
            .wait_timeout(guarded, retry)
            .unwrap_or_else(|held| held.into_inner());
        if guarded.stopped {
            return;
        }
    }
}

fn prepare_root(root: &Path) -> io::Result<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    protect_owner_only(root)
}

fn prune_locked(entries: &mut Vec<Entry>) {
    let now = Instant::now();
    let mut retained = Vec::new();
    for mut entry in std::mem::take(entries) {
        if entry.expires > now {
            retained.push(entry);
        } else if fs::remove_dir_all(&entry.directory).is_err() {
            entry.expires = now + Duration::from_secs(60);
            retained.push(entry);
        }
    }
    *entries = retained;
}

fn protect_owner_only(path: &Path) -> io::Result<()> {
    let sid = SecurityIdentifier::get_current_user_sid()
        .map_err(|_| io::Error::other("read account identifier"))?;
    let sddl = U16CString::from_str(format!("D:P(A;OICI;GA;;;{sid})"))
        .map_err(|_| io::Error::other("build access list"))?;
    let mut descriptor = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        SetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    fn metadata() -> FileMetadata {
        FileMetadata::new("recovered.txt", "application/octet-stream").unwrap()
    }

    fn wait_until(within: Duration, mut ready: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if ready() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        ready()
    }

    #[test]
    fn an_expired_staging_directory_is_deleted() {
        let parent = tempfile::tempdir().unwrap();
        let directory = tempfile::tempdir_in(parent.path()).unwrap().keep();
        std::fs::write(directory.join("fixture.bin"), b"synthetic").unwrap();
        let mut entries = vec![Entry {
            directory: directory.clone(),
            expires: Instant::now() - Duration::from_secs(1),
        }];

        prune_locked(&mut entries);

        assert!(entries.is_empty());
        assert!(!directory.exists());
    }

    #[test]
    fn a_failed_staging_deletion_stays_owned_for_retry() {
        let parent = tempfile::tempdir().unwrap();
        let file = parent.path().join("not-a-directory");
        std::fs::write(&file, b"synthetic").unwrap();
        let mut entries = vec![Entry {
            directory: file,
            expires: Instant::now() - Duration::from_secs(1),
        }];

        prune_locked(&mut entries);

        assert_eq!(entries.len(), 1);
        assert!(entries[0].expires > Instant::now());
    }

    #[test]
    fn locked_staging_from_the_previous_process_recovers_without_stopping_capture() {
        let data_dir = tempfile::tempdir().unwrap();
        let previous = StagingArea::new(data_dir.path()).unwrap();
        let previous_file = previous
            .materialize(b"previous plaintext", &metadata())
            .unwrap();
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&previous_file)
            .unwrap();
        drop(previous);
        assert!(
            previous_file.exists(),
            "deny-delete handle kept prior staging"
        );

        let staging = StagingArea::with_recovery_retry(data_dir.path(), Duration::from_millis(10))
            .expect("capture must still start");
        assert_eq!(
            staging
                .materialize(b"next", &metadata())
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock,
            "file paste-back must not create plaintext while stale cleanup is pending"
        );

        drop(held);
        let mut materialized = None;
        assert!(wait_until(Duration::from_secs(5), || {
            match staging.materialize(b"next", &metadata()) {
                Ok(path) => {
                    materialized = Some(path);
                    true
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
                Err(error) => panic!("staging did not recover: {error}"),
            }
        }));
        let path = materialized.expect("recovery materialized the file");
        assert!(
            !previous_file.exists(),
            "recovery did not remove the previous process's staged plaintext"
        );
        assert_eq!(std::fs::read(path).unwrap(), b"next");
        assert!(
            staging
                .worker
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|worker| !worker.is_finished()),
            "recovery must hand retained plaintext to the normal cleanup worker"
        );
    }
}
