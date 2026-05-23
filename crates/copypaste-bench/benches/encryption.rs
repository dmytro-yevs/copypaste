//! Beta-bonus benchmark: XChaCha20-Poly1305 encrypt + decrypt throughput.
//!
//! Sizes intentionally cover the three classes of clipboard payload:
//!   * 1 KB     — typical text item
//!   * 100 KB   — small image / large rich-text
//!   * 10 MB    — upper bound for a single clipboard item
//!
//! This crate is read-only against `copypaste-core` — we import the public
//! `encrypt_item_with_aad` / `decrypt_item_with_aad` API and never reach into
//! the impl. v0.3 (commit 1c55e57) removed the legacy empty-AAD wrappers, so
//! every call now binds ciphertext to a fixed `(item_id, schema_version)`
//! AAD; benches use a stable synthetic item_id so the AAD cost is captured
//! identically across runs.

use copypaste_core::{
    build_item_aad, decrypt_item_with_aad, encrypt_item_with_aad, AAD_SCHEMA_VERSION,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const KEY: [u8; 32] = [0x42u8; 32];

/// Stable synthetic item_id for the bench harness. Real items would carry a
/// per-row UUID; pinning a constant here keeps the AAD bytes (and therefore
/// the per-iteration cost) deterministic across runs.
const BENCH_ITEM_ID: &str = "bench-encryption-fixed-uuid-0000000000000000";

fn bench_aad() -> Vec<u8> {
    build_item_aad(BENCH_ITEM_ID, AAD_SCHEMA_VERSION)
}

const SIZES: &[(&str, usize)] = &[
    ("1KB", 1024),
    ("100KB", 100 * 1024),
    ("10MB", 10 * 1024 * 1024),
];

fn bench_encrypt(c: &mut Criterion) {
    let aad = bench_aad();
    let mut group = c.benchmark_group("xchacha20_encrypt");
    for &(label, size) in SIZES {
        let plaintext = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &plaintext, |b, p| {
            b.iter(|| {
                encrypt_item_with_aad(black_box(p), black_box(&KEY), black_box(&aad))
                    .expect("encrypt")
            });
        });
    }
    group.finish();
}

fn bench_decrypt(c: &mut Criterion) {
    let aad = bench_aad();
    let mut group = c.benchmark_group("xchacha20_decrypt");
    for &(label, size) in SIZES {
        let plaintext = vec![0xCDu8; size];
        let (nonce, ciphertext) =
            encrypt_item_with_aad(&plaintext, &KEY, &aad).expect("setup encrypt");
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(nonce, ciphertext),
            |b, (n, ct)| {
                b.iter(|| {
                    decrypt_item_with_aad(
                        black_box(ct),
                        black_box(n),
                        black_box(&KEY),
                        black_box(&aad),
                    )
                    .expect("decrypt")
                });
            },
        );
    }
    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let aad = bench_aad();
    let mut group = c.benchmark_group("xchacha20_roundtrip");
    for &(label, size) in SIZES {
        let plaintext = vec![0xEFu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &plaintext, |b, p| {
            b.iter(|| {
                let (n, ct) =
                    encrypt_item_with_aad(black_box(p), black_box(&KEY), black_box(&aad))
                        .expect("encrypt");
                let pt = decrypt_item_with_aad(
                    black_box(&ct),
                    black_box(&n),
                    black_box(&KEY),
                    black_box(&aad),
                )
                .expect("decrypt");
                black_box(pt);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_encrypt, bench_decrypt, bench_roundtrip);
criterion_main!(benches);
