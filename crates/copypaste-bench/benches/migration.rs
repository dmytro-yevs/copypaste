//! Criterion benchmarks for the startup migration / repair paths in
//! `copypaste-core::storage::migration_v4`.
//!
//! ## What is measured
//!
//! * `migrate_v1_to_v2_keys` — re-encrypts text rows from the v1 HKDF key
//!   family to v2. Benchmarked with 100 and 1 000 pre-seeded v1 rows.
//!   This function is called on every daemon start while any v1 rows remain.
//!
//! * `repair_mislabeled_kv2_blob_rows` — scans image/file rows stamped
//!   `key_version = 2` and re-encrypts any that were actually written with
//!   the v1 key (the mislabeled-writer bug). Benchmarked with 100 and 1 000
//!   pre-seeded mislabeled image rows.
//!   This function is also called on every daemon start.
//!
//! ## What could NOT be benchmarked from this crate
//!
//! * `migration_v4_sweep_resumable` — not a public symbol; the resumable
//!   cursor logic is internal to `copypaste-daemon` and cannot be called from
//!   an external bench crate.
//! * `count_dead_v1_rows` / `sweep_poison_rows` — these do not exist as public
//!   functions in the current codebase (as of schema v13). The functions
//!   referenced in the issue description are conceptual names; the actual public
//!   API surface is `migrate_v1_to_v2_keys` and `repair_mislabeled_kv2_blob_rows`.
//!
//! ## Timing note
//!
//! Both functions sleep `INTER_BATCH_SLEEP` (50 ms) between fetch pages of 100
//! rows. The 1 000-row variants therefore include ~450 ms of yielding sleep per
//! iteration — this is intentional: it mirrors the real-world daemon startup
//! cost and gives a realistic regression signal.  Use the 100-row variant for
//! faster iteration during development (single batch, one sleep).
//!
//! ## Seeding approach
//!
//! The public `insert_item` API writes rows with `key_version = 2` (current).
//! To seed `key_version = 1` or mislabeled rows the bench must reach through
//! `Database::conn()` (public) and issue raw SQL INSERTs, mirroring the pattern
//! used in `migration_v4`'s own unit tests.

use copypaste_core::crypto::chunks::{encrypt_chunks, EncryptedChunk};
use copypaste_core::image::{chunks_to_blob, IMAGE_CHUNK_SIZE};
use copypaste_core::storage::migration_v4::{
    migrate_v1_to_v2_keys, repair_mislabeled_kv2_blob_rows,
};
use copypaste_core::{build_item_aad, encrypt_item_with_aad, Database, AAD_SCHEMA_VERSION};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rusqlite::params;

/// v1 key material used to encrypt rows before migration.
const V1_KEY: [u8; 32] = [0x11u8; 32];
/// v2 key material used as the migration target.
const V2_KEY: [u8; 32] = [0x22u8; 32];

/// AAD schema version for the legacy v3 (pre-v4) format — must match
/// `migration_v4::AAD_SCHEMA_V3`. That constant is `pub(crate)` so we
/// re-state the value here; the `migration_v4` module has a compile-time
/// assert that keeps it in sync with `AAD_SCHEMA_VERSION`.
const AAD_SCHEMA_V3: u32 = AAD_SCHEMA_VERSION; // == 3

const ROW_COUNTS: &[usize] = &[100, 1_000];

// ---------------------------------------------------------------------------
// Seed helpers
// ---------------------------------------------------------------------------

/// Open a fresh in-memory SQLCipher DB (no disk I/O).
fn fresh_db() -> Database {
    Database::open_in_memory().expect("open in-memory db")
}

/// Seed `n` text rows encrypted with the v1 key at `key_version = 1`.
///
/// Each row is inserted via raw SQL because the public `insert_item` API always
/// writes `key_version = 2` (current). This mirrors the `seed_v1_row` helper
/// used in `migration_v4`'s own unit tests.
fn seed_v1_text_rows(db: &Database, n: usize) {
    let aad = build_item_aad(&"bench-item-id-fixed".into(), AAD_SCHEMA_V3);
    let plaintext = b"bench plaintext payload 64 bytes xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    let (nonce, ciphertext) =
        encrypt_item_with_aad(plaintext, &V1_KEY, &aad).expect("encrypt v1 row");
    let nonce_bytes = nonce.to_vec();

    for i in 0..n {
        let row_id = format!("row-{i:08}");
        let item_id = format!("item-{i:08}");
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
                params![row_id, item_id, ciphertext, nonce_bytes, i as i64],
            )
            .expect("insert v1 text row");
    }
}

/// Seed `n` image rows that are *mislabeled*: encrypted with the v1 key but
/// stamped `key_version = 2`. This is exactly the bug that
/// `repair_mislabeled_kv2_blob_rows` is designed to detect and fix.
fn seed_mislabeled_kv2_image_rows(db: &Database, n: usize) {
    // Minimal 4-byte image payload; real images would be larger but the
    // benchmark is measuring the per-row overhead, not AEAD throughput.
    let plaintext = b"PNG_";
    let file_id = [0xABu8; 16];

    // Encrypt chunks with the v1 key.
    let chunks: Vec<EncryptedChunk> =
        encrypt_chunks(plaintext, &V1_KEY, &file_id, IMAGE_CHUNK_SIZE)
            .expect("encrypt image chunks");
    let blob = chunks_to_blob(&chunks).expect("serialise chunk blob");

    // blob_ref JSON shape produced by daemon::handle_image — `file_id` as an
    // array of 16 u8 integers (Rust `{:?}` format). We pin a constant file_id
    // for all rows so the bench overhead is deterministic.
    let meta_json = format!(
        r#"{{"width":2,"height":2,"original_size":4,"chunk_count":{},"file_id":{:?}}}"#,
        chunks.len(),
        file_id
    );

    for i in 0..n {
        let row_id = format!("img-{i:08}");
        let item_id = format!("iitem-{i:08}");
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, blob_ref, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
                params![row_id, item_id, blob, meta_json, i as i64],
            )
            .expect("insert mislabeled kv2 image row");
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Measure `migrate_v1_to_v2_keys` on a DB pre-populated with N v1 text rows.
///
/// The benchmark re-creates the DB from scratch on every iteration
/// (`iter_with_setup`) so each run starts with a full set of v1 rows.
/// This captures the cold-start daemon cost rather than the idempotent
/// "nothing to do" second-run cost.
fn bench_migrate_v1_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration_v1_to_v2_text");
    // Keep sample_size small: the 1 000-row variant sleeps ~450 ms per iteration.
    group.sample_size(10);

    for &n in ROW_COUNTS {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &count| {
            b.iter_with_setup(
                || {
                    let db = fresh_db();
                    seed_v1_text_rows(&db, count);
                    db
                },
                |db| {
                    let rotated = migrate_v1_to_v2_keys(black_box(&db), &V1_KEY, &V2_KEY)
                        .expect("migrate_v1_to_v2_keys");
                    black_box(rotated);
                },
            );
        });
    }
    group.finish();
}

/// Measure `repair_mislabeled_kv2_blob_rows` on a DB pre-populated with N
/// mislabeled image rows (v1-encrypted but stamped kv=2).
fn bench_repair_mislabeled_kv2(c: &mut Criterion) {
    let mut group = c.benchmark_group("repair_mislabeled_kv2_blob_rows");
    group.sample_size(10);

    for &n in ROW_COUNTS {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &count| {
            b.iter_with_setup(
                || {
                    let db = fresh_db();
                    seed_mislabeled_kv2_image_rows(&db, count);
                    db
                },
                |db| {
                    let repaired =
                        repair_mislabeled_kv2_blob_rows(black_box(&db), &V1_KEY, &V2_KEY)
                            .expect("repair_mislabeled_kv2_blob_rows");
                    black_box(repaired);
                },
            );
        });
    }
    group.finish();
}

/// Measure the no-op scan path: both functions called on a clean DB with no
/// v1 or mislabeled rows. This is the common steady-state daemon restart cost
/// once migration and repair have already run.
fn bench_migration_noop_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration_noop_scan");
    group.sample_size(20); // cheap — no sleeps, no re-encryption

    for &n in ROW_COUNTS {
        // Populate with already-correct kv=2 text rows (no v1 rows to migrate).
        group.bench_with_input(
            BenchmarkId::new("migrate_v1_to_v2_text_noop", n),
            &n,
            |b, &count| {
                let db = fresh_db();
                // Insert plain kv=2 text rows directly so there is nothing
                // for migrate_v1_to_v2_keys to do.
                let aad = build_item_aad(&"bench-noop-item".into(), AAD_SCHEMA_VERSION);
                let plaintext = b"already v2";
                let (nonce, ct) = encrypt_item_with_aad(plaintext, &V2_KEY, &aad).expect("encrypt");
                let nonce_bytes = nonce.to_vec();
                for i in 0..count {
                    let row_id = format!("v2-{i}");
                    let item_id = format!("v2i-{i}");
                    db.conn()
                        .execute(
                            "INSERT INTO clipboard_items \
                             (id, item_id, content_type, content, content_nonce, \
                              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                             VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,2)",
                            params![row_id, item_id, ct, nonce_bytes, i as i64],
                        )
                        .expect("insert v2 text row");
                }
                b.iter(|| {
                    let rotated = migrate_v1_to_v2_keys(black_box(&db), &V1_KEY, &V2_KEY)
                        .expect("migrate noop");
                    black_box(rotated);
                });
            },
        );

        // No-op scan for repair: DB with correctly-encrypted kv=2 image rows
        // (v1-decrypt should FAIL → rows are left untouched → return 0).
        group.bench_with_input(
            BenchmarkId::new("repair_mislabeled_noop", n),
            &n,
            |b, &count| {
                let db = fresh_db();
                // Seed correctly-encrypted kv=2 image rows (v2 key, not v1).
                let file_id = [0xCDu8; 16];
                let plaintext = b"PNG_";
                let chunks = encrypt_chunks(plaintext, &V2_KEY, &file_id, IMAGE_CHUNK_SIZE)
                    .expect("encrypt chunks v2");
                let blob = chunks_to_blob(&chunks).expect("blob");
                let meta_json = format!(
                    r#"{{"width":2,"height":2,"original_size":4,"chunk_count":{},"file_id":{:?}}}"#,
                    chunks.len(),
                    file_id
                );
                for i in 0..count {
                    let row_id = format!("v2img-{i}");
                    let item_id = format!("v2iimg-{i}");
                    db.conn()
                        .execute(
                            "INSERT INTO clipboard_items \
                             (id, item_id, content_type, content, content_nonce, blob_ref, \
                              is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                             VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
                            params![row_id, item_id, blob, meta_json, i as i64],
                        )
                        .expect("insert v2 image row");
                }
                b.iter(|| {
                    let repaired =
                        repair_mislabeled_kv2_blob_rows(black_box(&db), &V1_KEY, &V2_KEY)
                            .expect("repair noop");
                    black_box(repaired);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_migrate_v1_text,
    bench_repair_mislabeled_kv2,
    bench_migration_noop_scan
);
criterion_main!(benches);
