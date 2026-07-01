//! Persistent state files: atomic 0600 writer, device_id persistence, and
//! the private-mode flag file.

/// Write `text` to `path` atomically with mode `0600` (Fix-2).
///
/// Thin wrapper around [`crate::fs_atomic::write_atomic_0600`] — see that
/// function for the full durability and security contract (tmp write → fsync →
/// rename → 0600 on temp before any bytes are written, parent tightened to
/// 0700; CopyPaste-54it #7 consolidation).
pub(crate) fn write_text_atomic_0600(path: &std::path::Path, text: &str) -> anyhow::Result<()> {
    crate::fs_atomic::write_atomic_0600(path, text.as_bytes())
}

/// Loads the persistent device_id from disk, creating it on first run.
///
/// Fixes arch LOW #24: previously the daemon regenerated a fresh UUID on
/// every restart, which broke P2P pairing and confused cloud peers. We now
/// persist a UUID v4 to `app_support_dir()/device_id` (or
/// `COPYPASTE_DEVICE_ID_PATH` when set) and chmod the file to `0o600` on
/// Unix so it is not world-readable.
///
/// On parse failure of an existing file we log + regenerate rather than
/// erroring — corrupt state should not block daemon startup.
#[tracing::instrument(name = "load_or_create_device_id")]
pub(crate) fn load_or_create_device_id() -> anyhow::Result<uuid::Uuid> {
    let path = crate::paths::device_id_path()?;

    if let Ok(contents) = std::fs::read_to_string(&path) {
        let trimmed = contents.trim();
        match uuid::Uuid::parse_str(trimmed) {
            Ok(id) => {
                tracing::info!(device_id = %id, "loaded persistent device_id");
                return Ok(id);
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "device_id file unparsable, regenerating"
                );
            }
        }
    }

    // Ensure parent dir exists before writing.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let id = uuid::Uuid::new_v4();
    // Fix-2 (atomic 0600 write): write via temp-then-rename so the device_id
    // is never world-readable between create and chmod.  The device_id is not
    // a secret per se, but it is used as the stable identity for pairing/sync
    // and should be owner-only for consistency with peers.json and config.json.
    write_text_atomic_0600(&path, &id.to_string())?;

    tracing::info!(device_id = %id, path = %path.display(), "created persistent device_id");
    Ok(id)
}

/// Restore the persisted private-mode flag at startup.
///
/// Returns `false` (capture enabled) when the file is absent, unreadable, or
/// holds anything other than `"1"` — a missing/corrupt flag must never leave
/// the daemon stuck in private mode, and on first run there is no file yet.
pub(crate) fn load_private_mode() -> bool {
    let path = match crate::paths::private_mode_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not resolve private_mode path ({e}); defaulting to disabled");
            return false;
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => contents.trim() == "1",
        Err(_) => false, // absent on first run; not an error
    }
}

/// Persist the private-mode flag so it survives a daemon restart.
///
/// Best-effort: a write failure is logged but does not fail the IPC call —
/// the in-memory atomic is still authoritative for the running process.
///
/// CopyPaste-ki7p: previously used `std::fs::write` which inherits the process
/// umask (typically 0022), creating the flag file world-readable at 0644.
/// The flag file is not a secret itself but its presence reveals whether the user
/// is in private/pause mode — information that should not leak to other local
/// users on a multi-user machine. We use `write_text_atomic_0600` which opens
/// the temp file with O_CREAT|mode(0600) before any bytes are written, so there
/// is never a window where the file exists at a permissive mode.
pub(crate) fn persist_private_mode(enabled: bool) {
    let path = match crate::paths::private_mode_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not resolve private_mode path ({e}); not persisting");
            return;
        }
    };
    // write_text_atomic_0600 creates the parent directory, writes atomically via
    // a temp-file rename, and sets mode 0600 before any bytes are written.
    if let Err(e) = write_text_atomic_0600(&path, if enabled { "1" } else { "0" }) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "could not persist private_mode flag"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// arch LOW #24 regression: the device_id must survive restarts.
    /// Two consecutive calls to `load_or_create_device_id` with the same
    /// backing file must return the same UUID.
    #[test]
    fn device_id_persists_across_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("device_id");

        // SAFETY: env mutation is process-global. We use a unique tmpdir path
        // so parallel tests don't collide on the value, and we restore the
        // previous value after the test.
        let prev = std::env::var_os("COPYPASTE_DEVICE_ID_PATH");
        unsafe {
            std::env::set_var("COPYPASTE_DEVICE_ID_PATH", &path);
        }

        let first = load_or_create_device_id().expect("first call must succeed");
        assert!(
            path.exists(),
            "device_id file must be written on first call"
        );

        let second = load_or_create_device_id().expect("second call must succeed");

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_DEVICE_ID_PATH", v),
                None => std::env::remove_var("COPYPASTE_DEVICE_ID_PATH"),
            }
        }

        assert_eq!(first, second, "device_id must persist across restarts");

        // On Unix the file must be 0o600.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "device_id file must be chmod 0600");
        }
    }

    /// daemon-core backlog #1 regression: private mode must survive a daemon
    /// restart. `persist_private_mode(true)` writes the flag; a fresh
    /// `load_private_mode()` (simulating the next startup) must read it back as
    /// `true`. Toggling back to `false` must also round-trip.
    #[test]
    fn private_mode_persists_across_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        // Serialise env mutation with every other env-mutating daemon test.
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored before returning.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        // First run: absent file => disabled.
        assert!(
            !load_private_mode(),
            "missing private_mode file must default to disabled"
        );

        // Enable + simulate restart: the next load must see it enabled.
        persist_private_mode(true);
        assert!(path.exists(), "persisting must create the flag file");
        let after_enable = load_private_mode();

        // Disable + reload: must round-trip back to false.
        persist_private_mode(false);
        let after_disable = load_private_mode();

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(
            after_enable,
            "enabled private mode must persist across restart"
        );
        assert!(
            !after_disable,
            "disabled private mode must persist across restart"
        );
    }

    /// CopyPaste-ki7p: `persist_private_mode` must create the flag file with
    /// mode 0600, not the umask-derived 0644. Verified on Unix only — Windows
    /// has no meaningful POSIX mode bits.
    #[cfg(unix)]
    #[test]
    fn private_mode_flag_file_is_created_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored unconditionally below.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        persist_private_mode(true);

        // Restore env before any assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(path.exists(), "flag file must be created");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "CopyPaste-ki7p: private_mode flag file must be 0600, got {mode:#o}"
        );
    }

    // -----------------------------------------------------------------------
    // PG-14 (CopyPaste-tpvi): degraded boot must not default capture to ON
    // -----------------------------------------------------------------------

    /// Regression guard for PG-14: when the user had private mode enabled
    /// before a degraded boot (e.g. Keychain locked), the degraded path must
    /// preserve `private_mode = true` (capture OFF), not silently reset it to
    /// false (capture ON).
    ///
    /// This is a pure unit test of `load_private_mode()` — the same function
    /// now called by `run_degraded`.  It simulates a persisted-ON flag being
    /// read at degraded-boot time and asserts that the value is `true`, which
    /// means the `AtomicBool` would be initialised capture-OFF.
    #[test]
    fn degraded_boot_respects_persisted_private_mode() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        // Serialise env mutation with all other env-mutating daemon tests.
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored unconditionally below.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        // Simulate: user had private mode ON at the time of the degraded boot.
        persist_private_mode(true);

        // This is what run_degraded now calls — must return true (capture OFF).
        let loaded = load_private_mode();

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(
            loaded,
            "PG-14: degraded boot with prior private_mode=ON must load true, \
             not silently reset to false (capture ON)"
        );

        // Explicitly release the first lock before acquiring it again for the
        // second scenario. `std::sync::Mutex` is NOT reentrant — holding `_guard`
        // while calling `.lock()` below would deadlock the current thread.
        drop(_guard);

        // Also verify the inverse: absent flag file (first-ever run or cleared)
        // correctly defaults to false (capture ON is the correct default for a
        // fresh install, not for a return from private mode).
        let tmp2 = tempfile::tempdir().expect("tempdir2");
        let path2 = tmp2.path().join("private_mode");

        let _guard2 = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev2 = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path2);
        }
        // path2 does not exist yet — first-run scenario.
        let loaded_absent = load_private_mode();
        unsafe {
            match prev2 {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }
        assert!(
            !loaded_absent,
            "absent private_mode file (first run) must default to false (capture ON)"
        );
    }
}
