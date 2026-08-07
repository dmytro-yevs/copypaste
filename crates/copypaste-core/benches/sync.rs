//! Merging a peer's session in: what N incoming items cost the store.
//!
//! The unit is the **session**, not one `apply`. Everything a session does per
//! item that a batch would do once — the retention sweeps, the settings read,
//! the local row lookup — is invisible at N = 1 and is the whole question here,
//! so the session is measured against three history depths: work that scales
//! with the history shows up as a per-item cost that grows with it.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::StoreSource;
use copypaste_ipc::ConfigData;
use copypaste_p2p::protocol::{content_hash, SyncItem};
use copypaste_p2p::sync::SyncSource;

mod support;
use support::{clipping, detector, fill, keyring, T0};

/// `ConfigData::history_limit` defaults to 10 000; these bracket it.
const HISTORIES: [usize; 3] = [500, 2_000, 8_000];
/// A first pairing moves a whole history; a later round moves a handful.
const SESSIONS: [usize; 2] = [10, 200];
/// Row size for both the primed history and the incoming items.
const ROW_BYTES: usize = 512;

static SEED: AtomicUsize = AtomicUsize::new(1);

fn seed() -> usize {
    SEED.fetch_add(1, Ordering::Relaxed)
}

/// `n` items from a peer, none of which this device has seen.
fn incoming(n: usize) -> Vec<SyncItem> {
    (0..n)
        .map(|i| {
            let content = clipping(ROW_BYTES, seed());
            SyncItem {
                item_id: format!("peer-{:016x}", seed()),
                content_hash: content_hash(&content),
                content,
                binary_content: Vec::new(),
                payload_metadata: None,
                content_type: "text".into(),
                created_at: T0 + i as i64 * 1_000,
                deleted: false,
                origin_device_id: "peer-device".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            }
        })
        .collect()
}

fn source(store: copypaste_core::Store, history: usize) -> StoreSource {
    StoreSource::new(
        store,
        Arc::new(keyring()),
        Arc::new(detector()),
        "bench-device".into(),
        "bench".into(),
        ConfigData {
            history_limit: history as u32,
            ..ConfigData::default()
        },
    )
}

fn apply_session(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("sync/apply_session");
    group.sample_size(10);

    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, ROW_BYTES);
        let source = source(store, history);

        for n in SESSIONS {
            group.throughput(Throughput::Elements(n as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{n}_items"), history),
                &n,
                |b, &n| {
                    b.iter_batched(
                        || incoming(n),
                        |items| {
                            for item in items {
                                black_box(source.apply(item).expect("apply"));
                            }
                        },
                        criterion::BatchSize::LargeInput,
                    );
                },
            );
        }
    }
    group.finish();
}

/// The read the other half of a session performs: every summary this device
/// can offer, which is what a round advertises.
fn summaries(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("sync/summaries");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, ROW_BYTES);
        let source = source(store, history);

        group.throughput(Throughput::Elements(history as u64));
        group.bench_with_input(BenchmarkId::from_parameter(history), &history, |b, _| {
            b.iter(|| black_box(source.summaries(0).expect("summaries")).len());
        });
    }
    group.finish();
}

criterion_group!(benches, apply_session, summaries);
criterion_main!(benches);
