//! Secret detection, which runs on every capture.
//!
//! The reason this is a bench and not a test: at `regex`'s default 2 MiB
//! `dfa_size_limit` the prefilter for these ~45 patterns thrashed its lazy-DFA
//! cache and ran at 1.0 MB/s with a ~55 µs floor per call. The constant that
//! fixes it is `PREFILTER_DFA_SIZE_LIMIT`, and nothing but a measurement tells
//! you it has regressed.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

mod support;
use support::{clipping, detector};

/// Small enough that the per-call floor dominates, up to a clipping at the cap.
const SIZES: [(&str, usize); 5] = [
    ("64B", 64),
    ("1KiB", 1024),
    ("116KB", 116_000),
    ("1MiB", 1024 * 1024),
    ("4MiB", 4 * 1024 * 1024),
];

fn benign(c: &mut Criterion) {
    let detector = detector();
    let mut group = c.benchmark_group("detect/benign");
    for (label, bytes) in SIZES {
        let text = clipping(bytes, 1);
        group.throughput(Throughput::Bytes(bytes as u64));
        if bytes >= 1024 * 1024 {
            group.sample_size(20);
        }
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter(|| detector.is_sensitive(black_box(&text)));
        });
    }
    group.finish();
}

/// A secret at the end of the text: the prefilter matches, so every stage runs
/// — normalise, the set pass, the individual regex, the validator.
fn matching(c: &mut Criterion) {
    let detector = detector();
    let mut group = c.benchmark_group("detect/matching");
    for (label, bytes) in SIZES {
        let mut text = clipping(bytes, 2);
        text.push_str(" AKIAIOSFODNN7EXAMPLE");
        group.throughput(Throughput::Bytes(text.len() as u64));
        if bytes >= 1024 * 1024 {
            group.sample_size(20);
        }
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter(|| detector.scan(black_box(&text)));
        });
    }
    group.finish();
}

criterion_group!(benches, benign, matching);
criterion_main!(benches);
