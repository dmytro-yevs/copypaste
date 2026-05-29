//! Beta-bonus: macOS Keychain wrapper smoke tests.
//!
//! Scope clarification: the original task ticket listed
//! `crates/copypaste-core/tests/keychain_macos.rs` as the target path,
//! but `copypaste-core` has no keychain module — the wrapper lives in
//! `copypaste_daemon::keychain` and depends on `security-framework`, which
//! is only pulled in by `copypaste-daemon`'s `[target.'cfg(macos)']` deps.
//! Placing tests in `copypaste-core` would therefore not compile.  The
//! tests live here instead and exercise the same underlying primitives
//! (`set_generic_password` / `get_generic_password` /
//! `delete_generic_password`) that `keychain::load_or_create` and
//! `keychain::delete_stored` use internally.
//!
//! The wrapper hardcodes its `SERVICE` / `ACCOUNT` constants, so the tests
//! use a unique service identifier per test (with a UUID-derived nonce) to
//! avoid clobbering the user's real device-key entry on the developer's
//! machine.  Every test cleans up its own keychain entry on the way out.
//!
//! Linux / Windows: the entire suite is gated `#[cfg(target_os = "macos")]`
//! and compiles to an empty test binary on other platforms.

#![cfg(target_os = "macos")]

use security_framework::base::Error as SfError;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serial_test::serial;
use uuid::Uuid;

/// macOS `errSecItemNotFound` — returned by `get`/`delete` when no item exists.
/// Defined in `<Security/SecBase.h>`; mirrored here to keep the test free of
/// `security-framework-sys` as a direct dependency.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// Build a unique `(service, account)` pair so parallel CI runs and
/// repeated local runs cannot collide with each other or with the
/// daemon's real keychain entry (`com.copypaste.daemon` / `device-secret-key`).
fn unique_account(label: &str) -> (String, String) {
    let nonce = Uuid::new_v4().simple().to_string();
    (
        format!("com.copypaste.test.{label}.{nonce}"),
        format!("test-account-{label}"),
    )
}

/// Best-effort cleanup helper.  Swallows `errSecItemNotFound` since several
/// tests intentionally exercise the "missing" path; any other error is
/// surfaced via a panic so a leaking entry is visible in test output.
fn cleanup(service: &str, account: &str) {
    match delete_generic_password(service, account) {
        Ok(()) => {}
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {}
        Err(e) => panic!(
            "cleanup failed for {service}/{account}: {e:?} code={}",
            e.code()
        ),
    }
}

#[test]
#[serial]
#[ignore = "touches real macOS Keychain; run explicitly with `cargo test -- --ignored`"]
fn store_and_retrieve_db_key_roundtrip() {
    let (service, account) = unique_account("roundtrip");
    // Simulate a 32-byte SQLCipher DB key — same shape as the device secret
    // the daemon stores via `load_or_create`.
    let db_key: [u8; 32] = [0xA5; 32];

    set_generic_password(&service, &account, &db_key)
        .expect("set_generic_password should succeed on macOS");

    let retrieved = get_generic_password(&service, &account)
        .expect("get_generic_password should retrieve what was just set");

    assert_eq!(retrieved.len(), 32, "retrieved key must be 32 bytes");
    assert_eq!(
        retrieved.as_slice(),
        &db_key,
        "retrieved key must byte-for-byte match what was stored"
    );

    cleanup(&service, &account);
}

#[test]
#[serial]
#[ignore = "touches real macOS Keychain; run explicitly with `cargo test -- --ignored`"]
fn retrieve_missing_key_returns_specific_error() {
    let (service, account) = unique_account("missing");
    // Belt-and-braces: ensure nothing is hanging around from a previous run.
    cleanup(&service, &account);

    let err: SfError = get_generic_password(&service, &account)
        .expect_err("expected errSecItemNotFound for non-existent entry");

    assert_eq!(
        err.code(),
        ERR_SEC_ITEM_NOT_FOUND,
        "missing keychain item must surface errSecItemNotFound (-25300), got {}",
        err.code()
    );
}

#[test]
#[serial]
#[ignore = "touches real macOS Keychain; run explicitly with `cargo test -- --ignored`"]
fn overwrite_existing_key() {
    let (service, account) = unique_account("overwrite");
    let first: [u8; 32] = [0x11; 32];
    let second: [u8; 32] = [0x22; 32];

    set_generic_password(&service, &account, &first).expect("first set");
    set_generic_password(&service, &account, &second).expect("second set (overwrite)");

    let retrieved = get_generic_password(&service, &account).expect("get after overwrite");
    assert_eq!(
        retrieved.as_slice(),
        &second,
        "retrieving after overwrite must return the second value, not the first"
    );
    assert_ne!(
        retrieved.as_slice(),
        &first,
        "first value must have been replaced, not coexist"
    );

    cleanup(&service, &account);
}

#[test]
#[serial]
#[ignore = "touches real macOS Keychain; run explicitly with `cargo test -- --ignored`"]
fn delete_key_idempotent() {
    let (service, account) = unique_account("delete-idempotent");
    let value: [u8; 32] = [0xCC; 32];

    set_generic_password(&service, &account, &value).expect("set");

    // First delete: entry exists, should succeed cleanly.
    delete_generic_password(&service, &account).expect("first delete must succeed");

    // Second delete: entry already gone — wrapper-style idempotence is up to
    // the caller.  The primitive returns errSecItemNotFound; the daemon's
    // `delete_stored` propagates that.  Both behaviors are acceptable here;
    // what matters is that the error is the specific "not found" code so
    // callers can distinguish it from genuine keychain failures.
    match delete_generic_password(&service, &account) {
        Ok(()) => {
            // Some macOS versions / keychain configurations report success
            // even when the item is already gone — that is also fine.
        }
        Err(e) => assert_eq!(
            e.code(),
            ERR_SEC_ITEM_NOT_FOUND,
            "second delete must either succeed or return errSecItemNotFound, got code {}",
            e.code()
        ),
    }

    // And a get on a deleted item must now report not-found.
    let err = get_generic_password(&service, &account).expect_err("get after delete must fail");
    assert_eq!(err.code(), ERR_SEC_ITEM_NOT_FOUND);
}
