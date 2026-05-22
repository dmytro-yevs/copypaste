use copypaste_core::{DeviceKeypair, encrypt_item, decrypt_item, detect};
use copypaste_core::crypto::chunks::encrypt_chunks;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_keypair(c: &mut Criterion) {
    c.bench_function("keypair_generate", |b| {
        b.iter(|| DeviceKeypair::generate())
    });

    let kp = DeviceKeypair::generate();
    let peer = DeviceKeypair::generate();
    let peer_pub = peer.public_key_bytes();
    c.bench_function("derive_enc_key", |b| {
        b.iter(|| kp.derive_enc_key(black_box(&peer_pub), "a", "b"))
    });
    c.bench_function("local_enc_key", |b| {
        b.iter(|| kp.local_enc_key())
    });
}

fn bench_encrypt_item(c: &mut Criterion) {
    let key = [0x42u8; 32];
    let mut group = c.benchmark_group("encrypt_item");
    for size in [64usize, 1024, 65536, 1_048_576] {
        let data = vec![0xABu8; size];
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, d| {
            b.iter(|| encrypt_item(black_box(d), black_box(&key)))
        });
    }
    group.finish();
}

fn bench_decrypt_item(c: &mut Criterion) {
    let key = [0x42u8; 32];
    let data = vec![0xABu8; 1024];
    let (nonce, ciphertext) = encrypt_item(&data, &key);
    c.bench_function("decrypt_item_1kb", |b| {
        b.iter(|| decrypt_item(black_box(&ciphertext), black_box(&nonce), black_box(&key)))
    });
}

fn bench_chunks(c: &mut Criterion) {
    let key = [0x77u8; 32];
    let file_id = [0x11u8; 16];
    let mut group = c.benchmark_group("encrypt_chunks");
    for size in [65_536usize, 1_048_576, 10_485_760] {
        let data = vec![0xCCu8; size];
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, d| {
            b.iter(|| encrypt_chunks(black_box(d), black_box(&key), black_box(&file_id), 64 * 1024))
        });
    }
    group.finish();
}

fn bench_sensitive_detect(c: &mut Criterion) {
    let texts = [
        ("clean_short", "Hello world, this is normal text."),
        ("clean_10kb", &"x".repeat(10_000) as &str),
        ("aws_key", "AKIAIOSFODNN7EXAMPLE"),
        ("jwt", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature"),
    ];
    let mut group = c.benchmark_group("sensitive_detect");
    for (name, text) in &texts {
        group.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
            b.iter(|| detect(black_box(t)))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_keypair, bench_encrypt_item, bench_decrypt_item, bench_chunks, bench_sensitive_detect);
criterion_main!(benches);
