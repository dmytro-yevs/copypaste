/// P2P sync engine.
///
/// `SyncEngine` orchestrates the item exchange loop between two peers over a
/// bidirectional byte stream (typically a TLS TCP socket).  It is intentionally
/// transport-agnostic: callers pass in an `AsyncRead + AsyncWrite` and the
/// engine drives the protocol to completion.
///
/// # Protocol overview
///
/// Both peers play symmetric roles after the initial HELLO handshake:
///
/// ```text
/// A ──HELLO──▶ B          (A sends first, B replies)
/// A ◀──HELLO── B
/// A ──HAVE───▶ B          (announce which item IDs each side has)
/// A ◀──HAVE─── B
/// A ──WANT───▶ B          (request what we don't have)
/// A ◀──WANT─── B
/// A ──ITEMS──▶ B          (send what the peer requested)
/// A ◀──ITEMS── B
/// A ──DONE───▶ B
/// A ◀──DONE─── B
/// ```
///
/// After both DONE messages are exchanged the connection can be dropped.
use std::collections::{HashMap, HashSet};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, info, warn};

use crate::clock::LamportClock;
use crate::merge::{local_to_wire, resolve, wire_to_local, MergeOutcome};
use crate::protocol::{Message, WireItem};
use copypaste_core::storage::items::ClipboardItem;

/// Maximum number of bytes allowed in a single protocol frame (16 MiB).
/// Protects against memory exhaustion from malicious/buggy peers.
const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Error type for sync operations.
#[derive(Debug)]
pub enum SyncError {
    /// I/O error on the underlying stream.
    Io(std::io::Error),
    /// JSON (de)serialisation failure.
    Json(serde_json::Error),
    /// Peer sent a frame larger than `MAX_FRAME_SIZE`.
    FrameTooLarge(u32),
    /// Peer sent a message out of sequence.
    ProtocolViolation(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Io(e) => write!(f, "IO error: {e}"),
            SyncError::Json(e) => write!(f, "JSON error: {e}"),
            SyncError::FrameTooLarge(n) => write!(f, "frame too large: {n} bytes"),
            SyncError::ProtocolViolation(s) => write!(f, "protocol violation: {s}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(e)
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Json(e)
    }
}

/// Outcome of a completed sync session.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SyncResult {
    /// Items accepted from the remote peer (after LWW merge).
    pub items_received: usize,
    /// Items sent to the remote peer.
    pub items_sent: usize,
    /// Items that were already present locally and not replaced (LWW kept local).
    pub items_skipped: usize,
}

/// State tracked for a known peer across sessions.
#[derive(Debug, Clone, Default)]
pub struct PeerState {
    /// Last known Lamport clock value reported by this peer.
    pub last_clock: u64,
}

/// The sync engine for a single device.
///
/// Holds the device identity, its Lamport clock, and known peer clock values.
/// Multiple sync sessions can be driven sequentially via `run_session`.
pub struct SyncEngine {
    /// This device's UUID (used as `origin_device_id` when sending items).
    pub device_id: String,
    /// Logical clock maintained across sessions.
    pub clock: LamportClock,
    /// Per-peer clock bookkeeping (persisted across sessions externally).
    pub peer_clocks: HashMap<String, PeerState>,
}

impl SyncEngine {
    /// Create a new engine for the given device.
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            clock: LamportClock::new(),
            peer_clocks: HashMap::new(),
        }
    }

    /// Restore an engine from persisted state.
    pub fn with_state(
        device_id: impl Into<String>,
        clock_value: u64,
        peer_clocks: HashMap<String, PeerState>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            clock: LamportClock::from_value(clock_value),
            peer_clocks,
        }
    }

    /// Record a local write event, advancing the Lamport clock.
    ///
    /// Returns the new clock value to be stamped on the written item.
    pub fn on_local_write(&mut self) -> i64 {
        self.clock.tick() as i64
    }

    /// Run one complete sync session over the given async stream.
    ///
    /// `local_items` — all items currently stored on this device.
    /// Returns `(SyncResult, Vec<ClipboardItem>)` where the vec contains items
    /// that should be upserted into local storage (new or LWW-replaced items
    /// from the peer).
    ///
    /// The caller is responsible for persisting the returned items and for
    /// saving `self.clock` and `self.peer_clocks` after the call returns.
    pub async fn run_session<S>(
        &mut self,
        stream: &mut S,
        local_items: &[ClipboardItem],
    ) -> Result<(SyncResult, Vec<ClipboardItem>), SyncError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut result = SyncResult::default();

        // --- HELLO handshake ---
        let my_hello = Message::Hello {
            device_id: self.device_id.clone(),
            clock: self.clock.get(),
            item_count: local_items.len() as u64,
        };
        send_message(stream, &my_hello).await?;
        debug!("sent HELLO clock={}", self.clock.get());

        let peer_hello = recv_message(stream).await?;
        let (peer_device_id, peer_clock) = match peer_hello {
            Message::Hello { device_id, clock, item_count } => {
                info!(
                    "peer HELLO device={} clock={} items={}",
                    device_id, clock, item_count
                );
                (device_id, clock)
            }
            other => {
                return Err(SyncError::ProtocolViolation(format!(
                    "expected HELLO, got {:?}",
                    other
                )));
            }
        };

        // Advance our clock on receiving peer's HELLO.
        self.clock.observe(peer_clock);

        // --- HAVE exchange ---
        // Build a map of id → lamport_ts for local items (for conflict detection).
        let local_clock_map: HashMap<String, i64> =
            local_items.iter().map(|i| (i.id.clone(), i.lamport_ts)).collect();
        let local_ids: HashSet<&String> = local_clock_map.keys().collect();

        let my_have = Message::Have {
            items: local_items
                .iter()
                .map(|i| (i.id.clone(), i.lamport_ts))
                .collect(),
        };
        send_message(stream, &my_have).await?;

        let peer_have = recv_message(stream).await?;
        // peer_clock_map: id → lamport_ts from the remote side.
        let peer_clock_map: HashMap<String, i64> = match peer_have {
            Message::Have { items } => items.into_iter().collect(),
            other => {
                return Err(SyncError::ProtocolViolation(format!(
                    "expected HAVE, got {:?}",
                    other
                )));
            }
        };
        let peer_ids: HashSet<&String> = peer_clock_map.keys().collect();

        // Items peer has that we don't have at all.
        let only_on_peer: Vec<String> = peer_ids
            .difference(&local_ids)
            .map(|s| (*s).clone())
            .collect();

        // Items on both sides where peer's Lamport clock is strictly higher
        // (peer has a more recent version — request it for LWW comparison).
        let peer_newer: Vec<String> = peer_ids
            .intersection(&local_ids)
            .filter(|id| {
                let peer_ts = peer_clock_map[id.as_str()];
                let local_ts = local_clock_map[id.as_str()];
                peer_ts > local_ts
            })
            .map(|s| (*s).clone())
            .collect();

        // We WANT: items we don't have + items where peer's version is newer.
        let mut we_want: Vec<String> = only_on_peer;
        we_want.extend(peer_newer);

        // Items we have that peer doesn't.
        let only_on_us: Vec<String> = local_ids
            .difference(&peer_ids)
            .map(|s| (*s).clone())
            .collect();

        // Items where our Lamport clock is strictly higher than peer's.
        let us_newer: Vec<String> = local_ids
            .intersection(&peer_ids)
            .filter(|id| {
                let our_ts = local_clock_map[id.as_str()];
                let peer_ts = peer_clock_map[id.as_str()];
                our_ts > peer_ts
            })
            .map(|s| (*s).clone())
            .collect();

        // Peer WANTS: items only on us + items where our version is newer.
        let mut peer_wants_hint: Vec<String> = only_on_us;
        peer_wants_hint.extend(us_newer);

        debug!(
            "we want {} items from peer, peer likely wants {} items from us",
            we_want.len(),
            peer_wants_hint.len()
        );

        // --- WANT exchange ---
        send_message(stream, &Message::Want { item_ids: we_want.clone() }).await?;

        let peer_want_msg = recv_message(stream).await?;
        let items_peer_wants: Vec<String> = match peer_want_msg {
            Message::Want { item_ids } => item_ids,
            other => {
                return Err(SyncError::ProtocolViolation(format!(
                    "expected WANT, got {:?}",
                    other
                )));
            }
        };

        // --- ITEMS exchange: we send what peer wants first ---
        let items_to_send: Vec<WireItem> = local_items
            .iter()
            .filter(|item| items_peer_wants.contains(&item.id))
            .map(|item| local_to_wire(item, &self.device_id))
            .collect();

        result.items_sent = items_to_send.len();
        send_message(stream, &Message::Items { items: items_to_send }).await?;
        debug!("sent {} items to peer", result.items_sent);

        // --- Receive items from peer ---
        let peer_items_msg = recv_message(stream).await?;
        let received_items: Vec<WireItem> = match peer_items_msg {
            Message::Items { items } => items,
            other => {
                return Err(SyncError::ProtocolViolation(format!(
                    "expected ITEMS, got {:?}",
                    other
                )));
            }
        };

        // Build a local index for fast lookup during merge.
        let local_by_id: HashMap<&str, &ClipboardItem> =
            local_items.iter().map(|i| (i.id.as_str(), i)).collect();

        let mut to_upsert: Vec<ClipboardItem> = Vec::new();

        for wire in received_items {
            // Advance clock with the item's timestamp.
            self.clock.observe(wire.lamport_ts as u64);

            if let Some(existing) = local_by_id.get(wire.id.as_str()) {
                // Item exists locally — apply LWW merge.
                match resolve(existing, &wire) {
                    MergeOutcome::TakeRemote => {
                        debug!("LWW: take remote for item {}", wire.id);
                        to_upsert.push(wire_to_local(wire));
                        result.items_received += 1;
                    }
                    MergeOutcome::KeepLocal => {
                        debug!("LWW: keep local for item {}", wire.id);
                        result.items_skipped += 1;
                    }
                }
            } else {
                // New item — accept unconditionally.
                debug!("accepting new item {} from peer", wire.id);
                to_upsert.push(wire_to_local(wire));
                result.items_received += 1;
            }
        }

        // --- DONE handshake ---
        send_message(stream, &Message::Done).await?;

        let peer_done = recv_message(stream).await?;
        if peer_done != Message::Done {
            warn!("expected DONE from peer, got {:?}", peer_done);
            return Err(SyncError::ProtocolViolation("expected DONE".to_string()));
        }

        // Record peer's last known clock.
        self.peer_clocks.insert(
            peer_device_id,
            PeerState { last_clock: peer_clock },
        );

        info!(
            "sync complete: received={} sent={} skipped={}",
            result.items_received, result.items_sent, result.items_skipped
        );

        Ok((result, to_upsert))
    }
}

// ---------------------------------------------------------------------------
// Internal framing helpers
// ---------------------------------------------------------------------------

/// Send a protocol message as a length-prefixed JSON frame.
async fn send_message<S: AsyncWrite + Unpin>(
    stream: &mut S,
    msg: &Message,
) -> Result<(), SyncError> {
    let frame = msg.encode()?;
    stream.write_all(&frame).await?;
    Ok(())
}

/// Read the next length-prefixed JSON frame and deserialise it.
async fn recv_message<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Message, SyncError> {
    // Read 4-byte length prefix.
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);

    if len > MAX_FRAME_SIZE {
        return Err(SyncError::FrameTooLarge(len));
    }

    // Read payload.
    let mut payload = vec![0u8; len as usize];
    stream.read_exact(&mut payload).await?;

    let msg = Message::decode(&payload)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Helpers — in-memory duplex stream simulation
    // ---------------------------------------------------------------------------

    /// Create a pair of in-memory duplex streams backed by tokio channels,
    /// simulating a bidirectional TCP connection without network I/O.
    fn make_duplex() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        // 1 MiB buffer is plenty for test payloads.
        tokio::io::duplex(1024 * 1024)
    }

    fn make_item(id: &str, lamport: i64) -> ClipboardItem {
        ClipboardItem {
            id: id.to_string(),
            item_id: format!("{id}-item"),
            content_type: "text".to_string(),
            content: Some(vec![0xAA, 0xBB]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts: lamport,
            wall_time: 1_700_000_000_000 + lamport,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
        }
    }

    // ---------------------------------------------------------------------------
    // Unit tests for framing helpers
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn send_recv_message_round_trips() {
        let (mut a, mut b) = make_duplex();
        let msg = Message::Hello {
            device_id: "test-dev".to_string(),
            clock: 5,
            item_count: 3,
        };
        send_message(&mut a, &msg).await.unwrap();
        let received = recv_message(&mut b).await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn frame_too_large_is_rejected() {
        let (mut a, mut b) = make_duplex();
        // Write a fake frame with length > MAX_FRAME_SIZE.
        let huge_len: u32 = MAX_FRAME_SIZE + 1;
        a.write_all(&huge_len.to_le_bytes()).await.unwrap();
        drop(a);
        let err = recv_message(&mut b).await.unwrap_err();
        assert!(matches!(err, SyncError::FrameTooLarge(_)));
    }

    // ---------------------------------------------------------------------------
    // Integration test: two engines exchange items
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn two_engines_exchange_disjoint_items() {
        let item_a = make_item("item-A", 1);
        let item_b = make_item("item-B", 2);

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        let (mut stream_a, mut stream_b) = make_duplex();

        // Run both sessions concurrently.
        let items_a = [item_a.clone()];
        let items_b = [item_b.clone()];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut stream_a, &items_a),
            engine_b.run_session(&mut stream_b, &items_b),
        );

        let (result_a, upsert_a) = res_a.expect("engine A must succeed");
        let (result_b, upsert_b) = res_b.expect("engine B must succeed");

        // Each engine should have received the other's item.
        assert_eq!(result_a.items_received, 1, "A should receive item-B");
        assert_eq!(result_b.items_received, 1, "B should receive item-A");
        assert_eq!(result_a.items_sent, 1, "A should send item-A");
        assert_eq!(result_b.items_sent, 1, "B should send item-B");

        assert_eq!(upsert_a.len(), 1);
        assert_eq!(upsert_a[0].id, "item-B");
        assert!(upsert_a[0].is_synced);

        assert_eq!(upsert_b.len(), 1);
        assert_eq!(upsert_b[0].id, "item-A");
        assert!(upsert_b[0].is_synced);
    }

    #[tokio::test]
    async fn already_synced_items_not_re_requested() {
        // Both engines have the same item — nothing should be exchanged.
        let shared = make_item("item-shared", 5);

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        let (mut stream_a, mut stream_b) = make_duplex();

        let items_a = [shared.clone()];
        let items_b = [shared.clone()];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut stream_a, &items_a),
            engine_b.run_session(&mut stream_b, &items_b),
        );

        let (result_a, upsert_a) = res_a.unwrap();
        let (result_b, upsert_b) = res_b.unwrap();

        assert_eq!(result_a.items_received, 0);
        assert_eq!(result_b.items_received, 0);
        assert_eq!(result_a.items_sent, 0);
        assert_eq!(result_b.items_sent, 0);
        assert!(upsert_a.is_empty());
        assert!(upsert_b.is_empty());
    }

    #[tokio::test]
    async fn lww_conflict_higher_lamport_wins() {
        // Same item ID, different lamport clocks — engine B's version wins.
        let item_a = make_item("item-conflict", 3); // local lamport = 3
        let mut item_b = make_item("item-conflict", 7); // remote lamport = 7 → wins
        item_b.content = Some(vec![0xFF]); // different content so we can verify

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        let (mut stream_a, mut stream_b) = make_duplex();

        let items_a = [item_a];
        let items_b = [item_b.clone()];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut stream_a, &items_a),
            engine_b.run_session(&mut stream_b, &items_b),
        );

        let (_result_a, upsert_a) = res_a.unwrap();
        let (_result_b, _upsert_b) = res_b.unwrap();

        // Engine A should have accepted item-conflict from B (lamport 7 > 3).
        assert_eq!(upsert_a.len(), 1);
        assert_eq!(upsert_a[0].lamport_ts, 7);
        assert_eq!(upsert_a[0].content, Some(vec![0xFF]));
    }

    #[tokio::test]
    async fn lamport_clock_advances_after_session() {
        let item_a = make_item("item-A", 10);

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        // Set engine B's clock to something high.
        engine_b.clock = LamportClock::from_value(50);

        let (mut stream_a, mut stream_b) = make_duplex();

        let items_a = [item_a];
        let items_b: [ClipboardItem; 0] = [];
        let _ = tokio::join!(
            engine_a.run_session(&mut stream_a, &items_a),
            engine_b.run_session(&mut stream_b, &items_b),
        );

        // Engine A's clock should have advanced past B's initial value (50).
        // After observe(50): max(0, 50) + 1 = 51 (minimum).
        assert!(engine_a.clock.get() >= 51, "clock should advance past peer's value");
    }

    #[tokio::test]
    async fn peer_clock_recorded_after_session() {
        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        let (mut stream_a, mut stream_b) = make_duplex();

        let items: [ClipboardItem; 0] = [];
        let _ = tokio::join!(
            engine_a.run_session(&mut stream_a, &items),
            engine_b.run_session(&mut stream_b, &items),
        );

        // engine_a should know about device-B.
        assert!(engine_a.peer_clocks.contains_key("device-B"));
        assert!(engine_b.peer_clocks.contains_key("device-A"));
    }

    #[tokio::test]
    async fn on_local_write_ticks_clock() {
        let mut engine = SyncEngine::new("device-A");
        let t1 = engine.on_local_write();
        let t2 = engine.on_local_write();
        assert_eq!(t1, 1);
        assert_eq!(t2, 2);
    }

    #[tokio::test]
    async fn empty_session_completes_without_error() {
        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");

        let (mut stream_a, mut stream_b) = make_duplex();

        let items: [ClipboardItem; 0] = [];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut stream_a, &items),
            engine_b.run_session(&mut stream_b, &items),
        );

        assert!(res_a.is_ok());
        assert!(res_b.is_ok());
    }
}
