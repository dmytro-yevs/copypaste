//! [`SyncItem`] — the encrypted envelope stored per-device in a sync inbox —
//! and the SSE wake-channel capacity constant.

/// A single encrypted item in the wall-clock push/pull sync protocol.
pub struct SyncItem {
    /// Auto-incremented integer ID (unique per device inbox, ascending).
    pub id: i64,
    pub content_type: String,
    /// Opaque base64 ciphertext. `Arc<str>` (CopyPaste-ux2i) so `pull_items`
    /// clones a refcount under the global store mutex instead of memcpy-ing the
    /// full payload; the cloned `Arc` is handed straight to the `PullItem`.
    pub content_b64: std::sync::Arc<str>,
    /// Sender wall-clock time (Unix epoch milliseconds).
    pub wall_time: u64,
    /// Server-side wall-clock time at insert (Unix epoch seconds). Used for
    /// TTL eviction independent of (untrusted) sender `wall_time`. Read by
    /// `prune_expired` (in this module) and the background evictor in `store.rs`.
    pub inserted_at_unix: u64,
}

/// Capacity of each per-device SSE wake channel. A small ring buffer is
/// sufficient because the payload is a contentless wake tick: if a burst of
/// pushes overflows it, the receiver observes `RecvError::Lagged` and simply
/// re-reads the inbox from its cursor, picking up every missed item. Sized to
/// absorb a modest burst without forcing a lag-driven full re-read on every push.
// Used by `subscribe_notifier`, which is called from the production SSE route
// (`routes/items.rs`). `#[path]`-include test binaries that compile state.rs
// without the routes module do not exercise this path; those test crates
// suppress dead_code at the crate level (see individual test file headers).
pub(super) const SYNC_NOTIFY_CHANNEL_CAP: usize = 64;
