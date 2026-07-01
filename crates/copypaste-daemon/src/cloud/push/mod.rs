//! Cloud push pipeline — prepare/encrypt, the retry-queue loop, queue
//! bookkeeping, and the HTTP transport/retry state machine.
//!
//! Split (CopyPaste-vp63.29) from a single flat `push.rs` into cohesion
//! clusters:
//! - [`prepare`] — `prepare_and_enqueue_item` (sensitive gate, decrypt,
//!   cloud re-encrypt).
//! - [`loop_task`] — `push_loop`, the long-running task, plus the shared
//!   `attempt_push_and_record` helper (dedups the throttle/push/record
//!   pipeline that used to be copy-pasted twice in the loop body).
//! - [`queue`] — `enqueue_for_retry` / `mark_item_synced`.
//! - [`transport`] — `PushOutcome`, `push_item_once`, `parse_retry_after_secs`,
//!   `push_item_with_retries`.

mod loop_task;
mod prepare;
mod queue;
mod transport;

pub(super) use loop_task::push_loop;
pub(crate) use queue::enqueue_for_retry;
pub(crate) use transport::parse_retry_after_secs;
// `push_item_with_retries` is only reached from `cloud`'s test-only e2e/bytea
// harnesses (`cloud/mod.rs`'s `#[cfg(test)] pub(crate) use push::{...}` and
// `cloud::auth`'s test module); production code (`loop_task.rs`) reaches it
// directly via `super::transport::push_item_with_retries`. Gate to avoid an
// unused-import warning in non-test builds.
#[cfg(test)]
pub(crate) use transport::push_item_with_retries;

use std::time::Duration;

// ── Push reliability tuning (Wave 2.7 edge #19/#20/#21) ───────────────────────

/// Maximum number of items the in-memory retry queue will hold before it starts
/// dropping the oldest entries. Bounded so a sustained outage cannot exhaust
/// daemon memory.
pub(crate) const PUSH_RETRY_QUEUE_CAP: usize = 1024;

/// Maximum delay between retry attempts for transient push failures.
pub(super) const PUSH_MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Initial delay between retry attempts. Doubles on each failure up to
/// `PUSH_MAX_BACKOFF`.
pub(super) const PUSH_INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// How often the push loop drains pending broadcast-channel items into the
/// retry queue when the retry queue is non-empty (CopyPaste-1t38).
///
/// Without a periodic drain, pin/delete/new-item events sent to `new_item_tx`
/// while the loop is busy draining a failed retry queue accumulate in the
/// broadcast ring buffer. If the ring buffer fills (default capacity: 16), the
/// oldest events are silently dropped (Lagged). A 10-second drain interval
/// ensures that mutations are picked up even during a sustained cloud outage.
pub(crate) const MUTATION_QUEUE_DRAIN_INTERVAL: Duration = Duration::from_secs(10);
