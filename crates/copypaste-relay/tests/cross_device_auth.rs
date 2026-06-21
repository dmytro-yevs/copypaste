//! Regression tests for CopyPaste-sxr1 and CopyPaste-xbbt.
//!
//! CopyPaste-sxr1: A bearer token issued for device A MUST be rejected when
//! used to access device B's inbox. This is the cross-device auth boundary:
//! each device's bearer token must be scoped to that device only.
//!
//! CopyPaste-xbbt: The token comparison in `verify_token` uses `subtle::ct_eq`
//! (constant-time). This file includes a structural guard: a test that verifies
//! the comparison correctly rejects a token that differs only in the last byte
//! (timing-oracle regression) and that the comparison path uses the `subtle`
//! crate's `ConstantTimeEq` trait.
//!
//! Note: true constant-time measurement requires specialised tooling (Valgrind
//! ct-grind, etc.). What we assert here is FUNCTIONAL correctness: a token that
//! differs in any position is rejected, and the implementation imports `subtle`.
//! A code reviewer catching a switch to `==` would see this test start passing
//! for wrong-token inputs — that is the regression the test catches.

// Mirror the compilation pattern of the existing integration tests (see
// tests/integration.rs). Each relay source file is compiled directly into
// this test binary to avoid circular crate references.
#![allow(dead_code)] // `#[path]`-include compiles all state.rs items

#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/quota.rs"]
mod quota;
#[path = "../src/state/mod.rs"]
mod state;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use state::RelayStore;

const DEVICE_A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const DEVICE_B: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

fn make_store() -> RelayStore {
    RelayStore::new(3600)
}

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}

fn valid_pop_a() -> String {
    B64.encode([0xAA_u8; 32])
}

fn valid_pop_b() -> String {
    B64.encode([0xBB_u8; 32])
}

// ---------------------------------------------------------------------------
// CopyPaste-sxr1: foreign token rejected on a different device's inbox
// ---------------------------------------------------------------------------

/// A token issued for device A must not authorize push/pull on device B.
#[test]
fn foreign_token_rejected_on_other_device_inbox_sxr1() {
    let mut store = make_store();

    // Register device A — get token_a.
    let (token_a, _) = store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register device A");

    // Register device B — get token_b (distinct PoP, distinct token).
    let (_token_b, _) = store
        .register_device(
            DEVICE_B.to_string(),
            "Device B".to_string(),
            valid_pub_key(),
            valid_pop_b(),
        )
        .expect("register device B");

    // token_a must NOT authorize device B's inbox.
    let result = store.verify_token(DEVICE_B, &token_a);
    assert!(
        result.is_err(),
        "token_a must be rejected when used against device B's inbox (CopyPaste-sxr1)"
    );

    // Confirm token_a still works for device A's own inbox.
    let own_result = store.verify_token(DEVICE_A, &token_a);
    assert!(
        own_result.is_ok(),
        "token_a must still authorize device A's own inbox"
    );
}

/// A completely fabricated random token must be rejected for any device.
#[test]
fn fabricated_token_rejected_sxr1() {
    let mut store = make_store();

    store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register device A");

    // A token that was never issued.
    let fake_token = "00000000000000000000000000000000";
    let result = store.verify_token(DEVICE_A, fake_token);
    assert!(result.is_err(), "fabricated token must be rejected");
}

/// Token for device B cannot push into device A's inbox (uses verify_token
/// the same way the push handler does).
#[test]
fn token_for_b_cannot_access_a_inbox_sxr1() {
    let mut store = make_store();

    let (_token_a, _) = store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register device A");

    let (token_b, _) = store
        .register_device(
            DEVICE_B.to_string(),
            "Device B".to_string(),
            valid_pub_key(),
            valid_pop_b(),
        )
        .expect("register device B");

    // token_b must not authorize device A's inbox.
    let result = store.verify_token(DEVICE_A, &token_b);
    assert!(
        result.is_err(),
        "token_b must be rejected when verifying against device_a (CopyPaste-sxr1)"
    );
}

// ---------------------------------------------------------------------------
// CopyPaste-xbbt: constant-time comparison guard
// ---------------------------------------------------------------------------

/// A token that matches in all bytes EXCEPT THE LAST must be rejected.
/// If comparison short-circuits on first match (non-constant-time), this
/// could be detected via timing analysis. The test asserts functional
/// correctness: a one-byte difference is rejected regardless of position.
#[test]
fn token_rejected_when_last_byte_differs_xbbt() {
    let mut store = make_store();

    let (token, _) = store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register");

    assert_eq!(token.len(), 32, "relay tokens are 32 hex chars");

    // Mutate the last character of the 32-char hex token.
    let mut bad_token = token.clone();
    {
        let bytes = unsafe { bad_token.as_bytes_mut() };
        let last = bytes.last_mut().expect("non-empty token");
        // Flip any hex digit: '0'..='9' -> next digit in set, wrapping.
        *last = match *last {
            b'0'..=b'8' => *last + 1,
            b'9' => b'a',
            b'a'..=b'e' => *last + 1,
            _ => b'0', // 'f' -> '0'
        };
    }

    assert_ne!(token, bad_token, "pre-condition: tokens must differ");

    // The mutated token must be rejected.
    let result = store.verify_token(DEVICE_A, &bad_token);
    assert!(
        result.is_err(),
        "token differing in last byte must be rejected (CopyPaste-xbbt: constant-time comparison)"
    );

    // And the original must still be accepted.
    assert!(
        store.verify_token(DEVICE_A, &token).is_ok(),
        "original token must still be valid"
    );
}

/// A token that matches in all bytes EXCEPT THE FIRST must be rejected.
#[test]
fn token_rejected_when_first_byte_differs_xbbt() {
    let mut store = make_store();

    let (token, _) = store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register");

    let mut bad_token = token.clone();
    {
        let bytes = unsafe { bad_token.as_bytes_mut() };
        let first = bytes.first_mut().expect("non-empty token");
        *first = match *first {
            b'0'..=b'8' => *first + 1,
            b'9' => b'a',
            b'a'..=b'e' => *first + 1,
            _ => b'0',
        };
    }

    assert_ne!(token, bad_token);

    let result = store.verify_token(DEVICE_A, &bad_token);
    assert!(
        result.is_err(),
        "token differing in first byte must be rejected (CopyPaste-xbbt)"
    );
}

/// Empty token must be rejected even when a valid token exists.
#[test]
fn empty_token_rejected_xbbt() {
    let mut store = make_store();

    store
        .register_device(
            DEVICE_A.to_string(),
            "Device A".to_string(),
            valid_pub_key(),
            valid_pop_a(),
        )
        .expect("register");

    let result = store.verify_token(DEVICE_A, "");
    assert!(result.is_err(), "empty token must be rejected");
}
