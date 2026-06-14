/// P2P sync engine.
///
/// **NOT on the daemon production path** (CopyPaste-j6r/ayvs): the live daemon
/// does not instantiate `SyncEngine`. P2P sync in the daemon runs through
/// `copypaste-daemon::sync_orch` (which calls [`crate::merge::resolve`]
/// directly), and cloud/relay reuse the same [`crate::merge::remote_wins`]
/// total order. This engine + its HELLO/HAVE/WANT/ITEMS/DONE protocol are kept
/// for completeness and tests; see the crate-root docs before wiring them in.
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

/// Maximum number of Lamport ticks a remote item is allowed to be ahead of the
/// local clock before its timestamp is clamped.
///
/// Why this bound? A real deployment might have thousands of devices, each
/// performing thousands of writes per day over years of uptime. 10^12 ticks is
/// larger than (10^6 devices × 10^6 writes each), so it accommodates any
/// realistic scenario while still preventing a single hostile/buggy peer from
/// jamming the local clock to u64::MAX (which would make that peer win every
/// future LWW conflict forever). The wall_time is a Unix-ms timestamp; 10^12 ms
/// is roughly 31.7 years in the future (1 year ≈ 3.156 × 10^10 ms), a similarly
/// generous but finite bound.
pub const MAX_LAMPORT_SKEW: u64 = 1_000_000_000_000; // 10^12 ticks
pub const MAX_WALL_TIME_SKEW_MS: i64 = 1_000_000_000_000_i64; // 10^12 ms ≈ 31.7 years

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
            Message::Hello {
                device_id,
                clock,
                item_count,
            } => {
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
        // Build a map of item_id → lamport_ts for local items (for conflict
        // detection). We key on the cross-device `item_id`, NOT the per-row
        // primary key `id`: `id` is a fresh `Uuid::new_v4()` on every device,
        // so the same logical item has a different `id` on each side and HAVE/
        // WANT/LWW would never match — duplicate rows would accumulate. The
        // stable `item_id` (bound into the AEAD AAD, UNIQUE-indexed) is the
        // identity every device agrees on.
        let local_clock_map: HashMap<String, i64> = local_items
            .iter()
            .map(|i| (i.item_id.clone(), i.lamport_ts))
            .collect();
        let local_ids: HashSet<&String> = local_clock_map.keys().collect();

        let my_have = Message::Have {
            items: local_items
                .iter()
                .map(|i| (i.item_id.clone(), i.lamport_ts))
                .collect(),
        };
        send_message(stream, &my_have).await?;

        let peer_have = recv_message(stream).await?;
        // peer_clock_map: item_id → lamport_ts from the remote side.
        // Build peer_clock_map from the HAVE list, taking the MAX lamport_ts when
        // the same item_id appears more than once.  `HashMap::collect` silently
        // collapses duplicates with "last wins" (undefined iteration order), which
        // could cause the engine to underestimate the peer's clock for an item and
        // skip requesting it even when the peer's true latest version is newer than
        // the local copy.  Taking MAX is the only semantically correct choice: the
        // peer holds the item at the highest timestamp it announced, regardless of
        // duplicates.
        let peer_clock_map: HashMap<String, i64> = match peer_have {
            Message::Have { items } => {
                let mut map: HashMap<String, i64> = HashMap::with_capacity(items.len());
                for (id, ts) in items {
                    let entry = map.entry(id).or_insert(ts);
                    if ts > *entry {
                        *entry = ts;
                    }
                }
                map
            }
            other => {
                return Err(SyncError::ProtocolViolation(format!(
                    "expected HAVE, got {:?}",
                    other
                )));
            }
        };
        let peer_ids: HashSet<&String> = peer_clock_map.keys().collect();

        // CopyPaste-ux2i: build `we_want` / `peer_wants_hint` in a single
        // collect each instead of materialising four intermediate `Vec<String>`
        // (only_on_peer, peer_newer, only_on_us, us_newer) that were each moved
        // or `extend`-ed exactly once. The difference/intersection iterators are
        // chained and collected directly.

        // We WANT: items peer has that we don't (difference) + items on both
        // sides where peer's Lamport clock is strictly higher (peer's version is
        // newer — request it for LWW comparison).
        let we_want: Vec<String> = peer_ids
            .difference(&local_ids)
            .copied()
            .chain(peer_ids.intersection(&local_ids).copied().filter(|id| {
                let peer_ts = peer_clock_map[id.as_str()];
                let local_ts = local_clock_map[id.as_str()];
                peer_ts > local_ts
            }))
            .cloned()
            .collect();

        // Peer WANTS: items only on us (difference) + items where our Lamport
        // clock is strictly higher than peer's.
        let peer_wants_hint: Vec<String> = local_ids
            .difference(&peer_ids)
            .copied()
            .chain(local_ids.intersection(&peer_ids).copied().filter(|id| {
                let our_ts = local_clock_map[id.as_str()];
                let peer_ts = peer_clock_map[id.as_str()];
                our_ts > peer_ts
            }))
            .cloned()
            .collect();

        debug!(
            "we want {} items from peer, peer likely wants {} items from us",
            we_want.len(),
            peer_wants_hint.len()
        );

        // --- WANT exchange ---
        // CopyPaste-ux2i: move `we_want` into the message (it is dead after this
        // send) instead of cloning the whole Vec<String>.
        send_message(stream, &Message::Want { item_ids: we_want }).await?;

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
        // `items_peer_wants` is a list of `item_id`s (the WANT/HAVE wire lists
        // carry item_ids), so filter on `item.item_id`, not the per-row `id`.
        // Build a HashSet for O(1) per-item lookup instead of O(n·m) linear scan.
        let peer_wants_set: HashSet<&str> = items_peer_wants.iter().map(String::as_str).collect();
        let items_to_send: Vec<WireItem> = local_items
            .iter()
            .filter(|item| peer_wants_set.contains(item.item_id.as_str()))
            .map(|item| local_to_wire(item, &self.device_id))
            .collect();

        result.items_sent = items_to_send.len();
        send_message(
            stream,
            &Message::Items {
                items: items_to_send,
            },
        )
        .await?;
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

        // Build a local index for fast lookup during merge, keyed on the
        // cross-device `item_id` so an incoming wire item resolves against the
        // local row that represents the SAME logical item (its per-row `id`
        // differs across devices).
        //
        // Use max-lamport dedup instead of plain `.collect()`: if the same
        // item_id appears more than once in local storage (e.g. a migration
        // anomaly or a race), `.collect()` would silently keep whichever row
        // happens to iterate last (undefined order), which could cause LWW to
        // compare the wire item against a *stale* local row and wrongly replace
        // a newer local version. The highest-lamport row is the authoritative
        // local copy, mirroring the same pattern used for the peer HAVE list.
        let mut local_by_item_id: HashMap<&str, &ClipboardItem> =
            HashMap::with_capacity(local_items.len());
        for item in local_items.iter() {
            let entry = local_by_item_id
                .entry(item.item_id.as_str())
                .or_insert(item);
            if item.lamport_ts > entry.lamport_ts {
                *entry = item;
            }
        }

        let mut to_upsert: Vec<ClipboardItem> = Vec::new();

        for mut wire in received_items {
            // Step 1 — lower-bound clamp (centralised in WireItem::clamp_timestamps):
            // zero out any negative lamport_ts / wall_time before they touch the
            // clock, the LWW merge, or storage.  i64 on the wire but u64 in the
            // clock; a negative cast would silently wrap to a huge positive (L1).
            wire.clamp_timestamps();

            // Step 2 — upper-bound clamp: reject implausibly large lamport_ts /
            // wall_time from a hostile or buggy peer.
            //
            // Without this bound a single peer can feed the local clock
            // `observe(u64::MAX)` and saturate it, making that peer win every
            // future LWW conflict forever — a silent, persistent data-loss attack.
            //
            // The ceiling is local_clock + MAX_LAMPORT_SKEW, i.e. the peer may
            // be at most MAX_LAMPORT_SKEW ticks ahead of us (10^12, far beyond
            // any real deployment).  Anything beyond that is clamped; the item
            // is still accepted so we don't silently drop peer data, but with a
            // safe timestamp that cannot jam the clock.
            let local_now = self.clock.get();
            let ceiling_lamport = local_now.saturating_add(MAX_LAMPORT_SKEW);
            let wire_u64 = wire.lamport_ts as u64; // safe: already >= 0 after step 1
            if wire_u64 > ceiling_lamport {
                warn!(
                    "received wire item {} with lamport_ts {} far ahead of local clock {} \
                     (ceiling {}); clamping to ceiling",
                    wire.id, wire.lamport_ts, local_now, ceiling_lamport
                );
                // Clamp to the ceiling as i64; saturating_as avoids overflow if
                // ceiling somehow exceeds i64::MAX (theoretical with a saturated clock).
                wire.lamport_ts = ceiling_lamport.min(i64::MAX as u64) as i64;
            }
            // Apply the same ceiling to wall_time (Unix ms).  A peer sending
            // wall_time=i64::MAX would make its item win every future wall-time
            // LWW tie-break, permanently shadowing all locally-captured items.
            // The ceiling is relative to now so that legitimate 2026+ timestamps
            // are not clamped (MAX_WALL_TIME_SKEW_MS is a ±delta, not an epoch).
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let wall_time_ceiling = now_ms.saturating_add(MAX_WALL_TIME_SKEW_MS);
            if wire.wall_time > wall_time_ceiling {
                warn!(
                    "received wire item {} with wall_time {} beyond ceiling {}; clamping",
                    wire.id, wire.wall_time, wall_time_ceiling
                );
                wire.wall_time = wall_time_ceiling;
            }

            // Advance our clock with the (now bounded, non-negative) item timestamp.
            // observe() uses saturating_add internally.
            self.clock.observe(wire.lamport_ts as u64);

            if let Some(existing) = local_by_item_id.get(wire.item_id.as_str()) {
                // Item exists locally (same cross-device item_id) — apply LWW.
                match resolve(existing, &wire) {
                    MergeOutcome::TakeRemote => {
                        debug!("LWW: take remote for item_id {}", wire.item_id);
                        to_upsert.push(wire_to_local(wire));
                        result.items_received += 1;
                    }
                    MergeOutcome::KeepLocal => {
                        debug!("LWW: keep local for item_id {}", wire.item_id);
                        result.items_skipped += 1;
                    }
                }
            } else {
                // New item (item_id not seen locally) — accept unconditionally.
                debug!("accepting new item_id {} from peer", wire.item_id);
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

        // Record the peer's last known clock.
        //
        // Use `self.clock.get()` (our clock AFTER the full item exchange and
        // observe() calls) rather than the stale `peer_clock` value from the
        // HELLO handshake. By the time DONE is exchanged our clock has been
        // advanced by every item we observed from the peer, so it accurately
        // reflects the highest timestamp either side has seen. Storing the raw
        // HELLO value would understate the peer's effective clock and cause the
        // next session to re-request items the peer already delivered.
        self.peer_clocks.insert(
            peer_device_id,
            PeerState {
                last_clock: self.clock.get(),
            },
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
            deleted: false,
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
            origin_device_id: format!("dev-{id}"),
            key_version: 1,
            pinned: false,
            pin_order: None,
            thumb: None,
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
        assert!(
            engine_a.clock.get() >= 51,
            "clock should advance past peer's value"
        );
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
    async fn identical_everything_merge_is_idempotent() {
        // edge-cases MEDIUM #16: merging the exact same item twice must not
        // mutate local state on the second pass. Verifies LWW determinism
        // for fully-identical (lamport, wall, device, payload) items.
        let shared = make_item("item-shared", 5);

        // Round 1: A and B exchange and both end up holding `shared`.
        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");
        let (mut sa, mut sb) = make_duplex();
        let items_a = [shared.clone()];
        let items_b = [shared.clone()];
        let (r1_a, r1_b) = tokio::join!(
            engine_a.run_session(&mut sa, &items_a),
            engine_b.run_session(&mut sb, &items_b),
        );
        let (_, upsert_a_1) = r1_a.unwrap();
        let (_, upsert_b_1) = r1_b.unwrap();
        // First pass: both have it already, nothing to upsert.
        assert!(upsert_a_1.is_empty(), "round 1: A should not upsert");
        assert!(upsert_b_1.is_empty(), "round 1: B should not upsert");

        // Round 2: identical inputs again → still no-op (idempotent).
        let (mut sa2, mut sb2) = make_duplex();
        let (r2_a, r2_b) = tokio::join!(
            engine_a.run_session(&mut sa2, &items_a),
            engine_b.run_session(&mut sb2, &items_b),
        );
        let (res_a_2, upsert_a_2) = r2_a.unwrap();
        let (res_b_2, upsert_b_2) = r2_b.unwrap();

        assert!(upsert_a_2.is_empty(), "round 2 must be idempotent on A");
        assert!(upsert_b_2.is_empty(), "round 2 must be idempotent on B");
        assert_eq!(res_a_2.items_received, 0);
        assert_eq!(res_b_2.items_received, 0);
        assert_eq!(res_a_2.items_sent, 0);
        assert_eq!(res_b_2.items_sent, 0);
    }

    #[tokio::test]
    async fn negative_lamport_does_not_panic() {
        // edge-cases LOW #34: a malicious/buggy peer sends a wire item with
        // negative lamport_ts; engine must clamp and not panic from the cast.
        let item = ClipboardItem {
            deleted: false,
            id: "neg".to_string(),
            item_id: "neg-item".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![0x01]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts: -1, // negative
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-A".to_string(),
            key_version: 1,
            pinned: false,
            pin_order: None,
            thumb: None,
        };

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");
        let (mut sa, mut sb) = make_duplex();
        let items_a: [ClipboardItem; 0] = [];
        let items_b = [item];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut sa, &items_a),
            engine_b.run_session(&mut sb, &items_b),
        );
        // Must not panic — both sides return Ok.
        let (_result_a, upsert_a) = res_a.expect("engine A must succeed");
        assert!(res_b.is_ok());

        // L1: A had no items, so it accepts "neg" from B. The stored item's
        // lamport_ts MUST be clamped to 0 at ingestion — a negative value must
        // never be persisted to the row.
        assert_eq!(upsert_a.len(), 1, "A should accept the new item from B");
        assert_eq!(upsert_a[0].id, "neg");
        assert_eq!(
            upsert_a[0].lamport_ts, 0,
            "negative inbound lamport_ts must be clamped to 0 before storage (L1)"
        );
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

    /// CRDT stable-identity regression: the SAME logical item captured on two
    /// devices has DIFFERENT per-row `id`s (each device runs `Uuid::new_v4()`)
    /// but the SAME cross-device `item_id`. HAVE/WANT/LWW must key on `item_id`
    /// so the two converge to ONE item — the higher-lamport version winning —
    /// instead of each device treating the other's copy as a brand-new item and
    /// accumulating a duplicate.
    #[tokio::test]
    async fn same_item_id_different_row_id_converges_to_one_row() {
        // Device A: row id A1, item_id X, lamport 5.
        let mut a = make_item("A1", 5);
        a.item_id = "X".to_string();
        a.content = Some(vec![0xAA]);
        // Device B: row id B9 (different!), item_id X (same logical item),
        // lamport 7, different content → B's version must win LWW.
        let mut b = make_item("B9", 7);
        b.item_id = "X".to_string();
        b.content = Some(vec![0xBB]);

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");
        let (mut sa, mut sb) = make_duplex();
        let items_a = [a];
        let items_b = [b];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut sa, &items_a),
            engine_b.run_session(&mut sb, &items_b),
        );
        let (result_a, upsert_a) = res_a.expect("engine A ok");
        let (result_b, upsert_b) = res_b.expect("engine B ok");

        // A must accept B's newer version of item_id X (LWW: lamport 7 > 5).
        assert_eq!(upsert_a.len(), 1, "A converges to the single shared item");
        assert_eq!(upsert_a[0].item_id, "X");
        assert_eq!(upsert_a[0].lamport_ts, 7, "higher-lamport version wins");
        assert_eq!(upsert_a[0].content, Some(vec![0xBB]), "B's content wins");
        assert_eq!(result_a.items_received, 1);

        // B already holds the winning (higher-lamport) version of item_id X. The
        // HAVE/WANT exchange — now keyed on item_id — recognises A's copy as the
        // SAME item, so B never even requests A's older version: nothing is
        // transferred to B and nothing is upserted. This is the convergence
        // guarantee: B keeps its winner, A adopts it, ONE row results. (Pre-fix,
        // A's differently-`id`'d copy looked like a brand-new item to B and would
        // have been ingested as a duplicate.)
        assert!(
            upsert_b.is_empty(),
            "B must not ingest A's older same-item_id copy as a new row"
        );
        assert_eq!(
            result_b.items_received, 0,
            "B receives nothing — A's copy is the same logical item, not new"
        );
        assert_eq!(
            result_b.items_sent, 1,
            "B sends its winning version of item_id X to A"
        );
    }

    /// The inverse guarantee: two items with the SAME content but DISTINCT
    /// `item_id`s (X1 / X2) are INTENTIONALLY-different captures and must BOTH
    /// survive a sync. Identity is `item_id`, never the content hash — keying on
    /// content would wrongly collapse deliberate duplicate captures.
    #[tokio::test]
    async fn intentional_duplicate_captures_stay_distinct() {
        // A holds item_id X1; B holds item_id X2 — identical content, distinct
        // logical items.
        let mut a = make_item("rowA", 3);
        a.item_id = "X1".to_string();
        a.content = Some(vec![0xEE]);
        let mut b = make_item("rowB", 3);
        b.item_id = "X2".to_string();
        b.content = Some(vec![0xEE]); // SAME content as A

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");
        let (mut sa, mut sb) = make_duplex();
        let items_a = [a];
        let items_b = [b];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut sa, &items_a),
            engine_b.run_session(&mut sb, &items_b),
        );
        let (_ra, upsert_a) = res_a.expect("engine A ok");
        let (_rb, upsert_b) = res_b.expect("engine B ok");

        // Each side learns the OTHER's distinct item_id — both survive.
        assert_eq!(upsert_a.len(), 1, "A must receive X2");
        assert_eq!(upsert_a[0].item_id, "X2");
        assert_eq!(upsert_b.len(), 1, "B must receive X1");
        assert_eq!(upsert_b[0].item_id, "X1");
    }

    /// CopyPaste-5on regression: HAVE/WANT exchange MUST be keyed on the stable
    /// `item_id`, never on the per-row `id`.  Each device mints its own
    /// `Uuid::new_v4()` row id at capture time, so two devices that hold the same
    /// logical item will have different row ids.  If the engine keyed HAVE/WANT on
    /// `id` instead of `item_id` it would treat each copy as a distinct item and
    /// accumulate duplicates on every sync.  This test verifies convergence by
    /// using deliberately mismatched row ids and asserting that exactly ONE item
    /// survives on each side after the session.
    #[tokio::test]
    async fn crdt_have_want_keyed_on_item_id_not_row_id() {
        // Device A captured item_id "shared-clip" and assigned row id "row-AAA".
        let mut item_a = make_item("row-AAA", 10);
        item_a.item_id = "shared-clip".to_string();
        item_a.content = Some(b"hello from A".to_vec());

        // Device B captured the SAME logical item (same item_id) but assigned a
        // completely different row id "row-BBB" and has a higher lamport clock.
        let mut item_b = make_item("row-BBB", 15);
        item_b.item_id = "shared-clip".to_string();
        item_b.content = Some(b"hello from B (newer)".to_vec());

        let mut engine_a = SyncEngine::new("device-A");
        let mut engine_b = SyncEngine::new("device-B");
        let (mut sa, mut sb) = make_duplex();
        let items_a = [item_a];
        let items_b = [item_b];
        let (res_a, res_b) = tokio::join!(
            engine_a.run_session(&mut sa, &items_a),
            engine_b.run_session(&mut sb, &items_b),
        );
        let (result_a, upsert_a) = res_a.expect("engine A ok");
        let (result_b, upsert_b) = res_b.expect("engine B ok");

        // A must receive B's newer version (lamport 15 > 10) — one upsert.
        assert_eq!(
            upsert_a.len(),
            1,
            "A must converge: exactly 1 upsert for shared-clip"
        );
        assert_eq!(upsert_a[0].item_id, "shared-clip");
        assert_eq!(upsert_a[0].lamport_ts, 15, "higher-lamport (B) wins LWW");
        assert_eq!(
            upsert_a[0].content,
            Some(b"hello from B (newer)".to_vec()),
            "B's content wins"
        );
        assert_eq!(result_a.items_received, 1);

        // B already holds the winning version — it must not ingest A's older copy
        // as a new row (which is what happens when HAVE/WANT keys on row `id`).
        assert!(
            upsert_b.is_empty(),
            "B must NOT treat A's row-AAA as a brand-new item (HAVE/WANT must key on item_id)"
        );
        assert_eq!(
            result_b.items_received, 0,
            "B receives nothing — it already has the winner"
        );
    }

    /// CopyPaste-5on regression: logical-clock unification.
    ///
    /// When a remote item arrives with a large `lamport_ts` (e.g. an Android
    /// device that historically stored wall-clock millis ≈ 1.7 × 10^12 in that
    /// field), `observe()` must advance the LOCAL clock to `max(local, remote) + 1`
    /// — the standard Lamport rule — so subsequent local events are causally
    /// later than anything received.  The clock must NOT stay below the received
    /// value (which would make future local writes appear "older").
    ///
    /// The engine's upper-bound clamp (MAX_LAMPORT_SKEW) is intentionally set high
    /// enough (10^12) that a legitimate wall-clock-millis value just above 1.7 × 10^12
    /// is clamped before it jams the clock forever, while a value within the window
    /// advances the local clock correctly.
    #[tokio::test]
    async fn logical_clock_observe_advances_to_max_plus_one() {
        // Simulate a peer whose lamport_ts reflects a small logical counter (42).
        // Our local clock starts at 0.  After the session our clock must be > 42.
        let mut peer_item = make_item("peer-row", 42);
        peer_item.item_id = "peer-item".to_string();

        let mut engine_local = SyncEngine::new("device-local");
        let mut engine_peer = SyncEngine::new("device-peer");
        assert_eq!(engine_local.clock.get(), 0, "local clock starts at 0");

        let (mut sl, mut sp) = make_duplex();
        let local_items: [ClipboardItem; 0] = [];
        let peer_items = [peer_item];
        let _ = tokio::join!(
            engine_local.run_session(&mut sl, &local_items),
            engine_peer.run_session(&mut sp, &peer_items),
        );

        // After observing peer's lamport=42, local clock must be >= 43
        // (Lamport rule: max(0, 42) + 1 = 43).
        assert!(
            engine_local.clock.get() >= 43,
            "local clock must advance to at least max(local, peer)+1 = 43, got {}",
            engine_local.clock.get()
        );
    }
}
