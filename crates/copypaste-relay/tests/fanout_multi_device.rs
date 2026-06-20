//! Regression tests for CopyPaste-xush.
//!
//! Relay fanout for N>2 devices: co-register 3+ tokens for the SAME
//! account-inbox `device_id` (R1a shared-account model) and assert that
//! each co-registered "virtual device" can read an item pushed by any one
//! of them.
//!
//! This tests the core delivery property of the relay protocol: the shared
//! account-inbox `device_id` acts as a fan-in/fan-out point. Any co-registered
//! token can push to the inbox AND every token can read from it.
//!
//! Additionally tests with DISTINCT device IDs (separate inboxes) to confirm
//! that fanout for separate devices requires the sender to push to each inbox
//! independently (the relay does NOT broadcast across device IDs).

#![allow(dead_code)]

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
#[path = "../src/state.rs"]
mod state;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use state::RelayStore;

/// The shared account-inbox UUID used by all "devices" in the shared-account tests.
const ACCOUNT_INBOX_ID: &str = "cccccccc-cccc-cccc-cccc-cccccccccccc";
const DEVICE_X: &str = "dddddddd-dddd-dddd-dddd-dddddddddddd";
const DEVICE_Y: &str = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee";
const DEVICE_Z: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}

/// All co-registrations of the same device_id use the same PoP.
fn shared_pop() -> String {
    B64.encode([0xCC_u8; 32])
}

fn pop_for(device: u8) -> String {
    B64.encode([device; 32])
}

fn make_store() -> RelayStore {
    RelayStore::new(3600)
}

// ---------------------------------------------------------------------------
// Shared-inbox fanout (3 co-registered tokens for the same device_id)
// ---------------------------------------------------------------------------

/// Register the same `device_id` with 3 distinct "device" tokens (R1a
/// co-registration). Push one item via token_1; verify tokens 2 and 3 can
/// both pull it. This is the CopyPaste-xush fanout invariant.
#[test]
fn three_co_registered_tokens_all_see_pushed_item_xush() {
    let mut store = make_store();

    // Co-register the same account-inbox ID three times. Each registration
    // returns a unique bearer token, all bound to the same inbox.
    let (token1, _) = store
        .register_device(
            ACCOUNT_INBOX_ID.to_string(),
            "Phone".to_string(),
            valid_pub_key(),
            shared_pop(),
        )
        .expect("co-register 1");

    let (token2, _) = store
        .register_device(
            ACCOUNT_INBOX_ID.to_string(),
            "Phone".to_string(), // same device_name for co-reg
            valid_pub_key(),
            shared_pop(), // same PoP required
        )
        .expect("co-register 2");

    let (token3, _) = store
        .register_device(
            ACCOUNT_INBOX_ID.to_string(),
            "Phone".to_string(),
            valid_pub_key(),
            shared_pop(),
        )
        .expect("co-register 3");

    // All three tokens must be distinct.
    assert_ne!(token1, token2, "tokens must be distinct");
    assert_ne!(token2, token3, "tokens must be distinct");
    assert_ne!(token1, token3, "tokens must be distinct");

    // Push one item via token_1 (after verifying it).
    store
        .verify_token(ACCOUNT_INBOX_ID, &token1)
        .expect("token1 authorizes push");
    let max_bytes = 10 * 1024 * 1024;
    let content = B64.encode(b"shared-blob");
    store
        .push_item(
            ACCOUNT_INBOX_ID,
            "text".to_string(),
            content.clone(),
            1000,
            max_bytes,
        )
        .expect("push via token1");

    // Pull via token_2 — must see the item.
    store
        .verify_token(ACCOUNT_INBOX_ID, &token2)
        .expect("token2 authorizes pull");
    let items2 = store
        .pull_items(ACCOUNT_INBOX_ID, 0, None, 100)
        .expect("pull via token2");
    assert_eq!(
        items2.len(),
        1,
        "token2 (co-registered) must see the item pushed by token1 (CopyPaste-xush)"
    );
    assert!(
        items2[0].content_b64.as_ref() == content,
        "content must match"
    );

    // Pull via token_3 — must also see the same item.
    store
        .verify_token(ACCOUNT_INBOX_ID, &token3)
        .expect("token3 authorizes pull");
    let items3 = store
        .pull_items(ACCOUNT_INBOX_ID, 0, None, 100)
        .expect("pull via token3");
    assert_eq!(
        items3.len(),
        1,
        "token3 (co-registered) must see the item pushed by token1 (CopyPaste-xush)"
    );
}

/// Co-registrations beyond 2 (a common path for N > 2 devices) works correctly.
/// Register 5 tokens; push one item; all 5 must be able to pull it.
#[test]
fn five_co_registered_tokens_all_see_pushed_item_xush() {
    let mut store = make_store();
    const N: usize = 5;

    let tokens: Vec<String> = (0..N)
        .map(|_| {
            store
                .register_device(
                    ACCOUNT_INBOX_ID.to_string(),
                    "MultiDevice".to_string(),
                    valid_pub_key(),
                    shared_pop(),
                )
                .expect("co-register")
                .0
        })
        .collect();

    // Verify all tokens are distinct.
    for i in 0..N {
        for j in (i + 1)..N {
            assert_ne!(tokens[i], tokens[j], "all tokens must be distinct");
        }
    }

    // Push via the first token.
    store.push_item(
        ACCOUNT_INBOX_ID,
        "text".to_string(),
        B64.encode(b"hello-5-devices"),
        2000,
        10 * 1024 * 1024,
    ).expect("push");

    // Every token must be able to verify and pull.
    for (i, tok) in tokens.iter().enumerate() {
        store
            .verify_token(ACCOUNT_INBOX_ID, tok)
            .unwrap_or_else(|e| panic!("token {i} must be valid: {e}"));
        let items = store
            .pull_items(ACCOUNT_INBOX_ID, 0, None, 100)
            .expect("pull");
        assert_eq!(
            items.len(),
            1,
            "token {i} must see the pushed item (CopyPaste-xush)"
        );
    }
}

// ---------------------------------------------------------------------------
// Distinct inboxes: each device has its own inbox
// ---------------------------------------------------------------------------

/// Three distinct device IDs each have their own inbox. A push to one inbox
/// must NOT appear in the others. The relay does not fan-out across device IDs.
#[test]
fn push_to_one_inbox_not_visible_in_others_xush() {
    let mut store = make_store();

    let (tok_x, _) = store
        .register_device(
            DEVICE_X.to_string(),
            "Dev X".to_string(),
            valid_pub_key(),
            pop_for(0xDD),
        )
        .expect("register X");
    let (tok_y, _) = store
        .register_device(
            DEVICE_Y.to_string(),
            "Dev Y".to_string(),
            valid_pub_key(),
            pop_for(0xEE),
        )
        .expect("register Y");
    let (tok_z, _) = store
        .register_device(
            DEVICE_Z.to_string(),
            "Dev Z".to_string(),
            valid_pub_key(),
            pop_for(0xFF),
        )
        .expect("register Z");

    // Push to X's inbox only.
    store
        .push_item(DEVICE_X, "text".to_string(), B64.encode(b"for-x-only"), 100, 10_000_000)
        .expect("push to X");

    // Y's and Z's inboxes must be empty.
    store.verify_token(DEVICE_Y, &tok_y).expect("tok_y valid");
    let y_items = store.pull_items(DEVICE_Y, 0, None, 100).expect("pull Y");
    assert!(y_items.is_empty(), "device Y inbox must be empty (only X was pushed to)");

    store.verify_token(DEVICE_Z, &tok_z).expect("tok_z valid");
    let z_items = store.pull_items(DEVICE_Z, 0, None, 100).expect("pull Z");
    assert!(z_items.is_empty(), "device Z inbox must be empty (only X was pushed to)");

    // X's inbox must have the item.
    store.verify_token(DEVICE_X, &tok_x).expect("tok_x valid");
    let x_items = store.pull_items(DEVICE_X, 0, None, 100).expect("pull X");
    assert_eq!(x_items.len(), 1, "device X inbox must have the item");
}

/// Push to each of 3 distinct inboxes independently. Each device sees only
/// its own item.
#[test]
fn each_of_three_inboxes_receives_its_own_blob_xush() {
    let mut store = make_store();

    let (_tok_x, _) = store
        .register_device(DEVICE_X.to_string(), "X".to_string(), valid_pub_key(), pop_for(0xDD))
        .expect("register X");
    let (_tok_y, _) = store
        .register_device(DEVICE_Y.to_string(), "Y".to_string(), valid_pub_key(), pop_for(0xEE))
        .expect("register Y");
    let (_tok_z, _) = store
        .register_device(DEVICE_Z.to_string(), "Z".to_string(), valid_pub_key(), pop_for(0xFF))
        .expect("register Z");

    // Push a distinct blob to each inbox.
    store.push_item(DEVICE_X, "text".to_string(), B64.encode(b"blob-x"), 100, 10_000_000).expect("push X");
    store.push_item(DEVICE_Y, "text".to_string(), B64.encode(b"blob-y"), 200, 10_000_000).expect("push Y");
    store.push_item(DEVICE_Z, "text".to_string(), B64.encode(b"blob-z"), 300, 10_000_000).expect("push Z");

    let x_items = store.pull_items(DEVICE_X, 0, None, 100).expect("pull X");
    let y_items = store.pull_items(DEVICE_Y, 0, None, 100).expect("pull Y");
    let z_items = store.pull_items(DEVICE_Z, 0, None, 100).expect("pull Z");

    assert_eq!(x_items.len(), 1);
    assert_eq!(y_items.len(), 1);
    assert_eq!(z_items.len(), 1);

    assert_eq!(x_items[0].content_b64.as_ref(), B64.encode(b"blob-x"), "X sees its blob");
    assert_eq!(y_items[0].content_b64.as_ref(), B64.encode(b"blob-y"), "Y sees its blob");
    assert_eq!(z_items[0].content_b64.as_ref(), B64.encode(b"blob-z"), "Z sees its blob");
}
