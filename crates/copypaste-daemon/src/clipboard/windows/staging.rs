//! Owner-only, short-lived plaintext files for Windows file paste-back.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct StagingArea {
    root: PathBuf,
    entries: Arc<Mutex<Vec<Entry>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

struct Entry {
    _directory: tempfile::TempDir,
    expires: Instant,
}

impl StagingArea {
    pub(super) fn new(data_dir: &Path) -> io::Result<Self> {
        let root = std::path::absolute(data_dir)?.join("paste-files");
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        protect_owner_only(&root)?;
        let entries = Arc::new(Mutex::new(Vec::<Entry>::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_entries = Arc::clone(&entries);
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("copypaste-file-sweeper".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    std::thread::sleep(SWEEP_INTERVAL);
                    prune(&worker_entries);
                }
            })?;
        Ok(Self {
            root,
            entries,
            stop,
            worker: Some(worker),
        })
    }

    pub(super) fn materialize(&self, bytes: &[u8], metadata: &FileMetadata) -> io::Result<PathBuf> {
        if !metadata.is_valid() || metadata.filename.contains(['/', '\\', ':']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid file metadata",
            ));
        }
        let filename = metadata.filename.as_str();
        let mut entries = self.entries.lock().unwrap_or_else(|held| held.into_inner());
        entries.retain(|entry| entry.expires > Instant::now());
        let directory = tempfile::Builder::new()
            .prefix("file-")
            .tempdir_in(&self.root)?;
        let path = directory.path().join(filename);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(bytes)?;
        entries.push(Entry {
            _directory: directory,
            expires: Instant::now() + MAX_AGE,
        });
        Ok(path)
    }
}

impl Drop for StagingArea {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.entries
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clear();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn prune(entries: &Mutex<Vec<Entry>>) {
    entries
        .lock()
        .unwrap_or_else(|held| held.into_inner())
        .retain(|entry| entry.expires > Instant::now());
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
