//! Wave 1.5 — Concurrent writers integration test (edge-cases CRITICAL #4).
//!
//! Simulates the three real-world writer subsystems that race against the
//! shared SQLite database in production:
//!   * `monitor`    — local clipboard polling
//!   * `sync`       — incoming peer items over LAN
//!   * `cloud-push` — items being mirrored to relay
//!
//! Each task inserts 1000 items through a shared `Arc<Mutex<Database>>`. After
//! all tasks complete, we assert:
//!   1. The row count equals exactly 3 × 1000 (no lost updates).
//!   2. Every `id` (UUIDv4 primary key) is unique (no duplicate-key panics).
//!   3. The Lamport timestamps for each writer ("device") are strictly
//!      monotonic in the order they were generated, matching causal-history
//!      expectations for the CRDT layer.

use std::collections::HashSet;
use std::sync::Arc;

use copypaste_core::{count_items, insert_item, ClipboardItem, Database};
use tempfile::tempdir;
use tokio::sync::Mutex;

const WRITERS: usize = 3;
const INSERTS_PER_WRITER: i64 = 1000;
const DEVICES: [&str; WRITERS] = ["monitor", "sync", "cloud-push"];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_no_lost_updates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("concurrent.db");
    let key = [0x42u8; 32];
    let db = Database::open(&path, &key).expect("open encrypted db");
    let shared = Arc::new(Mutex::new(db));

    // Spawn the three writer tasks. Each one owns one logical "device" and
    // emits 1000 items with Lamport timestamps 1..=1000. We tag each item's
    // `app_bundle_id` with the device name so we can later partition rows by
    // writer and verify per-writer monotonicity.
    let mut handles = Vec::with_capacity(WRITERS);
    for device in DEVICES {
        let db = Arc::clone(&shared);
        let handle = tokio::spawn(async move {
            for lamport in 1..=INSERTS_PER_WRITER {
                let mut item = ClipboardItem::new_text(vec![0u8; 16], vec![0u8; 24], lamport);
                item.app_bundle_id = Some(device.to_string());
                // Hold the mutex only for the single insert; this is exactly
                // how the daemon serialises writes across subsystems.
                let guard = db.lock().await;
                insert_item(&guard, &item).expect("insert under contention");
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.expect("writer task panicked");
    }

    // --- Assertion 1: no lost updates ---
    let total = {
        let guard = shared.lock().await;
        count_items(&*guard).expect("count")
    };
    assert_eq!(
        total,
        (WRITERS as i64) * INSERTS_PER_WRITER,
        "expected exactly {} rows after concurrent writes",
        WRITERS as i64 * INSERTS_PER_WRITER,
    );

    // --- Assertions 2 + 3: unique IDs + per-device monotonic Lamport ---
    let guard = shared.lock().await;
    let conn = guard.conn();

    let mut id_stmt = conn
        .prepare("SELECT id FROM clipboard_items")
        .expect("prepare id query");
    let ids: Vec<String> = id_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query ids")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect ids");
    let unique_ids: HashSet<&String> = ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        ids.len(),
        "duplicate primary keys produced under contention"
    );

    for device in DEVICES {
        let mut stmt = conn
            .prepare(
                "SELECT lamport_ts FROM clipboard_items \
                 WHERE app_bundle_id = ?1 ORDER BY rowid ASC",
            )
            .expect("prepare lamport query");
        let stamps: Vec<i64> = stmt
            .query_map([device], |row| row.get::<_, i64>(0))
            .expect("query lamport")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect lamport");
        assert_eq!(
            stamps.len() as i64,
            INSERTS_PER_WRITER,
            "device {device} produced wrong row count"
        );
        // Each writer inserted lamport 1..=1000 in order; rowid preserves
        // insertion order, so the read-back sequence must match exactly.
        for (i, ts) in stamps.iter().enumerate() {
            assert_eq!(
                *ts,
                (i as i64) + 1,
                "device {device} lamport non-monotonic at offset {i}"
            );
        }
    }
}
