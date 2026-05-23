//! Beta-bonus benchmark: sync protocol encode / decode throughput.
//!
//! We cover the five wire variants of [`copypaste_sync::Message`] at three
//! payload scales (10 / 100 / 1000 items) so the resulting Criterion report
//! shows how each frame type scales with item count.
//!
//! Throughput is reported in bytes of the encoded JSON frame (length-prefix
//! included) — this matches what would actually cross the wire and lets the
//! reader reason about MB/s instead of opaque ns/op numbers.
//!
//! This crate is a read-only consumer of `copypaste-sync`: we only touch the
//! public `Message` / `WireItem` API.

use copypaste_sync::{Message, WireItem};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const ITEM_COUNTS: &[usize] = &[10, 100, 1000];

/// Build a deterministic [`WireItem`] for a given index.
///
/// Sizes are kept small and constant per item so the bench measures protocol
/// framing overhead, not payload work.
fn make_wire_item(i: usize) -> WireItem {
    WireItem {
        id: format!("id-{i:08}"),
        item_id: format!("item-{i:08}"),
        content_type: "text".to_string(),
        // 64-byte ciphertext stand-in — realistic small clipboard text size.
        content: Some(vec![0xABu8; 64]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: i as i64,
        wall_time: 1_700_000_000_000 + i as i64,
        expires_at: None,
        app_bundle_id: Some("com.example.app".to_string()),
        origin_device_id: "device-bench".to_string(),
    }
}

/// Build all five [`Message`] variants populated with `n` items.
///
/// Returns `(label, message)` pairs so the bench groups stay readable.
fn build_messages(n: usize) -> Vec<(&'static str, Message)> {
    let have_items: Vec<(String, i64)> = (0..n).map(|i| (format!("id-{i:08}"), i as i64)).collect();
    let want_ids: Vec<String> = (0..n).map(|i| format!("id-{i:08}")).collect();
    let wire_items: Vec<WireItem> = (0..n).map(make_wire_item).collect();

    vec![
        (
            "Hello",
            Message::Hello {
                device_id: "device-bench-uuid".to_string(),
                clock: n as u64,
                item_count: n as u64,
            },
        ),
        ("Have", Message::Have { items: have_items }),
        (
            "Want",
            Message::Want {
                item_ids: want_ids,
            },
        ),
        ("Items", Message::Items { items: wire_items }),
        ("Done", Message::Done),
    ]
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_encode");
    for &n in ITEM_COUNTS {
        for (variant, msg) in build_messages(n) {
            // Precompute encoded size so Throughput is meaningful.
            let encoded_len = msg.encode().expect("encode setup").len() as u64;
            group.throughput(Throughput::Bytes(encoded_len));
            group.bench_with_input(
                BenchmarkId::new(variant, n),
                &msg,
                |b, m: &Message| {
                    b.iter(|| black_box(m).encode().expect("encode"));
                },
            );
        }
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_decode");
    for &n in ITEM_COUNTS {
        for (variant, msg) in build_messages(n) {
            let encoded = msg.encode().expect("encode setup");
            // Strip the 4-byte length prefix — decode operates on the JSON body.
            let body = encoded[4..].to_vec();
            group.throughput(Throughput::Bytes(encoded.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(variant, n),
                &body,
                |b, bytes: &Vec<u8>| {
                    b.iter(|| Message::decode(black_box(bytes.as_slice())).expect("decode"));
                },
            );
        }
    }
    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_roundtrip");
    for &n in ITEM_COUNTS {
        for (variant, msg) in build_messages(n) {
            let encoded_len = msg.encode().expect("encode setup").len() as u64;
            group.throughput(Throughput::Bytes(encoded_len));
            group.bench_with_input(
                BenchmarkId::new(variant, n),
                &msg,
                |b, m: &Message| {
                    b.iter(|| {
                        let encoded = black_box(m).encode().expect("encode");
                        let decoded =
                            Message::decode(black_box(&encoded[4..])).expect("decode");
                        black_box(decoded);
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode, bench_roundtrip);
criterion_main!(benches);
