pub const CONFIG_VERSION: u32 = 1;
// Intentionally generous: history should feel unbounded to the user; the local
// DB is a cache (cloud / P2P holds the long tail).
pub const HISTORY_LIMIT: usize = 100_000;
pub const POLL_INTERVAL_MS: u64 = 500;
pub const POLL_INTERVAL_MIN_MS: u64 = 100;
pub const POLL_INTERVAL_MAX_MS: u64 = 5000;
// 10 MiB — stays safely under the 16 MiB P2P/IPC wire-frame cap.
pub const MAX_TEXT_SIZE_BYTES: u64 = 10 * 1024 * 1024;
// 64 MiB — supports high-res screenshots at original quality.
pub const MAX_IMAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
// File-size ceiling layering (the single storable source of truth lives in
// `crate::file::MAX_FILE_BYTES`; `clamp_values` enforces it as an upper bound on
// this knob). All four numbers below describe the SAME blob travelling through
// the system, so the next editor sees how they relate before bumping any one:
//   * STORABLE cap = 100 MiB — `crate::file::MAX_FILE_BYTES`, the library hard
//     cap on the raw bytes a file item may occupy locally. `max_file_size_bytes`
//     is a user knob *below* this; it can never exceed it (clamped).
//   * SYNC cap     =   8 MiB — `sync_orch::SYNC_MAX_BLOB_BYTES`: the largest
//     reassembled plaintext re-keyed onto the wire. Files between 8 MiB and
//     100 MiB are stored/kept locally but SKIPPED for P2P/relay sync (warned).
//   * P2P frame    =  16 MiB — transport framing cap (`p2p` transport).
//   * Relay caps   = per content type (`copypaste-relay` `quota::Tier`):
//       - text         =  8 MiB — matches SYNC cap, so a 1–8 MiB text item that
//         stores locally and passes the sync caps is not rejected 413.
//       - image + file = 10 MiB each — matches the relay request-body cap.
// 100 MiB — matches `crate::file::MAX_FILE_BYTES` (the storable hard cap). Kept
// honest: a larger default would be silently clamped back down on load.
pub const MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;
// 10 GiB local quota; cloud / P2P back-fill is bounded by sync_ttl_secs.
pub const STORAGE_QUOTA_BYTES: u64 = 10 * 1024 * 1024 * 1024;
// Sane minimum floors for the size/quota caps. A previous bug set
// storage_quota_bytes to 200 bytes in config.toml; prune_to_cap then evicted
// almost every unpinned row after every insert (self-clearing history) and
// dropped fresh images. Flooring at .max(1) is not enough — these caps must
// stay large enough that normal clipboard use is never wiped. All values are
// well below their defaults; clamping to them only protects against absurd
// (sub-floor) input, never against legitimate small-but-reasonable limits.
// 50 MiB — below this the byte-cap prune would wipe normal history.
pub const MIN_STORAGE_QUOTA_BYTES: u64 = 50 * 1024 * 1024;
// 64 KiB — comfortably fits ordinary copied text.
pub const MIN_TEXT_SIZE_BYTES: u64 = 64 * 1024;
// 1 MiB — below this even a small screenshot would be rejected.
pub const MIN_IMAGE_SIZE_BYTES: u64 = 1024 * 1024;
// 1 MiB — keep file captures usable.
pub const MIN_FILE_SIZE_BYTES: u64 = 1024 * 1024;
// 30 days — cloud tail persists long enough for infrequent device pairs.
pub const SYNC_TTL_SECS: u64 = 2_592_000;
pub const SENSITIVE_TTL_RELAY_SECS: u64 = 1_800;
pub const SENSITIVE_TTL_LOCAL_SECS: u64 = 1_800;
pub const SENSITIVE_TTL_SECS: u64 = 30;
// sqlite_cache_mb is the per-connection SQLite page-cache size in MiB. It is
// wired through `Database::open_with_cache_mb` /
// `Database::open_in_memory_with_cache_mb` (and the pooled
// `open_pool_with_cache_mb`), which apply `PRAGMA cache_size = -(cache_mb * 1024)`
// (a negative cache_size value means KiB units). The plain `open` /
// `open_in_memory` / `open_pool` entry points delegate with this default, so
// callers that don't tune it keep an unchanged 8 MiB cache.
// `AppConfig::clamp_values` bounds the configured value to
// `SQLITE_CACHE_MB_MIN..=SQLITE_CACHE_MB_MAX` so a bad config can't request a
// pathological cache.
pub const SQLITE_CACHE_MB: u32 = 8;
// Sane bounds for `sqlite_cache_mb`: at least 1 MiB (a smaller cache hurts more
// than it helps) and at most 256 MiB (above this a hand-edited config could pin
// hundreds of MiB of resident memory per connection).
pub const SQLITE_CACHE_MB_MIN: u32 = 1;
pub const SQLITE_CACHE_MB_MAX: u32 = 256;
pub const ENCRYPTION_CHUNK_KB: u32 = 64;
pub const MAX_DECODED_IMAGE_MB: u32 = 50;
// TODO: not yet enforced — `max_bandwidth_kbps` is stored in `AppConfig` and
// propagated to config serialisation, but no code path in the daemon reads this
// value to throttle upload or download throughput. Value of 0 means "unlimited"
// by convention; the field exists as a future wiring point.
pub const MAX_BANDWIDTH_KBPS: u32 = 0;
