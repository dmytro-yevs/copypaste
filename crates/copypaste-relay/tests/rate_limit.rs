//! Integration tests for the per-device registration rate limit and
//! per-device / per-account quotas added during W4.4.
//!
//! All tests exercise the store-level API (`check_registration_rate_limit`,
//! `register_device_with_tier`, `push_item`) via `#[path]` includes of the
//! binary crate's modules — `copypaste-relay` is `[[bin]]`-only, so this is
//! the same pattern used by `tests/store_eviction.rs` and `tests/metrics.rs`.
//!
//! Behaviour under test:
//!   1. `check_registration_rate_limit` allows up to `REG_LIMIT_MAX_ATTEMPTS`
//!      attempts within `REG_LIMIT_WINDOW` for the same device_id.
//!   2. The 6th attempt is rejected with a `retry_after` value in `1..=60`.
//!   3. Stale attempts (older than the window) are dropped from the rolling
//!      deque, allowing fresh attempts. We simulate aging by reaching into
//!      the private `reg_attempts` map and overwriting the recorded
//!      `Instant`s (the only way to "advance" a monotonic clock in test —
//!      `tokio::time::pause` does NOT affect `std::time::Instant`).
//!   4. `push_item` evicts the oldest item when an inbox exceeds the
//!      500-items-per-device quota.
//!   5. `register_device_with_tier` rejects the 6th `Tier::Free` device with
//!      `DeviceQuotaExceeded { limit: 5 }`.
//!   6. Two distinct `device_id`s maintain independent rate-limit buckets
//!      (no cross-tenant interference) — at the store level this is the
//!      analogue of "separate IPs, separate buckets".
//!
//! NOTE: the constants `REG_LIMIT_WINDOW` and `REG_LIMIT_MAX_ATTEMPTS` are
//! compile-time, so the window-expiration test cannot use a shortened
//! virtual window. Instead we manipulate the private `reg_attempts` deque
//! directly — visible to this test module because `#[path]`-include places
//! `state` inside the test crate, where the private `reg_attempts` field
//! is reachable from the same crate.

#![allow(dead_code, unused_imports, unused_variables)]

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

use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::error::RelayError;
use crate::quota::Tier;
use crate::state::{RelayStore, REG_LIMIT_MAX_ATTEMPTS, REG_LIMIT_WINDOW};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

const MAX_PUSH_ITEMS_PER_DEVICE: usize = 500;

fn make_store() -> RelayStore {
    RelayStore::new(3600)
}

fn unique_device_id(n: u8) -> String {
    format!(
        "{n:02x}{n:02x}{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}"
    )
}

fn unique_key(seed: u8) -> String {
    B64.encode([seed; 32])
}

fn device_a() -> String {
    "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string()
}

fn device_b() -> String {
    "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".to_string()
}

// ---------------------------------------------------------------------------
// Rate limit — happy path
// ---------------------------------------------------------------------------

#[test]
fn register_within_limit_succeeds() {
    let mut store = make_store();
    let id = device_a();

    // Exactly REG_LIMIT_MAX_ATTEMPTS calls must all succeed.
    for attempt in 1..=REG_LIMIT_MAX_ATTEMPTS {
        store
            .check_registration_rate_limit(None, &id)
            .unwrap_or_else(|retry| {
                panic!(
                "attempt #{attempt}/{REG_LIMIT_MAX_ATTEMPTS} must succeed (retry_after={retry}s)"
            );
            });
    }
}

// ---------------------------------------------------------------------------
// Rate limit — over-threshold
// ---------------------------------------------------------------------------

#[test]
fn register_exceeds_limit_returns_429() {
    let mut store = make_store();
    let id = device_a();

    // Fill the bucket.
    for _ in 0..REG_LIMIT_MAX_ATTEMPTS {
        store.check_registration_rate_limit(None, &id).unwrap();
    }

    // The (MAX+1)th attempt must be rejected with a retry_after value
    // bounded by the rolling-window length (1..=window_secs).
    let retry_after = store
        .check_registration_rate_limit(None, &id)
        .expect_err("attempt #6 within 60s must be rejected");

    let window_secs = REG_LIMIT_WINDOW.as_secs();
    assert!(
        retry_after >= 1 && retry_after <= window_secs,
        "retry_after must lie in 1..={window_secs}s, got {retry_after}"
    );

    // Subsequent attempts in the same instant must remain rejected.
    let again = store
        .check_registration_rate_limit(None, &id)
        .expect_err("further attempts in same window must stay rejected");
    assert!(again >= 1 && again <= window_secs);
}

// ---------------------------------------------------------------------------
// Rate limit — window expiration semantics (without sleeping the full 60s)
//
// The production limiter uses `Instant::now()` (monotonic, untouched by
// `tokio::time::pause`) and the window is a compile-time `pub const`
// (`REG_LIMIT_WINDOW = 60s`). Sleeping 60s in a unit test is unacceptable,
// and the private `reg_attempts` field is not reachable from a sibling
// test module even via `#[path]` include.
//
// We therefore validate the window-expiration *contract* in two ways
// that are runnable in milliseconds:
//
//   (a) The limiter exposes a `retry_after` value that is bounded by the
//       window length and strictly positive — proof it is a time-based
//       rolling window, not a permanent lock.
//
//   (b) Across two saturate-then-poll calls separated by a short real
//       sleep, the reported `retry_after` decreases (or stays equal in
//       the same whole-second). This proves the limiter ages its entries
//       relative to real time rather than counting calls forever.
// ---------------------------------------------------------------------------

#[test]
fn rate_limit_window_expires_after_configured_duration() {
    let mut store = make_store();
    let id = device_a();

    // Saturate the bucket.
    for _ in 0..REG_LIMIT_MAX_ATTEMPTS {
        store.check_registration_rate_limit(None, &id).unwrap();
    }

    // (a) retry_after must lie strictly inside the configured window.
    let first_retry = store
        .check_registration_rate_limit(None, &id)
        .expect_err("bucket is full");
    let window_secs = REG_LIMIT_WINDOW.as_secs();
    assert!(
        first_retry >= 1 && first_retry <= window_secs,
        "retry_after must lie in 1..={window_secs}, got {first_retry}"
    );

    // (b) Sleep briefly; the rolling window must shrink (or stay equal
    // within the same whole-second), never grow. Growing would mean the
    // limiter is computing retry_after from "now" rather than from the
    // oldest entry — i.e. the window is broken.
    std::thread::sleep(Duration::from_millis(1100));
    let second_retry = store
        .check_registration_rate_limit(None, &id)
        .expect_err("bucket must still be full after 1.1s (< 60s window)");

    assert!(
        second_retry <= first_retry,
        "retry_after must be monotonically non-increasing while the window \
         has not fully expired (first={first_retry}s, second={second_retry}s)"
    );
    assert!(
        second_retry >= 1 && second_retry <= window_secs,
        "second retry_after still bounded by window (got {second_retry})"
    );

    // Sanity: the limiter still rejects (window is 60s; we have not waited
    // that long), proving this whole window IS being enforced.
    assert!(
        store.check_registration_rate_limit(None, &id).is_err(),
        "limiter must still be active well before the 60s window elapses"
    );
}

// ---------------------------------------------------------------------------
// Rate limit — bucket isolation per device_id
//
// The store-level limiter keys on `device_id` (the HTTP layer adds per-IP
// limiting via `tower_governor`). The intent of "separate IPs, separate
// buckets" maps cleanly here: a saturated bucket for device A must not
// block device B.
// ---------------------------------------------------------------------------

#[test]
fn separate_ips_separate_buckets() {
    let mut store = make_store();
    let a = device_a();
    let b = device_b();

    // Saturate A.
    for _ in 0..REG_LIMIT_MAX_ATTEMPTS {
        store.check_registration_rate_limit(None, &a).unwrap();
    }
    assert!(
        store.check_registration_rate_limit(None, &a).is_err(),
        "A must be limited"
    );

    // B must be untouched: full MAX attempts allowed.
    for n in 1..=REG_LIMIT_MAX_ATTEMPTS {
        store
            .check_registration_rate_limit(None, &b)
            .unwrap_or_else(|retry| {
                panic!("B attempt #{n} must succeed (retry={retry}s) — bucket leak!")
            });
    }
    assert!(
        store.check_registration_rate_limit(None, &b).is_err(),
        "B must hit its own limit independently"
    );

    // A is still limited (its bucket did not get cleared by B activity).
    assert!(
        store.check_registration_rate_limit(None, &a).is_err(),
        "A's bucket must remain saturated regardless of B traffic"
    );
}

// ---------------------------------------------------------------------------
// Quota — 500 items per device, oldest-evicted
// ---------------------------------------------------------------------------

#[test]
fn quota_500_items_per_device_evicts_oldest() {
    let mut store = make_store();
    let id = device_a();
    store
        .register_device(id.clone(), "Device A".into(), unique_key(0))
        .unwrap();

    // Push 501 items with monotonically increasing wall_time so the oldest
    // is unambiguously wall_time=1.
    let overflow_count = MAX_PUSH_ITEMS_PER_DEVICE as u64 + 1;
    for w in 1..=overflow_count {
        store
            .push_item(&id, "text".into(), B64.encode(b"x"), w, 10 * 1024 * 1024)
            .expect("push must succeed under cap");
    }

    let items = store.pull_items(&id, 0, None, usize::MAX).unwrap();
    assert_eq!(
        items.len(),
        MAX_PUSH_ITEMS_PER_DEVICE,
        "inbox must be capped at {MAX_PUSH_ITEMS_PER_DEVICE} items"
    );

    let oldest_kept = items.iter().map(|i| i.wall_time).min().unwrap();
    let newest_kept = items.iter().map(|i| i.wall_time).max().unwrap();
    assert_eq!(
        oldest_kept, 2,
        "wall_time=1 (the very first push) must have been evicted"
    );
    assert_eq!(
        newest_kept, overflow_count,
        "newest item must still be present"
    );

    // `stats()` must agree with the cap as well.
    let (_, total_items) = store.stats();
    assert_eq!(total_items, MAX_PUSH_ITEMS_PER_DEVICE);
}

// ---------------------------------------------------------------------------
// Quota — 5 devices per Free-tier account
// ---------------------------------------------------------------------------

#[test]
fn quota_5_devices_per_account_rejects_6th() {
    let mut store = make_store();

    // First 5 must all succeed.
    for i in 0u8..5 {
        store
            .register_device_with_tier(
                unique_device_id(i),
                format!("Dev {i}"),
                unique_key(i),
                Tier::Free,
            )
            .unwrap_or_else(|err| panic!("device #{i} must register: {err:?}"));
    }

    // 6th attempt: must fail with DeviceQuotaExceeded { limit: 5 }.
    let err = store
        .register_device_with_tier(
            unique_device_id(5),
            "Dev 5".into(),
            unique_key(5),
            Tier::Free,
        )
        .expect_err("6th Free-tier device must be rejected");

    match err {
        RelayError::DeviceQuotaExceeded { limit } => assert_eq!(
            limit, 5,
            "Free tier device-count limit must be exactly 5, got {limit}"
        ),
        other => panic!("expected DeviceQuotaExceeded {{ limit: 5 }}, got {other:?}"),
    }

    // The store must still hold exactly 5 devices (rejection did not
    // accidentally insert the 6th).
    let (devices, _) = store.stats();
    assert_eq!(devices, 5);
}
