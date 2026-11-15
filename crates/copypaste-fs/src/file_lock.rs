//! Cross-platform advisory file locks for storage-owned coordination files.

use std::fs::File;
use std::io;

/// Outcome of a nonblocking exclusive lease attempt.
#[derive(Debug, Eq, PartialEq)]
pub enum TryLock {
    Acquired,
    WouldBlock,
}

/// Wait for an exclusive lease held until unlock or the file is closed.
pub fn lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(target_os = "android")]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive).map_err(Into::into)
    }
    #[cfg(not(target_os = "android"))]
    {
        file.lock()
    }
}

/// Attempt an exclusive lease without waiting for another owner.
pub fn try_lock_exclusive(file: &File) -> io::Result<TryLock> {
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

/// Wait for a shared lifetime lease, including on Android's flock backend.
pub fn lock_shared(file: &File) -> io::Result<()> {
    #[cfg(target_os = "android")]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::LockShared).map_err(Into::into)
    }
    #[cfg(not(target_os = "android"))]
    {
        file.lock_shared()
    }
}

/// Release this file's lease before changing ownership mode.
pub fn unlock(file: &File) -> io::Result<()> {
    #[cfg(target_os = "android")]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::Unlock).map_err(Into::into)
    }
    #[cfg(not(target_os = "android"))]
    {
        file.unlock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_lease_blocks_a_second_owner_until_explicit_unlock() {
        let owner = tempfile::NamedTempFile::new().unwrap();
        let second = File::options()
            .read(true)
            .write(true)
            .open(owner.path())
            .unwrap();
        lock_exclusive(owner.as_file()).unwrap();
        assert_eq!(try_lock_exclusive(&second).unwrap(), TryLock::WouldBlock);
        unlock(owner.as_file()).unwrap();
        assert_eq!(try_lock_exclusive(&second).unwrap(), TryLock::Acquired);
    }

    #[test]
    fn shared_leases_block_cleanup_until_every_owner_closes() {
        let owner = tempfile::NamedTempFile::new().unwrap();
        let second = File::options()
            .read(true)
            .write(true)
            .open(owner.path())
            .unwrap();
        let cleanup = File::options()
            .read(true)
            .write(true)
            .open(owner.path())
            .unwrap();
        lock_shared(owner.as_file()).unwrap();
        lock_shared(&second).unwrap();
        assert_eq!(try_lock_exclusive(&cleanup).unwrap(), TryLock::WouldBlock);
        unlock(owner.as_file()).unwrap();
        assert_eq!(try_lock_exclusive(&cleanup).unwrap(), TryLock::WouldBlock);
        drop(second);
        assert_eq!(try_lock_exclusive(&cleanup).unwrap(), TryLock::Acquired);
    }
}
