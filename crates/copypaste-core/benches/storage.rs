//! The store calls the sync and capture paths spend their time in, against a
//! real keyed store.
//!
//! Every store here is SQLCipher on a file, so each page a statement touches
//! is an AES-256-CBC decrypt and an HMAC-SHA512 verify. That multiplier is the
//! reason a plain-SQLite figure is a lower bound and not an estimate, and it
//! is why the calls are measured at three history depths: what scales with the
//! row count scales with the crypto too.
//!
//! Depth is one of two dimensions, and at a fixed `ROW_BYTES` it cannot see a
//! cost that is per byte rather than per row. `storage/payload` sweeps the
//! other one.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::{compute_content_hash, encrypt, IncomingItem, ItemKey, Store};

mod support;
use support::{clipping, keyring, row, T0};

const HISTORIES: [usize; 3] = [500, 2_000, 8_000];
const ROW_BYTES: usize = 512;

/// Payload sizes for the write sweep, with the sample count each affords.
const PAYLOADS: [(&str, usize, usize); 4] = [
    ("512B", 512, 100),
    ("64KiB", 64 * 1024, 50),
    ("1MiB", 1024 * 1024, 20),
    ("4MiB", 4 * 1024 * 1024, 10),
];

/// Depth for the payload sweep, held constant so a number that moves is the
/// payload's doing and not the history's. Disk is what bounds it: `clipboard_fts`
/// is not contentless, so a primed 4 MiB row costs its ciphertext and its
/// plaintext both, and 32 of them is ~300 MB.
const PAYLOAD_HISTORY: usize = 32;

/// Outside the range `fill` writes, so a timed insert is never silently a bump
/// against a row the priming pass already put there.
static SEED: AtomicUsize = AtomicUsize::new(1_000_000);

/// The one content `bump` re-copies. Distinct from every `seed()`.
const DUPLICATE_SEED: usize = usize::MAX;

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

/// Everything an incoming version needs that must be built outside the timed
/// region: id, plaintext, ciphertext, nonce, hash.
type Sealed = (String, String, Vec<u8>, Vec<u8>, String);

fn sealed(key: &ItemKey, bytes: usize) -> Sealed {
    let content = clipping(bytes, seed());
    let id = format!("peer-{:016x}", seed());
    let (nonce, ciphertext) = encrypt(content.as_bytes(), key, &id).expect("seal");
    let hash = compute_content_hash(content.as_bytes());
    (id, content, ciphertext, nonce, hash)
}

fn apply(store: &Store, (id, content, ciphertext, nonce, hash): Sealed) -> bool {
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
}

/// The merge write: the only call that may overwrite an existing row.
fn upsert(c: &mut Criterion) {
    let key = keyring().item_key();
    let mut group = c.benchmark_group("storage/upsert");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.bench_with_input(BenchmarkId::from_parameter(history), &history, |b, _| {
            b.iter_batched(
                || {
                    store.evict_over_cap(history as u64).expect("evict");
                    sealed(&key, ROW_BYTES)
                },
                |item| apply(&store, item),
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// The two retention gates with nothing to delete, which is what they do on
/// every capture but one in `history_limit`.
///
/// **Read these across the three depths, not one at a time.** Each gate answers
/// a yes/no question that must not depend on how much history the answer is
/// about, so a figure that climbs with the row count is a scan on the capture
/// path — that is what `cap_nothing_to_do` did at 331 µs against 8 000 rows
/// while every absolute number still looked unremarkable.
fn gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/sweep");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = primed(dir.path(), history);

        group.bench_with_input(
            BenchmarkId::new("cap_nothing_to_do", history),
            &history,
            |b, &history| b.iter(|| store.evict_over_cap(black_box(history as u64))),
        );
        group.bench_with_input(
            BenchmarkId::new("byte_cap_nothing_to_do", history),
            &history,
            |b, _| b.iter(|| store.evict_over_byte_cap(black_box(u64::MAX))),
        );
    }
    group.finish();
}

/// The same writes swept over payload size instead of history depth.
///
/// Nothing in the groups above moves when a change costs per byte — a second
/// pass over the ciphertext, an UPDATE that rewrites a big row rather than a
/// stamp, an FTS write that carries the whole plaintext twice. The two sweeps
/// stay separate because the matrix is not affordable: 8 000 four-MiB rows is
/// 32 GB, so a size is stated against this depth or not at all.
///
/// `cap_nothing_to_do` is deliberately not here. It reads a single counter row,
/// so no payload can reach it; the byte-cap gate sums a per-row byte value and
/// is the one gate that has to be read on both axes.
fn payloads(c: &mut Criterion) {
    let keyring = keyring();
    let key = keyring.item_key();

    for (label, bytes, samples) in PAYLOADS {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        support::fill(&store, &keyring, PAYLOAD_HISTORY - 1, bytes);

        // Stamped newest, so neither the cap sweep below nor its
        // never-evict-the-newest rule can take the row `bump` has to find.
        let duplicate = clipping(bytes, DUPLICATE_SEED);
        let newest = T0 + PAYLOAD_HISTORY as i64 * 1_000;
        store
            .insert(row(&keyring, &duplicate, newest))
            .expect("duplicate");

        let mut group = c.benchmark_group(format!("storage/payload/{label}"));
        group.sample_size(samples);
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_function("insert", |b| {
            b.iter_batched(
                || {
                    store.evict_over_cap(PAYLOAD_HISTORY as u64).expect("evict");
                    row(&keyring, &clipping(bytes, seed()), T0)
                },
                |item| store.insert_or_bump(item).expect("insert"),
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function("bump", |b| {
            b.iter_batched(
                || row(&keyring, &duplicate, T0),
                |item| store.insert_or_bump(item).expect("bump"),
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function("upsert", |b| {
            b.iter_batched(
                || {
                    store.evict_over_cap(PAYLOAD_HISTORY as u64).expect("evict");
                    sealed(&key, bytes)
                },
                |item| apply(&store, item),
                criterion::BatchSize::SmallInput,
            );
        });

        // The gate is a SUM(LENGTH(...)) over every unpinned row (§5), and
        // SQLite takes a blob's length from the record header rather than its
        // overflow pages — so this should track rows and not bytes. Rows per
        // second is therefore the unit that stays flat across the sweep, and a
        // change that makes the gate read payloads is a fall in it.
        group.throughput(Throughput::Elements(PAYLOAD_HISTORY as u64));
        group.bench_function("byte_cap_nothing_to_do", |b| {
            b.iter(|| store.evict_over_byte_cap(black_box(u64::MAX)));
        });

        group.finish();
    }
}

criterion_group!(benches, summaries, insert_or_bump, upsert, gates, payloads);
criterion_main!(benches);
