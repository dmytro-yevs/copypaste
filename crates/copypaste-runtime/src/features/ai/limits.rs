//! Per-grant rate limits and per-session read volume.

/// Per-grant token bucket: calls allowed in one burst window.
#[expect(dead_code, reason = "read once tool calls are served")]
pub(super) const BURST_CALLS: u32 = 30;
#[expect(dead_code, reason = "read once tool calls are served")]
pub(super) const BURST_WINDOW_SECS: u64 = 10;
#[expect(dead_code, reason = "read once tool calls are served")]
pub(super) const HOURLY_CALLS: u32 = 600;

/// Content one session may read before the desktop must confirm more; each
/// approval grants this much again.
#[expect(dead_code, reason = "read once tool calls are served")]
pub(super) const SESSION_READ_ALLOWANCE_BYTES: u64 = 2 * 1024 * 1024;
