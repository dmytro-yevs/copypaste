/// copypaste-sync — P2P clipboard sync engine.
///
/// # ⚠️ Production path vs. dead code (read before maintaining)
///
/// The daemon does **NOT** drive sync through [`engine::SyncEngine`] or
/// [`clock::LamportClock`]. As of the sync-correctness audit (CopyPaste-j6r)
/// those types are **not on the production path** — they are retained for the
/// HELLO/HAVE/WANT/ITEMS/DONE session protocol and its tests, but the live
/// daemon never instantiates a `SyncEngine` and never advances a
/// `LamportClock`.
///
/// What the daemon actually uses from this crate is **only**:
///   * [`protocol::WireItem`] — the on-wire item shape, and
///   * [`merge::resolve`] / [`merge::remote_wins`] / [`merge::wire_to_local`] /
///     [`merge::local_to_wire`] — the Last-Write-Wins decision and conversions.
///
/// The daemon stamps `lamport_ts` itself via
/// `copypaste_core::next_lamport_ts(prev, now_ms) = max(prev + 1, now_ms)` at
/// every mutation (capture / recopy / pin / delete), giving one monotonic AND
/// time-ordered value space (CopyPaste-ojhe). All three transports — P2P,
/// Supabase cloud, and the relay — route their LWW through the SAME total order
/// (`merge::resolve` / `merge::remote_wins`: lamport → wall_time →
/// origin_device_id) so they converge identically (CopyPaste-ayvs).
///
/// Do not "revive" `SyncEngine`/`LamportClock` by wiring them into the daemon
/// without re-validating against this contract; the `merge::resolve` path is
/// the source of truth.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────┐
/// │              SyncEngine                  │
/// │  device_id: String                       │
/// │  clock: LamportClock                     │
/// │  peer_clocks: HashMap<DeviceId, State>   │
/// │                                          │
/// │  run_session(stream, local_items)        │
/// │    → drives HELLO/HAVE/WANT/ITEMS/DONE  │
/// └─────────────────────────────────────────┘
///          │ uses
///          ▼
/// ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
/// │  LamportClock│  │  protocol.rs │  │   merge.rs   │
/// │  tick/observe│  │  Message enum│  │  LWW resolve │
/// └──────────────┘  └──────────────┘  └──────────────┘
/// ```
///
/// # Usage
///
/// ```rust,no_run
/// use copypaste_sync::engine::SyncEngine;
/// use copypaste_core::storage::items::ClipboardItem;
///
/// # async fn example(mut stream: tokio::io::DuplexStream) -> Result<(), Box<dyn std::error::Error>> {
/// let mut engine = SyncEngine::new("my-device-uuid");
/// let local_items: Vec<ClipboardItem> = vec![];  // load from DB
///
/// let (result, to_upsert) = engine.run_session(&mut stream, &local_items).await?;
/// println!("received {} new items", result.items_received);
/// // persist to_upsert into SQLite here
/// # Ok(())
/// # }
/// ```
pub mod clock;
pub mod engine;
pub mod inbox;
pub mod merge;
pub mod metrics;
pub mod protocol;

// Convenience re-exports.
//
// CopyPaste-crh3.92: `SyncEngine` and `LamportClock` are NOT consumed by any
// production binary — the daemon drives sync through `crate::sync_orch`, not this
// HELLO/HAVE/WANT/ITEMS/DONE session engine. They are intentionally retained as
// `pub` because the session-protocol conformance tests (engine.rs / clock.rs and
// the protocol round-trip tests) exercise them as the executable specification of
// the wire handshake. Treat them as a tested reference implementation, not dead
// code: do not delete them when pruning unused exports.
pub use clock::LamportClock;
pub use engine::{PeerState, SyncEngine, SyncError, SyncResult};
pub use inbox::{ReplayGuard, SyncInboxForwarder, SyncInboxSender, REPLAY_GUARD_CAPACITY};
pub use merge::{local_to_wire, local_to_wire_owned, resolve, wire_to_local, MergeOutcome};
pub use metrics::SyncLagCounter;
pub use protocol::{ControlMsg, Message, PeerFrame, WireItem};
