//! CRDT convergence / idempotency / commutativity properties for the sync layer.
//!
//! The sync engine treats the per-item LWW merge (`merge::resolve`) plus the
//! Lamport clock (`clock::LamportClock`) as a de-facto state-based CRDT keyed
//! by `WireItem::id`. The merge function is total, deterministic and
//! tie-broken on (lamport_ts, wall_time, origin_device_id), so applying the
//! same multiset of ops in any order must converge to the same state, and
//! re-applying the same op must be a no-op.
//!
//! These tests exercise those algebraic properties as an integration test
//! (they live outside `src/` so they only touch the public API exported by
//! `copypaste-sync::lib`).

use copypaste_core::storage::items::ClipboardItem;
use copypaste_sync::{local_to_wire, resolve, wire_to_local, LamportClock, MergeOutcome, WireItem};
use std::collections::HashMap;

// --- helpers ---------------------------------------------------------------

fn mk_local(id: &str, lamport: i64, wall: i64, payload: u8) -> ClipboardItem {
    // Default device id "device-local" so tie-break tests have a stable,
    // known string to compare against (e.g. "zzz" > "device-local").
    mk_local_with_device(id, lamport, wall, payload, "device-local")
}

fn mk_local_with_device(
    id: &str,
    lamport: i64,
    wall: i64,
    payload: u8,
    device_id: &str,
) -> ClipboardItem {
    ClipboardItem {
        id: id.to_string().into(),
        item_id: format!("iid-{id}").into(),
        content_type: "text".to_string(),
        content: Some(vec![payload]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: device_id.to_string(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
        deleted: false,
    }
}

fn mk_wire(id: &str, lamport: i64, wall: i64, device: &str, payload: u8) -> WireItem {
    WireItem {
        id: id.to_string(),
        item_id: format!("iid-{id}"),
        content_type: "text".to_string(),
        content: Some(vec![payload]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: wall,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: device.to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
        deleted: false,
        pinned: false,
        pin_order: None,
    }
}

/// State-based CRDT apply: merge a single remote `WireItem` into a local
/// HashMap keyed by id. Returns the post-state. Pure function — no I/O.
fn apply(state: &mut HashMap<String, ClipboardItem>, remote: WireItem) {
    match state.get(&remote.id) {
        None => {
            // Unseen id — insert.
            state.insert(remote.id.clone(), wire_to_local(remote));
        }
        Some(local) => match resolve(local, &remote) {
            MergeOutcome::TakeRemote => {
                state.insert(remote.id.clone(), wire_to_local(remote));
            }
            MergeOutcome::KeepLocal => { /* no-op */ }
        },
    }
}

/// Convert local state into a canonical, comparable snapshot
/// (id → (lamport_ts, wall_time, content)).
/// `is_synced` is intentionally excluded — wire_to_local always flips it to
/// true, so it carries no convergence information.
fn snapshot(state: &HashMap<String, ClipboardItem>) -> Vec<(String, i64, i64, Option<Vec<u8>>)> {
    let mut out: Vec<_> = state
        .values()
        .map(|i| {
            (
                i.id.to_string(),
                i.lamport_ts,
                i.wall_time,
                i.content.clone(),
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// --- 1. Convergence: two replicas merging same op-set yield identical state -

#[test]
fn merge_convergent_two_replicas_yield_identical_state() {
    // Ops: 3 items, with later updates to id-A and id-B. Both replicas see
    // the full op-set but in different orders.
    let ops = vec![
        mk_wire("id-A", 1, 1000, "dev-1", 10),
        mk_wire("id-B", 2, 1100, "dev-1", 20),
        mk_wire("id-C", 3, 1200, "dev-2", 30),
        mk_wire("id-A", 5, 1500, "dev-2", 99), // later update to A
        mk_wire("id-B", 4, 1400, "dev-2", 88), // later update to B
    ];

    let order_1 = ops.clone();
    let order_2 = vec![
        ops[3].clone(),
        ops[0].clone(),
        ops[4].clone(),
        ops[2].clone(),
        ops[1].clone(),
    ];
    let order_3 = vec![
        ops[2].clone(),
        ops[4].clone(),
        ops[1].clone(),
        ops[3].clone(),
        ops[0].clone(),
    ];

    let mut r1: HashMap<String, ClipboardItem> = HashMap::new();
    let mut r2: HashMap<String, ClipboardItem> = HashMap::new();
    let mut r3: HashMap<String, ClipboardItem> = HashMap::new();

    for op in order_1 {
        apply(&mut r1, op);
    }
    for op in order_2 {
        apply(&mut r2, op);
    }
    for op in order_3 {
        apply(&mut r3, op);
    }

    assert_eq!(snapshot(&r1), snapshot(&r2), "r1 vs r2 must converge");
    assert_eq!(snapshot(&r2), snapshot(&r3), "r2 vs r3 must converge");

    // Spot-check: the latest writes (lamport 5 for A, lamport 4 for B) win.
    assert_eq!(r1["id-A"].lamport_ts, 5);
    assert_eq!(r1["id-A"].content, Some(vec![99]));
    assert_eq!(r1["id-B"].lamport_ts, 4);
    assert_eq!(r1["id-B"].content, Some(vec![88]));
}

// --- 2. Idempotency: applying the same event twice is a no-op ---------------

#[test]
fn idempotent_apply_same_event_twice_no_change() {
    let mut state: HashMap<String, ClipboardItem> = HashMap::new();

    // Seed with one item.
    apply(&mut state, mk_wire("id-X", 5, 1000, "dev-A", 42));
    let after_first = snapshot(&state);

    // Apply the EXACT same event again.
    apply(&mut state, mk_wire("id-X", 5, 1000, "dev-A", 42));
    let after_second = snapshot(&state);

    assert_eq!(
        after_first, after_second,
        "re-applying identical event must be a no-op"
    );

    // Apply it a third time for good measure.
    apply(&mut state, mk_wire("id-X", 5, 1000, "dev-A", 42));
    assert_eq!(snapshot(&state), after_first, "triple-apply still no-op");
}

// --- 3. Commutativity: two independent ops are order-independent ------------

#[test]
fn commutative_two_ops_independent_of_order() {
    let op_a = mk_wire("id-A", 3, 1000, "dev-1", 7);
    let op_b = mk_wire("id-B", 4, 1100, "dev-2", 8);

    let mut left: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut left, op_a.clone());
    apply(&mut left, op_b.clone());

    let mut right: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut right, op_b);
    apply(&mut right, op_a);

    assert_eq!(
        snapshot(&left),
        snapshot(&right),
        "A∘B and B∘A on disjoint ids must converge"
    );
}

// --- 4. Lamport causality preserved across merges ---------------------------

#[test]
fn causality_lamport_ordering_preserved_after_merge() {
    // Simulate: device-1 ticks, sends to device-2 which observes, ticks
    // locally, then both states are merged.
    let mut clock_1 = LamportClock::new();
    let mut clock_2 = LamportClock::new();

    let ts1 = clock_1.tick(); // dev-1 creates item-A
    let item_a = mk_local("id-A", ts1 as i64, 1000, 1);

    // dev-2 receives item-A and observes its lamport.
    let ts2 = clock_2.observe(ts1);
    assert!(ts2 > ts1, "observe must advance receiver clock past sender");

    // dev-2 creates item-B locally (causally after A).
    let ts3 = clock_2.tick();
    let item_b = mk_local("id-B", ts3 as i64, 1100, 2);
    assert!(
        ts3 > ts1,
        "B causally follows A — its lamport must be higher"
    );

    // Wire both items into a fresh "merged" replica.
    let mut merged: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut merged, local_to_wire(&item_a, "dev-1"));
    apply(&mut merged, local_to_wire(&item_b, "dev-2"));

    // Causal order (A < B by lamport) survives the merge.
    assert!(
        merged["id-A"].lamport_ts < merged["id-B"].lamport_ts,
        "lamport ordering A<B must be preserved post-merge"
    );

    // Apply in the OPPOSITE order — causality still observable in final state.
    let mut merged_rev: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut merged_rev, local_to_wire(&item_b, "dev-2"));
    apply(&mut merged_rev, local_to_wire(&item_a, "dev-1"));
    assert!(
        merged_rev["id-A"].lamport_ts < merged_rev["id-B"].lamport_ts,
        "lamport ordering invariant under apply-order reversal"
    );
    assert_eq!(snapshot(&merged), snapshot(&merged_rev));
}

// --- 5. Delete-after-insert: higher-lamport remote wins ---------------------
//
// Note: the current sync layer has no explicit tombstone — "delete" is
// modelled as a remote update with empty content and a higher Lamport
// timestamp (LWW). This test pins that behaviour: a remote "deletion"
// (empty payload, lamport+1) replaces the local insert.

#[test]
fn delete_after_insert_remote_wins_if_lamport_higher() {
    let mut state: HashMap<String, ClipboardItem> = HashMap::new();

    // Local insert at lamport=5.
    apply(&mut state, mk_wire("id-Z", 5, 1000, "dev-A", 77));
    assert_eq!(state["id-Z"].content, Some(vec![77]));

    // Remote "tombstone": same id, lamport=6, empty content.
    let mut tombstone = mk_wire("id-Z", 6, 1500, "dev-B", 0);
    tombstone.content = None;
    apply(&mut state, tombstone);

    assert_eq!(
        state["id-Z"].lamport_ts, 6,
        "higher-lamport remote must overwrite local"
    );
    assert_eq!(
        state["id-Z"].content, None,
        "remote tombstone (empty content) must replace local payload"
    );

    // Now an older remote (lamport=4) tries to revive — must be rejected.
    let stale = mk_wire("id-Z", 4, 9999, "dev-C", 11);
    apply(&mut state, stale);
    assert_eq!(
        state["id-Z"].lamport_ts, 6,
        "older lamport must not revive a tombstoned id"
    );
    assert_eq!(state["id-Z"].content, None);
}

// --- 5. Device-ID tie-break (the merge.rs:39 BUG fix) ----------------------

/// Two replicas write the same logical item at the same lamport_ts AND the
/// same wall_time but from different devices. The pre-v3 merge compared
/// `remote.origin_device_id` against `local.id` (the row UUID), which mixed
/// two unrelated identifier spaces and produced non-deterministic results:
/// each replica could pick a different winner, causing the state to diverge
/// permanently. The v3 fix compares `origin_device_id` on both sides, so the
/// peer with the lexicographically larger device id deterministically wins on
/// every replica.
#[test]
fn equal_lamport_equal_wall_tie_break_converges() {
    // Both replicas observe the same op-set (one write from "dev-A" and one
    // from "dev-zzz") in different orders. The final state on both replicas
    // MUST agree (convergence), and the content MUST be the "dev-zzz" write
    // because "dev-zzz" > "dev-A" lexicographically.
    let from_a = mk_wire("id-tie", 7, 5000, "dev-A", 0xAA);
    let from_z = mk_wire("id-tie", 7, 5000, "dev-zzz", 0xFF);

    let mut state_1: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut state_1, from_a.clone());
    apply(&mut state_1, from_z.clone());

    let mut state_2: HashMap<String, ClipboardItem> = HashMap::new();
    apply(&mut state_2, from_z.clone());
    apply(&mut state_2, from_a.clone());

    assert_eq!(
        snapshot(&state_1),
        snapshot(&state_2),
        "tie-break must converge regardless of apply order"
    );

    // The winner is the larger device id, dev-zzz.
    let winner = &state_1["id-tie"];
    assert_eq!(
        winner.origin_device_id, "dev-zzz",
        "tie-break must pick the lexicographically larger device id, \
         not compare device id against row UUID (the pre-v3 BUG)"
    );
    assert_eq!(winner.content, from_z.content);
}

/// Sanity: when the LOCAL side has the larger device id, it must win. The
/// pre-v3 code happened to "work" sometimes because row UUIDs occasionally
/// sorted favourably, but it never honoured the local device id at all.
#[test]
fn equal_lamport_equal_wall_local_wins_when_local_device_larger() {
    let local = mk_local_with_device("id-tie", 9, 8000, 0x11, "zzz-largest");
    let remote = mk_wire("id-tie", 9, 8000, "aaa-smaller", 0x22);

    let outcome = resolve(&local, &remote);
    assert_eq!(
        outcome,
        MergeOutcome::KeepLocal,
        "local must win when local.origin_device_id > remote.origin_device_id; \
         comparing remote.origin_device_id against local.id (pre-v3) would \
         have given a different and undefined answer"
    );
}
