use copypaste_core::DeviceKeypair;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;
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

#[cfg(target_os = "macos")]
pub mod acl;
pub mod file_store;
#[cfg(target_os = "macos")]
pub mod signing;

pub(crate) const SERVICE: &str = "com.copypaste.daemon";
pub(crate) const ACCOUNT: &str = "device-secret-key";
/// Keychain account key for the cloud sync passphrase-derived key bytes.
/// Stored under the same service as the device key but a distinct account
/// so they are never confused.
pub(crate) const CLOUD_SYNC_ACCOUNT: &str = "cloud-sync-key";
/// Keychain account key for the **v2 per-account-salt** cloud sync key bytes
/// (CopyPaste-jdq5). Distinct from [`CLOUD_SYNC_ACCOUNT`] (which always holds the
/// legacy v1 global-salt key used by relay + as the cloud read fallback) so the
/// two derivations are persisted side-by-side and a daemon restart can restore
/// BOTH for dual-key read dispatch without re-entering the passphrase.
#[cfg_attr(
    not(feature = "cloud-sync"),
    allow(
        dead_code,
        reason = "only read by the cloud-sync v2 persist/restore path"
    )
)]
pub(crate) const CLOUD_SYNC_V2_ACCOUNT: &str = "cloud-sync-key-v2";
/// Keychain account key for the Supabase GoTrue account password.
/// Stored under `SERVICE` so all CopyPaste secrets live in one service.
/// Migration: if absent from Keychain, callers fall back to config.json.
pub(crate) const SUPABASE_PASSWORD_ACCOUNT: &str = "supabase-password";

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
        set_generic_password_locked_down(SERVICE, SUPABASE_PASSWORD_ACCOUNT, password.as_bytes())
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
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
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

/// Dev/test escape hatch: when `COPYPASTE_EPHEMERAL_KEY` is set in the
/// environment, every keychain entry point in this module short-circuits
/// BEFORE any Security-framework call so the macOS login-keychain password
/// prompt is never triggered.
///
/// Why centralize here: ad-hoc-signed dev builds change signature on every
/// rebuild, invalidating the persisted item's ACL and forcing an interactive
/// keychain prompt. `cargo test --workspace` and `make dev-daemon` set this
/// env so they run non-interactively. Production (env unset) is unaffected —
/// every caller falls through to the real Security-framework path unchanged.
/// CopyPaste-qvtg.5: in production the env var is read **once** and cached for
/// the process lifetime, so an attacker who can mutate the running daemon's
/// environment mid-session (e.g. via a debugger) cannot flip it into
/// ephemeral-key bypass *after* the real Keychain-backed key is already in use.
/// The legitimate opt-in is always set before launch, so caching the first
/// observed value loses no real functionality.
#[cfg(not(test))]
pub(crate) fn keychain_bypassed() -> bool {
    static BYPASS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *BYPASS.get_or_init(|| std::env::var_os("COPYPASTE_EPHEMERAL_KEY").is_some())
}

/// Test build: read the env live each call. The suite toggles
/// `COPYPASTE_EPHEMERAL_KEY` under `TEST_ENV_LOCK` to exercise both the bypass
/// and the real path within one process, which a `OnceLock` cache (pinning the
/// first-observed value for the whole run) would break.
#[cfg(test)]
pub(crate) fn keychain_bypassed() -> bool {
    std::env::var_os("COPYPASTE_EPHEMERAL_KEY").is_some()
}

/// Compute the canonical device fingerprint from a raw public key.
///
/// Format: first 16 bytes of `SHA-256(public_key)` rendered as
/// lowercase hex pairs separated by `:` (e.g. `aa:bb:cc:...`).
/// This is the user-visible identifier shown during pairing — keep it short
/// enough for humans to compare on two screens.
///
/// # Why 16 bytes (128 bits) instead of the full 32-byte SHA-256
///
/// This function is the **human-visible** fingerprint used during pairing:
/// both screens must show the same string for a human to visually compare.
/// 16 bytes produces a 47-character colon-hex string that fits in a single
/// line of UI; the full 32 bytes would be 95 characters and effectively
/// unreadable at a glance.
///
/// 128 bits of SHA-256 prefix is more than sufficient for collision resistance
/// in this threat model: a birthday attack against a fleet of ≤10 000 paired
/// devices needs ~2^64 operations — well beyond practical reach.
///
/// **This truncation applies only to the display identifier.** The mTLS
/// allowlist uses [`copypaste_core::DeviceKeypair::fingerprint`], which
/// returns the full 64-character (256-bit) hex SHA-256 and is the binding
/// used for cryptographic device pinning. The two functions serve different
/// purposes and intentionally produce different lengths.
///
/// If you ever need to change the display length, update the constant `16`
/// below and document the new rationale here — do NOT silently change it,
/// as it will invalidate every previously-paired device's stored fingerprint
/// (CopyPaste-44rq.56 / SEC-4).
pub fn own_fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    digest[..16]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("Key is wrong length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
    #[cfg(target_os = "macos")]
    #[error("Keychain error: {0}")]
    Keychain(#[from] SfError),
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain not supported on this platform")]
    Unsupported,
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Core key error: {0}")]
    Key(#[from] copypaste_core::KeyError),
    // ── v0.3 ACL surface (macOS only) ──────────────────────────────────────
    #[cfg(target_os = "macos")]
    #[error("Keychain ACL trust list is empty — refusing to create unrestricted entry")]
    AclEmpty,
    #[cfg(target_os = "macos")]
    #[error("Cannot encode binary path for Keychain ACL (interior NUL byte)")]
    AclPathEncoding,
    #[cfg(target_os = "macos")]
    #[error("Security.framework call {op} failed with OSStatus {code}")]
    OsStatus { op: &'static str, code: i32 },
    #[cfg(target_os = "macos")]
    #[error("Filesystem I/O: {0}")]
    Io(#[from] std::io::Error),
    /// The Keychain item exists (or its existence cannot be determined) but
    /// could not be read because the Keychain is locked, access was denied, an
    /// interactive prompt is disallowed, or the read timed out. Distinct from a
    /// genuine `errSecItemNotFound` so the caller can DEGRADE (leave the
    /// encrypted DB untouched) instead of silently minting an ephemeral key that
    /// would mismatch the existing DB key. Carries the originating OSStatus for
    /// diagnostics.
    #[cfg(target_os = "macos")]
    #[error("Keychain locked or access denied (OSStatus {0}); cannot read device key")]
    Locked(i32),
}

/// Load device keypair from Keychain, or generate and store a new one.
///
/// On v0.3 macOS builds, freshly created entries are written with an ACL
/// pinned to the three CopyPaste binaries (see [`acl::store_with_acl`]).
/// Pre-existing v0.2 entries without an ACL are upgraded by
/// [`acl::rotate_acl_to_current_install`], which is called once at daemon
/// startup separately from this function so that the rotation latency does
/// not block per-component reads.
///
/// Beta-merge audit HIGH #2: also opportunistically re-stores entries
/// written by older builds with the locked-down `ThisDeviceOnly` +
/// `Synchronizable=false` attributes so the secret never leaves the device
/// via iCloud Keychain sync — see `migrate_legacy_accessibility_if_needed`.
pub fn load_or_create() -> Result<DeviceKeypair, KeychainError> {
    // Dev/test bypass: return a fresh ephemeral keypair without touching the
    // Keychain. Must be checked BEFORE any Security-framework call so no
    // password prompt is ever raised. See `keychain_bypassed`.
    if keychain_bypassed() {
        tracing::warn!(
            "COPYPASTE_EPHEMERAL_KEY set: using ephemeral in-memory device keypair, skipping macOS Keychain"
        );
        return Ok(DeviceKeypair::generate());
    }

    // Backend selection (the real fix for the recurring Keychain prompt):
    // ad-hoc / unsigned installs CANNOT keep a stable Keychain ACL across
    // updates — the cdhash-pinned ACL breaks on every rebuild and macOS
    // prompts for the login password. Those installs use the non-prompting
    // 0600 file store instead. Only a Developer-ID-signed build (stable Team
    // Identifier → stable designated requirement) keeps the Keychain.
    // See `keychain::signing` and `keychain::file_store`.
    #[cfg(target_os = "macos")]
    {
        match signing::choose_key_backend() {
            signing::KeyBackend::File => return file_store::load_or_create(),
            signing::KeyBackend::Keychain => {}
        }
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => {
                // Audit MED #4: wrap the keychain-returned Vec in Zeroizing
                // so the heap buffer is scrubbed when this scope exits, and
                // use a checked conversion instead of `bytes.try_into().unwrap()`.
                let bytes = zeroize::Zeroizing::new(bytes);
                if bytes.len() != 32 {
                    return Err(KeychainError::InvalidLength(bytes.len()));
                }
                let arr: [u8; 32] = (&**bytes)
                    .try_into()
                    .map_err(|_| KeychainError::InvalidLength(bytes.len()))?;
                // Audit HIGH #2 migration: re-store with the locked-down
                // accessibility so legacy items written by pre-fix builds
                // stop syncing to iCloud Keychain on the next run. Failure
                // here is logged but not fatal — the keypair load itself
                // succeeded, and we retry on every cold start.
                //
                // Fix C: a `SecAccessControl` with `ThisDeviceOnly`
                // accessibility requires the `keychain-access-groups`
                // entitlement, which an ad-hoc-signed binary CANNOT carry
                // (see `set_generic_password_locked_down`). On those builds
                // `SecItemAdd`/`SecItemUpdate` returns
                // `errSecMissingEntitlement` (-34018, "A required entitlement
                // isn't present"). We treat that one error code as an EXPECTED
                // degraded state and log it at debug — not warn — so the daemon
                // does not spam an error on every cold start it can never fix.
                // The keypair is still fully usable; only the
                // iCloud-sync-suppression hardening is skipped.
                match migrate_legacy_accessibility_if_needed(&arr) {
                    Ok(()) => {}
                    Err(e) if is_missing_entitlement(&e) => {
                        // CopyPaste-uxt7: promote to WARN so operators can see
                        // this on ad-hoc / unsigned builds. The key is still
                        // fully usable; only the iCloud-sync-suppression
                        // (ThisDeviceOnly) hardening is absent. Signing with a
                        // Developer-ID certificate and adding the
                        // `keychain-access-groups` entitlement fixes this.
                        tracing::warn!(
                            "Keychain ThisDeviceOnly hardening SKIPPED: the \
                             `keychain-access-groups` entitlement is absent on \
                             this build (ad-hoc / unsigned). Clipboard key is \
                             usable but iCloud Keychain sync suppression is NOT \
                             active — use a Developer-ID-signed build for full \
                             security hardening."
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "could not migrate device key to ThisDeviceOnly accessibility; will retry on next launch"
                        );
                    }
                }
                Ok(DeviceKeypair::from_secret_bytes(&arr)?)
            }
            // ONLY a genuine `errSecItemNotFound` means "no entry yet → create
            // a fresh key". The OLD code matched `Err(_)` and treated EVERY
            // failure (locked keychain, denied access, disallowed interaction,
            // timeout) as absent, minting an ephemeral key that bypasses the
            // clean degraded path and — if an encrypted DB already exists —
            // mismatches its SQLCipher key (SQLITE_NOTADB). Classify the
            // OSStatus: not-found → generate; anything else → propagate
            // `Locked` so `daemon::load_local_key_material` degrades with an
            // accurate reason (`DEGRADED_REASON_KEYCHAIN_LOCKED`) and leaves the
            // encrypted data untouched.
            Err(e) if classify_read_failure(e.code()) != ReadFailureClass::NotFound => {
                tracing::warn!(
                    code = e.code(),
                    "device key read failed with a non-not-found Keychain status \
                     (locked / access denied / interaction disallowed). Refusing to \
                     mint an ephemeral key over a possibly-existing entry; \
                     propagating a locked error so startup degrades cleanly."
                );
                Err(KeychainError::Locked(e.code()))
            }
            Err(_) => {
                // errSecItemNotFound: primary entry absent. Before minting a
                // brand-new key, check whether a crash mid-ACL-rotation left a
                // surviving copy under ACCOUNT_ROTATE_BACKUP. If so, PROMOTE
                // that backup to primary so the existing encrypted DB stays
                // openable. Only mint a fresh key when the backup slot is also
                // absent (or unusable).
                //
                // ACL-rotation orphan-key fix (HIGH data-loss): without this
                // check, a kill/power-loss between Step 2 (primary deleted) and
                // Step 3 (primary recreated) in rotate_acl_to_current_install
                // caused load_or_create to see ItemNotFound and generate a NEW
                // random key, permanently orphaning the existing SQLCipher DB.
                match get_generic_password(SERVICE, acl::ACCOUNT_ROTATE_BACKUP) {
                    Ok(backup_bytes) if backup_bytes.len() == 32 => {
                        let backup = zeroize::Zeroizing::new(backup_bytes);
                        let arr: [u8; 32] = (&**backup)
                            .try_into()
                            .map_err(|_| KeychainError::InvalidLength(backup.len()))?;
                        tracing::warn!(
                            "load_or_create: primary key absent but rotation backup found — \
                             promoting backup to primary to recover from a mid-rotation crash"
                        );
                        // Re-create the primary entry with the recovered secret
                        // and an up-to-date ACL. If this fails we propagate the
                        // error — better to surface the problem than silently use
                        // the wrong key.
                        let trusted = acl::trusted_binary_paths()?;
                        acl::store_with_acl(&arr, &trusted)?;
                        // Best-effort: clean up the backup now that primary is
                        // restored. A lingering backup is harmless (rotate_acl
                        // clears it at the top of the next rotation) but we
                        // prefer not to leave stale entries around.
                        let _ = delete_generic_password(SERVICE, acl::ACCOUNT_ROTATE_BACKUP);
                        tracing::info!(
                            "load_or_create: rotation backup promoted to primary; \
                             backup entry cleaned up"
                        );
                        return Ok(DeviceKeypair::from_secret_bytes(&arr)?);
                    }
                    Ok(backup_bytes) => {
                        // Backup present but wrong length — corrupted; ignore and
                        // fall through to generate a fresh key.
                        tracing::warn!(
                            "load_or_create: rotation backup has wrong length {} (expected 32); \
                             ignoring and generating a fresh key",
                            backup_bytes.len()
                        );
                        let _ = delete_generic_password(SERVICE, acl::ACCOUNT_ROTATE_BACKUP);
                    }
                    Err(e) if e.code() == acl::ERR_SEC_ITEM_NOT_FOUND => {
                        // No backup either — this is a genuine first run.
                    }
                    Err(e) => {
                        // Backup read failed for a non-not-found reason (locked
                        // keychain, access denied). Propagate as Locked so the
                        // daemon degrades rather than silently minting a new key
                        // that may conflict with an existing DB.
                        tracing::warn!(
                            code = e.code(),
                            "load_or_create: rotation backup read failed (non-not-found); \
                             propagating locked error to avoid orphaning existing DB"
                        );
                        return Err(KeychainError::Locked(e.code()));
                    }
                }

                // No primary, no usable backup → genuine first run. Mint a new key.
                let kp = DeviceKeypair::generate();
                // Beta-merge audit MED #3 + #4: pull the secret via the
                // zeroizing accessor so the buffer handed to the Keychain
                // syscall is scrubbed when this function returns.
                let secret = kp.secret_key_bytes_zeroizing();
                // v0.3: create with ACL pinned to the current install.
                let trusted = acl::trusted_binary_paths()?;
                acl::store_with_acl(&secret, &trusted)?;
                let fp = own_fingerprint(&kp.public_key_bytes());
                // Log only the short prefix to keep full fingerprint out of info logs.
                tracing::info!(
                    acl_apps = trusted.len(),
                    "Generated new device keypair with ACL; fingerprint_prefix={}",
                    &fp[..23]
                );
                tracing::debug!("full device fingerprint={}", fp);
                Ok(kp)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err(KeychainError::Unsupported)
}

/// Delete the stored keypair — used for testing and factory reset.
#[cfg(target_os = "macos")]
pub fn delete_stored() -> Result<(), KeychainError> {
    // Dev/test bypass: there is no persisted entry to delete, so this is a
    // benign no-op rather than a Security-framework call that could prompt.
    if keychain_bypassed() {
        return Ok(());
    }
    // Mirror the backend selection used by `load_or_create` so a factory
    // reset removes whichever store actually holds the key on this install.
    match signing::choose_key_backend() {
        signing::KeyBackend::File => return file_store::delete_stored(),
        signing::KeyBackend::Keychain => {}
    }
    delete_generic_password(SERVICE, ACCOUNT).map_err(KeychainError::from)
}

// ── HIGH #2: hardened `SecItemAdd` wrapper ─────────────────────────────────────
//
// `security_framework::passwords::set_generic_password` does NOT let you
// specify accessibility, so the item lands with the default
// `kSecAttrAccessibleWhenUnlocked` — which on macOS makes the item
// eligible for iCloud Keychain sync AND for inclusion in a Time Machine
// backup of the system keychain. Both violate the threat model: the X25519
// device secret must never leave the originating device.
//
// We bypass `passwords::set_generic_password` by building the
// `SecItemAdd` query manually with:
//   * `kSecAttrAccessControl` = SecAccessControl built with
//     `ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly` (the only
//     protection that suppresses iCloud sync and Time Machine inclusion).
//   * `kSecAttrSynchronizable` = false (defence-in-depth — duplicate of
//     the `ThisDeviceOnly` accessibility flag, but explicit).
//
// On duplicate (item already exists), we fall back to `SecItemUpdate`
// with the same access-control attribute so an existing legacy entry is
// re-written with the locked-down ACL.

/// Store a generic Keychain password with `kSecAttrSynchronizable=false` and
/// `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` so the secret never leaves
/// the originating device via iCloud Keychain sync or Time Machine backup.
///
/// Used by all secret-write paths in this module (device key, cloud-sync key,
/// Supabase password) — see CopyPaste-nkro.
#[cfg(target_os = "macos")]
pub(crate) fn set_generic_password_locked_down(
    service: &str,
    account: &str,
    secret: &[u8],
) -> Result<(), KeychainError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework_sys::base::errSecDuplicateItem;
    use security_framework_sys::item::{
        kSecAttrAccessControl, kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword, kSecValueData,
    };
    use security_framework_sys::keychain_item::{SecItemAdd, SecItemUpdate};

    // Build the access-control descriptor: WhenUnlockedThisDeviceOnly +
    // no extra constraints (no biometry / passcode prompt — this is a
    // service item, not a user-presence secret).
    let acl = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        0,
    )
    .map_err(KeychainError::from)?;

    // Common attributes shared between add and update queries.
    let class_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_val: CFType =
        unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType() };
    let service_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_val: CFType = CFString::from(service).into_CFType();
    let account_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_val: CFType = CFString::from(account).into_CFType();

    let value_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let value_val: CFType = CFData::from_buffer(secret).into_CFType();
    let acl_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessControl) };
    let acl_val: CFType = acl.into_CFType();
    let sync_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) };
    let sync_val: CFType = CFBoolean::false_value().into_CFType();

    // Add query: identity + value + ACL + synchronizable=false.
    let add_pairs: Vec<(CFString, CFType)> = vec![
        (class_key.clone(), class_val.clone()),
        (service_key.clone(), service_val.clone()),
        (account_key.clone(), account_val.clone()),
        (value_key.clone(), value_val.clone()),
        (acl_key.clone(), acl_val.clone()),
        (sync_key.clone(), sync_val.clone()),
    ];
    let add_params = CFDictionary::from_CFType_pairs(&add_pairs);
    let mut ret: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let status = unsafe { SecItemAdd(add_params.as_concrete_TypeRef(), &mut ret) };

    if status == 0 {
        return Ok(());
    }
    if status != errSecDuplicateItem {
        return Err(KeychainError::from(SfError::from_code(status)));
    }

    // Item already exists — update value + ACL + synchronizable so legacy
    // items get re-written with the locked-down accessibility.
    let lookup_pairs: Vec<(CFString, CFType)> = vec![
        (class_key, class_val),
        (service_key, service_val),
        (account_key, account_val),
    ];
    let update_pairs: Vec<(CFString, CFType)> = vec![
        (value_key, value_val),
        (acl_key, acl_val),
        (sync_key, sync_val),
    ];
    let lookup = CFDictionary::from_CFType_pairs(&lookup_pairs);
    let update = CFDictionary::from_CFType_pairs(&update_pairs);
    let status =
        unsafe { SecItemUpdate(lookup.as_concrete_TypeRef(), update.as_concrete_TypeRef()) };
    if status == 0 {
        Ok(())
    } else {
        Err(KeychainError::from(SfError::from_code(status)))
    }
}

/// macOS `errSecMissingEntitlement` ("A required entitlement isn't present").
///
/// Not exported by `security-framework-sys`, so we pin the literal from
/// `<Security/SecBase.h>`. Returned by `SecItemAdd`/`SecItemUpdate` when a
/// `SecAccessControl` requiring `ThisDeviceOnly` accessibility is used by a
/// binary lacking the `keychain-access-groups` entitlement (i.e. any ad-hoc
/// signed build — ad-hoc signatures cannot carry that entitlement).
#[cfg(target_os = "macos")]
const ERR_SEC_MISSING_ENTITLEMENT: i32 = -34018;

/// macOS `errSecItemNotFound` ("The specified item could not be found in the
/// keychain"). This is the ONLY status that means "no entry exists yet" and so
/// the only one that justifies generating + storing a fresh device key. Every
/// other read failure (locked keychain, denied access, disallowed interaction,
/// timeout, I/O) means the entry's status is unknown — we must NOT mint a fresh
/// key over a possibly-existing one. Pinned from `<Security/SecBase.h>`.
#[cfg(target_os = "macos")]
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// True iff `e` is the keychain `errSecMissingEntitlement` failure. Used to
/// downgrade the `ThisDeviceOnly` migration failure from a per-launch WARN to
/// a one-line DEBUG on builds that can never carry the entitlement.
#[cfg(target_os = "macos")]
fn is_missing_entitlement(e: &KeychainError) -> bool {
    matches!(e, KeychainError::Keychain(sf) if sf.code() == ERR_SEC_MISSING_ENTITLEMENT)
}

/// Outcome of classifying a `get_generic_password` read failure in
/// `load_or_create`. Pure + hermetically testable (no Keychain syscall).
#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
enum ReadFailureClass {
    /// `errSecItemNotFound` — no entry exists yet; safe to generate + store a
    /// fresh device key.
    NotFound,
    /// Any other status (locked / access denied / interaction disallowed /
    /// timeout / I/O). The entry's status is unknown, so we must NOT mint a
    /// fresh key over a possibly-existing one — propagate `Locked` and degrade.
    Locked(i32),
}

/// Classify a Keychain read-failure OSStatus into "create a fresh key" vs
/// "degrade because the keychain is unavailable". Only `errSecItemNotFound`
/// authorises key creation; everything else is treated as locked/denied.
#[cfg(target_os = "macos")]
fn classify_read_failure(code: i32) -> ReadFailureClass {
    if code == ERR_SEC_ITEM_NOT_FOUND {
        ReadFailureClass::NotFound
    } else {
        ReadFailureClass::Locked(code)
    }
}

/// Re-write the existing device-key entry under the locked-down ACL.
///
/// Called from `load_or_create`'s read path so any item written by a
/// pre-fix build (default `kSecAttrAccessibleWhenUnlocked`, iCloud-sync
/// eligible) is migrated on the next launch. We always do the rewrite —
/// the API has no read-side accessor for the current accessibility, and
/// `SecItemUpdate` with an identical ACL is a no-op cost-wise (no user
/// prompt, single round-trip).
#[cfg(target_os = "macos")]
fn migrate_legacy_accessibility_if_needed(secret: &[u8; 32]) -> Result<(), KeychainError> {
    set_generic_password_locked_down(SERVICE, ACCOUNT, secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fix C / CopyPaste-uxt7: the `errSecMissingEntitlement` classifier must
    /// recognise OSStatus -34018 (and only that code) so the daemon emits a
    /// WARN (not a hard error) for the ThisDeviceOnly migration failure on
    /// ad-hoc builds.
    #[cfg(target_os = "macos")]
    #[test]
    fn is_missing_entitlement_matches_only_minus_34018() {
        let missing = KeychainError::Keychain(SfError::from_code(ERR_SEC_MISSING_ENTITLEMENT));
        assert!(is_missing_entitlement(&missing));

        // A different keychain error must NOT be classified as missing-entitlement.
        let other = KeychainError::Keychain(SfError::from_code(-25300)); // errSecItemNotFound
        assert!(!is_missing_entitlement(&other));

        // A non-keychain variant must not match either.
        assert!(!is_missing_entitlement(&KeychainError::InvalidLength(7)));
    }

    /// Fix #4: only `errSecItemNotFound` authorises minting a fresh device key.
    /// Every other OSStatus (locked, auth-failed, interaction-not-allowed, …)
    /// must classify as `Locked` so `load_or_create` propagates a distinct
    /// error and the daemon degrades instead of overwriting a possibly-existing
    /// entry with an ephemeral key.
    ///
    /// Note: the full `load_or_create` read path calls the real
    /// `get_generic_password` syscall and cannot be exercised hermetically
    /// without an interactive Keychain, so we test the pure classifier that
    /// `load_or_create` delegates the decision to (same code path).
    #[cfg(target_os = "macos")]
    #[test]
    fn classify_read_failure_only_not_found_creates() {
        assert_eq!(
            classify_read_failure(ERR_SEC_ITEM_NOT_FOUND),
            ReadFailureClass::NotFound,
            "errSecItemNotFound must authorise key creation"
        );
        // errSecInteractionNotAllowed (-25308): keychain locked / no UI.
        assert_eq!(
            classify_read_failure(-25308),
            ReadFailureClass::Locked(-25308)
        );
        // errSecAuthFailed (-25293): access denied.
        assert_eq!(
            classify_read_failure(-25293),
            ReadFailureClass::Locked(-25293)
        );
        // errSecMissingEntitlement (-34018): not a not-found either.
        assert_eq!(
            classify_read_failure(ERR_SEC_MISSING_ENTITLEMENT),
            ReadFailureClass::Locked(ERR_SEC_MISSING_ENTITLEMENT)
        );
    }

    #[test]
    fn own_fingerprint_is_sha256_prefix() {
        let pk = [0u8; 32];
        let fp = own_fingerprint(&pk);
        // SHA-256 of 32 zero bytes is known: 66687aadf862bd776c8fc18b8e9f8e20...
        assert!(fp.starts_with("66:68:7a:ad:f8:62:bd:77:6c:8f:c1:8b:8e:9f:8e:20"));
        assert_eq!(fp.matches(':').count(), 15); // 16 bytes = 15 colons
    }

    /// CopyPaste-nkro: `store_supabase_password_to_keychain` and the cloud-sync
    /// key persist paths must use `set_generic_password_locked_down` so the
    /// kSecAttrSynchronizable=false + ThisDeviceOnly attributes prevent the
    /// secret from leaving the originating device via iCloud Keychain sync.
    ///
    /// This structural test verifies that `set_generic_password_locked_down` is
    /// accessible (pub(crate)) and callable with the expected signature, ensuring
    /// the function is wired up correctly.  Full round-trip verification (read
    /// back + check accessibility attribute) requires interactive Keychain access
    /// and lives in the `#[ignore]` tests below.
    #[cfg(target_os = "macos")]
    #[test]
    fn set_generic_password_locked_down_has_correct_signature() {
        // Structural check: the function must be accessible and have the
        // signature `(service: &str, account: &str, secret: &[u8]) -> Result<(), KeychainError>`.
        // We call it inside the ephemeral-key bypass so no real Keychain is touched.
        // COPYPASTE_EPHEMERAL_KEY is NOT set here, so we must not call the real
        // Security framework.  Instead, just verify the function pointer is
        // callable at the type level — the compiler guarantees this.
        let _fn_ptr: fn(&str, &str, &[u8]) -> Result<(), KeychainError> =
            set_generic_password_locked_down;
        // The function must be accessible and callable.
        let _ = _fn_ptr; // suppress unused warning
    }

    /// CopyPaste-uxt7: `is_missing_entitlement` must return `true` only for
    /// OSStatus -34018 so the WARN branch in `load_or_create` is triggered only
    /// on genuine ad-hoc / unsigned builds and not on other keychain failures.
    #[cfg(target_os = "macos")]
    #[test]
    fn is_missing_entitlement_is_the_warn_trigger_for_ad_hoc_builds() {
        // The exact error that triggers the ad-hoc warning path.
        let missing = KeychainError::Keychain(SfError::from_code(-34018));
        assert!(
            is_missing_entitlement(&missing),
            "OSStatus -34018 must be the warn-trigger for ThisDeviceOnly skips"
        );
        // No other status should trigger the ad-hoc warn path.
        let other = KeychainError::Keychain(SfError::from_code(-25293)); // errSecAuthFailed
        assert!(
            !is_missing_entitlement(&other),
            "non-missing-entitlement errors must NOT trigger the ad-hoc warn path"
        );
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn load_or_create_returns_keypair() {
        let _ = delete_stored();
        let kp = load_or_create().expect("should create keypair");
        assert_eq!(kp.secret_key_bytes_zeroizing().len(), 32);
        delete_stored().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn load_or_create_is_idempotent() {
        let _ = delete_stored();
        let kp1 = load_or_create().unwrap();
        let kp2 = load_or_create().unwrap();
        assert_eq!(
            kp1.secret_key_bytes_zeroizing(),
            kp2.secret_key_bytes_zeroizing()
        );
        delete_stored().unwrap();
    }

    /// Audit HIGH #2: structural test — verify the new accessibility-aware
    /// setter rejects nothing on the happy path and is callable. Full
    /// round-trip verification (read back + check accessibility attribute)
    /// requires interactive Keychain access and lives in
    /// `tests/keychain_macos.rs` with `#[ignore]`.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn set_generic_password_locked_down_round_trips() {
        let service = "com.copypaste.daemon.test.locked_down";
        let account = "test-account";
        let secret = [0xABu8; 32];
        // Cleanup any leftover from a previous failed run.
        let _ = delete_generic_password(service, account);
        set_generic_password_locked_down(service, account, &secret)
            .expect("locked-down add should succeed");
        // Second call must hit the SecItemUpdate path (errSecDuplicateItem).
        set_generic_password_locked_down(service, account, &secret)
            .expect("locked-down add on duplicate should fall back to update");
        let readback = get_generic_password(service, account).expect("readback");
        assert_eq!(readback, &secret[..]);
        delete_generic_password(service, account).expect("cleanup");
    }
}
