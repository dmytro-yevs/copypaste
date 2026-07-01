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

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{insert_item, ClipboardItem, Database};

    fn item(id: &str, sensitive: bool) -> ClipboardItem {
        let mut it = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        it.id = id.to_owned().into();
        it.item_id = id.to_owned().into();
        it.is_sensitive = sensitive;
        it
    }

    /// CopyPaste-20yw / P1-1: the P2P catch-up read must OMIT sensitive items so
    /// they never leave the device via the catch-up burst. Real guard test: a
    /// sensitive and a non-sensitive item are stored, and only the non-sensitive
    /// one appears in the wire set. Removing the `if item.is_sensitive` skip in
    /// `catchup_read_raw` makes the secret item appear and fails this test.
    #[test]
    fn catchup_read_raw_omits_sensitive_items() {
        let db = Database::open_in_memory().expect("in-memory DB");
        insert_item(&db, &item("plain-1", false)).expect("insert plain");
        insert_item(&db, &item("secret-1", true)).expect("insert secret");

        let wire = catchup_read_raw(&db, "dev-a");
        let ids: Vec<&str> = wire.iter().map(|w| w.item_id.as_str()).collect();

        assert!(
            ids.contains(&"plain-1"),
            "non-sensitive item must be in the catch-up set: {ids:?}"
        );
        assert!(
            !ids.contains(&"secret-1"),
            "sensitive item must be omitted from the catch-up set: {ids:?}"
        );
    }

    /// CopyPaste-716: catchup_items must use the connecting peer's pairwise
    /// key, not the first peer's key. With 2+ peers, the catch-up set for peer
    /// C must decrypt under K_AC only, not K_AB.
    ///
    /// This is the equivalent catch-up path test for the fanout fix covered by
    /// `sync_orch::rekey::tests::three_device_fanout_uses_per_peer_key_not_first_peer_key`.
    /// Relocated here (ADR-017, CopyPaste-vp63.3) from the former flat
    /// `sync_orch/mod.rs` test module.
    #[tokio::test]
    async fn catchup_items_uses_per_peer_key_not_first_peer_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, decrypt_from_cloud, derive_v2, encrypt_item_with_aad, insert_item,
            AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        let seed_a = [0x11u8; 32];
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);
        let fp_b = "bb:bb";
        let fp_c = "cc:cc";

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[
                    {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                    {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
                ]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // Insert one text item into the DB encrypted under A's v2 key.
        let db = Database::open_in_memory().expect("in-memory DB");
        let item_id = "catchup-716-item".to_string();
        let plaintext = b"catchup per-peer key test";
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(
            &copypaste_core::ItemId::from(item_id.as_str()),
            AAD_SCHEMA_VERSION_V4,
            2,
        );
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        let mut local = copypaste_core::ClipboardItem::new_text(ct_a, nonce_a.to_vec(), 1);
        local.item_id = item_id.clone().into();
        insert_item(&db, &local).unwrap();

        // Catch-up for peer B: items must be encrypted under K_AB.
        let items_for_b = catchup_items(&db, "device-A", &crypto_a, fp_b);
        assert_eq!(items_for_b.len(), 1, "catch-up for B must contain our item");
        let blob_b = items_for_b[0].content.as_ref().unwrap().clone();
        let key_b = copypaste_core::SyncKey::from_bytes(k_ab);
        let dec_b = decrypt_from_cloud(&key_b, &item_id, &blob_b)
            .expect("B's catch-up blob must decrypt under K_AB");
        assert_eq!(
            dec_b, plaintext,
            "B recovers original plaintext from catch-up"
        );

        // Catch-up for peer C: items must be encrypted under K_AC.
        let items_for_c = catchup_items(&db, "device-A", &crypto_a, fp_c);
        assert_eq!(items_for_c.len(), 1, "catch-up for C must contain our item");
        let blob_c = items_for_c[0].content.as_ref().unwrap().clone();
        let key_c = copypaste_core::SyncKey::from_bytes(k_ac);
        let dec_c = decrypt_from_cloud(&key_c, &item_id, &blob_c)
            .expect("C's catch-up blob must decrypt under K_AC");
        assert_eq!(
            dec_c, plaintext,
            "C recovers original plaintext from catch-up"
        );

        // Key isolation: C's catch-up blob must NOT decrypt under K_AB.
        assert!(
            decrypt_from_cloud(&key_b, &item_id, &blob_c).is_err(),
            "C's catch-up blob (K_AC) must not decrypt under K_AB — \
             this would be the CopyPaste-716 bug if it succeeded"
        );
    }
}
