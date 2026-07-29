//! Migration oracle: open real v1 database files and upgrade them to current.
//!
//! Invariant 1 of the storage port manifest is "open and migrate any v1..v15
//! database". Every other migration test in the tree builds its starting
//! database by running the current code, so it proves the migrations are
//! self-consistent — not that a file written by a released version still opens.
//! These fixtures are actual v1 files, committed as bytes.
//!
//! **This file must survive the rewrite.** It is the check that a
//! reimplementation has not quietly stranded existing installs.
//!
//! Each test copies the fixture into a temp dir first. Opening a database
//! migrates it in place, so running against the fixture directly would consume
//! the evidence on first run and pass vacuously ever after.

use copypaste_core::{build_item_aad, decrypt_item_with_aad, derive_storage_key_v1, Database};
use std::path::{Path, PathBuf};

const TEST_DEVICE_SECRET: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

/// Rows as written into the v1 fixtures, with the sensitivity flag that decides
/// whether the row is allowed to appear in the search index.
const EXPECTED: &[(&str, &str, &str, bool)] = &[
    ("row-0001", "item-0001", "first clipboard entry", false),
    (
        "row-0002",
        "item-0002",
        "друга з багатобайтовим текстом 🎉",
        false,
    ),
    ("row-0003", "item-0003", "AKIAIOSFODNN7EXAMPLE", true),
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-v0.4.1")
        .join(name)
}

/// Copy the fixture somewhere writable so the original stays pristine.
fn stage(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dst = dir.path().join(name);
    std::fs::copy(fixture(name), &dst)
        .unwrap_or_else(|e| panic!("copy fixture {name}: {e}"));
    (dir, dst)
}

fn item_key() -> [u8; 32] {
    *derive_storage_key_v1(&TEST_DEVICE_SECRET)
}

/// Open, migrate, and assert the schema landed on the current version with all
/// rows intact and still decryptable under the v1 key.
fn assert_migrates_and_preserves_rows(file: &str) {
    let (_guard, path) = stage(file);
    let key = item_key();

    let db = Database::open(&path, &key)
        .unwrap_or_else(|e| panic!("open + migrate {file}: {e:?}"));
    let conn = db.conn();

    // Asserted against the literal version these fixtures were captured at,
    // not against whatever constant the code currently defines — comparing the
    // code to itself would pass no matter how badly the ladder broke.
    const FIXTURE_MIN_SCHEMA_VERSION: i64 = 15;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("read user_version");
    assert!(
        version >= FIXTURE_MIN_SCHEMA_VERSION,
        "{file} reached schema v{version}, expected at least v{FIXTURE_MIN_SCHEMA_VERSION}"
    );

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .expect("count rows");
    assert_eq!(count, EXPECTED.len() as i64, "{file} lost or gained rows");

    for (id, item_id, plaintext, _) in EXPECTED {
        let (content, nonce, key_version): (Vec<u8>, Vec<u8>, i64) = conn
            .query_row(
                "SELECT content, content_nonce, key_version FROM clipboard_items WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or_else(|e| panic!("{file}: row {id} missing after migration: {e}"));

        // Migration must not silently relabel rows it did not actually
        // re-encrypt — that mislabelling is what once hid unrotated blobs from
        // the rotation sweep.
        assert_eq!(
            key_version, 1,
            "{file}: row {id} was stamped key_version={key_version} without being re-encrypted"
        );

        let nonce: [u8; 24] = nonce.try_into().expect("24-byte nonce");
        let aad = build_item_aad(&item_id.to_string().into(), 3);
        let got = decrypt_item_with_aad(&content, &nonce, &key, &aad)
            .unwrap_or_else(|e| panic!("{file}: row {id} no longer decrypts: {e:?}"));
        assert_eq!(
            String::from_utf8_lossy(&got),
            *plaintext,
            "{file}: row {id} plaintext changed"
        );
    }
}

#[test]
fn migrates_encrypted_v1_database_to_current() {
    assert_migrates_and_preserves_rows("v1-encrypted.db");
}

/// A pre-SQLCipher install: an unencrypted file must be transparently encrypted
/// in place on open. This is the path most likely to be dropped in a rewrite,
/// because nobody still has such a database — except the users who never
/// upgraded, who would lose everything.
#[test]
fn migrates_and_encrypts_plaintext_v1_database() {
    assert_migrates_and_preserves_rows("v1-plaintext.db");

    // And prove it really was encrypted rather than merely migrated: after the
    // upgrade the file must no longer be readable without the key.
    let (_guard, path) = stage("v1-plaintext.db");
    let key = item_key();
    {
        let db = Database::open(&path, &key).expect("open + migrate");
        let _ = db.conn();
    }
    let raw = rusqlite::Connection::open(&path).expect("reopen file");
    assert!(
        raw.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .is_err(),
        "plaintext database was migrated but never encrypted"
    );
}

/// ADR-015: a sensitive row must never be searchable. The v1 fixtures contain a
/// sensitive row that IS indexed, reproducing the pre-v13 leak, so this asserts
/// the migration actually purges it rather than only preventing new leaks.
#[test]
fn migration_purges_sensitive_rows_from_search_index() {
    let (_guard, path) = stage("v1-encrypted.db");
    let db = Database::open(&path, &item_key()).expect("open + migrate");
    let conn = db.conn();

    for (id, _, _, sensitive) in EXPECTED {
        let indexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .expect("count fts rows");
        if *sensitive {
            assert_eq!(
                indexed, 0,
                "sensitive row {id} is still in the search index after migration"
            );
        } else {
            assert_eq!(indexed, 1, "non-sensitive row {id} lost its index entry");
        }
    }
}
