//! Beta-bonus tests for the Lamport logical clock and its role in total
//! ordering of clipboard items.
//!
//! These tests live in an integration-test file (separate compilation unit)
//! to exercise the public API of `copypaste-sync` the same way downstream
//! crates would. They complement the in-module unit tests in `src/clock.rs`
//! and `src/merge.rs` and intentionally focus on:
//!
//!   1. Saturation behavior at `u64::MAX` (no panic).
//!   2. Concurrent local increments from multiple async tasks.
//!   3. The `observe()` "max(local, remote) + 1" rule under varied inputs.
//!   4. The total order used by LWW conflict resolution (Lamport →
//!      wall time → device id tie-break).

use std::collections::HashSet;
use std::sync::Arc;

use copypaste_core::storage::items::ClipboardItem;
use copypaste_sync::protocol::WireItem;
use copypaste_sync::{resolve, LamportClock, MergeOutcome};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// 1. Saturation
// ---------------------------------------------------------------------------

#[test]
fn saturating_increment_at_u64_max() {
    let mut c = LamportClock::from_value(u64::MAX);

    // First tick at saturation: must NOT panic, must remain at u64::MAX.
    let v1 = c.tick();
    assert_eq!(v1, u64::MAX, "tick at u64::MAX must stay at u64::MAX");
    assert_eq!(c.get(), u64::MAX);

    // Repeated ticks remain at MAX (idempotent at saturation).
    for _ in 0..1_000 {
        assert_eq!(c.tick(), u64::MAX);
    }
    assert_eq!(c.get(), u64::MAX);

    // observe() at saturation must also be a no-op rather than overflow.
    assert_eq!(c.observe(0), u64::MAX);
    assert_eq!(c.observe(u64::MAX), u64::MAX);
    assert_eq!(c.observe(u64::MAX - 1), u64::MAX);
}

// ---------------------------------------------------------------------------
// 2. Concurrent ticks from multiple tokio tasks
// ---------------------------------------------------------------------------

/// `LamportClock` is documented as *not* thread-safe by itself, so concurrent
/// callers must wrap it in a `Mutex`. Under that contract, every `tick()`
/// call is serialized and the returned values must form a strictly
/// monotonically increasing sequence — therefore unique.
///
/// We spawn 10 tasks, each calling `tick()` 100 times (1000 total), collect
/// the returned values into a `HashSet`, and assert:
///   * uniqueness (no two ticks returned the same value),
///   * the final clock value equals exactly the total number of ticks,
///   * the values are exactly the set `{1, 2, ..., 1000}`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_increment_from_multiple_tasks_yields_unique_values() {
    const TASKS: u64 = 10;
    const TICKS_PER_TASK: u64 = 100;
    const TOTAL: u64 = TASKS * TICKS_PER_TASK;

    let clock = Arc::new(Mutex::new(LamportClock::new()));
    let mut handles = Vec::with_capacity(TASKS as usize);

    for _ in 0..TASKS {
        let clock = Arc::clone(&clock);
        handles.push(tokio::spawn(async move {
            let mut local = Vec::with_capacity(TICKS_PER_TASK as usize);
            for _ in 0..TICKS_PER_TASK {
                let v = {
                    let mut guard = clock.lock().await;
                    guard.tick()
                };
                local.push(v);
            }
            local
        }));
    }

    let mut all_values: HashSet<u64> = HashSet::with_capacity(TOTAL as usize);
    for h in handles {
        let local = h.await.expect("task panicked");
        for v in local {
            assert!(
                all_values.insert(v),
                "duplicate tick value {} returned to concurrent callers",
                v
            );
        }
    }

    assert_eq!(
        all_values.len(),
        TOTAL as usize,
        "expected {TOTAL} unique tick values, got {}",
        all_values.len()
    );

    // Final clock value equals the total number of ticks performed.
    assert_eq!(clock.lock().await.get(), TOTAL);

    // The set is exactly {1, 2, ..., TOTAL}.
    for v in 1..=TOTAL {
        assert!(all_values.contains(&v), "missing tick value {}", v);
    }
}

// ---------------------------------------------------------------------------
// 3. observe() — Lamport receive rule
// ---------------------------------------------------------------------------

#[test]
fn merge_observed_remote_clock_takes_max_plus_one() {
    // local < remote → result = remote + 1
    let mut c = LamportClock::from_value(3);
    assert_eq!(c.observe(10), 11);
    assert_eq!(c.get(), 11);

    // local > remote → result = local + 1
    let mut c = LamportClock::from_value(100);
    assert_eq!(c.observe(7), 101);
    assert_eq!(c.get(), 101);

    // local == remote → result = either + 1
    let mut c = LamportClock::from_value(42);
    assert_eq!(c.observe(42), 43);
    assert_eq!(c.get(), 43);

    // Fresh clock observing remote = R → R + 1.
    let mut c = LamportClock::new();
    assert_eq!(c.observe(999), 1000);

    // Observing remote = 0 from a non-zero local clock still increments local.
    let mut c = LamportClock::from_value(5);
    assert_eq!(c.observe(0), 6);
}

// ---------------------------------------------------------------------------
// 4. Total order — (lamport, wall_time, device_id) tie-break
// ---------------------------------------------------------------------------
//
// `copypaste-sync` does not expose a standalone `total_order` function;
// the canonical place where Lamport ordering plus tie-break is applied is
// `merge::resolve`, which decides the LWW outcome for two competing
// versions of the same clipboard item. The contract is:
//   1. Higher `lamport_ts` wins.
//   2. On equal Lamport, higher `wall_time` wins.
//   3. On equal Lamport and wall_time, larger `origin_device_id` wins
//      (compared as a string on both sides as of schema v3; previously the
//      remote side was compared against `local.id` (the row UUID) — the
//      merge.rs:39 BUG).
//
// The tests below pin that ordering.

fn make_local(id: &str, lamport: i64, wall: i64) -> ClipboardItem {
    ClipboardItem {
        id: id.to_string(),
        item_id: format!("iid-{id}"),
        content_type: "text".to_string(),
        content: Some(b"local-payload".to_vec()),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        // Pinned so the device-id tie-break has a known reference string.
        origin_device_id: "device-local".to_string(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

fn make_remote(id: &str, lamport: i64, wall: i64, device_id: &str) -> WireItem {
    WireItem {
        id: id.to_string(),
        item_id: format!("iid-{id}"),
        content_type: "text".to_string(),
        content: Some(b"remote-payload".to_vec()),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: device_id.to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    }
}

#[test]
fn ordering_total_order_via_clock_and_node_id() {
    // (a) Lamport clock dominates — even if remote wall time is lower.
    let local = make_local("item-x", 5, 9_999);
    let remote = make_remote("item-x", 10, 1, "peer-A");
    assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);

    // And the reverse: local Lamport higher → keep local.
    let local = make_local("item-x", 20, 0);
    let remote = make_remote("item-x", 1, i64::MAX, "peer-Z");
    assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);

    // (b) Equal Lamport → wall time decides.
    let local = make_local("item-x", 7, 100);
    let remote = make_remote("item-x", 7, 200, "peer-A");
    assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);

    let local = make_local("item-x", 7, 500);
    let remote = make_remote("item-x", 7, 100, "peer-A");
    assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);

    // (c) Equal Lamport + equal wall time → device id tie-break.
    //
    // As of schema v3, `resolve` compares `remote.origin_device_id` against
    // `local.origin_device_id` (not against the row UUID — that was the
    // merge.rs:39 BUG). `make_local` stamps `origin_device_id =
    // "device-local"`, so we pick remote ids on either side of that string
    // to exercise both branches deterministically.
    let local = make_local("item-x", 7, 100);
    let remote = make_remote("item-x", 7, 100, "zzz"); // "zzz" > "device-local"
    assert_eq!(
        resolve(&local, &remote),
        MergeOutcome::TakeRemote,
        "larger device id must win the final tie-break"
    );

    let local = make_local("item-x", 7, 100);
    let remote = make_remote("item-x", 7, 100, "aaa"); // "aaa" < "device-local"
    assert_eq!(
        resolve(&local, &remote),
        MergeOutcome::KeepLocal,
        "smaller remote device id must lose the final tie-break"
    );

    // (d) Exact equality (same device id on both sides) → KeepLocal,
    // because the strict `>` comparison in the tie-break is false.
    let local = make_local("item-x", 7, 100);
    let remote = make_remote("item-x", 7, 100, "device-local");
    assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
}
