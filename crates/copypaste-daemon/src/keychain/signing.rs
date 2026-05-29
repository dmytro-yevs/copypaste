//! Runtime detection of the daemon's own code-signature flavour.
//!
//! The choice of key-storage backend hinges on ONE question: does the running
//! binary have a STABLE code identity that the macOS Keychain ACL can pin to
//! across app updates?
//!
//! * **Developer-ID-signed** (a real `TeamIdentifier`): the designated
//!   requirement is `anchor apple generic and identifier "…" and certificate
//!   leaf[subject.OU] = "TEAMID"` — it survives rebuilds. The Keychain ACL
//!   stays valid across updates, so the Keychain is the right, prompt-free
//!   store. Prefer it.
//! * **Ad-hoc / unsigned** (`TeamIdentifier=not set`, `flags=…adhoc…`): the
//!   designated requirement is `cdhash H"…"`, which changes on every rebuild.
//!   The Keychain ACL breaks on every update and prompts. Use the file store
//!   instead (see [`super::file_store`]).
//!
//! We detect the flavour by inspecting the running executable's own signature
//! via the Security framework's `SecCode` API — no subprocess, no `codesign`
//! fork. The single signal we need is whether a Team Identifier is present,
//! which is absent for both ad-hoc and unsigned binaries and present for any
//! Developer-ID (or App Store) signature.

/// Force-override for the storage backend decision, primarily for tests and
/// for power users who want to opt into one path explicitly:
///
/// * `COPYPASTE_KEY_BACKEND=file`     → always use the 0600 file store.
/// * `COPYPASTE_KEY_BACKEND=keychain` → always use the macOS Keychain.
///
/// Any other value (or unset) falls through to runtime signature detection.
const KEY_BACKEND_ENV: &str = "COPYPASTE_KEY_BACKEND";

/// Which key-storage backend `keychain::load_or_create` should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBackend {
    /// macOS login Keychain (prompt-free only under a stable Developer-ID
    /// signature whose ACL survives updates).
    Keychain,
    /// `0600` file under the app data dir (prompt-free under ad-hoc/unsigned).
    File,
}

/// Decide the key-storage backend for the current process.
///
/// Order of precedence:
/// 1. explicit `COPYPASTE_KEY_BACKEND` override,
/// 2. runtime signature detection: Developer-ID → Keychain, otherwise File.
///
/// On a detection error we fail SAFE toward the file store: a false negative
/// (using the file on a Developer-ID build) is merely a slightly weaker — but
/// still prompt-free — store, whereas a false positive (using the Keychain on
/// an ad-hoc build) reintroduces the recurring password prompt this whole
/// change exists to kill.
#[cfg(target_os = "macos")]
pub fn choose_key_backend() -> KeyBackend {
    match std::env::var(KEY_BACKEND_ENV).ok().as_deref() {
        Some("file") => return KeyBackend::File,
        Some("keychain") => return KeyBackend::Keychain,
        _ => {}
    }

    match self_has_stable_team_identifier() {
        Ok(true) => {
            tracing::debug!(
                "code signature has a Team Identifier (Developer-ID); using Keychain key store"
            );
            KeyBackend::Keychain
        }
        Ok(false) => {
            tracing::debug!(
                "code signature is ad-hoc/unsigned (no Team Identifier); using file key store \
                 to avoid the recurring Keychain password prompt on every update"
            );
            KeyBackend::File
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not determine code-signature flavour; defaulting to file key store \
                 (fail-safe: avoids the recurring Keychain prompt)"
            );
            KeyBackend::File
        }
    }
}

/// Inspect THIS process's code signature and report whether it carries a Team
/// Identifier. Returns `Ok(true)` only for a Developer-ID / App-Store style
/// signature; `Ok(false)` for ad-hoc or unsigned binaries.
#[cfg(target_os = "macos")]
fn self_has_stable_team_identifier() -> Result<bool, super::KeychainError> {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    // Symbols + keys live in security-framework-sys; bind the few we need.
    extern "C" {
        fn SecCodeCopySelf(flags: u32, self_ref: *mut core_foundation_sys::base::CFTypeRef) -> i32;
        fn SecCodeCopySigningInformation(
            code: core_foundation_sys::base::CFTypeRef,
            flags: u64,
            information: *mut core_foundation_sys::dictionary::CFDictionaryRef,
        ) -> i32;
    }

    // `kSecCSSigningInformation` (= 2) asks for the signing-cert / identifier
    // details, which is where the team identifier lives.
    const K_SEC_CS_SIGNING_INFORMATION: u64 = 2;

    let mut code: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let st = unsafe { SecCodeCopySelf(0, &mut code) };
    if st != 0 || code.is_null() {
        return Err(super::KeychainError::OsStatus {
            op: "SecCodeCopySelf",
            code: st,
        });
    }
    // Take ownership so it is released on every exit path.
    let code_guard: core_foundation::base::CFType =
        unsafe { core_foundation::base::CFType::wrap_under_create_rule(code) };

    let mut info: core_foundation_sys::dictionary::CFDictionaryRef = std::ptr::null();
    let st = unsafe {
        SecCodeCopySigningInformation(
            code_guard.as_concrete_TypeRef(),
            K_SEC_CS_SIGNING_INFORMATION,
            &mut info,
        )
    };
    if st != 0 || info.is_null() {
        return Err(super::KeychainError::OsStatus {
            op: "SecCodeCopySigningInformation",
            code: st,
        });
    }
    let info_dict: CFDictionary = unsafe { CFDictionary::wrap_under_create_rule(info) };

    // `kSecCodeInfoTeamIdentifier` == the CFString key "teamid". Present iff a
    // Team Identifier was embedded (Developer-ID / App-Store signatures only).
    let team_key = CFString::from_static_string("teamid");
    let has_team = info_dict
        .find(team_key.as_CFTypeRef() as *const _)
        .is_some();
    Ok(has_team)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialise env mutation with every other env-touching daemon test.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    struct BackendEnv {
        original: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl BackendEnv {
        fn set(value: &str) -> Self {
            let guard = env_lock();
            let original = std::env::var_os(KEY_BACKEND_ENV);
            // SAFETY: serialised via TEST_ENV_LOCK.
            unsafe { std::env::set_var(KEY_BACKEND_ENV, value) };
            Self {
                original,
                _guard: guard,
            }
        }
    }

    impl Drop for BackendEnv {
        fn drop(&mut self) {
            // SAFETY: restoring under TEST_ENV_LOCK.
            unsafe {
                match self.original.take() {
                    Some(v) => std::env::set_var(KEY_BACKEND_ENV, v),
                    None => std::env::remove_var(KEY_BACKEND_ENV),
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn env_override_file_forces_file_backend() {
        let _e = BackendEnv::set("file");
        assert_eq!(choose_key_backend(), KeyBackend::File);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn env_override_keychain_forces_keychain_backend() {
        let _e = BackendEnv::set("keychain");
        assert_eq!(choose_key_backend(), KeyBackend::Keychain);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unrecognised_override_falls_through_to_detection() {
        // The cargo-test binary is itself ad-hoc/unsigned, so detection must
        // resolve to the File backend — the prompt-free default.
        let _e = BackendEnv::set("bogus-value");
        assert_eq!(choose_key_backend(), KeyBackend::File);
    }
}
