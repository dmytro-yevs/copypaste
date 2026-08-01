//! Fixtures the benches share.
//!
//! Every store here is a real SQLCipher **file**, not `open_in_memory`: the
//! daemon writes to disk, and an in-memory store measures a different thing.

// Each bench is its own crate and uses a subset of this module, so anything
// only one of them needs is dead code in the other two.
#![allow(dead_code)]

use std::path::Path;

use copypaste_core::{
    compute_content_hash, encrypt, Detector, Keyring, NewItem, Store, StoredItem,
};

/// Deterministic, so a re-run compares against the same key schedule.
pub const SECRET: [u8; 32] = [0x5a; 32];

/// Epoch stamp every fixture is dated from.
pub const T0: i64 = 1_700_000_000_000;

pub fn keyring() -> Keyring {
    Keyring::from_secret(&SECRET)
}

pub fn detector() -> Detector {
    Detector::new().expect("the ruleset must compile")
}

pub fn store_in(dir: &Path) -> Store {
    Store::open(&dir.join("bench-v2.db"), &keyring().db_key()).expect("store")
}

/// A clipping of `bytes` benign ASCII that no rule matches.
///
/// `seed` makes it unique, which is what keeps the dedup probe missing and the
/// insert doing real work.
pub fn clipping(bytes: usize, seed: usize) -> String {
    let head = format!("clipping {seed:016x} ");
    let mut text = String::with_capacity(bytes.max(head.len()));
    text.push_str(&head);
    while text.len() < bytes {
        text.push_str("the quick brown fox jumps over the lazy dog ");
    }
    text.truncate(bytes.max(head.len()));
    text
}

/// The row the ingest path would have written, without going through it.
pub fn row(keyring: &Keyring, text: &str, created_at: i64) -> NewItem {
    let id = uuid::Uuid::new_v4().to_string();
    let (nonce, ciphertext) = encrypt(text.as_bytes(), &keyring.item_key(), &id).expect("seal");
    NewItem {
        id,
        content_ciphertext: ciphertext,
        nonce,
        content_type: "text".to_string(),
        content_hash: compute_content_hash(text.as_bytes()),
        is_sensitive: false,
        search_text: Some(text.to_string()),
        created_at,
        app_bundle_id: None,
    }
}

/// Fill a store with `count` distinct rows of `bytes` each, newest last.
pub fn fill(store: &Store, keyring: &Keyring, count: usize, bytes: usize) {
    for n in 0..count {
        store
            .insert(row(keyring, &clipping(bytes, n), T0 + n as i64 * 1_000))
            .expect("insert");
    }
}

/// Decrypt a page the way `server::items::decrypt_rows` does.
pub fn decrypt_page(keyring: &Keyring, rows: &[StoredItem]) -> usize {
    let key = keyring.item_key();
    rows.iter()
        .filter_map(|r| copypaste_core::decrypt(&r.content_ciphertext, &r.nonce, &key, &r.id).ok())
        .map(|plain| plain.len())
        .sum()
}
