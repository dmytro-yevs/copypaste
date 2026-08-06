//! The three store calls the sync and capture paths spend their time in,
//! against a real keyed store.
//!
//! Every store here is SQLCipher on a file, so each page a statement touches
//! is an AES-256-CBC decrypt and an HMAC-SHA512 verify. That multiplier is the
//! reason a plain-SQLite figure is a lower bound and not an estimate, and it
//! is why the same three calls are measured at three history depths: what
//! scales with the row count scales with the crypto too.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::{compute_content_hash, encrypt, IncomingItem, Store};

mod support;
use support::{clipping, keyring, row, T0};

const HISTORIES: [usize; 3] = [500, 2_000, 8_000];
const ROW_BYTES: usize = 512;

static SEED: AtomicUsize = AtomicUsize::new(1);

fn seed() -> usize {
    SEED.fetch_add(1, Ordering::Relaxed)
}

fn primed(dir: &std::path::Path, history: usize) -> Store {
    let store = support::store_in(dir);
    support::fill(&store, &keyring(), history, ROW_BYTES);
    store
}

/// What a sync round reads before it can say anything: every eligible row.
/// `i64::MAX` is the limit the source passes.
fn summaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/summaries");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.throughput(Throughput::Elements(history as u64));
        group.bench_with_input(BenchmarkId::from_parameter(history), &history, |b, _| {
            b.iter(|| {
                store
                    .summaries(black_box(i64::MAX))
                    .expect("summaries")
                    .len()
            });
        });
    }
    group.finish();
}

/// The capture write, on both of its branches. `bump` re-copies a row that is
/// already in the history, which is the branch a user's re-copy takes.
fn insert_or_bump(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("storage/insert_or_bump");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.bench_with_input(BenchmarkId::new("insert", history), &history, |b, _| {
            b.iter_batched(
                || {
                    // Untimed: every timed call then runs at the same depth and
                    // the file stays bounded.
                    store.evict_over_cap(history as u64).expect("evict");
                    row(&keyring, &clipping(ROW_BYTES, seed()), T0)
                },
                |item| store.insert_or_bump(item).expect("insert"),
                criterion::BatchSize::SmallInput,
            );
        });

        let duplicate = clipping(ROW_BYTES, 0);
        group.bench_with_input(BenchmarkId::new("bump", history), &history, |b, _| {
            b.iter_batched(
                || row(&keyring, &duplicate, T0),
                |item| store.insert_or_bump(item).expect("bump"),
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// The merge write: the only call that may overwrite an existing row.
fn upsert(c: &mut Criterion) {
    let keyring = keyring();
    let key = keyring.item_key();
    let mut group = c.benchmark_group("storage/upsert");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.bench_with_input(BenchmarkId::from_parameter(history), &history, |b, _| {
            b.iter_batched(
                || {
                    store.evict_over_cap(history as u64).expect("evict");
                    let content = clipping(ROW_BYTES, seed());
                    let id = format!("peer-{:016x}", seed());
                    let (nonce, ciphertext) = encrypt(content.as_bytes(), &key, &id).expect("seal");
                    let hash = compute_content_hash(content.as_bytes());
                    (id, content, ciphertext, nonce, hash)
                },
                |(id, content, ciphertext, nonce, hash)| {
                    store
                        .upsert(&IncomingItem {
                            id: &id,
                            content_ciphertext: Some(&ciphertext),
                            nonce: Some(&nonce),
                            content_type: "text",
                            content_hash: &hash,
                            created_at: T0,
                            deleted: false,
                            is_sensitive: false,
                            origin_device_id: "peer-device",
                            pinned: false,
                            pin_order: None,
                            pin_updated_at: 0,
                            search_text: Some(&content),
                            payload_metadata: None,
                        })
                        .expect("upsert")
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn byte_cap(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/sweep");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.bench_with_input(
            BenchmarkId::new("byte_cap_nothing_to_do", history),
            &history,
            |b, _| b.iter(|| store.evict_over_byte_cap(black_box(u64::MAX))),
        );
    }
    group.finish();
}

criterion_group!(benches, summaries, insert_or_bump, upsert, byte_cap);
criterion_main!(benches);
