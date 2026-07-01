//! `SyncEngine::run_session` — the HELLO/HAVE/WANT/ITEMS/DONE protocol loop.

use std::collections::{HashMap, HashSet};

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, info, warn};

use super::bounds::{MAX_LAMPORT_SKEW, MAX_WALL_TIME_SKEW_MS};
use super::error::SyncError;
use super::framing::{recv_message, send_message};
use super::{PeerState, SyncEngine, SyncResult};
use crate::merge::{local_to_wire, resolve, wire_to_local, MergeOutcome};
use crate::protocol::{Message, WireItem};
use copypaste_core::storage::items::ClipboardItem;

impl SyncEngine {
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
            .map(|i| (i.item_id.to_string(), i.lamport_ts))
            .collect();
        let local_ids: HashSet<&String> = local_clock_map.keys().collect();

        let my_have = Message::Have {
            items: local_items
                .iter()
                .map(|i| (i.item_id.to_string(), i.lamport_ts))
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
