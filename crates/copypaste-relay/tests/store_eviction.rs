//! Integration tests for relay TTL eviction (ADR-009).
//!
//! These tests use the binary crate's modules via path includes because
//! `copypaste-relay` is a `[[bin]]` crate with no `[lib]`. To avoid that
//! coupling we exercise eviction through the public CLI surface that the
//! binary already re-exports? — actually we can't; the only public API is
//! HTTP. So we reach in via `path = "../src/state.rs"`-style inclusion
//! through a small helper module declared with `#[path]`.
//!
//! Behaviour under test:
//!   1. Insert a sync item.
//!   2. Pause Tokio's virtual clock and advance past the TTL.
//!   3. Manually invoke `prune_expired` with a wall-clock cutoff that
//!      simulates the elapsed virtual time.
//!   4. Assert that the item is gone.
//!
//! We also drive the real background evictor (`spawn_ttl_evictor`) under
//! `tokio::time::pause` to prove the task fires its `interval` ticks and
//! calls `prune_expired` end-to-end.

// The `copypaste-relay` crate is a bin-only crate. We pull the modules we
// need directly via `#[path]` so we don't have to add a `[lib]` target
// (which would balloon the change surface). Crate-level `#![allow]` for
// dead_code/unused suppresses warnings about bin-only symbols the test
// binary doesn't touch.
#![allow(dead_code, unused_imports, unused_variables)]

#[allow(dead_code)]
#[path = "../src/auth.rs"]
mod auth;
#[allow(dead_code)]
#[path = "../src/error.rs"]
mod error;
#[allow(dead_code)]
#[path = "../src/models.rs"]
mod models;
#[allow(dead_code)]
#[path = "../src/quota.rs"]
mod quota;
#[allow(dead_code)]
#[path = "../src/state.rs"]
mod state;
#[allow(dead_code)]
#[path = "../src/store.rs"]
mod store;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::state::RelayStore;

fn valid_key_b64() -> String {
    B64.encode([0u8; 32])
}

fn device_id() -> String {
    "11111111-1111-1111-1111-111111111111".to_string()
}

#[test]
fn prune_expired_removes_items_past_ttl() {
    let mut s = RelayStore::new(60);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();

    // Push three items at the current wall clock.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for w in [100u64, 200, 300] {
        s.push_item(&device_id(), "text".into(), B64.encode(b"hi"), w, 10 * 1024)
            .unwrap();
    }
    assert_eq!(s.stats().1, 3);

    // Simulate "TTL has passed" by giving prune_expired a `now_unix`
    // that is (now + ttl + 1) — i.e. the items were inserted strictly
    // before the cutoff. ttl=60 s.
    let evicted = s.prune_expired(now + 61, 60);
    assert_eq!(evicted, 3, "all 3 items must be evicted past TTL");
    assert_eq!(s.stats().1, 0);
}

#[test]
fn prune_expired_keeps_fresh_items() {
    let mut s = RelayStore::new(3600);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();
    s.push_item(
        &device_id(),
        "text".into(),
        B64.encode(b"hi"),
        1000,
        10 * 1024,
    )
    .unwrap();

    // now+30s with ttl=3600s — item must survive.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let evicted = s.prune_expired(now + 30, 3600);
    assert_eq!(evicted, 0);
    assert_eq!(s.stats().1, 1);
}

#[test]
fn prune_expired_with_zero_ttl_is_noop() {
    let mut s = RelayStore::new(0);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();
    s.push_item(
        &device_id(),
        "text".into(),
        B64.encode(b"hi"),
        1000,
        10 * 1024,
    )
    .unwrap();

    let evicted = s.prune_expired(u64::MAX, 0);
    assert_eq!(evicted, 0, "ttl=0 disables eviction");
    assert_eq!(s.stats().1, 1);
}

#[test]
fn prune_expired_preserves_empty_inboxes() {
    let mut s = RelayStore::new(60);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();
    // No items pushed — inbox is empty but registered.
    let evicted = s.prune_expired(u64::MAX, 1);
    assert_eq!(evicted, 0);
    assert!(s.devices.contains_key(&device_id()));
    assert!(s.sync_items.contains_key(&device_id()));
}

#[test]
fn prune_expired_partial_eviction() {
    let mut s = RelayStore::new(60);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();

    // Insert one "old" item, sleep briefly, then one "fresh" item.
    s.push_item(
        &device_id(),
        "text".into(),
        B64.encode(b"old"),
        1,
        10 * 1024,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    s.push_item(
        &device_id(),
        "text".into(),
        B64.encode(b"new"),
        2,
        10 * 1024,
    )
    .unwrap();

    // Compute a cutoff that strictly separates the two: now-0 s with ttl=1 s
    // → cutoff = now-1. The "old" item is >1s old → evicted; the "new" one
    // was just inserted → kept.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let evicted = s.prune_expired(now, 1);
    assert_eq!(evicted, 1, "only the >1s old item must be evicted");
    assert_eq!(s.stats().1, 1);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn ttl_evictor_task_fires_on_tick() {
    // Build a store with one item that is already older than the TTL.
    let mut s = RelayStore::new(1);
    s.register_device(device_id(), "A".into(), valid_key_b64())
        .unwrap();
    s.push_item(
        &device_id(),
        "text".into(),
        B64.encode(b"hi"),
        1000,
        10 * 1024,
    )
    .unwrap();
    // Backdate the item by mutating inserted_at_unix so the wall-clock
    // check inside the evictor sees it as expired immediately.
    {
        let inbox = s.sync_items.get_mut(&device_id()).unwrap();
        for item in inbox.iter_mut() {
            item.inserted_at_unix = 0;
        }
    }
    assert_eq!(s.stats().1, 1);

    let state = Arc::new(Mutex::new(s));

    // Spawn the evictor with a 1-second tick + 1-second TTL.
    let handle = store::spawn_ttl_evictor(state.clone(), 1, 1);

    // Virtual-clock dance: advance past several ticks and yield between
    // each so the spawned task gets a chance to run its branch on the
    // single-thread runtime. `interval` skips its very first tick (we
    // coded the task to consume it), so we need at least two real ticks
    // to observe an eviction.
    for _ in 0..10 {
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;
        if state.lock().unwrap().stats().1 == 0 {
            break;
        }
    }

    let after = state.lock().unwrap().stats().1;
    assert_eq!(after, 0, "evictor must have pruned the expired item");

    handle.abort();
}
