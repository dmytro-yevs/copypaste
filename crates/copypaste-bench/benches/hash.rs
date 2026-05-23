//! Beta-bonus benchmark: content-hash dedup pipeline.
//!
//! The daemon path is:
//!   1. `Sha256::digest(raw_bytes)` → hex digest
//!   2. `find_recent_by_hash(db, &hex, now_ms, within_ms)` → Option<id>
//!
//! We benchmark each stage separately and the combined "is-this-duplicate?"
//! decision used on every clipboard tick.

use copypaste_core::{find_recent_by_hash, insert_item, ClipboardItem, Database};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const DB_KEY: [u8; 32] = [0x91u8; 32];
const WITHIN_MS: i64 = 60_000;

const PAYLOAD_SIZES: &[(&str, usize)] = &[
    ("1KB", 1024),
    ("100KB", 100 * 1024),
    ("10MB", 10 * 1024 * 1024),
];

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn hex_digest(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    let mut s = String::with_capacity(d.len() * 2);
    for b in d.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// SHA-256 digest of raw clipboard bytes. Dominant cost for large payloads.
fn bench_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_sha256");
    for &(label, size) in PAYLOAD_SIZES {
        let data = vec![0xA5u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &data, |b, d| {
            b.iter(|| {
                let h = Sha256::digest(black_box(d));
                black_box(h);
            });
        });
    }
    group.finish();
}

/// Query a populated encrypted DB by content_hash. Measures the index path
/// on `clipboard_items.content_hash` under SQLCipher.
fn bench_lookup(c: &mut Criterion) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("hash.db");
    let db = Database::open(&path, &DB_KEY).expect("open sqlcipher");

    // Seed 1 000 items with deterministic hashes so we have both hit + miss.
    let mut seeded_hashes = Vec::with_capacity(1_000);
    for i in 0..1_000i64 {
        let mut item = ClipboardItem::new_text(vec![0xAAu8; 64], vec![0u8; 24], i);
        let h = hex_digest(format!("seed-{i}").as_bytes());
        item.content_hash = Some(h.clone());
        insert_item(&db, &item).expect("insert");
        seeded_hashes.push(h);
    }

    let hit_hash = seeded_hashes[500].clone();
    let miss_hash = hex_digest(b"definitely-not-stored");
    let now = now_ms() + 1_000; // ensure all seeded rows are within window

    let mut group = c.benchmark_group("dedup_lookup");
    group.bench_with_input("hit", &hit_hash, |b, h| {
        b.iter(|| {
            let r =
                find_recent_by_hash(black_box(&db), black_box(h), now, WITHIN_MS).expect("query");
            black_box(r);
        });
    });
    group.bench_with_input("miss", &miss_hash, |b, h| {
        b.iter(|| {
            let r =
                find_recent_by_hash(black_box(&db), black_box(h), now, WITHIN_MS).expect("query");
            black_box(r);
        });
    });
    group.finish();
}

/// Full pipeline per clipboard tick: hash → lookup. Worst case is a unique
/// 10 MB payload that always misses.
fn bench_pipeline(c: &mut Criterion) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("pipe.db");
    let db = Database::open(&path, &DB_KEY).expect("open sqlcipher");
    for i in 0..200i64 {
        let mut item = ClipboardItem::new_text(vec![0xAAu8; 64], vec![0u8; 24], i);
        item.content_hash = Some(hex_digest(format!("warm-{i}").as_bytes()));
        insert_item(&db, &item).expect("insert");
    }
    let now = now_ms() + 1_000;

    let mut group = c.benchmark_group("dedup_pipeline");
    for &(label, size) in PAYLOAD_SIZES {
        let data = vec![0xC3u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &data, |b, d| {
            b.iter(|| {
                let hex = hex_digest(black_box(d));
                let r = find_recent_by_hash(&db, &hex, now, WITHIN_MS).expect("query");
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_sha256, bench_lookup, bench_pipeline);
criterion_main!(benches);
