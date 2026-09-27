//! The capture path: what one clipboard poll costs once something is on it.
//!
//! `ingest` is the whole pipeline — dedup probe, detect, encrypt, insert,
//! index, cap sweep, retention sweep — and the `stage/*` group is the same
//! pipeline taken apart, so a total that moves can be attributed rather than
//! guessed at.
//!
//! Every store is primed to a steady state and capped there, because the two
//! sweeps are `O(history)` and measuring them on an empty database measures
//! nothing a user would ever see.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::{compute_content_hash, encrypt, ingest_into, Store};
use copypaste_ipc::ConfigData;

mod support;
use support::{clipping, detector, fill, keyring, row, T0};

/// Sizes, the history each is measured against, and how many samples are
/// affordable. A store of 10 000 four-MiB items does not exist, so the cap case
/// is measured against a short history and labelled as such.
const CASES: [(&str, usize, usize, usize); 3] = [
    ("256B", 256, 10_000, 100),
    ("64KiB", 64 * 1024, 10_000, 50),
    ("4MiB", 4 * 1024 * 1024, 32, 10),
];

/// Unique across the whole run, so no iteration is ever deduplicated away.
static SEED: AtomicUsize = AtomicUsize::new(1_000_000);

fn seed() -> usize {
    SEED.fetch_add(1, Ordering::Relaxed)
}

fn settings(history: usize) -> ConfigData {
    ConfigData {
        history_limit: history as u32,
        ..ConfigData::default()
    }
}

fn ingest(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture/ingest");
    for (label, bytes, history, samples) in CASES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        let keyring = keyring();
        let detector = detector();
        let settings = settings(history);
        fill(&store, &keyring, history, bytes.min(4 * 1024));

        group.throughput(Throughput::Bytes(bytes as u64));
        group.sample_size(samples);
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter_batched(
                || clipping(bytes, seed()),
                |text| {
                    ingest_into(
                        &store,
                        &detector,
                        &keyring,
                        &text,
                        "text",
                        T0,
                        black_box(&settings),
                    )
                    .expect("ingest")
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn stages(c: &mut Criterion) {
    let keyring = keyring();
    let detector = detector();
    let key = keyring.item_key();

    for (label, bytes, history, samples) in CASES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, bytes.min(4 * 1024));
        let text = clipping(bytes, 7);
        let hash = compute_content_hash(text.as_bytes());

        let mut group = c.benchmark_group(format!("capture/stage/{label}"));
        group.throughput(Throughput::Bytes(bytes as u64));
        group.sample_size(samples);

        group.bench_function("detect", |b| {
            b.iter(|| detector.is_sensitive(black_box(&text)));
        });
        group.bench_function("hash", |b| {
            b.iter(|| compute_content_hash(black_box(text.as_bytes())));
        });
        group.bench_function("encrypt", |b| {
            b.iter(|| encrypt(black_box(text.as_bytes()), &key, "bench-id").expect("seal"));
        });
        group.bench_function("dedup_probe", |b| {
            b.iter(|| store.find_recent_by_hash(black_box(&hash), T0 - 60_000));
        });
        group.bench_function("insert", |b| {
            b.iter_batched(
                || {
                    // Untimed: keeps the file bounded so a 4 MiB run does not
                    // fill the disk, and puts every timed insert at the same
                    // history depth.
                    store.evict_over_cap(history as u64).expect("evict");
                    row(&keyring, &clipping(bytes, seed()), T0)
                },
                |item| store.insert(item).expect("insert"),
                criterion::BatchSize::SmallInput,
            );
        });
        group.finish();
    }
}

/// The two sweeps that ride every capture, measured with nothing to delete —
/// which is what they do on all but one capture in `history_limit`.
fn sweeps(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("capture/sweep");
    for history in [100usize, 2_000, 10_000] {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, 256);

        group.bench_with_input(
            BenchmarkId::new("cap_nothing_to_do", history),
            &history,
            |b, &history| b.iter(|| store.evict_over_cap(black_box(history as u64))),
        );
        group.bench_with_input(
            BenchmarkId::new("retention_nothing_to_do", history),
            &history,
            |b, _| b.iter(|| store.evict_older_than(black_box(T0 - 86_400_000))),
        );
    }
    group.finish();
}

/// What the poll loop costs when the clipboard has not moved: the whole tick
/// below `ClipboardSource::poll` is a no-op, so this is the sweep pair alone.
fn idle_tick(c: &mut Criterion) {
    let keyring = keyring();
    let dir = tempfile::tempdir().expect("tempdir");
    let store = support::store_in(dir.path());
    fill(&store, &keyring, 2_000, 256);
    let detector = detector();
    let key = keyring.item_key();

    c.bench_function("capture/idle_tick/sensitive_sweep_disabled", |b| {
        b.iter(|| {
            copypaste_core::sweep_sensitive(&store, &detector, &key, Duration::ZERO, black_box(T0))
        });
    });
    c.bench_function("capture/idle_tick/sensitive_sweep_enabled", |b| {
        b.iter(|| {
            copypaste_core::sweep_sensitive(
                &store,
                &detector,
                &key,
                Duration::from_secs(30),
                black_box(T0),
            )
        });
    });
}

fn open(c: &mut Criterion) {
    c.bench_function("capture/detector_new", |b| {
        b.iter(|| copypaste_core::Detector::new().expect("ruleset"));
    });
    c.bench_function("capture/store_open", |b| {
        b.iter_batched(
            || tempfile::tempdir().expect("tempdir"),
            |dir| {
                let store: Store = support::store_in(dir.path());
                black_box(store.count().expect("count"))
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, ingest, stages, sweeps, idle_tick, open);
criterion_main!(benches);
