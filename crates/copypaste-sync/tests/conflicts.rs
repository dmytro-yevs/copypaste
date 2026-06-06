//! Integration tests for sync conflict resolution edge cases.
//!
//! Complements `tests/lamport.rs` (clock-focused) and the in-module unit tests
//! in `src/merge.rs`. These tests focus on the *boundaries* of LWW resolution:
//!
//!   1. Simultaneous edits with same logical id but different Lamport values.
//!   2. Equal-Lamport ties broken by wall time.
//!   3. Equal Lamport + equal wall time → device-id lex order (deterministic
//!      drift guard: both sides compare `origin_device_id`, larger wins).
//!   4. Three-way re-application idempotence (A → B → A produces no flap).
//!   5. Concurrent delete-vs-update LWW determinism.
//!
//! All tests use the public API (`resolve`, `MergeOutcome`, `WireItem`).

use copypaste_core::storage::items::ClipboardItem;
use copypaste_sync::protocol::WireItem;
use copypaste_sync::{resolve, MergeOutcome};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Build a local item with a given id, lamport, wall time, and content.
/// `origin_device_id` is stamped to "device-local" so tie-break tests have a
/// known string to compare against (e.g. "zzz" > "device-local").
fn local_item(id: &str, lamport: i64, wall: i64, content: &[u8]) -> ClipboardItem {
    ClipboardItem {
        deleted: false,
        id: id.to_string(),
        item_id: format!("iid-{id}"),
        content_type: "text".to_string(),
        content: Some(content.to_vec()),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: "device-local".to_string(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

/// Build a wire item with the same id (same logical item) but possibly
/// different lamport / wall / device / payload.
fn wire_item(
    id: &str,
    lamport: i64,
    wall: i64,
    origin_device_id: &str,
    content: Option<Vec<u8>>,
) -> WireItem {
    WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: id.to_string(),
        item_id: format!("iid-{id}"),
        content_type: "text".to_string(),
        content,
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: origin_device_id.to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    }
}

/// Apply a merge outcome: returns the "effective" item after resolution.
/// Mirrors what the engine would persist (no DB I/O).
fn apply(local: ClipboardItem, remote: &WireItem) -> ClipboardItem {
    match resolve(&local, remote) {
        MergeOutcome::KeepLocal => local,
        MergeOutcome::TakeRemote => ClipboardItem {
            deleted: false,
            id: remote.id.clone(),
            item_id: remote.item_id.clone(),
            content_type: remote.content_type.clone(),
            content: remote.content.clone(),
            content_nonce: remote.content_nonce.clone(),
            blob_ref: remote.blob_ref.clone(),
            is_sensitive: remote.is_sensitive,
            is_synced: true,
            lamport_ts: remote.lamport_ts,
            wall_time: remote.wall_time,
            expires_at: remote.expires_at,
            app_bundle_id: remote.app_bundle_id.clone(),
            content_hash: None,
            origin_device_id: remote.origin_device_id.clone(),
            key_version: 1,
            pinned: false,
            pin_order: None,
            thumb: None,
        },
    }
}

// ---------------------------------------------------------------------------
// 1. Same id, different Lamport → higher Lamport wins regardless of wall time
// ---------------------------------------------------------------------------

#[test]
fn simultaneous_edit_same_id_higher_lamport_wins() {
    // Local was written at lamport=4 "later" wall time.
    let local = local_item("item-X", 4, 9_000, b"local-v1");
    // Remote was written at lamport=7 with an *earlier* wall time.
    // Lamport must dominate — remote wins.
    let remote = wire_item("item-X", 7, 100, "peer-A", Some(b"remote-v2".to_vec()));

    assert_eq!(
        resolve(&local, &remote),
        MergeOutcome::TakeRemote,
        "Lamport=7 must beat Lamport=4 regardless of wall_time"
    );

    // Symmetric: lower remote Lamport must lose even with newer wall time.
    let local2 = local_item("item-X", 20, 100, b"local-v3");
    let remote2 = wire_item(
        "item-X",
        5,
        9_999_999,
        "peer-B",
        Some(b"remote-v4".to_vec()),
    );
    assert_eq!(
        resolve(&local2, &remote2),
        MergeOutcome::KeepLocal,
        "Lamport=5 must lose to Lamport=20 even with much newer wall_time"
    );
}

// ---------------------------------------------------------------------------
// 2. Equal Lamport → wall time decides
// ---------------------------------------------------------------------------

#[test]
fn equal_lamport_uses_wall_time_tie_break() {
    let local = local_item("item-Y", 10, 500, b"local");
    let remote_newer = wire_item("item-Y", 10, 2_000, "peer-A", Some(b"remote".to_vec()));
    assert_eq!(
        resolve(&local, &remote_newer),
        MergeOutcome::TakeRemote,
        "equal lamport, newer wall time → remote wins"
    );

    let remote_older = wire_item("item-Y", 10, 100, "peer-A", Some(b"remote".to_vec()));
    assert_eq!(
        resolve(&local, &remote_older),
        MergeOutcome::KeepLocal,
        "equal lamport, older wall time → keep local"
    );
}

// ---------------------------------------------------------------------------
// 3. Equal lamport + equal wall → device-id lex order (deterministic tie-break)
// ---------------------------------------------------------------------------

/// Regression test for the device-id tie-break in `src/merge.rs`.
///
/// When both Lamport timestamp and wall time are equal, the winner is
/// determined by comparing `remote.origin_device_id` against
/// `local.origin_device_id` lexicographically — the LARGER device id wins.
///
/// The local fixture has `origin_device_id = "device-local"`. Probes are
/// chosen to straddle that value so the test verifies both directions of the
/// comparison using the *same* field on both sides.
///
/// Historical note: a prior implementation compared `remote.origin_device_id`
/// against `local.id` (the row UUID — a completely different identifier space).
/// That made the tie-break non-deterministic across peers because row UUIDs are
/// random per-write while device ids are stable per-peer. This test locks in
/// the correct `origin_device_id` vs `origin_device_id` comparison.
#[test]
fn equal_lamport_and_wall_time_uses_device_id_lex_order_drift_guard() {
    // local.origin_device_id == "device-local".
    let local = local_item("item-001", 5, 1_000, b"local");

    // "zzz" > "device-local" → remote wins.
    let remote_above = wire_item("item-001", 5, 1_000, "zzz", Some(b"r".to_vec()));
    assert_eq!(
        resolve(&local, &remote_above),
        MergeOutcome::TakeRemote,
        "'zzz' > 'device-local' → remote origin_device_id wins"
    );

    // "aaa" < "device-local" → local keeps.
    let remote_below = wire_item("item-001", 5, 1_000, "aaa", Some(b"r".to_vec()));
    assert_eq!(
        resolve(&local, &remote_below),
        MergeOutcome::KeepLocal,
        "'aaa' < 'device-local' → local origin_device_id wins"
    );

    // "peer-A" > "device-local" ('p' > 'd') → remote wins.
    let remote_realistic = wire_item("item-001", 5, 1_000, "peer-A", Some(b"r".to_vec()));
    assert_eq!(
        resolve(&local, &remote_realistic),
        MergeOutcome::TakeRemote,
        "'peer-A' > 'device-local' → remote wins (correct device-id comparison)"
    );

    // "device-A" < "device-local" ('A' < 'l' at byte 7) → local keeps.
    let remote_realistic_lo = wire_item("item-001", 5, 1_000, "device-A", Some(b"r".to_vec()));
    assert_eq!(
        resolve(&local, &remote_realistic_lo),
        MergeOutcome::KeepLocal,
        "'device-A' < 'device-local' → local wins (correct device-id comparison)"
    );

    // Equal device ids → KeepLocal (strict `>`, not `>=`).
    let remote_same = wire_item("item-001", 5, 1_000, "device-local", Some(b"r".to_vec()));
    assert_eq!(
        resolve(&local, &remote_same),
        MergeOutcome::KeepLocal,
        "identical origin_device_id → KeepLocal (strict greater-than)"
    );
}

// ---------------------------------------------------------------------------
// 4. Re-applying older updates must not flap (idempotence / no oscillation).
// ---------------------------------------------------------------------------

#[test]
fn three_way_merge_no_oscillation() {
    // Scenario: receive A (lamport=10), then B (lamport=15), then A again.
    // After step 2, local has B. Re-applying A must NOT revert to A.
    let initial = local_item("item-Z", 1, 100, b"v0");

    let update_a = wire_item("item-Z", 10, 500, "peer-A", Some(b"vA".to_vec()));
    let after_a = apply(initial.clone(), &update_a);
    assert_eq!(after_a.lamport_ts, 10);
    assert_eq!(after_a.content.as_deref(), Some(b"vA".as_ref()));

    let update_b = wire_item("item-Z", 15, 700, "peer-B", Some(b"vB".to_vec()));
    let after_b = apply(after_a.clone(), &update_b);
    assert_eq!(after_b.lamport_ts, 15);
    assert_eq!(after_b.content.as_deref(), Some(b"vB".as_ref()));

    // Re-deliver A out of order — must be ignored, no flap.
    let after_b_then_a = apply(after_b.clone(), &update_a);
    assert_eq!(
        after_b_then_a.lamport_ts, 15,
        "re-applying older Lamport=10 must NOT overwrite Lamport=15"
    );
    assert_eq!(
        after_b_then_a.content.as_deref(),
        Some(b"vB".as_ref()),
        "content must remain vB after late-delivered vA"
    );

    // Idempotence: applying B again is a no-op.
    let after_b_twice = apply(after_b_then_a.clone(), &update_b);
    assert_eq!(after_b_twice.lamport_ts, after_b_then_a.lamport_ts);
    assert_eq!(after_b_twice.content, after_b_then_a.content);
    assert_eq!(after_b_twice.wall_time, after_b_then_a.wall_time);
}

// ---------------------------------------------------------------------------
// 5. Concurrent delete-vs-update — LWW resolves deterministically.
//
// The wire protocol represents a "delete" as a `WireItem` with `content=None`
// and `blob_ref=None` (tombstone). Either side of a delete-vs-update conflict
// must resolve the same way on both peers given the same inputs.
// ---------------------------------------------------------------------------

#[test]
fn delete_concurrent_with_update_lww_resolves_deterministically() {
    // Case A: tombstone with HIGHER lamport beats update.
    let local_update = local_item("item-D", 5, 1_000, b"updated");
    let remote_tombstone = wire_item("item-D", 9, 500, "peer-A", None);

    let outcome_a = resolve(&local_update, &remote_tombstone);
    assert_eq!(
        outcome_a,
        MergeOutcome::TakeRemote,
        "tombstone with higher Lamport must win → item gets deleted"
    );
    let after_a = apply(local_update.clone(), &remote_tombstone);
    assert!(
        after_a.content.is_none() && after_a.blob_ref.is_none(),
        "after applying tombstone, content must be cleared"
    );

    // Case B: update with HIGHER lamport beats older tombstone.
    let local_tombstone = ClipboardItem {
        deleted: false,
        content: None,
        blob_ref: None,
        ..local_item("item-D", 3, 1_000, b"")
    };
    let remote_update = wire_item("item-D", 12, 2_000, "peer-B", Some(b"reborn".to_vec()));
    let outcome_b = resolve(&local_tombstone, &remote_update);
    assert_eq!(
        outcome_b,
        MergeOutcome::TakeRemote,
        "newer Lamport update must overwrite older tombstone"
    );
    let after_b = apply(local_tombstone, &remote_update);
    assert_eq!(after_b.content.as_deref(), Some(b"reborn".as_ref()));

    // Case C: determinism — same inputs on both peers must give same outcome.
    // Simulate "peer 1's view" (local=update, remote=tombstone) and
    // "peer 2's view" (local=tombstone, remote=update) for the *same* logical
    // edit pair. Both peers must converge to the higher-Lamport state.
    let p1_local = local_item("item-E", 5, 1_000, b"upd");
    let p1_remote = wire_item("item-E", 8, 1_500, "peer-Y", None);
    let p1_final = apply(p1_local, &p1_remote);

    let p2_local = ClipboardItem {
        deleted: false,
        content: None,
        blob_ref: None,
        ..local_item("item-E", 8, 1_500, b"")
    };
    let p2_remote = wire_item("item-E", 5, 1_000, "peer-X", Some(b"upd".to_vec()));
    let p2_final = apply(p2_local, &p2_remote);

    assert_eq!(
        p1_final.lamport_ts, p2_final.lamport_ts,
        "both peers must converge to the same Lamport value"
    );
    assert_eq!(
        p1_final.content, p2_final.content,
        "both peers must converge to the same content (tombstone)"
    );
    assert!(
        p1_final.content.is_none(),
        "deterministic convergence: tombstone (higher Lamport) wins on both sides"
    );
}
