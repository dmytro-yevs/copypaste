//! Beta-bonus benchmark: XChaCha20-Poly1305 encrypt + decrypt throughput.
//!
//! Sizes intentionally cover the three classes of clipboard payload:
//!   * 1 KB     — typical text item
//!   * 100 KB   — small image / large rich-text
//!   * 10 MB    — upper bound for a single clipboard item
//!
//! This crate is read-only against `copypaste-core` — we import the public
//! `encrypt_item` / `decrypt_item` API and never reach into the impl.

use copypaste_core::{decrypt_item, encrypt_item};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const KEY: [u8; 32] = [0x42u8; 32];

const SIZES: &[(&str, usize)] = &[
    ("1KB", 1024),
    ("100KB", 100 * 1024),
    ("10MB", 10 * 1024 * 1024),
];

fn bench_encrypt(c: &mut Criterion) {
    let mut group = c.benchmark_group("xchacha20_encrypt");
    for &(label, size) in SIZES {
        let plaintext = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &plaintext, |b, p| {
            b.iter(|| encrypt_item(black_box(p), black_box(&KEY)).expect("encrypt"));
        });
    }
    group.finish();
}

fn bench_decrypt(c: &mut Criterion) {
    let mut group = c.benchmark_group("xchacha20_decrypt");
    for &(label, size) in SIZES {
        let plaintext = vec![0xCDu8; size];
        let (nonce, ciphertext) = encrypt_item(&plaintext, &KEY).expect("setup encrypt");
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(nonce, ciphertext),
            |b, (n, ct)| {
                b.iter(|| {
                    decrypt_item(black_box(ct), black_box(n), black_box(&KEY)).expect("decrypt")
                });
            },
        );
    }
    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("xchacha20_roundtrip");
    for &(label, size) in SIZES {
        let plaintext = vec![0xEFu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &plaintext, |b, p| {
            b.iter(|| {
                let (n, ct) = encrypt_item(black_box(p), black_box(&KEY)).expect("encrypt");
                let pt =
                    decrypt_item(black_box(&ct), black_box(&n), black_box(&KEY)).expect("decrypt");
                black_box(pt);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_encrypt, bench_decrypt, bench_roundtrip);
criterion_main!(benches);
