//! The binary capture path: what one screenshot or dragged file costs.
//!
//! `benches/capture.rs` measures the text pipeline; this one measures the
//! bytes pipeline, which is a different implementation end to end — chunked
//! STREAM rather than a single AEAD, a digest-derived item id, and no search
//! index.
//!
//! `item_id` is measured on its own because it is one full SHA-256 pass over
//! the payload, and `ingest` runs three more of them: multiply the `item_id`
//! figure by the number of passes to price a change in how many there are.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::{
    binary_item_id, ingest_binary_into_with_capture_source, open_binary, seal_binary,
};
use copypaste_ipc::{content_type::IMAGE_PNG, ConfigData};

mod support;
use support::{keyring, T0};

/// Size, the history it is measured against, and the samples affordable.
///
/// The histories are short and shrink with size for one reason: 10 000 rows of
/// 4 MiB is 40 GB. They are deep enough that both retention sweeps have rows
/// to scan and shallow enough to fit on a disk.
const CASES: [(&str, usize, usize, usize); 3] = [
    ("256KiB", 256 * 1024, 64, 50),
    ("1MiB", 1024 * 1024, 64, 30),
    ("4MiB", 4 * 1024 * 1024, 16, 10),
];

static SEED: AtomicUsize = AtomicUsize::new(1);

/// Unique bytes per call, so nothing is deduplicated away and every seal gets
/// a distinct item id.
fn payload(bytes: usize, seed: usize) -> Vec<u8> {
    let mut out = vec![0u8; bytes];
    out[..8].copy_from_slice(&(seed as u64).to_le_bytes());
    out
}

fn settings(history: usize) -> ConfigData {
    ConfigData {
        history_limit: history as u32,
        ..ConfigData::default()
    }
}

fn crypto(c: &mut Criterion) {
    let keyring = keyring();
    let key = keyring.item_key();

    for (label, bytes, _, samples) in CASES {
        let plain = payload(bytes, 7);
        let id = binary_item_id(&plain);
        let envelope = seal_binary(&plain, &key, &id).expect("seal");

        let mut group = c.benchmark_group("capture/binary");
        group.throughput(Throughput::Bytes(bytes as u64));
        group.sample_size(samples);

        group.bench_function(BenchmarkId::new("item_id", label), |b| {
            b.iter(|| binary_item_id(black_box(&plain)));
        });
        group.bench_function(BenchmarkId::new("seal", label), |b| {
            b.iter(|| seal_binary(black_box(&plain), &key, &id).expect("seal"));
        });
        group.bench_function(BenchmarkId::new("open", label), |b| {
            b.iter(|| open_binary(black_box(&envelope), &key, &id).expect("open"));
        });
        group.finish();
    }
}

/// The whole path a captured image takes: identity, seal, insert, both sweeps.
fn ingest(c: &mut Criterion) {
    let keyring = keyring();

    for (label, bytes, history, samples) in CASES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        let settings = settings(history);
        for n in 0..history {
            ingest_binary_into_with_capture_source(
                &store,
                &keyring,
                &payload(bytes, SEED.fetch_add(1, Ordering::Relaxed)),
                IMAGE_PNG,
                T0 + n as i64 * 1_000,
                false,
                None,
                None,
                None,
                &settings,
            )
            .expect("prime");
        }

        let mut group = c.benchmark_group("capture/binary");
        group.throughput(Throughput::Bytes(bytes as u64));
        group.sample_size(samples);
        group.bench_function(BenchmarkId::new("ingest", label), |b| {
            b.iter_batched(
                || payload(bytes, SEED.fetch_add(1, Ordering::Relaxed)),
                |plain| {
                    ingest_binary_into_with_capture_source(
                        &store,
                        &keyring,
                        &plain,
                        IMAGE_PNG,
                        T0,
                        false,
                        None,
                        None,
                        None,
                        black_box(&settings),
                    )
                    .expect("ingest")
                },
                criterion::BatchSize::LargeInput,
            );
        });
        group.finish();
    }
}

criterion_group!(benches, crypto, ingest);
criterion_main!(benches);
