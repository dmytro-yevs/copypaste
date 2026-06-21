use copypaste_core::{get_page, Database};
use copypaste_sync::{merge::local_to_wire_owned, protocol::WireItem};
use tracing::{debug, warn};

use super::rekey::{rekey_outbound_for_peer, RekeyOutcome, SyncCrypto};

/// Page size used when iterating local history to build the catch-up set.
/// Keeping pages small avoids materialising thousands of structs at once and
/// keeps peak heap usage proportional to this constant rather than to the total
/// item count.
pub(super) const CATCHUP_PAGE_SIZE: usize = 500;

/// Read raw local history pages from the DB into wire items WITHOUT re-keying.
///
/// Used by the two-phase catch-up path (Fix B): the caller holds the DB lock
/// only for this read step and releases it before calling
/// [`rekey_catchup_items`] so the CPU-heavy per-image re-key (decrypt-chunks +
/// re-encrypt-for-cloud) does not stall other DB writers while holding the
/// `Arc<Mutex<Database>>`.
///
/// Returns raw `WireItem`s with at-rest local ciphertext — callers MUST pass
/// them through `rekey_catchup_items` before forwarding to a peer.
pub fn catchup_read_raw(db: &Database, device_id: &str) -> Vec<WireItem> {
    let mut out = Vec::new();
    let mut offset: usize = 0;
    loop {
        let page: Vec<copypaste_core::ClipboardItem> = match get_page(db, CATCHUP_PAGE_SIZE, offset)
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("sync_orch: catchup_read_raw get_page (offset={offset}) failed: {e}");
                break;
            }
        };
        let page_len = page.len();
        // CopyPaste-ux2i: move each item's content blob into the wire item
        // instead of cloning it. P1-1: skip sensitive items — they must never
        // leave this device, including via the P2P catch-up burst.
        for item in page {
            if item.is_sensitive {
                debug!(
                    item_id = %item.item_id,
                    "sync_orch: catchup_read_raw: omitting sensitive item from catch-up set"
                );
                continue;
            }
            out.push(local_to_wire_owned(item, device_id));
        }
        if page_len < CATCHUP_PAGE_SIZE {
            break;
        }
        offset += CATCHUP_PAGE_SIZE;
    }
    out
}

/// Re-key raw catch-up wire items under the per-peer sync key (CPU step).
///
/// Second half of the two-phase catch-up (Fix B): runs WITHOUT the DB lock so
/// the image chunk-decrypt + shared-key re-encrypt does not contend with DB
/// writers. Items that cannot be re-keyed (`NotApplicable` or `Failed`) are
/// dropped so the peer never receives an undecryptable blob (sync H2).
pub fn rekey_catchup_items(
    raw: Vec<WireItem>,
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
) -> Vec<WireItem> {
    raw.into_iter()
        .filter_map(|mut wire| {
            // Re-key under this peer's pairwise key (CopyPaste-716).
            // Only forward items we could actually re-key — a
            // still-locally-encrypted (NotApplicable) or failed payload is
            // useless — or worse, undecryptable — to the peer (sync H2).
            if rekey_outbound_for_peer(crypto, peer_fingerprint, &mut wire)
                == RekeyOutcome::Rewrapped
            {
                Some(wire)
            } else {
                None
            }
        })
        .collect()
}

/// Build the set of local items to push to a specific peer that has just
/// connected (P2P Phase 3 "sync on connect" / catch-up).
///
/// Fanout is fire-and-forget to *currently* connected sinks, so an item
/// captured/imported before the mTLS link came up would otherwise never reach
/// the peer (and the both-sides-dial race makes the exact connect instant
/// non-deterministic). When a connection is established we therefore replay the
/// full local history to it once: each row is converted to a wire item and
/// re-keyed under the **per-peer** sync key for `peer_fingerprint` so only
/// the target peer can decrypt it. LWW on the receiver makes the replay
/// idempotent (already-present items lose or no-op).
///
/// CopyPaste-716: the previous signature had no `peer_fingerprint` parameter
/// and used `shared_sync_key()` (the first peer's key), so on 3+ device
/// topologies peers B and C both received catch-up blobs encrypted under K_AB.
/// Peer C (holding K_AC) could never decrypt them — silent sync failure.
/// Now each catch-up call passes the connecting peer's fingerprint and uses
/// that peer's specific pairwise key.
///
/// Returns an empty vec when the peer has no sync key (nothing decryptable to
/// send) or the DB read fails — catch-up is best-effort.
///
/// NOTE: This single-phase variant holds the DB lock across both the read and
/// the re-key steps.  The preferred path in the daemon uses [`catchup_read_raw`]
/// then [`rekey_catchup_items`] so the DB lock is released before the CPU-heavy
/// re-key work.  This function is retained for callers that already hold a
/// `&Database` (e.g. internal tests).
pub fn catchup_items(
    db: &Database,
    device_id: &str,
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
) -> Vec<WireItem> {
    // Pre-flight: only bother paginating if the connecting peer has a sync key.
    // H8 fix preserved: uses the in-memory cache — no peers.json disk read.
    if crypto.sync_key_for_peer(peer_fingerprint).is_none() {
        return Vec::new();
    }

    let raw = catchup_read_raw(db, device_id);
    rekey_catchup_items(raw, crypto, peer_fingerprint)
}
