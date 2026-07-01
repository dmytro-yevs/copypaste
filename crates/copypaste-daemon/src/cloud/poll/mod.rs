//! Cloud download (poll) pipeline — the keyset cursor, the long-running
//! realtime/poll loop, the per-batch ingest pipeline, and the HTTP transport.
//!
//! Split (CopyPaste-vp63.28) from a single flat `poll.rs` into cohesion
//! clusters:
//! - [`cursor`] — `PollCursor` alias, `build_poll_url`, watermark load/save.
//! - [`loop_task`] — `realtime_loop`, the long-running task.
//! - [`ingest`] — `poll_once` and the per-row `ingest_row` helper (the
//!   dedup/LWW/tombstone/decrypt pipeline carved out of the old inline loop).
//! - [`transport`] — `FetchOutcome`, `fetch_remote_rows(_with_refresh)`.

use std::time::Duration;

mod cursor;
mod ingest;
mod loop_task;
mod transport;

pub(super) use loop_task::realtime_loop;

// These re-exports are only reached by `cloud`'s test-only facade
// (`cloud/mod.rs`'s `#[cfg(test)] pub(crate) use poll::{...}`); production
// code (`loop_task.rs`/`ingest.rs`/`transport.rs` themselves) reaches its
// siblings directly via `super::cursor::...` / `super::transport::...`
// without going through this facade. Gate to avoid unused-import warnings in
// non-test builds.
#[cfg(test)]
pub(crate) use cursor::{build_poll_url, load_poll_watermark, save_poll_watermark, PollCursor};
#[cfg(test)]
pub(crate) use ingest::poll_once;
#[cfg(test)]
pub(crate) use transport::{fetch_remote_rows, fetch_remote_rows_with_refresh, FetchOutcome};

// ── Realtime / poll-interval tuning (v0.5.3) ─────────────────────────────────

/// HTTP poll interval when the Realtime WebSocket is **connected** *and the
/// Phoenix Channel join has been confirmed* (`phx_reply ok`).
///
/// The WS delivers INSERT events instantly once the channel is subscribed, so
/// the poll loop runs only as a catch-up / missed-event safety net at a lower
/// frequency.  Lowered from 120 s → 60 s (Phase 3) to halve the worst-case
/// missed-event window while still keeping the HTTP load negligible compared
/// to full-speed fallback polling.
const POLL_INTERVAL_WS_CONNECTED: Duration = Duration::from_secs(60);

/// HTTP poll interval when the Realtime WebSocket is **disconnected** or
/// has never connected (original behaviour — full-speed polling as the sole
/// sync path).
const POLL_INTERVAL_WS_FALLBACK: Duration = Duration::from_secs(10);

/// Maximum number of rows fetched per poll tick.
///
/// When a batch comes back full (== POLL_BATCH_SIZE rows), the poll loop
/// immediately re-polls without waiting for the full interval (burst-drain).
/// This prevents a burst of simultaneous remote inserts from stalling at the
/// watermark for a full interval when the batch was exactly exhausted.
const POLL_BATCH_SIZE: usize = 20;
