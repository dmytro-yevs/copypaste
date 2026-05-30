pub const CONFIG_VERSION: u32 = 1;
// Intentionally generous: history should feel unbounded to the user; the local
// DB is a cache (cloud / P2P holds the long tail).
pub const HISTORY_LIMIT: usize = 100_000;
pub const POLL_INTERVAL_MS: u64 = 500;
pub const POLL_INTERVAL_MIN_MS: u64 = 100;
pub const POLL_INTERVAL_MAX_MS: u64 = 5000;
// 15 MiB — stays safely under the 16 MiB P2P/IPC wire-frame cap.
pub const MAX_TEXT_SIZE_BYTES: u64 = 15 * 1024 * 1024;
// 64 MiB — supports high-res screenshots at original quality.
pub const MAX_IMAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
// 1 GiB — generous; local DB is a cache, not the bottleneck.
pub const MAX_FILE_SIZE_BYTES: u64 = 1024 * 1024 * 1024;
// 10 GiB local quota; cloud / P2P back-fill is bounded by sync_ttl_secs.
pub const STORAGE_QUOTA_BYTES: u64 = 10 * 1024 * 1024 * 1024;
// 30 days — cloud tail persists long enough for infrequent device pairs.
pub const SYNC_TTL_SECS: u64 = 2_592_000;
pub const SENSITIVE_TTL_RELAY_SECS: u64 = 1_800;
pub const SENSITIVE_TTL_LOCAL_SECS: u64 = 1_800;
pub const SENSITIVE_TTL_SECS: u64 = 30;
// 100 = lossless / original quality (field is currently a no-op for PNG; kept
// for future JPEG support — never compress by default).
pub const IMAGE_QUALITY: u8 = 100;
// FIXWAVE: sqlite_cache_mb is stored in AppConfig and exposed to users, but the
// actual SQLite cache size is hardcoded to 8 MB in schema.rs (`PRAGMA cache_size`).
// To wire this up: read AppConfig in db.rs open() and apply
// `PRAGMA cache_size = -<sqlite_cache_mb * 1024>` after schema init.
// Owned by the schema/db agent — do not change schema.rs here.
pub const SQLITE_CACHE_MB: u32 = 8;
pub const ENCRYPTION_CHUNK_KB: u32 = 64;
pub const MAX_DECODED_IMAGE_MB: u32 = 50;
pub const MAX_BANDWIDTH_KBPS: u32 = 0;
// FIXWAVE: INLINE_THRESHOLD_BYTES is defined here but never read by any production
// code path (grep shows zero callsites outside this file). Either wire it into the
// daemon's image-inlining decision or remove it to avoid dead-const confusion.
pub const INLINE_THRESHOLD_BYTES: u64 = 512_000;
