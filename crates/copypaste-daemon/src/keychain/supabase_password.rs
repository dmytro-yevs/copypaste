use super::{keychain_bypassed, KeychainError};

// `SERVICE`/`SUPABASE_PASSWORD_ACCOUNT` are only referenced from the macOS
// Keychain code paths below; gate the imports so a non-macOS build (which
// only compiles the in-memory fallback) does not trip an unused-import
// warning under `-D warnings`.
#[cfg(target_os = "macos")]
use super::{SERVICE, SUPABASE_PASSWORD_ACCOUNT};

#[cfg(target_os = "macos")]
use security_framework::passwords::{delete_generic_password, get_generic_password};

// ── Non-macOS in-memory Supabase password slot (CopyPaste-crh3.104) ─────────
//
// On non-macOS there is no system Keychain. `store_supabase_password_to_keychain`
// returns `Err(Unsupported)` so the IPC caller correctly reports `persisted: false`
// (the password will be lost on daemon restart). However the password MUST be
// readable by `CloudConfig::from_env` (via `read_supabase_password_from_keychain`)
// within the same daemon session so that `cloud_sign_in` can authenticate.
//
// The process-global `OnceLock<Mutex<Option<Zeroizing<String>>>>` is the bridge:
// `store_supabase_password_to_keychain` writes here as a side effect (before
// returning the error), and `read_supabase_password_from_keychain` reads from
// here on non-macOS. `delete_supabase_password_from_keychain` clears the slot
// so `cloud_sign_out` leaves no lingering credential in memory.
#[cfg(not(target_os = "macos"))]
static IN_MEMORY_SUPABASE_PASSWORD: std::sync::OnceLock<
    std::sync::Mutex<Option<zeroize::Zeroizing<String>>>,
> = std::sync::OnceLock::new();

/// Read the Supabase GoTrue password from the macOS Keychain.
///
/// Returns `Some(password)` if a non-empty entry is present.
/// Returns `None` when the entry is absent (first run / pre-migration) or
/// when the Keychain is unavailable (non-macOS, ephemeral-key env, locked).
/// Callers should fall back to `config.json` on `None`.
pub fn read_supabase_password_from_keychain() -> Option<String> {
    // Dev/test bypass: never read the real Keychain in ephemeral mode.
    if keychain_bypassed() {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        match get_generic_password(SERVICE, SUPABASE_PASSWORD_ACCOUNT) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).ok()?;
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            // Any error (not-found, locked, denied) → treat as absent; caller
            // falls back to config.json for the migration path.
            Err(_) => None,
        }
    }
    // Non-macOS: read from the in-memory slot populated by
    // `store_supabase_password_to_keychain`. This is the fourth lookup in
    // `CloudConfig::from_env` (env → Keychain → config.json → here) and the
    // only path that makes `cloud_sign_in` work on Linux within the same
    // daemon session.
    #[cfg(not(target_os = "macos"))]
    {
        let slot = IN_MEMORY_SUPABASE_PASSWORD.get_or_init(|| std::sync::Mutex::new(None));
        // Clone out of the guard so the lock is released before returning.
        slot.lock()
            .ok()
            // crh3.80: the slot holds `Zeroizing<String>`, so `as_deref` yields
            // `Option<&String>` (not `&str`). `str::to_owned` expects `&str`
            // (E0631 on Linux), so clone explicitly via `ToString`.
            .and_then(|g| g.as_deref().map(|s| s.to_string()))
    }
}

/// Store the Supabase GoTrue password in the macOS Keychain.
///
/// Silently succeeds on non-macOS and in ephemeral-key mode so call sites
/// do not need to be conditional. On macOS a failure is logged at warn
/// level and bubbled to the caller as `Err` so the caller can decide
/// whether to fall back to config.json persistence.
pub fn store_supabase_password_to_keychain(password: &str) -> Result<(), KeychainError> {
    if keychain_bypassed() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        // CopyPaste-nkro: use the locked-down write path so the Supabase
        // password is stored with kSecAttrSynchronizable=false +
        // ThisDeviceOnly accessibility and never syncs to iCloud Keychain.
        super::secure_write::set_generic_password_locked_down(
            SERVICE,
            SUPABASE_PASSWORD_ACCOUNT,
            password.as_bytes(),
        )
    }
    // Non-macOS: no Keychain is available. Write the password to the
    // in-memory global so `read_supabase_password_from_keychain` can find
    // it within the same daemon session, then return Err(Unsupported) so the
    // IPC caller correctly reports `persisted: false` (the password is
    // session-scoped and will be lost on daemon restart).
    //
    // Security: the password is wrapped in Zeroizing so the heap buffer is
    // scrubbed when the slot is overwritten or the process exits. It is never
    // logged — the caller must ensure it is not present in error payloads.
    #[cfg(not(target_os = "macos"))]
    {
        let slot = IN_MEMORY_SUPABASE_PASSWORD.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = Some(zeroize::Zeroizing::new(password.to_owned()));
        }
        Err(KeychainError::Unsupported)
    }
}

/// Delete the Supabase GoTrue password from the macOS Keychain (CopyPaste-crh3.100).
///
/// Used by `cloud_sign_out` so the credential is not re-resolved by
/// `CloudConfig::from_env` on the next daemon start. A missing entry is treated
/// as success (idempotent sign-out). No-op (Ok) on non-macOS and in
/// ephemeral-key mode so callers need no platform branch.
pub fn delete_supabase_password_from_keychain() -> Result<(), KeychainError> {
    if keychain_bypassed() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        match delete_generic_password(SERVICE, SUPABASE_PASSWORD_ACCOUNT) {
            Ok(()) => Ok(()),
            // A not-found entry means there was nothing to sign out of — treat
            // as success so sign-out is idempotent.
            Err(e) if e.code() == super::device_key::ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(e) => Err(KeychainError::from(e)),
        }
    }
    // Non-macOS: clear the in-memory slot so `cloud_sign_out` removes the
    // credential from memory. Idempotent — clearing an already-None slot
    // is a no-op.
    #[cfg(not(target_os = "macos"))]
    {
        let slot = IN_MEMORY_SUPABASE_PASSWORD.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CopyPaste-crh3.100: `delete_supabase_password_from_keychain` must be
    /// idempotent — a sign-out when the entry is already absent (or in
    /// ephemeral-key bypass) returns `Ok` so `cloud_sign_out` never fails just
    /// because there was nothing to delete.
    #[test]
    fn delete_supabase_password_is_idempotent_in_bypass() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("COPYPASTE_EPHEMERAL_KEY");
        // SAFETY: single-threaded under the TEST_ENV_LOCK guard.
        unsafe { std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1") };
        let result = delete_supabase_password_from_keychain();
        // Restore the original env before asserting so it is always cleaned up.
        match prev {
            Some(v) => unsafe { std::env::set_var("COPYPASTE_EPHEMERAL_KEY", v) },
            None => unsafe { std::env::remove_var("COPYPASTE_EPHEMERAL_KEY") },
        }
        assert!(
            result.is_ok(),
            "delete must be a no-op Ok in bypass mode: {result:?}"
        );
    }

    /// CopyPaste-crh3.104: on non-macOS, `store_supabase_password_to_keychain`
    /// must write the password to the in-memory global so that
    /// `read_supabase_password_from_keychain` returns it within the same daemon
    /// session (enabling `cloud_sign_in` on Linux). `delete_supabase_password_from_keychain`
    /// must clear it (mirroring the macOS Keychain delete path).
    ///
    /// We hold the TEST_ENV_LOCK to serialise with all other tests that mutate
    /// the keychain bypass env var or the global password slot.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn store_then_read_in_memory_password_round_trips_on_non_macos() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Ensure the bypass is OFF so the non-macOS real path runs (not the
        // bypass that returns Ok(()) before touching the global).
        let prev = std::env::var_os("COPYPASTE_EPHEMERAL_KEY");
        // SAFETY: single-threaded under TEST_ENV_LOCK.
        unsafe { std::env::remove_var("COPYPASTE_EPHEMERAL_KEY") };

        // Use a unique password to avoid contamination from parallel test runs
        // on the same process (tests are serialised by TEST_ENV_LOCK, but we
        // use a distinctive value for clarity).
        let pw = "crh3-104-unit-test-password-xyz";

        // store_supabase_password_to_keychain writes to the global AND returns Err.
        let store_result = store_supabase_password_to_keychain(pw);
        assert!(
            matches!(store_result, Err(KeychainError::Unsupported)),
            "non-macOS must return Unsupported so caller reports persisted:false; got {store_result:?}"
        );

        // read_supabase_password_from_keychain must now find the in-memory value.
        let got = read_supabase_password_from_keychain();
        assert_eq!(
            got.as_deref(),
            Some(pw),
            "read after store must return the stored password"
        );

        // delete must clear the slot (mirrors cloud_sign_out behaviour).
        delete_supabase_password_from_keychain().unwrap();
        assert_eq!(
            read_supabase_password_from_keychain(),
            None,
            "read after delete must return None"
        );

        // Restore original env.
        match prev {
            Some(v) => unsafe { std::env::set_var("COPYPASTE_EPHEMERAL_KEY", v) },
            None => unsafe { std::env::remove_var("COPYPASTE_EPHEMERAL_KEY") },
        }
    }

    /// CopyPaste-nkro: on non-macOS, `store_supabase_password_to_keychain` must
    /// return `Err(KeychainError::Unsupported)` — the locked-down path is a
    /// macOS-only security hardening, not a cross-platform behaviour change.
    ///
    /// We explicitly unset `COPYPASTE_EPHEMERAL_KEY` so the keychain-bypass
    /// short-circuit does not fire (it returns `Ok(())`, masking the
    /// `Unsupported` path we are testing). The `TEST_ENV_LOCK` serialises all
    /// tests that mutate the process environment so they cannot race.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn store_supabase_password_to_keychain_returns_unsupported_on_non_macos() {
        // Hold the env lock for the full test body so no other test can concurrently
        // set COPYPASTE_EPHEMERAL_KEY while we have it cleared.
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Ensure the bypass is off so we exercise the real non-macOS path.
        let prev = std::env::var_os("COPYPASTE_EPHEMERAL_KEY");
        // SAFETY: single-threaded under the TEST_ENV_LOCK guard.
        unsafe { std::env::remove_var("COPYPASTE_EPHEMERAL_KEY") };
        let result = store_supabase_password_to_keychain("test-password");
        // Restore original value (if any) before any assert so the env is always
        // cleaned up even on panic.
        if let Some(v) = prev {
            unsafe { std::env::set_var("COPYPASTE_EPHEMERAL_KEY", v) };
        }
        assert!(
            matches!(result, Err(KeychainError::Unsupported)),
            "expected Unsupported on non-macOS, got {result:?}"
        );
    }
}
