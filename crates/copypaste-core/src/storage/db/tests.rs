use super::*;
use tempfile::tempdir;

#[test]
fn database_opens_with_wal_mode() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("wal_test.db");
    let key = [0x01u8; 32];
    let db = Database::open(&path, &key).unwrap();
    let mode: String = db
        .conn()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn cache_size_pragma_maps_mb_to_negative_kib() {
    // Negative cache_size means "KiB of memory"; N MiB → -(N * 1024).
    assert_eq!(cache_size_pragma(8), "PRAGMA cache_size = -8192;\n");
    assert_eq!(cache_size_pragma(16), "PRAGMA cache_size = -16384;\n");
    assert_eq!(cache_size_pragma(1), "PRAGMA cache_size = -1024;\n");
}

#[test]
fn cache_size_pragma_clamps_out_of_range() {
    use crate::config::{SQLITE_CACHE_MB_MAX, SQLITE_CACHE_MB_MIN};
    // 0 is below the floor → clamped up to the minimum.
    assert_eq!(
        cache_size_pragma(0),
        format!(
            "PRAGMA cache_size = -{};\n",
            i64::from(SQLITE_CACHE_MB_MIN) * 1024
        )
    );
    // A pathological value is clamped down to the ceiling.
    assert_eq!(
        cache_size_pragma(u32::MAX),
        format!(
            "PRAGMA cache_size = -{};\n",
            i64::from(SQLITE_CACHE_MB_MAX) * 1024
        )
    );
}

#[test]
fn open_with_cache_mb_applies_configured_cache_size() {
    // A configured cache size is reflected in the live connection's
    // PRAGMA cache_size (negative ⇒ KiB), surviving apply_migrations which
    // would otherwise reset it to the default.
    let db = Database::open_in_memory_with_cache_mb(32).unwrap();
    let cache: i64 = db
        .conn()
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cache, -(32 * 1024));
}

#[test]
fn open_uses_default_cache_size() {
    // The plain open path keeps the historical 8 MiB (-8192 KiB) cache.
    let db = Database::open_in_memory().unwrap();
    let cache: i64 = db
        .conn()
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cache, -(i64::from(crate::config::SQLITE_CACHE_MB) * 1024));
}

#[test]
fn open_with_cache_mb_clamps_out_of_range_on_connection() {
    // An out-of-range request is clamped to the ceiling on the real conn.
    let db = Database::open_in_memory_with_cache_mb(u32::MAX).unwrap();
    let cache: i64 = db
        .conn()
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        cache,
        -(i64::from(crate::config::SQLITE_CACHE_MB_MAX) * 1024)
    );
}

#[test]
fn schema_creates_all_tables() {
    let db = Database::open_in_memory().unwrap();
    for table in &["clipboard_items", "devices", "settings", "pending_uploads"] {
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table: {}", table);
    }
}

#[test]
fn migration_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.db");
    let key = [0x02u8; 32];
    Database::open(&path, &key).unwrap();
    Database::open(&path, &key).unwrap();
}

// --- SQLCipher tests ---

#[test]
fn encrypted_db_rejects_wrong_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("enc.db");
    let key_a = [0xAAu8; 32];
    let key_b = [0xBBu8; 32];
    Database::open(&path, &key_a).unwrap();
    let result = Database::open(&path, &key_b);
    assert!(result.is_err(), "wrong key should not open encrypted DB");
}

#[test]
fn encrypted_db_round_trips_with_correct_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("enc2.db");
    let key = [0xCCu8; 32];
    {
        let db = Database::open(&path, &key).unwrap();
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time) \
                 VALUES (?1,?2,?3,?4,?5,0,0,1,1000)",
                rusqlite::params![
                    "test-id-1",
                    "item-id-1",
                    "text/plain",
                    b"payload" as &[u8],
                    b"nonce123456789012345678901" as &[u8],
                ],
            )
            .unwrap();
    }
    let db2 = Database::open(&path, &key).unwrap();
    let count: i64 = db2
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn rekey_changes_encryption_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rekey.db");
    let old_key = [0x11u8; 32];
    let new_key = [0x22u8; 32];
    {
        let db = Database::open(&path, &old_key).unwrap();
        let _db = db.rekey(&new_key).unwrap();
    }
    assert!(Database::open(&path, &old_key).is_err());
    assert!(Database::open(&path, &new_key).is_ok());
}

#[test]
fn plaintext_db_is_migrated_on_first_encrypted_open() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("migrate.db");
    // Create plaintext DB (simulates pre-Phase-2c database on disk)
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        conn.execute_batch(include_str!("../schema_v1.sql"))
            .unwrap();
        conn.execute_batch("PRAGMA user_version=1;").unwrap();
    }
    let key = [0x55u8; 32];
    let db = Database::open(&path, &key).expect("migration should succeed");
    let _count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    drop(db);
    assert!(Database::open(&path, &[0x66u8; 32]).is_err());
}

#[test]
fn open_in_memory_still_works_without_key() {
    let db = Database::open_in_memory().unwrap();
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

// ── Write-gate release after a stuck migration sweep (HIGH / v0.4) ─────
//
// Regression for the live-install bug: an install with legacy
// `key_version = 1` rows that can NEVER be rotated (their auth tag does
// not verify under the current v1 key) left the migration sweep logging
// `rotated=0 failed=N` forever, `completed_at` stuck NULL, and EVERY new
// capture rejected with `MigrationInProgress`. After a full sweep pass
// attempts those rows and fails, the gate must release.

/// Seed a `key_version = 1` text row whose ciphertext was produced under a
/// DIFFERENT v1 key, so the real sweep key can never decrypt it (auth tag
/// mismatch). These rows are the permanently-unrotatable legacy rows from
/// the live install.
fn seed_unrotatable_v1_text_row(db: &Database, foreign_v1_key: &[u8; 32]) {
    use crate::crypto::encrypt::{build_item_aad, encrypt_item_with_aad, AAD_SCHEMA_VERSION};
    let row_id = uuid::Uuid::new_v4().to_string();
    let item_id = crate::storage::items::ItemId::from(uuid::Uuid::new_v4().to_string());
    let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) =
        encrypt_item_with_aad(b"legacy payload", foreign_v1_key, &aad).unwrap();
    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
             VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
            rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
        )
        .unwrap();
}

/// A freshly-inserted `ClipboardItem` for the gate test. Content shape is
/// irrelevant here — we only care that `insert_item` is no longer rejected.
fn make_text_item() -> crate::storage::items::ClipboardItem {
    crate::storage::items::ClipboardItem::new_text(b"new capture".to_vec(), vec![0u8; 24], 1)
}

#[test]
fn stuck_sweep_releases_write_gate_and_insert_succeeds() {
    let db = Database::open_in_memory().unwrap();
    // The sweep's real key.
    let v1_key = [0x10u8; 32];
    let v2_key = [0x20u8; 32];
    // Rows encrypted under a key the sweep will never have.
    let foreign = [0xFEu8; 32];

    for _ in 0..37 {
        seed_unrotatable_v1_text_row(&db, &foreign);
    }
    // Arm the gate as InProgress to model the live install precisely (the
    // v6 schema migration leaves `completed_at = NULL` for an upgrade that
    // still has key_version=1 rows). `open_in_memory` seeds it Complete
    // because the DB was empty when migrations ran; override that here.
    db.conn()
        .execute(
            "UPDATE migration_state SET completed_at = NULL \
             WHERE key = 'v4-key-version-sweep'",
            [],
        )
        .unwrap();
    assert!(
        matches!(
            db.migration_state().unwrap(),
            MigrationState::InProgress { .. }
        ),
        "precondition: gate armed before the sweep"
    );

    // Run the sweep + the new force-complete pass.
    let rotated = db.migration_v4_sweep_resumable(&v1_key, &v2_key).unwrap();
    db.force_complete_if_no_v1_rows().unwrap();

    assert_eq!(rotated, 0, "no row was decryptable, so none may rotate");

    // (b) the gate must now read Complete even though 37 v1 rows remain.
    assert_eq!(
        db.migration_state().unwrap(),
        MigrationState::Complete,
        "gate must release after a full sweep pass attempts the unrotatable rows"
    );

    // The unrotatable rows are left at key_version=1 (still unreadable).
    let remaining_v1: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining_v1, 37, "corrupt rows stay at key_version=1");

    // (c) a subsequent insert must SUCCEED (no MigrationInProgress).
    let item = make_text_item();
    crate::storage::items::insert_item(&db, &item)
        .expect("insert must succeed after the gate releases");
}

#[test]
fn force_migration_complete_env_clears_a_stuck_gate() {
    // Escape hatch for already-stuck installs: even before any sweep runs,
    // COPYPASTE_FORCE_MIGRATION_COMPLETE=1 force-clears the gate.
    let db = Database::open_in_memory().unwrap();
    let foreign = [0xABu8; 32];
    for _ in 0..5 {
        seed_unrotatable_v1_text_row(&db, &foreign);
    }
    // Manually arm the gate as InProgress (the live install's state).
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at, completed_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'), NULL)",
            [],
        )
        .unwrap();
    assert!(matches!(
        db.migration_state().unwrap(),
        MigrationState::InProgress { .. }
    ));

    db.force_migration_complete().unwrap();

    assert_eq!(
        db.migration_state().unwrap(),
        MigrationState::Complete,
        "force_migration_complete must clear the gate unconditionally"
    );
    let item = make_text_item();
    crate::storage::items::insert_item(&db, &item)
        .expect("insert must succeed after force_migration_complete");
}

// ── Fix A: surfacing + purging permanently-dead key_version=1 rows ─────

#[test]
fn count_and_purge_dead_v1_rows() {
    let db = Database::open_in_memory().unwrap();
    let foreign = [0xCDu8; 32];

    // Seed 7 undecryptable legacy rows + 1 readable v2 row (must survive).
    for _ in 0..7 {
        seed_unrotatable_v1_text_row(&db, &foreign);
    }
    let live = make_text_item();
    crate::storage::items::insert_item(&db, &live).expect("insert live v2 row");

    // count_dead_v1_rows surfaces exactly the stranded rows.
    assert_eq!(db.count_dead_v1_rows().unwrap(), 7);

    // purge removes only the v1 rows and reports the deleted count.
    let deleted = db.purge_dead_v1_rows().unwrap();
    assert_eq!(deleted, 7, "purge must delete all undecryptable v1 rows");
    assert_eq!(db.count_dead_v1_rows().unwrap(), 0, "no dead rows remain");

    // The live v2 row is untouched.
    let total: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 1, "the readable v2 row must survive the purge");

    // Purge is idempotent — a second run deletes nothing.
    assert_eq!(db.purge_dead_v1_rows().unwrap(), 0);
}

/// Fix 1: `purge_dead_v1_rows` must wrap both DELETEs in a single
/// transaction so a crash between the two cannot leave items rows without
/// their FTS counterparts. This test verifies that after a successful call
/// the database is consistent: clipboard_items and clipboard_fts agree.
#[test]
fn purge_dead_v1_rows_is_atomic_fts_and_items_consistent() {
    let db = Database::open_in_memory().unwrap();
    let foreign = [0xABu8; 32];

    // Seed 3 dead v1 rows, each with a matching FTS entry.
    for _ in 0..3 {
        seed_unrotatable_v1_text_row(&db, &foreign);
    }
    let dead_ids: Vec<String> = db
        .conn()
        .prepare("SELECT id FROM clipboard_items WHERE key_version = 1")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for id in &dead_ids {
        db.conn()
            .execute(
                "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, 'dead text')",
                rusqlite::params![id],
            )
            .unwrap();
    }

    // After purge: clipboard_items and clipboard_fts must both be empty
    // for the purged ids (no orphan rows in either direction).
    let deleted = db.purge_dead_v1_rows().unwrap();
    assert_eq!(deleted, 3, "all 3 v1 rows must be deleted");

    let fts_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        fts_count, 0,
        "FTS must be empty after purge — no orphan rows"
    );
    let items_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(items_count, 0, "clipboard_items must be empty after purge");
}

#[test]
fn purge_dead_v1_rows_removes_orphaned_fts_entries() {
    let db = Database::open_in_memory().unwrap();
    let foreign = [0xEFu8; 32];

    // Seed a dead v1 row and give it a matching FTS entry, mirroring the
    // (id, content_text) shape that insert_item writes.
    seed_unrotatable_v1_text_row(&db, &foreign);
    let dead_id: String = db
        .conn()
        .query_row(
            "SELECT id FROM clipboard_items WHERE key_version = 1 LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, 'stale text')",
            rusqlite::params![dead_id],
        )
        .unwrap();

    db.purge_dead_v1_rows().unwrap();

    let fts_remaining: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            rusqlite::params![dead_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fts_remaining, 0, "orphaned FTS entry must be purged too");
}
