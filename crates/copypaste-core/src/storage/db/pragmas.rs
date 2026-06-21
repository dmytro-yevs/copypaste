/// Format a 32-byte key as the hex string SQLCipher expects:
///   PRAGMA key = "x'<64 hex chars>'"
///
/// Returns a `Zeroizing<String>` so the key hex is scrubbed from the heap
/// as soon as the returned value is dropped, limiting the window during
/// which plaintext key material appears in a heap dump.
pub(super) fn key_pragma(key: &[u8; 32]) -> zeroize::Zeroizing<String> {
    use std::fmt::Write;
    let mut hex = zeroize::Zeroizing::new(String::with_capacity(64));
    for b in key {
        // Infallible: `fmt::Write for String` only grows a heap buffer and
        // never returns Err, so this formatted write cannot fail.
        write!(*hex, "{:02x}", b).unwrap();
    }
    zeroize::Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", *hex))
}

/// Per-connection PRAGMAs that must follow `PRAGMA key`. These are NOT
/// persisted to the database file — every fresh `Connection` must apply them
/// again. Skipping these is the root cause of two production issues:
///   * Missing `busy_timeout` ⇒ UI reader and daemon writer race instantly,
///     surfacing as silent `SQLITE_BUSY`.
///   * Missing `foreign_keys=ON` ⇒ any `ON DELETE CASCADE` FK silently no-ops.
///
/// NOTE (CopyPaste-6fd): the schema currently declares NO `ON DELETE CASCADE`
/// foreign keys. In particular `pending_uploads(item_id)` is a bare PK with no
/// FK back to `clipboard_items`, so this pragma does NOT cascade-clean it when
/// an item is hard-deleted. That cleanup is done explicitly in code by
/// `storage::items::delete_pending_uploads_for_ids`, called from every
/// hard-delete / prune / evict path. Keep `foreign_keys=ON` set anyway so any
/// future cascading FK behaves; do not rely on it for `pending_uploads`.
///
/// Keep this in sync with `pool::open_pool` and `schema::apply_migrations`
/// — every code path that opens a SQLCipher connection must apply the same
/// set so behaviour is uniform across UI reader, daemon writer, and the
/// migration pass.
///
/// The `cache_size` pragma is NOT included here because it is configurable:
/// it is applied separately via [`cache_size_pragma`] so a caller's
/// `AppConfig::sqlite_cache_mb` can be honoured. Every open path applies both
/// this static set and a `cache_size_pragma(..)` (see [`connection_pragmas`]).
pub(crate) const CONNECTION_PRAGMAS: &str = "\
PRAGMA busy_timeout = 5000;\n\
PRAGMA synchronous = NORMAL;\n\
PRAGMA foreign_keys = ON;\n\
PRAGMA temp_store = MEMORY;\n\
PRAGMA wal_autocheckpoint = 1000;\n\
PRAGMA journal_size_limit = 67108864;\n";
// wal_autocheckpoint=1000: trigger a passive checkpoint after every 1000
// WAL pages (~4 MiB at the 4 KiB default page size). Without this the WAL
// file grows without bound during the v4 migration sweep, which can write
// tens of thousands of rows in a single session. The default is 1000 pages
// (SQLite default), but explicitly setting it here ensures the pool and the
// single-connection path both see the same value (CopyPaste-ayg).
//
// journal_size_limit=64 MiB: cap the WAL file size so even if the
// checkpoint cannot shrink the file immediately (e.g. active reader holding
// a snapshot), it is truncated back to at most 64 MiB on the next successful
// checkpoint. This bounds disk usage during migration sweeps that touch many
// rows in one run.

/// Build the `PRAGMA cache_size` statement for `cache_mb` MiB of page cache.
///
/// SQLite treats a NEGATIVE `cache_size` as a memory budget in KiB, so
/// `cache_mb` MiB maps to `-(cache_mb * 1024)`. `cache_mb` is clamped to
/// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` here too (defence in depth: the
/// value normally arrives already clamped from `AppConfig::clamp_values`, but
/// callers may pass a raw value). The default (8 MiB) yields the historical
/// `PRAGMA cache_size = -8192;`.
pub(crate) fn cache_size_pragma(cache_mb: u32) -> String {
    let mb = cache_mb.clamp(
        crate::config::SQLITE_CACHE_MB_MIN,
        crate::config::SQLITE_CACHE_MB_MAX,
    );
    // mb <= 256, so mb * 1024 (<= 262_144) fits in i64 without overflow.
    let kib = i64::from(mb) * 1024;
    format!("PRAGMA cache_size = -{kib};\n")
}

/// The full per-connection pragma batch (static set + configurable cache_size).
pub(crate) fn connection_pragmas(cache_mb: u32) -> String {
    format!("{CONNECTION_PRAGMAS}{}", cache_size_pragma(cache_mb))
}
