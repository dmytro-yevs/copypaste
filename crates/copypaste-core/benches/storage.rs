// CopyPaste-9vcn: open_in_memory is now #[cfg(test)]-gated (no encryption →
// never use in production code).  Benchmarks use an on-disk encrypted DB in a
// temp directory so the production `Database::open` path is exercised instead.
use copypaste_core::{insert_item, search_items, upsert_fts, ClipboardItem, Database};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempfile::tempdir;

/// Bench key: constant 32 bytes, fine for perf measurement (not a secret).
const BENCH_KEY: [u8; 32] = [0x5A; 32];

fn make_item(lamport: i64) -> ClipboardItem {
    ClipboardItem::new_text(vec![0xAAu8; 64], vec![0u8; 24], lamport)
}

/// Open a fresh encrypted on-disk DB in a temp directory for one bench run.
/// The returned `TempDir` must be kept alive for the duration of the bench.
fn open_bench_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("bench.db");
    let db = Database::open(&path, &BENCH_KEY).expect("open bench db");
    (dir, db)
}

/// Benchmark: insert 100 clipboard items sequentially into a fresh encrypted DB.
fn bench_insert_item(c: &mut Criterion) {
    c.bench_function("bench_insert_item_100", |b| {
        b.iter(|| {
            let (_dir, db) = open_bench_db();
            for i in 0..100i64 {
                insert_item(&db, black_box(&make_item(i))).unwrap();
            }
        });
    });
}

/// Benchmark: FTS5 search over a corpus of 1 000 items.
///
/// Setup is performed once outside the timing loop; only the `search_items`
/// call is timed so we measure pure query throughput.
fn bench_fts5_search(c: &mut Criterion) {
    // Build a corpus of 1 000 items with varied search text.
    let (_dir, db) = open_bench_db();
    let words = [
        "apple", "banana", "cherry", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
    ];
    for i in 0..1000i64 {
        let item = make_item(i);
        insert_item(&db, &item).unwrap();
        let text = format!(
            "{} {} item number {}",
            words[(i as usize) % words.len()],
            words[((i as usize) + 3) % words.len()],
            i
        );
        upsert_fts(&db, &item.id, &text).unwrap();
    }

    c.bench_function("bench_fts5_search_1000_items", |b| {
        b.iter(|| {
            // Query that matches ~100 of the 1 000 entries.
            let results = search_items(black_box(&db), black_box("apple"), 50).unwrap();
            black_box(results);
        });
    });
}

criterion_group!(benches, bench_insert_item, bench_fts5_search);
criterion_main!(benches);
