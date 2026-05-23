use copypaste_core::{insert_item, search_items, upsert_fts, ClipboardItem, Database};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn make_item(lamport: i64) -> ClipboardItem {
    ClipboardItem::new_text(vec![0xAAu8; 64], vec![0u8; 24], lamport)
}

/// Benchmark: insert 100 clipboard items sequentially into an in-memory DB.
fn bench_insert_item(c: &mut Criterion) {
    c.bench_function("bench_insert_item_100", |b| {
        b.iter(|| {
            let db = Database::open_in_memory().unwrap();
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
    let db = Database::open_in_memory().unwrap();
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
