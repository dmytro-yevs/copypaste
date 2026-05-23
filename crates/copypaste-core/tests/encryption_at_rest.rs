//! At-rest encryption verification tests (SQLCipher).
//!
//! These tests assert that data written via `Database::open` + `insert_item`
//! is genuinely encrypted on disk — i.e. the literal payload bytes are NOT
//! recoverable by a raw scan of the database file (or its WAL/SHM sidecars).
//!
//! Scan strategy:
//!   1. Open DB with a 32-byte key, INSERT a row whose `content` field carries
//!      a unique magic substring.
//!   2. Drop the `Database` (closes the connection, flushes WAL via WAL
//!      checkpoint on the last close in `PRAGMA journal_mode=WAL`).
//!   3. For belt-and-braces durability we also force a `wal_checkpoint(TRUNCATE)`
//!      while the connection is still open, so all encrypted pages live in the
//!      main `.db` file at scan time (the `-wal` file may be empty/absent).
//!   4. Read raw bytes of the main `.db` file AND any `-wal` / `-shm` sidecars
//!      that exist, concatenate them, and search for the magic substring.
//!   5. Encrypted file MUST NOT contain the literal bytes anywhere.
//!
//! For the SQLite-header test we exploit the fact that an unencrypted SQLite
//! file begins with the fixed ASCII string `"SQLite format 3\0"` (16 bytes).
//! SQLCipher encrypts page 1 in full — including the header — so the first 16
//! bytes on disk MUST NOT match that string. We do not assert any specific
//! alternative pattern (it's pseudo-random ciphertext); we only assert the
//! plaintext header is absent.

use copypaste_core::storage::{Database, insert_item, ClipboardItem, DbError};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Read the main DB file and every SQLite sidecar (`-wal`, `-shm`, `-journal`)
/// that exists, then concatenate the bytes. Returns the combined buffer.
///
/// Scanning the sidecars is critical: in WAL mode, recently-written pages may
/// live in the `-wal` file until checkpointed. A test that only scanned the
/// main file could pass even if plaintext leaked into the WAL.
fn read_all_db_bytes(db_path: &Path) -> Vec<u8> {
    let mut all = Vec::new();

    let candidates: Vec<PathBuf> = vec![
        db_path.to_path_buf(),
        path_with_suffix(db_path, "-wal"),
        path_with_suffix(db_path, "-shm"),
        path_with_suffix(db_path, "-journal"),
    ];

    for p in candidates {
        if p.exists() {
            if let Ok(bytes) = fs::read(&p) {
                all.extend_from_slice(&bytes);
            }
        }
    }

    all
}

fn path_with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Returns true if `haystack` contains `needle` as a contiguous subslice.
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Force every dirty page out of the WAL into the main DB file so the scan
/// covers all written pages even if the sidecar gets truncated/deleted.
fn checkpoint_wal(db: &Database) {
    let _ = db.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
}

#[test]
fn db_file_bytes_do_not_contain_plaintext_payload() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("atrest_text.db");
    let key = [0x11u8; 32];

    const MAGIC: &[u8] = b"MAGIC_SECRET_STRING_12345";

    {
        let db = Database::open(&path, &key).unwrap();
        // Put the magic directly into `content` (the encrypted payload column).
        // In production this column holds ciphertext, but here we feed it the
        // plaintext literal — that's exactly the worst case we want to verify
        // SQLCipher catches: even if a caller forgot to encrypt at the app
        // layer, the DB-level encryption must still hide the bytes on disk.
        let item = ClipboardItem::new_text(MAGIC.to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).unwrap();
        checkpoint_wal(&db);
    } // Database drops here → connection closes → final flush.

    let bytes = read_all_db_bytes(&path);
    assert!(
        !bytes.is_empty(),
        "expected non-empty DB file at {}",
        path.display()
    );
    assert!(
        !contains_subslice(&bytes, MAGIC),
        "plaintext payload {:?} was found in encrypted DB bytes — at-rest encryption FAILED",
        std::str::from_utf8(MAGIC).unwrap_or("<binary>")
    );
}

#[test]
fn db_file_bytes_do_not_contain_plaintext_image() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("atrest_image.db");
    let key = [0x22u8; 32];

    // Synthetic "image" payload with a unique 12-byte magic header that won't
    // appear naturally in SQLite metadata or ciphertext-by-chance.
    let mut image_bytes = Vec::with_capacity(4096);
    let magic_header: &[u8] = b"\xDE\xAD\xBE\xEFIMGMAGIC\x00";
    image_bytes.extend_from_slice(magic_header);
    // Pad with deterministic but non-trivial bytes so the total payload is
    // multiple SQLite pages worth — exercises >1 page of encryption.
    for i in 0..4000 {
        image_bytes.push((i % 251) as u8);
    }

    {
        let db = Database::open(&path, &key).unwrap();
        let item = ClipboardItem::new_image(image_bytes.clone(), "{}".to_string(), 1);
        insert_item(&db, &item).unwrap();
        checkpoint_wal(&db);
    }

    let bytes = read_all_db_bytes(&path);
    assert!(
        !bytes.is_empty(),
        "expected non-empty DB file at {}",
        path.display()
    );
    assert!(
        !contains_subslice(&bytes, magic_header),
        "image magic header was found in encrypted DB bytes — at-rest encryption FAILED"
    );
}

#[test]
fn db_file_starts_with_sqlite_format_header_only_if_unencrypted() {
    let dir = tempdir().unwrap();

    // --- Half 1: encrypted DB must NOT start with the plaintext header. ---
    let enc_path = dir.path().join("encrypted.db");
    let key = [0x33u8; 32];
    {
        let db = Database::open(&enc_path, &key).unwrap();
        // Insert a row so the file is populated past just the header page.
        let item = ClipboardItem::new_text(vec![0xAA, 0xBB, 0xCC], vec![0u8; 24], 1);
        insert_item(&db, &item).unwrap();
        checkpoint_wal(&db);
    }

    let enc_bytes = fs::read(&enc_path).unwrap();
    assert!(
        enc_bytes.len() >= 16,
        "encrypted DB file should be at least one page (got {} bytes)",
        enc_bytes.len()
    );
    let enc_prefix = &enc_bytes[..16];
    assert_ne!(
        enc_prefix, &SQLITE_HEADER[..],
        "encrypted DB file MUST NOT start with the plaintext 'SQLite format 3\\0' header — \
         header leak indicates page 1 is not encrypted"
    );

    // --- Half 2: control — a plain SQLite file SHOULD start with the header.
    // This proves the assertion in half 1 is meaningful: SQLite normally writes
    // that signature, and only SQLCipher's full-page encryption hides it.
    let plain_path = dir.path().join("plaintext.db");
    {
        let conn = rusqlite::Connection::open(&plain_path).unwrap();
        // Trigger header materialization by creating any table.
        conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);").unwrap();
    }
    let plain_bytes = fs::read(&plain_path).unwrap();
    assert!(plain_bytes.len() >= 16, "plain SQLite file should have a header");
    assert_eq!(
        &plain_bytes[..16],
        &SQLITE_HEADER[..],
        "control failed: a plain SQLite file should begin with the format-3 header — \
         test environment is not behaving as expected"
    );
}

#[test]
fn wrong_key_open_returns_invalid_key_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("wrong_key.db");

    let key_good = [0x44u8; 32];
    let key_bad = [0x55u8; 32];

    // Create + populate with the good key.
    {
        let db = Database::open(&path, &key_good).unwrap();
        let item = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        insert_item(&db, &item).unwrap();
        checkpoint_wal(&db);
    }

    // Re-open with a wrong key must return an error (NOT panic, NOT silently
    // open an empty DB). SQLCipher surfaces this as SQLITE_NOTADB internally,
    // which `Database::open` maps to `DbError::Sqlite(SqliteFailure(..))`.
    let result = Database::open(&path, &key_bad);
    assert!(
        result.is_err(),
        "opening encrypted DB with wrong key must fail, got Ok"
    );

    match result {
        Err(DbError::Sqlite(e)) => {
            // Accept either the raw SQLITE_NOTADB code or the higher-level
            // DatabaseCorrupt classification — both are valid signals for
            // "the key did not decrypt this file".
            let msg = format!("{}", e);
            // Sanity: message should be non-empty and indicate an SQLite error.
            assert!(!msg.is_empty(), "expected non-empty error message");
        }
        Err(other) => {
            panic!(
                "expected DbError::Sqlite (wrong-key signal), got {:?}",
                other
            );
        }
        Ok(_) => unreachable!(),
    }

    // And the good key must still work — proves the file wasn't corrupted by
    // the failed open attempt.
    let db_ok = Database::open(&path, &key_good).expect("good key must still open");
    let count: i64 = db_ok
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "row inserted before wrong-key probe must survive");
}

#[test]
fn key_rotation_old_key_no_longer_works_new_key_works() {
    // The exposed API is `Database::rekey(&mut self, new_key: &[u8; 32])`.
    let dir = tempdir().unwrap();
    let path = dir.path().join("rotate.db");

    let old_key = [0x66u8; 32];
    let new_key = [0x77u8; 32];

    // Phase 1: create with old key, insert known marker, rotate, drop.
    {
        let mut db = Database::open(&path, &old_key).unwrap();
        let item = ClipboardItem::new_text(b"rotation-marker".to_vec(), vec![0u8; 24], 1);
        insert_item(&db, &item).unwrap();
        db.rekey(&new_key).expect("rekey should succeed");
        checkpoint_wal(&db);
    }

    // Phase 2: old key MUST be rejected.
    let old_result = Database::open(&path, &old_key);
    assert!(
        old_result.is_err(),
        "after rekey, old key must NOT open the database"
    );

    // Phase 3: new key MUST work AND the original row must still be readable
    // (proves rekey re-encrypted in-place, didn't wipe data).
    let db_new = Database::open(&path, &new_key).expect("new key must open after rekey");
    let count: i64 = db_new
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "row inserted before rekey must remain after rekey (data preservation)"
    );
}
