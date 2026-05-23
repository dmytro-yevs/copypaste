//! Last-Write-Wins (LWW) merge logic for clipboard items.
//!
//! Conflict resolution rules (in priority order):
//!  1. Higher `lamport_ts` wins — the causally-later write takes precedence.
//!  2. On equal Lamport timestamps, higher `wall_time` (Unix ms) wins.
//!  3. On equal wall times, lexicographically larger `origin_device_id` wins
//!     (deterministic tie-break so both sides converge to the same item).
//!
//! This module is pure logic — no I/O, no database access.
use crate::protocol::WireItem;
use copypaste_core::storage::items::ClipboardItem;

/// Outcome of comparing two versions of the *same* logical item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Keep the local version unchanged.
    KeepLocal,
    /// Replace the local version with the remote one.
    TakeRemote,
}

/// Compare a locally-stored item against a remote version of the same item.
///
/// `local.id` and `remote.id` must be equal (same logical item).
/// Returns `TakeRemote` if the remote version should win, `KeepLocal` otherwise.
pub fn resolve(local: &ClipboardItem, remote: &WireItem) -> MergeOutcome {
    debug_assert_eq!(local.id, remote.id, "resolve called on different items");

    match remote.lamport_ts.cmp(&local.lamport_ts) {
        std::cmp::Ordering::Greater => MergeOutcome::TakeRemote,
        std::cmp::Ordering::Less => MergeOutcome::KeepLocal,
        std::cmp::Ordering::Equal => {
            // Tie-break by wall time.
            match remote.wall_time.cmp(&local.wall_time) {
                std::cmp::Ordering::Greater => MergeOutcome::TakeRemote,
                std::cmp::Ordering::Less => MergeOutcome::KeepLocal,
                std::cmp::Ordering::Equal => {
                    // Final tie-break by `origin_device_id` (lexicographic,
                    // larger wins). Before schema v3 this branch compared
                    // `remote.origin_device_id` against `local.id` (the row
                    // UUID) — two completely different identifier spaces.
                    // Row UUIDs are random per-write while device ids are
                    // stable per-peer, so the result was non-deterministic
                    // and frequently bogus: two peers could pick different
                    // winners, causing CRDT divergence (merge.rs:39 BUG).
                    // v3 added `ClipboardItem::origin_device_id` so we now
                    // compare the same field on both sides, matching the
                    // module contract above and converging every peer to
                    // the same winner.
                    if remote.origin_device_id > local.origin_device_id {
                        MergeOutcome::TakeRemote
                    } else {
                        MergeOutcome::KeepLocal
                    }
                }
            }
        }
    }
}

/// Convert a `WireItem` received from a peer into a `ClipboardItem` ready to
/// be persisted locally, marking it as synced.
pub fn wire_to_local(wire: WireItem) -> ClipboardItem {
    ClipboardItem {
        id: wire.id,
        item_id: wire.item_id,
        content_type: wire.content_type,
        content: wire.content,
        content_nonce: wire.content_nonce,
        blob_ref: wire.blob_ref,
        is_sensitive: wire.is_sensitive,
        is_synced: true,
        lamport_ts: wire.lamport_ts,
        wall_time: wire.wall_time,
        expires_at: wire.expires_at,
        app_bundle_id: wire.app_bundle_id,
        content_hash: None,
        // Preserve the peer's origin so future tie-breaks remain
        // deterministic regardless of which peer replays the merge.
        origin_device_id: wire.origin_device_id,
        // WireItem does not carry key_version yet (added in wave1a-atomic).
        // Default to 1 so the v4 sweep treats received items conservatively
        // and re-encrypts them to v2 if needed.
        key_version: 1,
        // Received items are never pinned by default; the user must pin them
        // explicitly on this device after syncing.
        pinned: false,
    }
}

/// Convert a local `ClipboardItem` into a `WireItem` for transmission.
///
/// If the item already carries an `origin_device_id` (received from a peer or
/// stamped on local creation) that value is preserved; otherwise
/// `local_device_id` is used. Preserving the existing origin is essential for
/// the LWW tie-break: every peer must see the same origin for the same item
/// regardless of who relays it.
pub fn local_to_wire(item: &ClipboardItem, local_device_id: &str) -> WireItem {
    let origin = if item.origin_device_id.is_empty() {
        local_device_id.to_string()
    } else {
        item.origin_device_id.clone()
    };
    WireItem {
        id: item.id.clone(),
        item_id: item.item_id.clone(),
        content_type: item.content_type.clone(),
        content: item.content.clone(),
        content_nonce: item.content_nonce.clone(),
        blob_ref: item.blob_ref.clone(),
        is_sensitive: item.is_sensitive,
        lamport_ts: item.lamport_ts,
        wall_time: item.wall_time,
        expires_at: item.expires_at,
        app_bundle_id: item.app_bundle_id.clone(),
        origin_device_id: origin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::storage::items::ClipboardItem;

    fn make_local(lamport: i64, wall: i64) -> ClipboardItem {
        ClipboardItem {
            id: "item-001".to_string(),
            item_id: "iid-001".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![1, 2, 3]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts: lamport,
            wall_time: wall,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-local".to_string(),
            key_version: 1,
            pinned: false,
        }
    }

    fn make_remote(lamport: i64, wall: i64, device_id: &str) -> WireItem {
        WireItem {
            id: "item-001".to_string(),
            item_id: "iid-001".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![4, 5, 6]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: lamport,
            wall_time: wall,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: device_id.to_string(),
        }
    }

    // --- Lamport clock ordering ---

    #[test]
    fn higher_remote_lamport_wins() {
        let local = make_local(5, 1000);
        let remote = make_remote(10, 500, "peer-A"); // higher lamport, lower wall
        assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);
    }

    #[test]
    fn higher_local_lamport_keeps_local() {
        let local = make_local(15, 500);
        let remote = make_remote(3, 9999, "peer-A"); // lower lamport, higher wall
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- Wall-time tie-break ---

    #[test]
    fn equal_lamport_higher_remote_wall_wins() {
        let local = make_local(5, 1000);
        let remote = make_remote(5, 2000, "peer-A");
        assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);
    }

    #[test]
    fn equal_lamport_higher_local_wall_keeps_local() {
        let local = make_local(5, 9000);
        let remote = make_remote(5, 1000, "peer-A");
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- Device-ID tie-break (determinism) ---

    #[test]
    fn equal_lamport_equal_wall_larger_device_id_wins() {
        // local.origin_device_id == "device-local"; remote "zzz" > "device-local"
        let local = make_local(5, 1000);
        let remote_wins = make_remote(5, 1000, "zzz");
        assert_eq!(resolve(&local, &remote_wins), MergeOutcome::TakeRemote);

        // remote "aaa" < "device-local" → local keeps
        let local_wins = make_remote(5, 1000, "aaa");
        assert_eq!(resolve(&local, &local_wins), MergeOutcome::KeepLocal);
    }

    #[test]
    fn equal_lamport_equal_wall_equal_device_keeps_local() {
        // Two peers with the same origin (e.g. same item replayed) converge
        // to KeepLocal — the comparison is a strict `>`.
        let local = make_local(5, 1000);
        let remote = make_remote(5, 1000, "device-local");
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- wire_to_local ---

    #[test]
    fn wire_to_local_marks_synced_and_preserves_origin() {
        let wire = make_remote(7, 2000, "dev-X");
        let local = wire_to_local(wire.clone());
        assert!(local.is_synced);
        assert_eq!(local.lamport_ts, 7);
        assert_eq!(local.content, wire.content);
        assert_eq!(
            local.origin_device_id, "dev-X",
            "peer origin must survive wire_to_local so tie-breaks stay \
             deterministic across hops"
        );
    }

    // --- local_to_wire ---

    #[test]
    fn local_to_wire_preserves_existing_origin() {
        // Item already has an origin (received from a peer earlier and now
        // being relayed). local_to_wire must keep the original origin, not
        // overwrite it with the local device id.
        let mut item = make_local(3, 500);
        item.origin_device_id = "peer-A".to_string();
        let wire = local_to_wire(&item, "my-device");
        assert_eq!(wire.origin_device_id, "peer-A");
    }

    #[test]
    fn local_to_wire_stamps_local_when_origin_empty() {
        // Pre-backfill row (or legacy v2 row migrated up). local_to_wire
        // stamps the local device id as a safe default.
        let mut item = make_local(3, 500);
        item.origin_device_id = String::new();
        let wire = local_to_wire(&item, "my-device");
        assert_eq!(wire.id, item.id);
        assert_eq!(wire.lamport_ts, 3);
        assert_eq!(wire.origin_device_id, "my-device");
        assert_eq!(wire.content, item.content);
    }
}
