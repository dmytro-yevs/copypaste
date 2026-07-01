use super::images::parse_file_id;
use super::repair::fetch_kv2_blob_batch;
use super::*;
use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks};
use crate::crypto::encrypt::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad, NONCE_SIZE,
};
use crate::image::{chunks_from_blob, chunks_to_blob, IMAGE_CHUNK_SIZE};
use crate::storage::db::Database;
use crate::storage::items::ItemId;
use rusqlite::params;
use uuid::Uuid;

/// Seed a row that looks exactly like a v1-key-encrypted text item:
/// `key_version = 1`, AEAD built with the legacy 2-arg AAD format
/// `"{item_id}|3"`. Returns `(row_id, item_id, plaintext)`.
fn seed_v1_row(
    db: &Database,
    v1_key: &[u8; 32],
    plaintext: &[u8],
) -> (String, String, Vec<u8>) {
    let row_id = Uuid::new_v4().to_string();
    let item_id = Uuid::new_v4().to_string();
    let aad = build_item_aad(&ItemId::from(item_id.as_str()), AAD_SCHEMA_V3);
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, v1_key, &aad).unwrap();

    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
             VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
            params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
        )
        .unwrap();

    (row_id, item_id, plaintext.to_vec())
}

#[test]
fn migrate_50_rows_all_land_on_key_version_2() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x11u8; 32];
    let v2_key = [0x22u8; 32];

    let mut originals: Vec<(String, Vec<u8>)> = Vec::with_capacity(50);
    for i in 0..50u8 {
        let pt = format!("plaintext-{}", i).into_bytes();
        let (row_id, _item_id, _) = seed_v1_row(&db, &v1_key, &pt);
        originals.push((row_id, pt));
    }

    let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 50, "all 50 v1 rows must be re-encrypted");

    // Every row must now be at key_version=2.
    let remaining_v1: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining_v1, 0);

    // Every row must decrypt cleanly with the v2 key + v4 AAD AND yield
    // the original plaintext (proves migration preserved content).
    for (row_id, expected_pt) in &originals {
        let (item_id, content, nonce_blob): (String, Vec<u8>, Vec<u8>) = db
            .conn()
            .query_row(
                "SELECT item_id, content, content_nonce \
                 FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&nonce_blob);
        let aad_v2 = build_item_aad_v2(&ItemId::from(item_id.as_str()), AAD_SCHEMA_V4, 2);
        let pt = decrypt_item_with_aad(&content, &nonce, &v2_key, &aad_v2).unwrap();
        assert_eq!(&pt, expected_pt, "v2 plaintext must match v1 plaintext");
    }
}

#[test]
fn migration_is_idempotent() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x33u8; 32];
    let v2_key = [0x44u8; 32];

    for i in 0..5u8 {
        seed_v1_row(&db, &v1_key, &[i, i, i, i]);
    }
    let first = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    let second = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();

    assert_eq!(first, 5);
    assert_eq!(second, 0, "second run must find no v1 rows");
}

#[test]
fn migration_with_no_v1_rows_returns_zero() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x55u8; 32];
    let v2_key = [0x66u8; 32];

    let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 0);
}

#[test]
fn migrated_row_is_undecryptable_with_v1_key() {
    // The whole point of the rotation: a v2-encrypted row must NOT be
    // decryptable with the v1 key even by an attacker who knows the
    // item_id and tries every plausible AAD format.
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x77u8; 32];
    let v2_key = [0x88u8; 32];

    let (row_id, item_id, _plain) = seed_v1_row(&db, &v1_key, b"super secret");
    migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();

    let (content, nonce_blob): (Vec<u8>, Vec<u8>) = db
        .conn()
        .query_row(
            "SELECT content, content_nonce FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_blob);

    // Attempt #1: v1 key + v3 AAD (legacy combo)
    let aad_v3 = build_item_aad(&ItemId::from(item_id.as_str()), AAD_SCHEMA_V3);
    assert!(decrypt_item_with_aad(&content, &nonce, &v1_key, &aad_v3).is_err());

    // Attempt #2: v1 key + v4 AAD with key_version=2
    let aad_v4 = build_item_aad_v2(&ItemId::from(item_id.as_str()), AAD_SCHEMA_V4, 2);
    assert!(decrypt_item_with_aad(&content, &nonce, &v1_key, &aad_v4).is_err());
}

#[test]
fn corrupt_v1_row_does_not_abort_the_sweep() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x99u8; 32];
    let v2_key = [0xAAu8; 32];

    // Seed one good v1 row + one row that was encrypted under a
    // *different* v1 key (simulating an undecryptable-under-current-key
    // row — could happen after a key rotation race).
    let (good_id, _item, _pt) = seed_v1_row(&db, &v1_key, b"good");
    let other_v1_key = [0xBBu8; 32];
    let (bad_id, _item2, _pt2) = seed_v1_row(&db, &other_v1_key, b"bad");

    let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 1, "sweep must rotate the one decryptable row");

    // Good row is now at key_version=2; bad row is still at 1.
    let good_kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![good_id],
            |r| r.get(0),
        )
        .unwrap();
    let bad_kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![bad_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(good_kv, 2);
    assert_eq!(bad_kv, 1, "undecryptable row must be left at key_version=1");
}

// ── Image-chunk migration (Cnew / TODO(v0.4)) ──────────────────────────
//
// Image rows store their content as a chunk blob (no item-level
// `content_nonce`; nonces live per-chunk inside the blob). The per-chunk
// AEAD AAD binds `(CHUNK_FORMAT_V1, file_id, chunk_index, total_chunks,
// is_final)` but NOT `key_version`. The row's `key_version` column is the
// binding that records which HKDF key generation the chunks were encrypted
// under. To carry an image row through the v1→v2 rotation we must decrypt
// every chunk with the v1 key, re-encrypt with the v2 key (fresh nonces),
// re-serialise the blob, and bump `key_version` to 2.

/// Seed a row that looks exactly like a v1-key-encrypted image item:
/// `content_type = 'image'`, `key_version = 1`, `content` holding a chunk
/// blob produced with the v1 key, `content_nonce = NULL`, and `blob_ref`
/// carrying the JSON metadata (`file_id` as a 16-element byte array, the
/// same shape `daemon::handle_image` writes).
///
/// Returns `(row_id, file_id, plaintext)`.
fn seed_v1_image_row(
    db: &Database,
    v1_key: &[u8; 32],
    plaintext: &[u8],
    chunk_size: usize,
) -> (String, [u8; 16], Vec<u8>) {
    let row_id = Uuid::new_v4().to_string();
    let item_id = Uuid::new_v4().to_string();
    let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();

    let chunks = encrypt_chunks(plaintext, v1_key, &file_id, chunk_size).unwrap();
    let blob = chunks_to_blob(&chunks).unwrap();

    // Mirror the JSON shape from daemon::handle_image: a `file_id` array
    // of 16 numbers (Rust `{:?}` debug-format of the byte array).
    let meta_json = format!(
        r#"{{"width":2,"height":2,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        plaintext.len(),
        chunks.len(),
        file_id
    );

    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
             VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,1)",
            params![row_id, item_id, blob, meta_json, 1i64],
        )
        .unwrap();

    (row_id, file_id, plaintext.to_vec())
}

/// RED→GREEN: a v1-key image-chunk row must be carried through the v4
/// rotation — decrypted with the v1 key, re-encrypted with the v2 key, and
/// landed on `key_version = 2`, while remaining decryptable to the original
/// plaintext under the v2 key (and the preserved `file_id` AAD).
#[test]
fn image_chunk_row_migrates_to_key_version_2() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x11u8; 32];
    let v2_key = [0x22u8; 32];

    // Multi-chunk payload to exercise the per-chunk re-encryption loop.
    let plaintext: Vec<u8> = (0..(IMAGE_CHUNK_SIZE + 137))
        .map(|i| (i % 251) as u8)
        .collect();
    let (row_id, file_id, expected) =
        seed_v1_image_row(&db, &v1_key, &plaintext, IMAGE_CHUNK_SIZE);

    let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 1, "the one v1 image row must be rotated");

    // Row must now be at key_version=2.
    let kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kv, 2, "image row must land on key_version=2");

    // The blob must now decrypt with the v2 key + preserved file_id AAD,
    // yielding the original plaintext.
    let blob: Vec<u8> = db
        .conn()
        .query_row(
            "SELECT content FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| r.get(0),
        )
        .unwrap();
    let chunks = chunks_from_blob(&blob).unwrap();
    let recovered = decrypt_chunks(&chunks, &v2_key, &file_id).unwrap();
    assert_eq!(
        recovered, expected,
        "v2 chunk decrypt must match v1 plaintext"
    );

    // And it must NOT decrypt with the old v1 key anymore.
    assert!(
        decrypt_chunks(&chunks, &v1_key, &file_id).is_err(),
        "migrated image chunks must not decrypt with the v1 key"
    );
}

/// The image migration must be idempotent: a second run finds no v1 image
/// rows (the first run bumped them all to key_version=2).
#[test]
fn image_chunk_migration_is_idempotent() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x33u8; 32];
    let v2_key = [0x44u8; 32];

    for i in 0..3u8 {
        seed_v1_image_row(&db, &v1_key, &[i; 64], 16);
    }
    let first = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
    let second = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(first, 3);
    assert_eq!(second, 0, "second run must find no v1 image rows");
}

/// A corrupt/undecryptable image row (encrypted under a different v1 key)
/// must be left at key_version=1 and must not abort the sweep.
#[test]
fn corrupt_image_row_does_not_abort_the_sweep() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x55u8; 32];
    let v2_key = [0x66u8; 32];
    let other_v1_key = [0x77u8; 32];

    let (good_id, _fid, _pt) = seed_v1_image_row(&db, &v1_key, b"good image bytes", 8);
    let (bad_id, _fid2, _pt2) = seed_v1_image_row(&db, &other_v1_key, b"bad image bytes", 8);

    let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 1, "only the decryptable image row must rotate");

    let good_kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![good_id],
            |r| r.get(0),
        )
        .unwrap();
    let bad_kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![bad_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(good_kv, 2);
    assert_eq!(
        bad_kv, 1,
        "undecryptable image row must be left at key_version=1"
    );
}

/// `parse_file_id` must extract the 16-byte array from the exact JSON shape
/// that `daemon::handle_image` writes (Rust `{:?}` debug-format of a byte
/// array, embedded among other metadata fields).
#[test]
fn parse_file_id_reads_daemon_json_shape() {
    let file_id: [u8; 16] = [0, 255, 1, 2, 3, 200, 17, 99, 16, 44, 78, 123, 5, 6, 7, 250];
    let json = format!(
        r#"{{"width":2,"height":2,"original_size":42,"chunk_count":1,"file_id":{:?}}}"#,
        file_id
    );
    let parsed = parse_file_id("row-1", Some(&json)).unwrap();
    assert_eq!(parsed, file_id);
}

/// `parse_file_id` must reject malformed / missing metadata with
/// `ImageMeta` rather than panicking.
#[test]
fn parse_file_id_rejects_bad_metadata() {
    // Missing blob_ref entirely.
    assert!(matches!(
        parse_file_id("r", None),
        Err(MigrationV4Error::ImageMeta { .. })
    ));
    // No file_id field.
    assert!(matches!(
        parse_file_id("r", Some(r#"{"width":2}"#)),
        Err(MigrationV4Error::ImageMeta { .. })
    ));
    // Wrong length (15 elements).
    let short = r#"{"file_id":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]}"#;
    assert!(matches!(
        parse_file_id("r", Some(short)),
        Err(MigrationV4Error::ImageMeta { .. })
    ));
    // Non-u8 element.
    let bad = r#"{"file_id":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,999]}"#;
    assert!(matches!(
        parse_file_id("r", Some(bad)),
        Err(MigrationV4Error::ImageMeta { .. })
    ));
}

// ── Termination guard regression (HIGH) ───────────────────────────────
//
// If an ENTIRE batch (BATCH_SIZE rows) all fail to rotate, the
// `WHERE key_version = 1` predicate would re-fetch the exact same rows on
// the next iteration and the sweep would loop forever, hanging daemon
// startup. The guard breaks out once a full batch produces zero
// successful rotations. These tests seed a full batch of undecryptable
// rows and assert the sweep TERMINATES (and leaves them at v1).
//
// The sweep itself is already bounded by the fix, but to guarantee the
// test can never hang the whole suite if the guard ever regresses, we arm
// a watchdog thread before the call: if the sweep hasn't returned (and
// cleared the flag) within a generous budget, the watchdog aborts the
// process with a clear message instead of letting CI block forever. A
// worker-thread approach isn't usable here because rusqlite's in-memory
// `Connection` is per-connection and `!Send`, so the sweep must run inline
// on this thread against the borrowed `&Database`.

#[test]
fn full_batch_of_undecryptable_text_rows_terminates() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0xC1u8; 32];
    let v2_key = [0xC2u8; 32];
    // Every seeded row is encrypted under a DIFFERENT key, so none of them
    // decrypt with `v1_key` — a full batch of guaranteed failures.
    let other_v1_key = [0xCEu8; 32];

    for i in 0..BATCH_SIZE {
        let pt = format!("undecryptable-{i}").into_bytes();
        seed_v1_row(&db, &other_v1_key, &pt);
    }

    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag = timed_out.clone();
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            // The sweep never set the flag to false → it hung.
            eprintln!(
                "full_batch_of_undecryptable_text_rows_terminates: \
                 sweep hung (>10s) — termination guard regressed"
            );
            std::process::abort();
        }
    });

    let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    timed_out.store(false, std::sync::atomic::Ordering::SeqCst);
    let _ = watchdog.join();

    assert_eq!(rotated, 0, "no row was decryptable, so none may rotate");

    // All BATCH_SIZE rows must still be at key_version=1.
    let remaining_v1: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        remaining_v1 as usize, BATCH_SIZE,
        "every stuck row must be left at key_version=1"
    );
}

#[test]
fn full_batch_of_undecryptable_image_rows_terminates() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0xD1u8; 32];
    let v2_key = [0xD2u8; 32];
    let other_v1_key = [0xDEu8; 32];

    for i in 0..BATCH_SIZE {
        // Distinct payloads, all encrypted under a key the sweep won't have.
        let pt = vec![(i % 256) as u8; 32];
        seed_v1_image_row(&db, &other_v1_key, &pt, 8);
    }

    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag = timed_out.clone();
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            eprintln!(
                "full_batch_of_undecryptable_image_rows_terminates: \
                 sweep hung (>10s) — termination guard regressed"
            );
            std::process::abort();
        }
    });

    let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
    timed_out.store(false, std::sync::atomic::Ordering::SeqCst);
    let _ = watchdog.join();

    assert_eq!(
        rotated, 0,
        "no image row was decryptable, so none may rotate"
    );

    let remaining_v1: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items \
             WHERE key_version = 1 AND content_type = 'image'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        remaining_v1 as usize, BATCH_SIZE,
        "every stuck image row must be left at key_version=1"
    );
}

/// The combined `migrate_v1_to_v2_keys` sweep must rotate BOTH text and
/// image rows so the v4 migration has no remaining `key_version = 1` rows
/// of either type. This is the regression that closes the documented Cnew
/// gap (image chunks previously skipped).
#[test]
fn full_sweep_rotates_text_and_image_rows() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0x88u8; 32];
    let v2_key = [0x99u8; 32];

    // One text row, one image row, both at key_version=1.
    let (text_id, _text_item, _text_pt) = {
        let row_id = Uuid::new_v4().to_string();
        let item_id = Uuid::new_v4().to_string();
        let aad = build_item_aad(&ItemId::from(item_id.as_str()), AAD_SCHEMA_V3);
        let (nonce, ct) = encrypt_item_with_aad(b"text payload", &v1_key, &aad).unwrap();
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,1,1,1)",
                params![row_id, item_id, ct, nonce.to_vec()],
            )
            .unwrap();
        (row_id, item_id, b"text payload".to_vec())
    };
    let (image_id, _fid, _pt) = seed_v1_image_row(&db, &v1_key, b"image payload bytes", 8);

    let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(rotated, 2, "both text and image rows must be rotated");

    let remaining_v1: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        remaining_v1, 0,
        "no key_version=1 rows of any type may remain"
    );

    // Sanity: both rows are at key_version=2.
    for id in [&text_id, &image_id] {
        let kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kv, 2);
    }
}

// ── Mislabeled kv=2 blob repair ──────────────────────────────────────
//
// Before the writer fix, handle_image/handle_file encrypted chunks with
// the v1 key but stamped key_version=2 on the row. The repair function
// must detect these "mislabeled" rows (v1-decrypt succeeds) and
// re-encrypt them with the v2 key. Correctly v2-encrypted rows (v1-
// decrypt fails) must be left unchanged.

/// Seed a mislabeled kv=2 row: content_type='image', key_version=2,
/// BUT the chunk blob was encrypted with the v1 key (the old writer bug).
/// Returns `(row_id, file_id, plaintext)`.
fn seed_mislabeled_kv2_image_row(
    db: &Database,
    v1_key: &[u8; 32],
    plaintext: &[u8],
) -> (String, [u8; 16], Vec<u8>) {
    let row_id = Uuid::new_v4().to_string();
    let item_id = Uuid::new_v4().to_string();
    let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();

    // Encrypt with v1 key (the bug) but stamp key_version=2 (the lie).
    let chunks = encrypt_chunks(plaintext, v1_key, &file_id, IMAGE_CHUNK_SIZE).unwrap();
    let blob = chunks_to_blob(&chunks).unwrap();

    let meta_json = format!(
        r#"{{"width":4,"height":4,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        plaintext.len(),
        chunks.len(),
        file_id
    );

    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
             VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
            params![row_id, item_id, blob, meta_json, 1i64],
        )
        .unwrap();

    (row_id, file_id, plaintext.to_vec())
}

/// A mislabeled kv=2 image row (encrypted with v1, stamped kv=2) must be
/// re-encrypted with the v2 key by `repair_mislabeled_kv2_blob_rows`.
/// After repair: repaired_count=1, row is genuinely v2-decryptable, and
/// v1-decrypt fails.
#[test]
fn kv2_mislabeled_image_row_repairs_via_migration() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0xA1u8; 32];
    let v2_key = [0xA2u8; 32];

    let plaintext = b"mislabeled image bytes for repair test";
    let (row_id, file_id, expected_pt) = seed_mislabeled_kv2_image_row(&db, &v1_key, plaintext);

    let repaired = repair_mislabeled_kv2_blob_rows(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(repaired, 1, "exactly one mislabeled row must be repaired");

    // Retrieve the updated blob.
    let blob: Vec<u8> = db
        .conn()
        .query_row(
            "SELECT content FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| r.get(0),
        )
        .unwrap();
    let chunks = chunks_from_blob(&blob).unwrap();

    // Must now decrypt with v2 key.
    let recovered = decrypt_chunks(&chunks, &v2_key, &file_id)
        .expect("repaired row must decrypt with v2 key");
    assert_eq!(recovered, expected_pt, "v2 plaintext must match original");

    // Must NOT decrypt with v1 key anymore.
    assert!(
        decrypt_chunks(&chunks, &v1_key, &file_id).is_err(),
        "repaired row must NOT decrypt with v1 key"
    );

    // key_version must still be 2 (stamp unchanged).
    let kv: i64 = db
        .conn()
        .query_row(
            "SELECT key_version FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kv, 2, "key_version stamp must remain 2 after repair");
}

/// CopyPaste-44rq.32/60: repair must process rows in BATCH_SIZE pages and
/// not load all rows into memory at once.  Seed 3×BATCH_SIZE mislabeled
/// rows; every row must be repaired and each fetch page must return at most
/// BATCH_SIZE rows.
///
/// The pagination is verified indirectly: `fetch_kv2_blob_batch` is called
/// with a `rowid` cursor, so each call returns a disjoint window.  We
/// confirm correctness by asserting total repaired == 3×BATCH_SIZE and
/// that every row becomes genuinely v2-decryptable (v1-decrypt now fails).
#[test]
fn repair_processes_multi_batch_in_pages() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0xC1u8; 32];
    let v2_key = [0xC2u8; 32];

    // Seed 3 × BATCH_SIZE mislabeled rows.
    let n = BATCH_SIZE * 3;
    let mut row_ids: Vec<(String, [u8; 16])> = Vec::with_capacity(n);
    for i in 0..n {
        let plaintext = vec![(i % 256) as u8; 64];
        let (row_id, file_id, _) = seed_mislabeled_kv2_image_row(&db, &v1_key, &plaintext);
        row_ids.push((row_id, file_id));
    }

    let repaired = repair_mislabeled_kv2_blob_rows(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(repaired, n, "all {n} mislabeled rows must be repaired");

    // Every row must now genuinely v2-decrypt (v1-decrypt must fail).
    for (row_id, file_id) in &row_ids {
        let blob: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        let chunks = chunks_from_blob(&blob).unwrap();
        assert!(
            decrypt_chunks(&chunks, &v2_key, file_id).is_ok(),
            "row {row_id} must decrypt with v2 key after repair"
        );
        assert!(
            decrypt_chunks(&chunks, &v1_key, file_id).is_err(),
            "row {row_id} must NOT decrypt with v1 key after repair"
        );
    }

    // Verify pagination: fetch the first page at cursor=0 and assert it
    // returns exactly BATCH_SIZE rows (confirming the query is bounded).
    let first_page = fetch_kv2_blob_batch(&db, 0).unwrap();
    // After repair all rows are correctly v2-encrypted, so v1-decrypt
    // will fail for each — but that doesn't affect cursor pagination.
    // What we care about is that the page size is BATCH_SIZE.
    assert_eq!(
        first_page.len(),
        BATCH_SIZE,
        "first page must contain exactly BATCH_SIZE rows, not all {n}"
    );
}

/// A correctly v2-encrypted kv=2 row (v1-decrypt fails) must be left
/// completely unchanged by `repair_mislabeled_kv2_blob_rows`.
/// repaired_count must be 0.
#[test]
fn kv2_correctly_encrypted_row_not_touched_by_repair_migration() {
    let db = Database::open_in_memory().unwrap();
    let v1_key = [0xB1u8; 32];
    let v2_key = [0xB2u8; 32];

    let plaintext = b"genuinely v2-encrypted image bytes";
    let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();
    let row_id = Uuid::new_v4().to_string();
    let item_id = Uuid::new_v4().to_string();

    // Encrypt with v2 key (correct).
    let chunks = encrypt_chunks(plaintext, &v2_key, &file_id, IMAGE_CHUNK_SIZE).unwrap();
    let blob = chunks_to_blob(&chunks).unwrap();
    let original_blob = blob.clone();

    let meta_json = format!(
        r#"{{"width":2,"height":2,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        plaintext.len(),
        chunks.len(),
        file_id
    );

    db.conn()
        .execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, content, content_nonce, blob_ref, \
              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
             VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
            params![row_id, item_id, blob, meta_json, 1i64],
        )
        .unwrap();

    let repaired = repair_mislabeled_kv2_blob_rows(&db, &v1_key, &v2_key).unwrap();
    assert_eq!(
        repaired, 0,
        "correctly v2-encrypted row must NOT be repaired"
    );

    // Content blob must be byte-for-byte identical (untouched).
    let stored_blob: Vec<u8> = db
        .conn()
        .query_row(
            "SELECT content FROM clipboard_items WHERE id = ?1",
            params![row_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        stored_blob, original_blob,
        "content must be unchanged for a correctly v2-encrypted row"
    );
}
