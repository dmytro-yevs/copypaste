//! Cross-platform advisory file locks for storage-owned coordination files.

use std::fs::File;
use std::io;

pub(super) enum TryLock {
    Acquired,
    WouldBlock,
}

pub(super) fn lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(target_os = "android")]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive).map_err(Into::into)
    }
    #[cfg(not(target_os = "android"))]
    {
        file.lock()
    }
}

pub(super) fn try_lock_exclusive(file: &File) -> io::Result<TryLock> {
    #[cfg(target_os = "android")]
    {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(TryLock::Acquired),
            Err(error)
                if error == rustix::io::Errno::AGAIN || error == rustix::io::Errno::ACCESS =>
            {
                Ok(TryLock::WouldBlock)
            }
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        match file.try_lock() {
            Ok(()) => Ok(TryLock::Acquired),
            Err(std::fs::TryLockError::WouldBlock) => Ok(TryLock::WouldBlock),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
}
