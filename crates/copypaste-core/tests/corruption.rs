//! Wave 3.3 — Database corruption + rekey integration tests.
//!
//! Covers:
//!   * arch MEDIUM #16 — `Database::rekey` is the supported key-rotation API.
//!     The old key MUST stop working and the new key MUST decrypt all data.
//!   * edge LOW #35  — when SQLite/SQLCipher cannot read the file (truncated
//!     WAL, scribbled-over main file), `Database::open` MUST return an error
//!     rather than silently producing an empty database.
//!
//! These are integration tests (separate binary) so they exercise the public
//! `copypaste_core::Database` surface exactly as downstream crates use it.
//!
//! NOTE: the existing `db.rs` unit test `rekey_changes_encryption_key`
//! covers the happy path with an empty schema. This file additionally
//! verifies that *data* survives the rekey, and that hostile file edits
//! cannot bypass the encryption layer.

use copypaste_core::{count_items, insert_item, ClipboardItem, Database};
use tempfile::tempdir;

/// arch MEDIUM #16: rekey rotates the key and preserves all rows.
#[test]
fn rekey_changes_key_and_data_still_readable() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rekey_data.db");
    let old_key = [0x11u8; 32];
    let new_key = [0x22u8; 32];

    // Phase 1: open with old key, insert one row, rekey, drop.
    {
        let db = Database::open(&path, &old_key).expect("open with old key");
        let item = ClipboardItem::new_text(b"payload".to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).expect("insert pre-rekey row");

        let _db = db.rekey(&new_key).expect("rekey to new key");
    }

    // Phase 2: old key must NOT open the file anymore.
    let reopen_old = Database::open(&path, &old_key);
    assert!(
        reopen_old.is_err(),
        "old key should be rejected after rekey, got Ok"
    );

    // Phase 3: new key opens the file AND the row is intact.
    let db2 = Database::open(&path, &new_key).expect("open with new key");
    let n = count_items(&db2).expect("count items");
    assert_eq!(n, 1, "rekey must not lose rows");
}

/// arch MEDIUM #16: rekey to the *same* key is a no-op (does not corrupt).
#[test]
fn rekey_to_same_key_is_noop() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rekey_same.db");
    let key = [0x33u8; 32];

    {
        let db = Database::open(&path, &key).expect("open");
        let item = ClipboardItem::new_text(b"x".to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).expect("insert");
        let _db = db.rekey(&key).expect("rekey to same key");
    }

    let db = Database::open(&path, &key).expect("reopen with same key");
    assert_eq!(count_items(&db).unwrap(), 1);
}

/// edge LOW #35: truncated/garbled WAL must NOT silently lose data.
///
/// Open the DB, insert a row, drop. Then scribble over the `-wal` sidecar.
/// Re-opening MUST either succeed and return the committed row (WAL replay
/// failed gracefully) OR return an error — it must NEVER return `Ok` with
/// `count == 0`, which would mean the user lost their clipboard silently.
#[test]
fn corrupted_wal_does_not_silently_lose_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("wal_corrupt.db");
    let key = [0x44u8; 32];

    // Insert one row and drop the connection so the WAL is flushed.
    {
        let db = Database::open(&path, &key).expect("open");
        let item = ClipboardItem::new_text(b"committed".to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).expect("insert");
    }

    // Scribble over the WAL file if present. With wal_checkpoint(TRUNCATE)
    // on close the WAL may be 0 bytes; in that case there's nothing to
    // corrupt and the test is trivially safe.
    let wal = path.with_extension("db-wal");
    if wal.exists() && std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0) > 0 {
        std::fs::write(&wal, b"GARBAGE-NOT-A-VALID-WAL-FRAME").expect("scribble wal");
    }

    // Reopen. Either branch is acceptable; silent data loss is NOT.
    match Database::open(&path, &key) {
        Ok(db) => {
            let n = count_items(&db).expect("count after wal scribble");
            assert!(
                n >= 1,
                "DB opened but committed row vanished — silent data loss"
            );
        }
        Err(_) => {
            // Acceptable: SQLite refused to open the corrupted database.
        }
    }
}

/// edge LOW #35: a completely scribbled main DB file must return an error
/// from `Database::open`, never an `Ok` with a fresh-looking empty schema.
#[test]
fn corrupted_main_file_returns_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("main_corrupt.db");
    let key = [0x55u8; 32];

    {
        let db = Database::open(&path, &key).expect("create");
        let item = ClipboardItem::new_text(b"row".to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).expect("insert");
    }

    // Overwrite the first 4 KiB of the main DB file with garbage. This
    // destroys the SQLite header and any encrypted pages SQLCipher would
    // need to derive the page key, guaranteeing an open failure.
    let mut bytes = std::fs::read(&path).expect("read db");
    let scribble_len = bytes.len().min(4096);
    for b in &mut bytes[..scribble_len] {
        *b = 0xAB;
    }
    std::fs::write(&path, &bytes).expect("write scribbled db");

    let result = Database::open(&path, &key);
    assert!(
        result.is_err(),
        "scribbled main file must fail to open, got Ok — silent data loss"
    );
}
