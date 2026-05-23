//! Beta-bonus benchmark: SQLCipher-backed insert + query throughput.
//!
//! Uses `tempfile::TempDir` to host a real on-disk SQLCipher database via
//! `Database::open(path, key)` — this exercises the encrypted code path, not
//! the in-memory shortcut. Sizes: 100 / 1 000 / 10 000 rows.

use copypaste_core::{insert_item, search_items, upsert_fts, ClipboardItem, Database};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;

const DB_KEY: [u8; 32] = [0x37u8; 32];

const ROW_COUNTS: &[usize] = &[100, 1_000, 10_000];

fn make_item(lamport: i64) -> ClipboardItem {
    ClipboardItem::new_text(vec![0xAAu8; 64], vec![0u8; 24], lamport)
}

fn fresh_db() -> (TempDir, Database) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("bench.db");
    let db = Database::open(&path, &DB_KEY).expect("open sqlcipher");
    (dir, db)
}

/// Insert N rows into a fresh encrypted DB. The TempDir + DB construction
/// is included in the measurement on purpose — it reflects the cold-start
/// cost a fresh daemon pays.
fn bench_sqlcipher_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlcipher_insert");
    group.sample_size(10); // 10 000-row variant is slow; keep wall-time bounded.
    for &n in ROW_COUNTS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &count| {
            b.iter(|| {
                let (_dir, db) = fresh_db();
                for i in 0..count as i64 {
                    insert_item(&db, black_box(&make_item(i))).expect("insert");
                }
            });
        });
    }
    group.finish();
}

/// Query throughput on a pre-populated encrypted DB. Setup happens outside
/// the timing loop so we measure pure FTS5 + SQLCipher read cost.
fn bench_sqlcipher_query(c: &mut Criterion) {
    let words = [
        "apple", "banana", "cherry", "delta", "echo", "foxtrot", "golf", "hotel",
    ];
    let mut group = c.benchmark_group("sqlcipher_query");
    group.sample_size(10);
    for &n in ROW_COUNTS {
        let (_dir, db) = fresh_db();
        for i in 0..n as i64 {
            let item = make_item(i);
            insert_item(&db, &item).expect("insert");
            let text = format!(
                "{} {} row {}",
                words[(i as usize) % words.len()],
                words[((i as usize) + 3) % words.len()],
                i
            );
            upsert_fts(&db, &item.id, &text).expect("fts upsert");
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &db, |b, db| {
            b.iter(|| {
                let r = search_items(black_box(db), black_box("apple"), 50).expect("search");
                black_box(r);
            });
        });
        // _dir dropped at end of loop iteration → cleans up tempdir.
    }
    group.finish();
}

criterion_group!(benches, bench_sqlcipher_insert, bench_sqlcipher_query);
criterion_main!(benches);
