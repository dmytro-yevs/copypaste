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
//   * Relay body   =  10 MiB — relay request-body cap.
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
// 100 = lossless / original quality (field is currently a no-op for PNG; kept
// for future JPEG support — never compress by default).
pub const IMAGE_QUALITY: u8 = 100;
// [P2] sqlite_cache_mb is stored in AppConfig and exposed to users, but the
// actual SQLite cache size is hardcoded to 8 MB in both db.rs
// (CONNECTION_PRAGMAS: `PRAGMA cache_size = -8192`) and schema.rs
// (`PRAGMA cache_size = -8192` in apply_migrations). The hardcoded value
// intentionally equals SQLITE_CACHE_MB * 1024 = 8 * 1024 = 8192, so the
// default matches. To honour a user-supplied value, thread AppConfig through
// Database::open() and replace the two literal `-8192` values with
// `-<sqlite_cache_mb as i64 * 1024>` computed at runtime. Deferred because
// the open() signature ripple touches multiple callers across crates.
pub const SQLITE_CACHE_MB: u32 = 8;
pub const ENCRYPTION_CHUNK_KB: u32 = 64;
pub const MAX_DECODED_IMAGE_MB: u32 = 50;
pub const MAX_BANDWIDTH_KBPS: u32 = 0;
// FIXWAVE: INLINE_THRESHOLD_BYTES is defined here but never read by any production
// code path (grep shows zero callsites outside this file). Either wire it into the
// daemon's image-inlining decision or remove it to avoid dead-const confusion.
pub const INLINE_THRESHOLD_BYTES: u64 = 512_000;
