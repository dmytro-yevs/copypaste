//! Regression test for CopyPaste-a8i4.
//!
//! Assert that opening a SQLCipher database with the WRONG key returns `Err`
//! rather than silently succeeding. A silent success with a wrong key would
//! mean an attacker who obtains the encrypted database file but not the key
//! could open it, enumerate any unencrypted metadata, or trigger an untested
//! code path that later causes data corruption.

use copypaste_core::Database;
use tempfile::tempdir;

/// Create a valid encrypted database with `correct_key`, then attempt to
/// open the SAME file with `wrong_key`. Expect `Err`.
#[test]
fn open_with_wrong_key_returns_err() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("wrong_key_test.db");

    let correct_key = [0x11u8; 32];
    let wrong_key = [0xFFu8; 32];

    // First open: create and seal the encrypted database.
    {
        let _db = Database::open(&path, &correct_key).expect("should open with correct key");
        // `_db` is dropped here — WAL is flushed, connection closed.
    }

    // Second open: wrong key must return Err, never Ok.
    let result = Database::open(&path, &wrong_key);
    assert!(
        result.is_err(),
        "Database::open with wrong key must return Err (CopyPaste-a8i4), got Ok"
    );
}

/// Confirm the correct key still opens the same file successfully after the
/// failed wrong-key attempt (the failure must not corrupt the file).
#[test]
fn correct_key_works_after_failed_wrong_key_attempt() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("key_after_fail_test.db");

    let correct_key = [0x22u8; 32];
    let wrong_key = [0x99u8; 32];

    // Create.
    {
        let _db = Database::open(&path, &correct_key).expect("create");
    }

    // Wrong key attempt — ignore the error, just ensure no panic.
    let _ = Database::open(&path, &wrong_key);

    // Correct key must still open cleanly.
    let result = Database::open(&path, &correct_key);
    assert!(
        result.is_ok(),
        "correct key must still work after a failed wrong-key attempt"
    );
}

/// An all-zeros key and an all-ones key are distinct — wrong key must be rejected.
#[test]
fn all_zeros_key_vs_all_ones_key_are_distinct() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("distinct_keys_test.db");

    let key_zeros = [0x00u8; 32];
    let key_ones = [0x01u8; 32];

    {
        let _db = Database::open(&path, &key_zeros).expect("create with zeros key");
    }

    let result = Database::open(&path, &key_ones);
    assert!(
        result.is_err(),
        "opening a zeros-keyed database with a ones key must fail"
    );
}
