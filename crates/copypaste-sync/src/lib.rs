/// copypaste-sync — P2P clipboard sync engine.
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
pub mod backoff;
pub mod clock;
pub mod engine;
pub mod merge;
pub mod protocol;

// Convenience re-exports.
pub use backoff::{
    BackoffScheduler, DEFAULT_BASE_DELAY, DEFAULT_MAX_DELAY, DEFAULT_SUCCESS_HOLD_THRESHOLD,
};
pub use clock::LamportClock;
pub use engine::{PeerState, SyncEngine, SyncError, SyncResult};
pub use merge::{local_to_wire, resolve, wire_to_local, MergeOutcome};
pub use protocol::{ControlMsg, Message, PeerFrame, WireItem};
