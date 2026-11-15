//! Peak heap of sealing and opening one full cloud file object. The file
//! round runs in the app process on Android, so every extra 4 MiB copy of an
//! object is retained memory there.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use copypaste_cloud::crypto::file::{file_keys, FileManifest, OBJECT_PLAINTEXT_BYTES};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Heap bytes `work` holds at its peak beyond what was live before it.
fn peak_above_baseline<T>(work: impl FnOnce() -> T) -> (T, usize) {
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);
    let value = work();
    (value, PEAK.load(Ordering::Relaxed) - baseline)
}

#[test]
fn a_full_object_is_sealed_and_opened_with_one_object_buffer() {
    const MIB: usize = 1024 * 1024;
    let keys = file_keys(&[9; 32], 0);
    let manifest = FileManifest {
        size_bytes: OBJECT_PLAINTEXT_BYTES as u64,
        sha256: "a".repeat(64),
    };
    let id = keys.object_id("file-offer", 0);
    let plaintext = vec![0x5a; OBJECT_PLAINTEXT_BYTES];

    let (object, sealing) =
        peak_above_baseline(|| keys.seal_chunk(&id, &manifest, 1, &plaintext).unwrap());
    let (opened, opening) =
        peak_above_baseline(|| keys.open_chunk(&id, &manifest, 1, object).unwrap());
    assert_eq!(*opened, plaintext);
    println!(
        "seal peak above plaintext: {:.2} MiB; open peak above object: {:.2} MiB",
        sealing as f64 / MIB as f64,
        opening as f64 / MIB as f64
    );
    assert!(
        sealing < OBJECT_PLAINTEXT_BYTES + MIB,
        "seal held {sealing} B"
    );
    assert!(opening < MIB, "open held {opening} B");
}
