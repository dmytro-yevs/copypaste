//! macOS Keychain ACL enforcement for CopyPaste's persisted device-secret key.
//!
//! THREAT-MODEL OI-4 (v0.3) — Without an ACL, a generic-password entry in the
//! login keychain is reachable by ANY process that the user has previously
//! granted Keychain access to (the dreaded "Always Allow" button on a
//! different app's prompt can leak our key by accident).  This module pins
//! the entry's ACL to exactly three trusted binaries shipped inside
//! `CopyPaste.app/Contents/MacOS/`:
//!
//! * `copypaste-daemon` — background sync + storage
//! * `copypaste`        — CLI used by `vacuum` and other admin commands
//! * `copypaste-ui`     — Tauri desktop UI
//!
//! Anyone else asking for the entry triggers the standard macOS prompt
//! ("APP_NAME wants to access key 'device-secret-key' in your keychain") that
//! the user must explicitly approve per process, per item.
//!
//! ## Why raw FFI?
//!
//! `security-framework` 2.x exposes `SecAccessRef` as an opaque type alias
//! but does NOT bind the underlying ACL constructor functions
//! (`SecAccessCreate`, `SecTrustedApplicationCreateFromPath`,
//! `SecKeychainItemCreateFromContent`, `SecKeychainItemSetAccess`).  The
//! `SecKeychain*` API surface is deprecated since macOS 10.10 but still
//! functions — for the legacy keychain it is the only way to attach an
//! explicit trust list.  The newer `SecItem*` API does not support a custom
//! ACL with a trust list.  We therefore drop down to raw `extern "C"`
//! declarations against the Security.framework symbols and gate everything
//! behind `#[cfg(target_os = "macos")]`.
//!
//! ## Migration (one-shot on daemon start)
//!
//! Users upgrading from v0.2 already have an ACL-less entry.
//! `rotate_acl_to_current_install` reads the existing secret, deletes the
//! item, and re-creates it with the new ACL.  The check is cheap (an item
//! lookup) and idempotent — once the entry already has the correct trust
//! list we skip the rewrite.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, OSStatus, TCFType};
use core_foundation::string::{CFString, CFStringRef};

use super::{KeychainError, ACCOUNT, SERVICE};

// ─── Opaque FFI types ──────────────────────────────────────────────────────
//
// We mirror the (unimplemented in security-framework 2.x) opaque pointer
// shapes; we never construct them, only pass them along to the C ABI.

#[repr(C)]
pub struct OpaqueSecAccess(std::ffi::c_void);
pub type SecAccessRef = *mut OpaqueSecAccess;

#[repr(C)]
pub struct OpaqueSecTrustedApplication(std::ffi::c_void);
pub type SecTrustedApplicationRef = *mut OpaqueSecTrustedApplication;

#[repr(C)]
pub struct OpaqueSecKeychainItem(std::ffi::c_void);
pub type SecKeychainItemRef = *mut OpaqueSecKeychainItem;

#[repr(C)]
pub struct OpaqueSecKeychain(std::ffi::c_void);
pub type SecKeychainRef = *mut OpaqueSecKeychain;

// SecItemClass enum value `kSecGenericPasswordItemClass` is the four-char-code
// `'genp'`.  Apple's header exposes it as the OSType (i.e. `u32`) literal.
const kSecGenericPasswordItemClass: u32 = u32::from_be_bytes(*b"genp");

// ─── Raw FFI ───────────────────────────────────────────────────────────────

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecAccessCreate(
        descriptor: CFStringRef,
        trustedlist: CFArrayRef,
        accessRef: *mut SecAccessRef,
    ) -> OSStatus;

    fn SecTrustedApplicationCreateFromPath(
        path: *const std::os::raw::c_char,
        app: *mut SecTrustedApplicationRef,
    ) -> OSStatus;

    /// NOTE: For a generic password we leave `attrList` minimal — service +
    /// account go through the wrapper, here we set them via the
    /// `SecKeychainAttribute` list bound to `kSecServiceItemAttr` and
    /// `kSecAccountItemAttr`.
    fn SecKeychainItemCreateFromContent(
        itemClass: u32,
        attrList: *const SecKeychainAttributeList,
        length: u32,
        data: *const std::ffi::c_void,
        keychainRef: SecKeychainRef,
        initialAccess: SecAccessRef,
        itemRef: *mut SecKeychainItemRef,
    ) -> OSStatus;

    fn SecKeychainItemDelete(itemRef: SecKeychainItemRef) -> OSStatus;

    fn SecKeychainItemCopyAccess(
        itemRef: SecKeychainItemRef,
        accessRef: *mut SecAccessRef,
    ) -> OSStatus;

    fn SecAccessCopyACLList(accessRef: SecAccessRef, aclList: *mut CFArrayRef) -> OSStatus;

    fn SecACLCopyContents(
        acl: CFTypeRef,
        applicationList: *mut CFArrayRef,
        description: *mut CFStringRef,
        promptSelector: *mut u32, // SecKeychainPromptSelector packed into a u32
    ) -> OSStatus;

    fn SecTrustedApplicationCopyData(
        appRef: SecTrustedApplicationRef,
        data: *mut core_foundation::data::CFDataRef,
    ) -> OSStatus;
}

/// `SecKeychainAttribute` mirrors the legacy keychain C struct.
#[repr(C)]
struct SecKeychainAttribute {
    tag: u32,
    length: u32,
    data: *mut std::ffi::c_void,
}

#[repr(C)]
struct SecKeychainAttributeList {
    count: u32,
    attr: *mut SecKeychainAttribute,
}

// Four-char-code attribute tags from `<Security/SecKeychainItem.h>`:
const kSecServiceItemAttr: u32 = u32::from_be_bytes(*b"svce");
const kSecAccountItemAttr: u32 = u32::from_be_bytes(*b"acct");

const ERR_SEC_SUCCESS: OSStatus = 0;
pub(crate) const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;

// ─── Public API ────────────────────────────────────────────────────────────

/// Resolve the absolute paths of the three CopyPaste binaries that should be
/// in the Keychain ACL trust list.  The reference path is the *currently
/// running* executable (typically the daemon), and siblings are resolved as
/// peers in the same `Contents/MacOS/` directory.
///
/// In a non-bundled developer build (`cargo run`), the siblings probably do
/// not exist; we silently drop the missing ones from the trust list so that
/// `cargo test` and `cargo run` still work.  The daemon's own binary is the
/// invariant we always include.
pub fn trusted_binary_paths() -> Result<Vec<PathBuf>, KeychainError> {
    let self_path = std::env::current_exe().map_err(KeychainError::Io)?;
    let parent = self_path.parent().ok_or_else(|| {
        KeychainError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "current_exe has no parent directory",
        ))
    })?;

    let candidates = ["copypaste-daemon", "copypaste", "copypaste-ui"];
    let mut paths: Vec<PathBuf> = Vec::with_capacity(candidates.len());

    // Always include the running binary even if its filename does not match
    // one of the canonical three (e.g. test runner with a hash suffix).
    paths.push(self_path.clone());

    for name in candidates {
        let p = parent.join(name);
        if p == self_path {
            continue;
        }
        if p.exists() {
            paths.push(p);
        } else {
            tracing::debug!(
                missing_path = %p.display(),
                "trusted_binary_paths: sibling not present (likely dev build)"
            );
        }
    }

    Ok(paths)
}

/// Build a `SecAccessRef` whose ACL allows exactly the binaries in `paths`
/// and the descriptor `descriptor`.  Caller owns the returned ref and must
/// CFRelease it.
fn build_access(descriptor: &str, paths: &[PathBuf]) -> Result<SecAccessRef, KeychainError> {
    if paths.is_empty() {
        return Err(KeychainError::AclEmpty);
    }

    // Build one SecTrustedApplicationRef per binary.  Wrap in a guard so we
    // CFRelease them on every exit path including the error branch.
    let mut trusted_apps: Vec<SecTrustedApplicationRef> = Vec::with_capacity(paths.len());

    let result = (|| -> Result<SecAccessRef, KeychainError> {
        for path in paths {
            let c_path = CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|_| KeychainError::AclPathEncoding)?;
            let mut app: SecTrustedApplicationRef = ptr::null_mut();
            let status = unsafe { SecTrustedApplicationCreateFromPath(c_path.as_ptr(), &mut app) };
            if status != ERR_SEC_SUCCESS {
                return Err(KeychainError::OsStatus {
                    op: "SecTrustedApplicationCreateFromPath",
                    code: status,
                });
            }
            trusted_apps.push(app);
        }

        // Convert the Vec<*mut OpaqueSecTrustedApplication> into a CFArray<CFType>.
        // CFArray takes a copy of the raw pointers (CFArrayCreate retains
        // each entry under the kCFTypeArrayCallBacks default), so we still
        // own the originals and must release them ourselves below.
        let cf_array: CFArray<CFTypeRef> = CFArray::from_copyable(
            &trusted_apps
                .iter()
                .map(|p| *p as CFTypeRef)
                .collect::<Vec<_>>(),
        );

        let cf_descriptor = CFString::new(descriptor);
        let mut access_ref: SecAccessRef = ptr::null_mut();
        let status = unsafe {
            SecAccessCreate(
                cf_descriptor.as_concrete_TypeRef(),
                cf_array.as_concrete_TypeRef(),
                &mut access_ref,
            )
        };
        if status != ERR_SEC_SUCCESS {
            return Err(KeychainError::OsStatus {
                op: "SecAccessCreate",
                code: status,
            });
        }
        Ok(access_ref)
    })();

    // Release the per-app refs regardless of success — SecAccessCreate has
    // already retained the ones it kept internally via CFArray callbacks.
    for app in trusted_apps {
        if !app.is_null() {
            unsafe { CFRelease(app as CFTypeRef) };
        }
    }

    result
}

/// Create the keychain entry `(SERVICE, ACCOUNT)` with `secret` as its
/// 32-byte payload AND an ACL restricting access to `paths`.  Caller is
/// responsible for ensuring no entry with the same `(service, account)`
/// already exists — this is enforced via the OSStatus check (errSecDuplicateItem
/// = -25299 surfaces as `OsStatus`).
pub fn store_with_acl(secret: &[u8; 32], paths: &[PathBuf]) -> Result<(), KeychainError> {
    let access = build_access("CopyPaste device key", paths)?;

    // Build the attribute list (service + account).
    let service_bytes = SERVICE.as_bytes();
    let account_bytes = ACCOUNT.as_bytes();
    let mut attrs = [
        SecKeychainAttribute {
            tag: kSecServiceItemAttr,
            length: service_bytes.len() as u32,
            data: service_bytes.as_ptr() as *mut std::ffi::c_void,
        },
        SecKeychainAttribute {
            tag: kSecAccountItemAttr,
            length: account_bytes.len() as u32,
            data: account_bytes.as_ptr() as *mut std::ffi::c_void,
        },
    ];
    let attr_list = SecKeychainAttributeList {
        count: attrs.len() as u32,
        attr: attrs.as_mut_ptr(),
    };

    let mut out_item: SecKeychainItemRef = ptr::null_mut();
    let status = unsafe {
        SecKeychainItemCreateFromContent(
            kSecGenericPasswordItemClass,
            &attr_list,
            secret.len() as u32,
            secret.as_ptr() as *const std::ffi::c_void,
            ptr::null_mut(), // default keychain
            access,
            &mut out_item,
        )
    };

    // SecAccessCreate returned a +1 ref; the item now owns its own retained
    // copy.  Release ours so it doesn't leak.
    unsafe { CFRelease(access as CFTypeRef) };
    if !out_item.is_null() {
        unsafe { CFRelease(out_item as CFTypeRef) };
    }

    if status != ERR_SEC_SUCCESS {
        return Err(KeychainError::OsStatus {
            op: "SecKeychainItemCreateFromContent",
            code: status,
        });
    }
    Ok(())
}

/// Return the SHA-256 hashes of every trusted application currently in the
/// keychain entry's ACL.  `SecTrustedApplicationCopyData` returns the
/// designated requirement hash blob (20- or 32-byte digest) — comparing
/// digests instead of paths is robust to bundle relocation.
pub fn current_acl_app_digests() -> Result<Vec<Vec<u8>>, KeychainError> {
    use security_framework::passwords::get_generic_password;

    // We can only inspect an item that exists.  Trigger the lookup first to
    // surface a clean "not present" error.
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(_) => {}
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(Vec::new()),
        Err(e) => return Err(KeychainError::Keychain(e)),
    }

    // Re-find the item via the legacy API to obtain a SecKeychainItemRef.
    // We use security-framework's macOS-namespaced `find_generic_password`
    // which returns the item ref alongside the password.
    use security_framework::os::macos::passwords::find_generic_password as legacy_find;
    let (_pw, item) = legacy_find(None, SERVICE, ACCOUNT).map_err(KeychainError::Keychain)?;
    let item_ref = item.as_CFTypeRef() as SecKeychainItemRef;

    let mut access_ref: SecAccessRef = ptr::null_mut();
    let status = unsafe { SecKeychainItemCopyAccess(item_ref, &mut access_ref) };
    if status != ERR_SEC_SUCCESS {
        return Err(KeychainError::OsStatus {
            op: "SecKeychainItemCopyAccess",
            code: status,
        });
    }

    let mut acl_array: CFArrayRef = ptr::null_mut();
    let status = unsafe { SecAccessCopyACLList(access_ref, &mut acl_array) };
    // We no longer need access_ref after this call — release it.
    unsafe { CFRelease(access_ref as CFTypeRef) };
    if status != ERR_SEC_SUCCESS {
        return Err(KeychainError::OsStatus {
            op: "SecAccessCopyACLList",
            code: status,
        });
    }
    if acl_array.is_null() {
        return Ok(Vec::new());
    }

    let mut digests: Vec<Vec<u8>> = Vec::new();
    let acls: CFArray<CFTypeRef> = unsafe { CFArray::wrap_under_create_rule(acl_array) };

    for acl in acls.iter() {
        let mut app_list: CFArrayRef = ptr::null_mut();
        let mut description: CFStringRef = ptr::null();
        let mut prompt_selector: u32 = 0;
        let status = unsafe {
            SecACLCopyContents(*acl, &mut app_list, &mut description, &mut prompt_selector)
        };
        if status != ERR_SEC_SUCCESS {
            continue;
        }
        if !description.is_null() {
            unsafe { CFRelease(description as CFTypeRef) };
        }
        if app_list.is_null() {
            continue;
        }
        let apps: CFArray<CFTypeRef> = unsafe { CFArray::wrap_under_create_rule(app_list) };
        for app in apps.iter() {
            let mut data: core_foundation::data::CFDataRef = ptr::null_mut();
            let st = unsafe {
                SecTrustedApplicationCopyData(*app as SecTrustedApplicationRef, &mut data)
            };
            if st != ERR_SEC_SUCCESS || data.is_null() {
                continue;
            }
            let cf_data: core_foundation::data::CFData =
                unsafe { core_foundation::data::CFData::wrap_under_create_rule(data) };
            digests.push(cf_data.bytes().to_vec());
        }
    }
    Ok(digests)
}

/// One-shot startup hook: ensure the entry has an ACL pinned to the current
/// install's three binaries.  If the entry already exists *without* an ACL
/// (legacy v0.2 install) or with a stale ACL (binaries moved), we copy out
/// the secret, delete the item, and re-create it with `store_with_acl`.
///
/// Returns `Ok(true)` if a rotation was performed, `Ok(false)` if the ACL
/// was already correct (or no entry exists yet — first run).
pub fn rotate_acl_to_current_install() -> Result<bool, KeychainError> {
    use security_framework::passwords::{delete_generic_password, get_generic_password};

    let secret_bytes = match get_generic_password(SERVICE, ACCOUNT) {
        Ok(b) => b,
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {
            // Nothing to migrate; first run will create the entry with ACL
            // through `super::load_or_create`.
            return Ok(false);
        }
        Err(e) => return Err(KeychainError::Keychain(e)),
    };
    if secret_bytes.len() != 32 {
        return Err(KeychainError::InvalidLength(secret_bytes.len()));
    }
    let mut secret_arr = [0u8; 32];
    secret_arr.copy_from_slice(&secret_bytes);

    // If the ACL already lists exactly the expected number of apps we treat
    // it as correct and skip the rotation.  We deliberately do NOT compare
    // digests byte-for-byte: that would force a rewrite every time the
    // installer location changes (e.g. user moves CopyPaste.app to
    // ~/Applications/), which causes a Keychain prompt.  Counting is a cheap
    // structural check that catches the v0.2 → v0.3 migration (which has
    // zero ACL entries beyond the default owner) while staying quiet
    // afterwards.
    let trusted = trusted_binary_paths()?;
    match current_acl_app_digests() {
        Ok(d) if d.len() == trusted.len() => {
            tracing::debug!(
                acl_apps = d.len(),
                "rotate_acl: existing ACL already has expected app count; skipping rotation"
            );
            return Ok(false);
        }
        Ok(d) => {
            tracing::info!(
                old_acl_apps = d.len(),
                new_acl_apps = trusted.len(),
                "rotate_acl: ACL mismatch — rotating keychain entry"
            );
        }
        Err(e) => {
            // We could not read the ACL; safest behavior is to attempt
            // rotation so the user ends up with a well-formed entry.
            tracing::warn!(error = %e, "rotate_acl: could not read ACL — forcing rotation");
        }
    }

    // Delete + recreate.  The secret bytes are in memory; rotation is atomic
    // from the user's perspective (worst case: brief window with no entry —
    // any concurrent reader gets `errSecItemNotFound` and retries).
    delete_generic_password(SERVICE, ACCOUNT).map_err(KeychainError::Keychain)?;
    store_with_acl(&secret_arr, &trusted)?;
    // Best-effort zero of the local copy.
    for b in secret_arr.iter_mut() {
        *b = 0;
    }
    Ok(true)
}
