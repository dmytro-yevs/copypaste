//! Non-blocking inbox forwarder for the P2P sync pipeline.
//!
//! # Problem (CopyPaste-bxsa)
//!
//! The daemon wires P2P accept/connector tasks to `sync_orch` via an
//! `mpsc::channel::<WireItem>(64)` (`sync_incoming_tx` / `sync_incoming_rx`).
//! Every per-peer read task calls `incoming_tx.send(wire).await` when a frame
//! arrives from the peer. If `sync_orch` is slow to consume from
//! `sync_incoming_rx` — e.g. blocked on a DB write — all per-peer tasks park
//! on that `.await`. Back-pressure propagates: TCP receive-buffers fill,
//! mTLS accept() stalls on the OS socket queue, and new connections cannot
//! be established.
//!
//! # Fix
//!
//! [`SyncInboxForwarder`] decouples the two sides with:
//!
//! 1. **A bounded ring buffer** (capacity `N`) protected by a `Mutex` +
//!    `Notify`. P2P tasks call [`SyncInboxSender::try_enqueue`] which is
//!    lock-take + push + notify — never blocks on the downstream consumer.
//!
//! 2. **Drop-oldest policy when full**: if the ring already holds `N` items,
//!    the *oldest* entry is evicted (not the new one) so the consumer always
//!    receives the most recent data. Evictions are counted in a
//!    [`crate::metrics::SyncLagCounter`] so operators can observe them.
//!
//! 3. **A dedicated forwarding task** spawned by
//!    [`SyncInboxForwarder::start`] that reads from the ring (`.await` on
//!    `Notify`) and forwards each item to the real downstream
//!    `mpsc::Sender<WireItem>` via `.send().await`. When the downstream
//!    channel closes the task exits cleanly.
//!
//! P2P tasks now hold a [`SyncInboxSender`] instead of the raw downstream
//! sender. The forwarding task is the only place that blocks on the downstream;
//! it is spawned once and is isolated from the accept loop.
//!
//! See the `replay_guard` submodule docs for the replay-attack protection
//! (CopyPaste-4cyh) applied on enqueue.
//!
//! # `ReplayGuard` construction site (CopyPaste-sreb)
//!
//! [`ReplayGuard`] is constructed inline, per-connection, directly in
//! `copypaste-daemon`'s `p2p::framed_pump::run_peer_connection_framed` —
//! the single duplex pump shared by both the inbound (accept-side) and
//! outbound (connector-side) connection tasks — rather than inside
//! [`SyncInboxForwarder`]/[`SyncInboxSender`]. [`SyncInboxForwarder`] remains
//! available but currently unused in the daemon's wiring; it stays in the
//! crate for a future consumer that needs the ring-buffer/backpressure
//! decoupling described above, independent of replay protection.

mod forwarder;
mod replay_guard;
mod sender;
mod state;
#[cfg(test)]
mod tests;

pub use forwarder::SyncInboxForwarder;
pub use replay_guard::{ReplayGuard, REPLAY_GUARD_CAPACITY};
pub use sender::SyncInboxSender;
