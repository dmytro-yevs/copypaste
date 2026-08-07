//! One sync round between two paired devices, end to end.
//!
//! Both sides hold the same history, so nothing is fetched and nothing is
//! applied: what is measured is the round a pair of already-converged devices
//! performs every tick, forever. That is the steady state, and the byte count
//! printed beside each size is what crosses the wire to discover that there is
//! nothing to do.
//!
//! The duplex is an in-process channel — this is a bench over a pipe, not a
//! network test. It runs the real encoder and the real decoder, so the JSON
//! passes and the protocol bounds are in the measurement; TCP, Noise and the
//! LAN are not.

use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tokio::sync::mpsc;

use copypaste_p2p::protocol::{content_hash, ItemSummary, SyncItem, SyncMessage};
use copypaste_p2p::sync::{
    merge_decision, run_initiator, run_responder, MergeDecision, SyncChannel, SyncCursor,
    SyncError, SyncSource,
};

/// `MAX_SUMMARIES_PER_MESSAGE` is 10 000, so the largest case is the biggest
/// history that still advertises itself in a single page.
const SIZES: [usize; 3] = [100, 1_000, 10_000];

const T0: i64 = 1_700_000_000_000;

/// A history held in memory. The daemon's source is a SQLCipher store; this one
/// deliberately is not, so what the session itself costs is not buried under a
/// database.
struct BenchSource {
    device_id: String,
    items: Mutex<HashMap<String, SyncItem>>,
}

impl BenchSource {
    fn new(device_id: &str, items: Vec<SyncItem>) -> Self {
        Self {
            device_id: device_id.into(),
            items: Mutex::new(items.into_iter().map(|i| (i.item_id.clone(), i)).collect()),
        }
    }
}

impl SyncSource for BenchSource {
    fn device_id(&self) -> String {
        self.device_id.clone()
    }

    fn device_name(&self) -> String {
        format!("{} name", self.device_id)
    }

    fn summaries(&self, since_ms: i64) -> Result<Vec<ItemSummary>, SyncError> {
        let mut v: Vec<_> = self
            .items
            .lock()
            .unwrap()
            .values()
            .map(SyncItem::summary)
            .filter(|s| s.created_at.max(s.pin_updated_at) >= since_ms)
            .collect();
        v.sort_by_key(|s| (s.created_at.max(s.pin_updated_at), s.item_id.clone()));
        Ok(v)
    }

    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        let items = self.items.lock().unwrap();
        Ok(ids.iter().filter_map(|id| items.get(id).cloned()).collect())
    }

    // The re-check the trait requires: the session decided from summaries, which
    // carry no origin.
    fn apply(&self, incoming: SyncItem) -> Result<bool, SyncError> {
        let mut items = self.items.lock().unwrap();
        if let Some(local) = items.get(&incoming.item_id) {
            if merge_decision(
                &local.summary(),
                &local.origin_device_id,
                &incoming.summary(),
                &incoming.origin_device_id,
            ) == MergeDecision::KeepLocal
            {
                return Ok(false);
            }
        }
        items.insert(incoming.item_id.clone(), incoming);
        Ok(true)
    }
}

/// One end of an in-memory duplex that also weighs what it carries.
struct MeteredChannel {
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
    bytes: Arc<AtomicUsize>,
}

impl SyncChannel for MeteredChannel {
    async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
        let bytes = msg.encode()?;
        self.bytes.fetch_add(bytes.len(), Ordering::Relaxed);
        self.tx
            .send(bytes)
            .await
            .map_err(|_| SyncError::Channel("peer went away".into()))
    }

    async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
        let bytes = self
            .rx
            .recv()
            .await
            .ok_or_else(|| SyncError::Channel("peer went away".into()))?;
        Ok(SyncMessage::decode(&bytes)?)
    }
}

fn duplex(bytes: &Arc<AtomicUsize>) -> (MeteredChannel, MeteredChannel) {
    let (a_tx, b_rx) = mpsc::channel(8);
    let (b_tx, a_rx) = mpsc::channel(8);
    (
        MeteredChannel {
            tx: a_tx,
            rx: a_rx,
            bytes: Arc::clone(bytes),
        },
        MeteredChannel {
            tx: b_tx,
            rx: b_rx,
            bytes: Arc::clone(bytes),
        },
    )
}

fn history(n: usize, origin: &str) -> Vec<SyncItem> {
    (0..n)
        .map(|i| {
            let content = format!("clipping {i:016x} the quick brown fox");
            SyncItem {
                item_id: format!("00000000-0000-4000-8000-{i:012x}"),
                content_hash: content_hash(&content),
                content,
                binary_content: Vec::new(),
                payload_metadata: None,
                content_type: "text".into(),
                created_at: T0 + i as i64 * 1_000,
                deleted: false,
                origin_device_id: origin.into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            }
        })
        .collect()
}

async fn round(
    a: &BenchSource,
    b: &BenchSource,
    bytes: &Arc<AtomicUsize>,
    ca_cursor: SyncCursor,
    cb_cursor: SyncCursor,
) -> (SyncCursor, SyncCursor) {
    let (mut ca, mut cb) = duplex(bytes);
    let (ra, rb) = tokio::join!(
        async move {
            let out = run_initiator(&mut ca, a, None, ca_cursor).await;
            drop(ca);
            out
        },
        async move {
            let out = run_responder(&mut cb, b, None, cb_cursor).await;
            drop(cb);
            out
        }
    );
    let (ra, rb) = (ra.expect("initiator"), rb.expect("responder"));
    let cursors = (ra.cursor, rb.cursor);
    black_box(cursors)
}

fn converged_round(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");

    let mut group = c.benchmark_group("p2p/session/converged");
    group.sample_size(10);

    for n in SIZES {
        // Both sides hold the same items, so the round settles with nothing to
        // move: the cost is the advertisement itself.
        let a = BenchSource::new("device-a", history(n, "device-a"));
        let b = BenchSource::new("device-b", history(n, "device-a"));
        let bytes = Arc::new(AtomicUsize::new(0));

        let (ca, cb) = rt.block_on(round(
            &a,
            &b,
            &bytes,
            SyncCursor::default(),
            SyncCursor::default(),
        ));
        let first_round = bytes.load(Ordering::Relaxed);
        bytes.store(0, Ordering::Relaxed);
        rt.block_on(round(&a, &b, &bytes, ca, cb));
        let per_round = bytes.load(Ordering::Relaxed);
        println!("p2p/session/converged/{n}: {first_round} bytes on the first round, {per_round} bytes across the duplex per round after it");

        group.throughput(Throughput::Bytes(per_round.max(1) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| rt.block_on(round(&a, &b, &bytes, ca, cb)));
        });
    }
    group.finish();
}

criterion_group!(benches, converged_round);
criterion_main!(benches);
