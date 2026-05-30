//! Integration test: v4 migration gate clears after startup sweep.
//!
//! Regression for the production bug where `Database::migration_v4_sweep_resumable`
//! was never called from `daemon::run_with_quit_flag`, leaving every
//! `insert_item_with_fts` call gated behind `ItemsError::MigrationInProgress`.
//!
//! The fix adds a `spawn_blocking` call at daemon startup that runs both:
//!   1. `migration_v4_sweep_resumable` — rotates any v1 rows to v2
//!   2. `force_complete_if_no_v1_rows` — clears InProgress for fresh installs
//!      that had zero clipboard rows at schema-migration time
//!
//! This test exercises the exact code path the daemon executes:
//!   * Pre-seeded DB with `migration_state` row in InProgress, zero v1 rows
//!   * Run sweep (rotates 0 rows) then force_complete
//!   * Assert `insert_item_with_fts` succeeds (migration gate is open)

use copypaste_core::{
    build_item_aad_v2, encrypt_item_with_aad, insert_item_with_fts, ClipboardItem, Database,
    MigrationState, AAD_SCHEMA_VERSION_V4,
};
use rusqlite::params;

/// Derive the v2 storage key from the raw seed — mirrors what the daemon does
/// via `derive_v2(seed)`.
fn v2_key_from_seed(seed: &[u8; 32]) -> [u8; 32] {
    // derive_v2 now returns Zeroizing<[u8;32]>; deref to get the raw array.
    *copypaste_core::derive_v2(seed)
}

/// Construct a minimal `ClipboardItem` encrypted with the given key/nonce pair.
fn make_item(seed: &[u8; 32]) -> (ClipboardItem, String) {
    let v2_key = v2_key_from_seed(seed);
    let item_id = uuid::Uuid::new_v4().to_string();
    let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ct) =
        encrypt_item_with_aad(b"hello from test", &v2_key, &aad).expect("encrypt must succeed");
    let item = ClipboardItem::new_text(ct, nonce.to_vec(), 1);
    (item, "hello from test".to_string())
}

/// Seed the `migration_state` table as InProgress (completed_at = NULL),
/// simulating a fresh install where the v6 schema migration ran `INSERT OR
/// IGNORE` with no clipboard rows present.
fn seed_inprogress(db: &Database) {
    db.conn()
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS migration_state (
                key                     TEXT PRIMARY KEY,
                key_version_in_progress INTEGER,
                last_processed_id       INTEGER NOT NULL DEFAULT 0,
                started_at              INTEGER,
                completed_at            INTEGER
            );",
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at, completed_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'), NULL)",
            params![],
        )
        .unwrap();
}

/// The core regression test:
/// pre-seeded InProgress DB (zero v1 rows) + sweep + force_complete
/// → insert_item_with_fts succeeds.
#[test]
fn migration_gate_clears_after_sweep_on_fresh_db() {
    let seed = [0xAAu8; 32];
    let v1_key = copypaste_core::derive_storage_key_v1(&seed);
    let v2_key = copypaste_core::derive_v2(&seed);

    let db = Database::open_in_memory().expect("in-memory DB must open");

    // Arm the migration gate: seed InProgress with zero v1 rows.
    seed_inprogress(&db);

    assert!(
        matches!(
            db.migration_state().unwrap(),
            MigrationState::InProgress { .. }
        ),
        "gate must be armed before sweep"
    );

    // Gate is armed — insert must be rejected.
    let (item, plaintext) = make_item(&seed);
    let gate_err = insert_item_with_fts(&db, &item, &plaintext);
    assert!(
        gate_err.is_err(),
        "insert_item_with_fts must fail while gate is InProgress"
    );

    // Run the same sweep sequence the daemon now executes at startup.
    let rotated = db
        .migration_v4_sweep_resumable(&v1_key, &v2_key)
        .expect("sweep must succeed");
    assert_eq!(rotated, 0, "no v1 rows → 0 rotated");

    db.force_complete_if_no_v1_rows()
        .expect("force_complete must succeed");

    // Gate must now be open.
    assert_eq!(
        db.migration_state().unwrap(),
        MigrationState::Complete,
        "migration_state must be Complete after sweep + force_complete"
    );

    // Insert must now succeed.
    let (item2, plaintext2) = make_item(&seed);
    insert_item_with_fts(&db, &item2, &plaintext2)
        .expect("insert_item_with_fts must succeed after migration gate clears");
}

/// Variant: DB has v1 rows. Sweep rotates them; insert succeeds after.
#[test]
fn migration_gate_clears_after_sweep_with_v1_rows() {
    let seed = [0xBBu8; 32];
    let v1_key = copypaste_core::derive_storage_key_v1(&seed);
    let v2_key = copypaste_core::derive_v2(&seed);

    let db = Database::open_in_memory().expect("in-memory DB must open");

    // Insert a raw v1 row (bypassing the gate — direct SQL as the schema migration would).
    let row_id = uuid::Uuid::new_v4().to_string();
    let item_id = uuid::Uuid::new_v4().to_string();
    let aad = copypaste_core::build_item_aad(&item_id, copypaste_core::AAD_SCHEMA_VERSION);
    let (nonce, ct) = encrypt_item_with_aad(b"legacy v1 content", &v1_key, &aad).expect("encrypt");
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version, origin_device_id) \
             VALUES (?1,?2,'text',?3,?4,0,0,1,1000,1,'')",
            params![row_id, item_id, ct, nonce.to_vec()],
        )
        .unwrap();

    // Arm the gate.
    seed_inprogress(&db);

    // Run sweep.
    let rotated = db
        .migration_v4_sweep_resumable(&v1_key, &v2_key)
        .expect("sweep must succeed");
    assert_eq!(rotated, 1, "one v1 row must be rotated");

    db.force_complete_if_no_v1_rows()
        .expect("force_complete must succeed");

    assert_eq!(
        db.migration_state().unwrap(),
        MigrationState::Complete,
        "migration must be Complete after rotating v1 row"
    );

    // Insert must succeed.
    let (item, plaintext) = make_item(&seed);
    insert_item_with_fts(&db, &item, &plaintext)
        .expect("insert_item_with_fts must succeed after sweep clears gate");
}
