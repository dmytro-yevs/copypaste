//! Reading the history: what the app's list poll costs the daemon.
//!
//! The app fetches `PAGE_SIZE = 200` and re-fetches on a timer, and the daemon
//! answers by decrypting every row in the page — so the page fetch and the
//! decrypt are measured separately, and the keyset seek is measured against
//! the `LIMIT/OFFSET` it replaced at the same depth.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use copypaste_core::{ItemCursor, Store};

mod support;
use support::{decrypt_page, fill, keyring};

/// `copypaste-ui/src/lib/layout.ts`: `PAGE_SIZE`.
const PAGE: u32 = 200;
/// Row size for the fixtures: a URL, a paragraph, a stack frame.
const ROW_BYTES: usize = 512;
/// `ConfigData::history_limit` defaults to 10 000, so that is the deep end of
/// what a user's list actually reaches.
const HISTORIES: [usize; 2] = [1_000, 10_000];

/// The cursor `pages` pages into the list, and the rows of that page.
fn seek(store: &Store, pages: usize) -> Option<ItemCursor> {
    let mut cursor = None;
    for _ in 0..pages {
        let page = store.list_from(cursor.as_ref(), PAGE).expect("page");
        cursor = page.next;
        cursor.as_ref()?;
    }
    cursor
}

fn pages(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("history/page");
    group.throughput(Throughput::Elements(u64::from(PAGE)));

    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, ROW_BYTES);
        let deep = seek(&store, history / PAGE as usize - 1);

        group.bench_with_input(
            BenchmarkId::new("keyset_first", history),
            &history,
            |b, _| {
                b.iter(|| store.list_from(None, black_box(PAGE)).expect("page"));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("keyset_last", history),
            &history,
            |b, _| {
                b.iter(|| {
                    store
                        .list_from(deep.as_ref(), black_box(PAGE))
                        .expect("page")
                });
            },
        );
        // The construction keyset pagination replaced (`CopyPaste-8ebg.57`).
        // Kept as a bench, not as a caller: it is what "why keyset" costs.
        let last_offset = (history as u32).saturating_sub(PAGE);
        group.bench_with_input(
            BenchmarkId::new("offset_last", history),
            &history,
            |b, _| {
                b.iter(|| store.list(black_box(PAGE), last_offset).expect("list"));
            },
        );
    }
    group.finish();
}

/// The AEAD open the daemon does per row before a page goes on the wire.
fn decrypt(c: &mut Criterion) {
    let keyring = keyring();
    let dir = tempfile::tempdir().expect("tempdir");
    let store = support::store_in(dir.path());
    fill(&store, &keyring, 1_000, ROW_BYTES);
    let page = store.list_from(None, PAGE).expect("page").items;

    let mut group = c.benchmark_group("history/decrypt_page");
    group.throughput(Throughput::Bytes((PAGE as usize * ROW_BYTES) as u64));
    group.bench_function("200x512B", |b| {
        b.iter(|| decrypt_page(&keyring, black_box(&page)));
    });
    group.finish();
}

fn search(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("history/search");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, ROW_BYTES);

        // A token every row carries, so the index does the most work it can.
        group.bench_with_input(BenchmarkId::new("common", history), &history, |b, _| {
            b.iter(|| store.search(black_box("quick"), 500).expect("search"));
        });
        // A token exactly one row carries.
        group.bench_with_input(BenchmarkId::new("unique", history), &history, |b, _| {
            b.iter(|| store.search(black_box("clipping"), 500).expect("search"));
        });
    }
    group.finish();
}

fn count(c: &mut Criterion) {
    let keyring = keyring();
    let mut group = c.benchmark_group("history/count");
    for history in HISTORIES {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = support::store_in(dir.path());
        fill(&store, &keyring, history, ROW_BYTES);
        // Every `status` call, and the app polls that every 2 s.
        group.bench_with_input(BenchmarkId::new("count", history), &history, |b, _| {
            b.iter(|| store.count().expect("count"));
        });
    }
    group.finish();
}

criterion_group!(benches, pages, decrypt, search, count);
criterion_main!(benches);
