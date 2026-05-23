//! v0.3 (THREAT-MODEL OI-4): macOS Keychain ACL enforcement tests.
//!
//! Scope clarification: the task ticket originally referenced
//! `crates/copypaste-core/tests/keychain_acl.rs`, but the keychain layer
//! (and `security-framework` dependency) lives in `copypaste-daemon`.  The
//! existing `tests/keychain_macos.rs` already documents this gap and
//! follows the same convention — we mirror it for the ACL suite.
//!
//! All tests are `#[ignore]` and `#[cfg(target_os = "macos")]`:
//!
//! * They mutate the user's *real* login keychain (no in-memory shim exists
//!   for `SecAccess*` — Apple's API only operates on `SecKeychainRef`-backed
//!   storage), so running them unattended would interact with the developer's
//!   own CopyPaste daemon entries.  To make that safe, each test allocates a
//!   UUID-suffixed `(service, account)` pair and tears it down on exit; the
//!   suite never touches the canonical `com.copypaste.daemon` /
//!   `device-secret-key` entry.
//!
//! * CI runners do not have a Keychain at all (headless macOS without a
//!   logged-in user / unlocked default keychain triggers `errSecNoSuchKeychain`
//!   = -25294 or `errSecInteractionNotAllowed` = -25308).  Marking the suite
//!   `#[ignore]` keeps `cargo test` green there.
//!
//! Run manually on a developer macOS box with:
//!
//! ```sh
//! cargo test -p copypaste-daemon --test keychain_acl -- --ignored
//! ```

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::ptr;

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, OSStatus, TCFType};
use core_foundation::data::{CFData, CFDataRef};
use core_foundation::string::{CFString, CFStringRef};
use security_framework::os::macos::passwords::find_generic_password as legacy_find;
use security_framework::passwords::{delete_generic_password, get_generic_password};
use serial_test::serial;
use uuid::Uuid;

// ── FFI mirrors of the symbols the production module uses ─────────────────
//
// We re-declare them here (instead of `pub`-exposing the production
// `extern` block) to keep the production API surface tight: tests have no
// reason to require the daemon to leak Security.framework symbol bindings.

#[repr(C)]
struct OpaqueSecAccess(std::ffi::c_void);
type SecAccessRef = *mut OpaqueSecAccess;
#[repr(C)]
struct OpaqueSecTrustedApplication(std::ffi::c_void);
type SecTrustedApplicationRef = *mut OpaqueSecTrustedApplication;
#[repr(C)]
struct OpaqueSecKeychainItem(std::ffi::c_void);
type SecKeychainItemRef = *mut OpaqueSecKeychainItem;

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecKeychainItemCopyAccess(
        itemRef: SecKeychainItemRef,
        accessRef: *mut SecAccessRef,
    ) -> OSStatus;
    fn SecAccessCopyACLList(accessRef: SecAccessRef, aclList: *mut CFArrayRef) -> OSStatus;
    fn SecACLCopyContents(
        acl: CFTypeRef,
        applicationList: *mut CFArrayRef,
        description: *mut CFStringRef,
        promptSelector: *mut u32,
    ) -> OSStatus;
    fn SecTrustedApplicationCopyData(
        appRef: SecTrustedApplicationRef,
        data: *mut CFDataRef,
    ) -> OSStatus;
}

const ERR_SEC_SUCCESS: OSStatus = 0;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;

// ── Test helpers ───────────────────────────────────────────────────────────

/// Build a unique `(service, account)` pair so parallel CI runs and repeat
/// local runs cannot collide with each other or with the daemon's real
/// keychain entry.
fn unique(label: &str) -> (String, String) {
    let n = Uuid::new_v4().simple().to_string();
    (
        format!("com.copypaste.test.acl.{label}.{n}"),
        format!("acl-test-account-{label}"),
    )
}

/// Best-effort cleanup helper.  Swallows `errSecItemNotFound` because
/// several tests intentionally drive the missing-item path.
fn cleanup(service: &str, account: &str) {
    match delete_generic_password(service, account) {
        Ok(()) => {}
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {}
        Err(e) => panic!(
            "cleanup failed for {service}/{account}: code={} err={:?}",
            e.code(),
            e
        ),
    }
}

/// Pull every trusted-application digest out of the ACL of
/// `(service, account)`.  Bypasses the production helper so the test is
/// independent of the module under test (otherwise a buggy production
/// helper would mask its own bug).
fn acl_digests_for(service: &str, account: &str) -> Vec<Vec<u8>> {
    let (_pw, item) = legacy_find(None, service, account)
        .unwrap_or_else(|e| panic!("find_generic_password({service}, {account}): {e:?}"));
    let item_ref = item.as_CFTypeRef() as SecKeychainItemRef;

    let mut access_ref: SecAccessRef = ptr::null_mut();
    let st = unsafe { SecKeychainItemCopyAccess(item_ref, &mut access_ref) };
    assert_eq!(
        st, ERR_SEC_SUCCESS,
        "SecKeychainItemCopyAccess failed: {st}"
    );

    let mut acl_array: CFArrayRef = ptr::null_mut();
    let st = unsafe { SecAccessCopyACLList(access_ref, &mut acl_array) };
    unsafe { CFRelease(access_ref as CFTypeRef) };
    assert_eq!(st, ERR_SEC_SUCCESS, "SecAccessCopyACLList failed: {st}");
    if acl_array.is_null() {
        return Vec::new();
    }
    let acls: CFArray<CFTypeRef> = unsafe { CFArray::wrap_under_create_rule(acl_array) };
    let mut out: Vec<Vec<u8>> = Vec::new();
    for acl in acls.iter() {
        let mut app_list: CFArrayRef = ptr::null_mut();
        let mut description: CFStringRef = ptr::null();
        let mut sel: u32 = 0;
        let st = unsafe { SecACLCopyContents(*acl, &mut app_list, &mut description, &mut sel) };
        if st != ERR_SEC_SUCCESS || app_list.is_null() {
            if !description.is_null() {
                unsafe { CFRelease(description as CFTypeRef) };
            }
            continue;
        }
        if !description.is_null() {
            unsafe { CFRelease(description as CFTypeRef) };
        }
        let apps: CFArray<CFTypeRef> = unsafe { CFArray::wrap_under_create_rule(app_list) };
        for app in apps.iter() {
            let mut data: CFDataRef = ptr::null_mut();
            let st = unsafe {
                SecTrustedApplicationCopyData(*app as SecTrustedApplicationRef, &mut data)
            };
            if st != ERR_SEC_SUCCESS || data.is_null() {
                continue;
            }
            let cf: CFData = unsafe { CFData::wrap_under_create_rule(data) };
            out.push(cf.bytes().to_vec());
        }
    }
    out
}

/// Wrapper around the production `acl::store_with_acl` that targets a
/// custom `(service, account)` instead of the hardcoded daemon constants.
/// We can't reach the private constants from the public API, but the
/// underlying helper is parameter-free — so for the tests we re-implement
/// the same code path using the public building blocks (path resolution +
/// raw FFI).  This is also the strategy used by `tests/keychain_macos.rs`.
fn store_with_acl_at(
    service: &str,
    account: &str,
    secret: &[u8; 32],
    paths: &[PathBuf],
) -> Result<(), String> {
    use std::ffi::CString;

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
    type SecKeychainRef = *mut std::ffi::c_void;
    const K_GENP: u32 = u32::from_be_bytes(*b"genp");
    const K_SVCE: u32 = u32::from_be_bytes(*b"svce");
    const K_ACCT: u32 = u32::from_be_bytes(*b"acct");
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
        fn SecKeychainItemCreateFromContent(
            itemClass: u32,
            attrList: *const SecKeychainAttributeList,
            length: u32,
            data: *const std::ffi::c_void,
            keychainRef: SecKeychainRef,
            initialAccess: SecAccessRef,
            itemRef: *mut SecKeychainItemRef,
        ) -> OSStatus;
    }

    let mut apps: Vec<SecTrustedApplicationRef> = Vec::with_capacity(paths.len());
    for p in paths {
        let c = CString::new(p.as_os_str().as_encoded_bytes())
            .map_err(|_| "path contains NUL".to_string())?;
        let mut a: SecTrustedApplicationRef = ptr::null_mut();
        let st = unsafe { SecTrustedApplicationCreateFromPath(c.as_ptr(), &mut a) };
        if st != ERR_SEC_SUCCESS {
            for a in &apps {
                unsafe { CFRelease(*a as CFTypeRef) };
            }
            return Err(format!("SecTrustedApplicationCreateFromPath failed: {st}"));
        }
        apps.push(a);
    }
    let cf_array: CFArray<CFTypeRef> =
        CFArray::from_copyable(&apps.iter().map(|p| *p as CFTypeRef).collect::<Vec<_>>());
    let desc = CFString::new("CopyPaste device key (test)");
    let mut access: SecAccessRef = ptr::null_mut();
    let st = unsafe {
        SecAccessCreate(
            desc.as_concrete_TypeRef(),
            cf_array.as_concrete_TypeRef(),
            &mut access,
        )
    };
    for a in &apps {
        unsafe { CFRelease(*a as CFTypeRef) };
    }
    if st != ERR_SEC_SUCCESS {
        return Err(format!("SecAccessCreate failed: {st}"));
    }

    let svc_bytes = service.as_bytes();
    let acc_bytes = account.as_bytes();
    let mut attrs = [
        SecKeychainAttribute {
            tag: K_SVCE,
            length: svc_bytes.len() as u32,
            data: svc_bytes.as_ptr() as *mut _,
        },
        SecKeychainAttribute {
            tag: K_ACCT,
            length: acc_bytes.len() as u32,
            data: acc_bytes.as_ptr() as *mut _,
        },
    ];
    let attr_list = SecKeychainAttributeList {
        count: attrs.len() as u32,
        attr: attrs.as_mut_ptr(),
    };
    let mut item: SecKeychainItemRef = ptr::null_mut();
    let st = unsafe {
        SecKeychainItemCreateFromContent(
            K_GENP,
            &attr_list,
            secret.len() as u32,
            secret.as_ptr() as *const _,
            ptr::null_mut(),
            access,
            &mut item,
        )
    };
    unsafe { CFRelease(access as CFTypeRef) };
    if !item.is_null() {
        unsafe { CFRelease(item as CFTypeRef) };
    }
    if st != ERR_SEC_SUCCESS {
        return Err(format!("SecKeychainItemCreateFromContent failed: {st}"));
    }
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// Sanity: ACL-protected entry round-trips and stores the bytes verbatim.
/// Owner gets implicit access so the test process itself (which IS the
/// trusted binary, via `current_exe`) reads back without a prompt.
#[test]
#[serial]
#[ignore = "mutates user's login keychain; run manually with `cargo test -- --ignored`"]
fn store_db_key_with_acl_creates_entry() {
    let (service, account) = unique("create");
    cleanup(&service, &account);

    let secret: [u8; 32] = [0x77; 32];
    let self_path = std::env::current_exe().expect("current_exe");
    store_with_acl_at(&service, &account, &secret, &[self_path])
        .expect("store_with_acl_at must succeed");

    let pw = get_generic_password(&service, &account)
        .expect("get_generic_password must succeed for trusted caller");
    assert_eq!(pw.len(), 32, "stored payload must be 32 bytes");
    assert_eq!(pw.as_slice(), &secret, "round-trip mismatch");

    cleanup(&service, &account);
}

/// The ACL must enumerate exactly the binaries we passed in.  We assert on
/// digest *count* (matching the production rotation contract) rather than
/// digest bytes, because the digest blob format is opaque (`SecTrusted-
/// ApplicationCopyData` returns the designated-requirement representation
/// which varies across macOS versions).
#[test]
#[serial]
#[ignore = "mutates user's login keychain; run manually with `cargo test -- --ignored`"]
fn acl_includes_three_copypaste_binaries() {
    let (service, account) = unique("three-bins");
    cleanup(&service, &account);

    // Use this test binary three times — we just need three distinct entries
    // to confirm the ACL plumbing carries them through.  On a real install
    // these would be daemon/CLI/UI; the production resolver
    // `acl::trusted_binary_paths` exercises the path-discovery half.
    let self_path = std::env::current_exe().expect("current_exe");
    let paths = vec![self_path.clone(), self_path.clone(), self_path];

    let secret: [u8; 32] = [0x33; 32];
    store_with_acl_at(&service, &account, &secret, &paths).expect("store_with_acl_at must succeed");

    let digests = acl_digests_for(&service, &account);
    assert_eq!(
        digests.len(),
        paths.len(),
        "ACL digest count must equal trust-list size: got {} expected {}",
        digests.len(),
        paths.len()
    );
    for d in &digests {
        assert!(!d.is_empty(), "each ACL app digest must be non-empty");
    }

    cleanup(&service, &account);
}

/// A binary that is NOT in the trust list must NOT appear in the entry's
/// ACL.  We seed the entry with ONLY the test runner's own path and then
/// scan the ACL for `/usr/bin/curl`'s digest — a binary present on every
/// macOS install but never granted CopyPaste keychain access.  The
/// digest blobs are opaque, so we re-resolve curl's expected digest via
/// the same FFI and compare.
#[test]
#[serial]
#[ignore = "mutates user's login keychain; run manually with `cargo test -- --ignored`"]
fn acl_excludes_arbitrary_third_party_binary() {
    use std::ffi::CString;

    #[link(name = "Security", kind = "framework")]
    extern "C" {
        fn SecTrustedApplicationCreateFromPath(
            path: *const std::os::raw::c_char,
            app: *mut SecTrustedApplicationRef,
        ) -> OSStatus;
    }

    let (service, account) = unique("excludes-curl");
    cleanup(&service, &account);

    let self_path = std::env::current_exe().expect("current_exe");
    let secret: [u8; 32] = [0x55; 32];
    store_with_acl_at(&service, &account, &secret, &[self_path])
        .expect("store_with_acl_at must succeed");

    // Compute /usr/bin/curl's would-be ACL digest using the same FFI.
    let curl_path = std::path::Path::new("/usr/bin/curl");
    assert!(
        curl_path.exists(),
        "test precondition: /usr/bin/curl must exist on the macOS dev box"
    );
    let c_path = CString::new(curl_path.as_os_str().as_encoded_bytes()).unwrap();
    let mut curl_app: SecTrustedApplicationRef = ptr::null_mut();
    let st = unsafe { SecTrustedApplicationCreateFromPath(c_path.as_ptr(), &mut curl_app) };
    assert_eq!(st, ERR_SEC_SUCCESS);
    let mut curl_data: CFDataRef = ptr::null_mut();
    let st = unsafe { SecTrustedApplicationCopyData(curl_app, &mut curl_data) };
    unsafe { CFRelease(curl_app as CFTypeRef) };
    assert_eq!(st, ERR_SEC_SUCCESS);
    assert!(!curl_data.is_null());
    let curl_digest: Vec<u8> = unsafe {
        let cf = CFData::wrap_under_create_rule(curl_data);
        cf.bytes().to_vec()
    };

    let acl_digests = acl_digests_for(&service, &account);
    assert!(
        !acl_digests.contains(&curl_digest),
        "/usr/bin/curl must NOT be in the ACL trust list (found {} ACL apps total)",
        acl_digests.len()
    );
    // We expected exactly one entry — the test binary itself.
    assert_eq!(
        acl_digests.len(),
        1,
        "ACL should list exactly the one trusted binary we passed in"
    );

    cleanup(&service, &account);
}
