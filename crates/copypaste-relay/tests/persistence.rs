//! R1b — relay persistence ("relay works as a database").
//!
//! These tests drive the SQLite-backed `RelayStore` directly (the durable
//! layer lives behind the store, not the HTTP surface). They verify that
//! device records, their R1a token *sets*, and inbox items survive a process
//! restart, that TTL eviction is reflected in SQL, and that the `(wall_time,
//! id)` cursor ordering is preserved after a reopen.
//!
//! Pattern: open a store on a temp-file db path, write, DROP the store (closing
//! the connection — simulating process shutdown), then reopen a NEW store on
//! the SAME path and assert the state rehydrated.

#![allow(dead_code, unused_imports)]

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

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

use state::RelayStore;

const TTL: u64 = 3600;
const MAX_ITEMS: usize = 500;
const DEVICE_A: &str = "11111111-1111-1111-1111-111111111111";

fn valid_key() -> String {
    B64.encode([0u8; 32])
}
fn valid_pop() -> String {
    B64.encode([0xDE_u8; 32])
}

fn push_text(store: &mut RelayStore, device_id: &str, wall_time: u64) -> i64 {
    store
        .push_item(
            device_id,
            "text".to_string(),
            B64.encode(format!("payload-{wall_time}").as_bytes()),
            wall_time,
            10 * 1024 * 1024,
        )
        .expect("push must succeed")
}

/// The headline R1b requirement: write with one store/connection, drop it, then
/// reopen the SAME file path and assert devices + token sets + inbox items all
/// persisted across the simulated restart.
#[test]
fn devices_tokens_and_items_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("relay.db");
    let db_path = db_path.to_str().unwrap().to_string();

    // First "process": register a device (mint two co-registered tokens) and
    // push three items, then drop the store to close the connection.
    let (token1, token2, ids) = {
        let mut store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();
        let (token1, _) = store
            .register_device(DEVICE_A.into(), "Device A".into(), valid_key(), valid_pop())
            .unwrap();
        // Co-registration (R1a): a second independent token on the SAME id.
        let (token2, _) = store
            .register_device(DEVICE_A.into(), "Device A".into(), valid_key(), valid_pop())
            .unwrap();
        let id1 = push_text(&mut store, DEVICE_A, 1000);
        let id2 = push_text(&mut store, DEVICE_A, 2000);
        let id3 = push_text(&mut store, DEVICE_A, 3000);
        // CopyPaste-crh3.70: insert_item is deferred; flush the pending writes
        // to DB now so they survive the simulated "process restart" below.
        store.flush_pending_db_writes_for_test();
        (token1, token2, vec![id1, id2, id3])
    }; // <- store dropped here: connection closed, "process" exited.

    // Second "process": reopen the same file. State must rehydrate.
    let store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();

    // Device record survived.
    let record = store.get_device(DEVICE_A).expect("device must persist");
    assert_eq!(record.device_name, "Device A");
    assert_eq!(record.public_key_b64, valid_key());

    // BOTH co-registered tokens survived and still authorize.
    assert_eq!(record.tokens.len(), 2, "both tokens must persist");
    assert!(
        store.verify_token(DEVICE_A, &token1).is_ok(),
        "first token must verify after reopen"
    );
    assert!(
        store.verify_token(DEVICE_A, &token2).is_ok(),
        "co-registered token must verify after reopen"
    );

    // Inbox items survived, with ids and ordering intact.
    let items = store.pull_items(DEVICE_A, 0, None, usize::MAX).unwrap();
    assert_eq!(items.len(), 3, "all three items must persist");
    assert_eq!(
        items.iter().map(|i| i.id).collect::<Vec<_>>(),
        ids,
        "item ids must persist unchanged"
    );
    assert_eq!(
        items.iter().map(|i| i.wall_time).collect::<Vec<_>>(),
        vec![1000, 2000, 3000],
        "items must rehydrate in ascending (wall_time, id) order"
    );

    // The next push after reopen must NOT reuse a persisted id (next_sync_id
    // counter survived) — guards security HIGH #3 across restart.
    let mut store = store;
    let id4 = push_text(&mut store, DEVICE_A, 4000);
    assert!(
        id4 > *ids.iter().max().unwrap(),
        "id must keep ascending after reopen"
    );
}

/// TTL eviction must be reflected in the durable store: items pruned by
/// `prune_expired` are gone after a reopen, not silently resurrected.
#[test]
fn ttl_eviction_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("relay.db");
    let db_path = db_path.to_str().unwrap().to_string();

    {
        let mut store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();
        store
            .register_device(DEVICE_A.into(), "Device A".into(), valid_key(), valid_pop())
            .unwrap();
        push_text(&mut store, DEVICE_A, 1000);
        push_text(&mut store, DEVICE_A, 2000);
        // CopyPaste-crh3.70: flush deferred writes so items are in DB before
        // prune_expired attempts to delete them by inserted_at_unix.
        store.flush_pending_db_writes_for_test();
        // All items were inserted "just now" (inserted_at_unix ~= now). Prune
        // with a now far in the future and a TTL of 1 second so the cutoff
        // (now - ttl) is well past every item's insert time → all evicted.
        let far_future = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 10_000;
        let evicted = store.prune_expired(far_future, 1);
        assert_eq!(evicted, 2, "both items must be TTL-evicted");
        assert!(store
            .pull_items(DEVICE_A, 0, None, usize::MAX)
            .unwrap()
            .is_empty());
    }

    // Reopen: the evicted items must stay gone (the device itself persists).
    let store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();
    assert!(store.get_device(DEVICE_A).is_ok(), "device record persists");
    let items = store.pull_items(DEVICE_A, 0, None, usize::MAX).unwrap();
    assert!(
        items.is_empty(),
        "TTL-evicted items must not resurrect after reopen, got {}",
        items.len()
    );
}

/// The `(wall_time, id)` composite cursor must keep returning items in the
/// correct, gapless order after a reopen — including a run of items that share
/// a `wall_time` (the relay H-1 tie case).
#[test]
fn cursor_ordering_correct_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("relay.db");
    let db_path = db_path.to_str().unwrap().to_string();

    let (id_a, id_b, id_c) = {
        let mut store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();
        store
            .register_device(DEVICE_A.into(), "Device A".into(), valid_key(), valid_pop())
            .unwrap();
        // Three items, two sharing wall_time == 10, distinct ascending ids.
        let id_a = push_text(&mut store, DEVICE_A, 10);
        let id_b = push_text(&mut store, DEVICE_A, 10);
        let id_c = push_text(&mut store, DEVICE_A, 20);
        // CopyPaste-crh3.70: flush deferred insert_item writes to DB before the
        // simulated "process restart."
        store.flush_pending_db_writes_for_test();
        (id_a, id_b, id_c)
    };

    let store = RelayStore::new_persistent(TTL, MAX_ITEMS, &db_path).unwrap();

    // Full ascending order preserved.
    let all = store.pull_items(DEVICE_A, 0, None, usize::MAX).unwrap();
    assert_eq!(
        all.iter().map(|i| i.id).collect::<Vec<_>>(),
        vec![id_a, id_b, id_c]
    );

    // Composite-cursor pagination across the tie must walk every item once,
    // with no gap and no duplicate, after the reopen.
    let mut seen = Vec::new();
    let mut since = 0u64;
    let mut since_id: Option<i64> = None;
    loop {
        let page = store.pull_items(DEVICE_A, since, since_id, 1).unwrap();
        if page.is_empty() {
            break;
        }
        let last = page.last().unwrap();
        since = last.wall_time;
        since_id = Some(last.id);
        seen.extend(page.iter().map(|i| i.id));
    }
    assert_eq!(
        seen,
        vec![id_a, id_b, id_c],
        "tuple-cursor pagination must walk all items in order after reopen"
    );
}

/// The in-memory default (`:memory:`) must NOT persist across reopen — this
/// guards the documented default behaviour (ephemeral, as before R1b).
#[test]
fn in_memory_default_does_not_persist() {
    {
        let mut store = RelayStore::new_persistent(TTL, MAX_ITEMS, db::IN_MEMORY_PATH).unwrap();
        store
            .register_device(DEVICE_A.into(), "Device A".into(), valid_key(), valid_pop())
            .unwrap();
        push_text(&mut store, DEVICE_A, 1000);
        assert!(store.get_device(DEVICE_A).is_ok());
    }
    // A brand-new in-memory store shares nothing with the previous one.
    let store = RelayStore::new_persistent(TTL, MAX_ITEMS, db::IN_MEMORY_PATH).unwrap();
    assert!(
        store.get_device(DEVICE_A).is_err(),
        ":memory: must start empty — nothing persists across instances"
    );
}
