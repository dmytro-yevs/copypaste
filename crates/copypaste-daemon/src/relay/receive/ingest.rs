//! Ingest pipeline: apply one pulled relay page into the local DB.
//!
//! The relay wire is ciphertext-only end-to-end — `decrypt_from_cloud` is the
//! only place plaintext is produced, and it stays in-process (never logged,
//! never re-serialized to the relay). LWW + quota-prune here are byte-for-byte
//! the Supabase poll path (`crate::cloud`).

use copypaste_core::{
    decrypt_from_cloud, exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone,
    prune_to_cap, soft_delete_item, Database, SyncKey,
};
// CopyPaste-ayvs: relay LWW now routes through the SAME total order the P2P and
// cloud paths use (lamport -> wall_time -> origin_device_id) so all transports
// converge identically.
use copypaste_sync::merge::{remote_wins, RemoteMeta};

use crate::sync_common::{build_local_item, replace_cloud_item_by_item_id};

use super::super::types::PullItem;
use super::super::watermark::Watermark;
use super::super::wire::decode_payload;

/// Ingest one pulled page into the local DB on a blocking thread (SQLCipher +
/// AEAD). Returns the advanced watermark and how many rows were stored.
///
/// LWW + quota-prune are byte-for-byte the Supabase poll path: dedup on
/// `item_id`, a strictly-newer remote `lamport_ts` replaces in place (preserving
/// the local PK + pin state), an older/equal one is skipped (this is also what
/// makes our OWN pushed rows a no-op when they echo back — self-echo dedup).
// `pub(in super::super)`: visible to `relay` — this fn moved one directory
// level deeper (into `relay::receive::ingest`), so it needs one extra `super`
// to reach the same `relay`-wide audience the flat `receive.rs` file
// exposed; consumed (test-only) by `relay::pasteboard`.
pub(in super::super) fn ingest_page_blocking(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key_bytes: &[u8; 32],
    page: &[PullItem],
    start: Watermark,
    storage_quota_bytes: u64,
    // Item 3 (CopyPaste-8ebg.7): live AppConfig decode-bomb budget, threaded
    // the same way `storage_quota_bytes` is, so `build_local_item` no longer
    // falls back to the compile-time `MAX_DECODED_IMAGE_MB` default.
    max_decoded_image_mb: u32,
) -> (Watermark, u32) {
    let mut wm = start;
    let mut stored = 0u32;
    let sk = SyncKey::from_bytes(*sync_key_bytes);

    for row in page {
        // Advance the watermark for EVERY readable row (even skipped ones) so the
        // next page does not re-request them.
        if (row.wall_time, row.id) > (wm.wall, wm.id) {
            wm = Watermark {
                wall: row.wall_time,
                id: row.id,
            };
        }

        // CopyPaste-crh3.69: version-gated decode of EITHER wire format —
        // legacy V1 `base64(JSON{..,ct_b64})` (in-flight inbox items written by
        // older daemons) OR the new V2 single-base64 frame
        // `base64(0x01||u32_le(meta_len)||meta_json||raw_ct)`. Both funnel into
        // the same metadata + raw ciphertext shape.
        let env = match decode_payload(&row.content_b64) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} wire decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let blob: &[u8] = &env.ct;

        // LWW dedup on the cross-device item_id.
        let existing = match get_item_by_item_id(db, &env.item_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("relay-sync: get_item_by_item_id error: {e}; skipping");
                continue;
            }
        };
        // The envelope's wall_time is authoritative for LWW; fall back to the
        // relay row's wall_time when an older envelope omitted it (=> 0).
        let env_wall = if env.wall_time != 0 {
            env.wall_time
        } else {
            row.wall_time as i64
        };
        let preserved_pk = if let Some(local) = existing.as_ref() {
            // CopyPaste-ayvs: same total order as P2P/cloud (lamport ->
            // wall_time -> origin_device_id) instead of the old bare
            // `env.lamport_ts <= local -> keep`, which never converged on ties.
            let wins = remote_wins(
                local.lamport_ts,
                local.wall_time,
                &local.origin_device_id,
                &RemoteMeta {
                    lamport_ts: env.lamport_ts,
                    wall_time: env_wall,
                    origin_device_id: &env.origin_device_id,
                },
            );
            if !wins {
                // Local wins LWW — keep it (self-echo no-op + remote-edit loser).
                continue;
            }
            Some(local.id.clone())
        } else {
            match exists_item_by_item_id(db, &env.item_id) {
                Ok(true) => continue,
                Ok(false) => None,
                Err(e) => {
                    tracing::warn!("relay-sync: exists_item_by_item_id error: {e}; skipping");
                    continue;
                }
            }
        };

        // ── Tombstone fast-path (CopyPaste-cm0u / CopyPaste-bfiu) ─────────────
        // A delete envelope carries deleted=true and an empty ct_b64 (NULL
        // content). Apply it via the SAME soft_delete / insert_tombstone path as
        // P2P and cloud so deletes propagate over relay-only topologies, and a
        // delete that races ahead of the create still leaves a tombstone the
        // later create loses LWW against.
        if env.deleted {
            if let Some(local_pk) = preserved_pk.as_ref() {
                match soft_delete_item(db, local_pk, env.lamport_ts, env_wall) {
                    Ok(n) if n > 0 => {
                        stored += 1;
                        tracing::info!("relay-sync: applied tombstone (item known locally)");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("relay-sync: soft_delete_item failed: {e}"),
                }
            } else {
                match insert_tombstone(
                    db,
                    &env.item_id,
                    &env.item_id,
                    env.lamport_ts,
                    env_wall,
                    &env.origin_device_id,
                ) {
                    Ok(_) => {
                        stored += 1;
                        tracing::info!(
                            "relay-sync: inserted tombstone for unknown item \
                             (delete-before-create)"
                        );
                    }
                    Err(e) => tracing::warn!("relay-sync: insert_tombstone failed: {e}"),
                }
            }
            continue;
        }

        // Decrypt with the sync key (AAD = item_id + cloud schema v5).
        let plaintext = match decrypt_from_cloud(&sk, &env.item_id, blob) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: decrypt_from_cloud failed for item_id (wrong passphrase or \
                     tampered blob): {e}; skipping"
                );
                continue;
            }
        };

        let mut local_item = match build_local_item(
            // Use the cross-device item_id as the local PK seed when this is a
            // fresh insert; build_local_item sets `id` from this first arg.
            &env.item_id,
            &env.item_id,
            &row.content_type,
            &plaintext,
            env.lamport_ts,
            env_wall,
            None,
            None,
            // CopyPaste-ayvs: preserve the sender's origin so future tie-breaks
            // on this device stay deterministic across hops.
            env.origin_device_id.clone(),
            local_key,
            // CopyPaste-8ebg.7: now threaded from the live AppConfig via
            // `ingest_page_blocking`'s new `max_decoded_image_mb` param
            // (mirrors `storage_quota_bytes`), instead of the compiled
            // default.
            max_decoded_image_mb,
        ) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("relay-sync: build_local_item failed: {e}; skipping");
                continue;
            }
        };

        // LWW replace preserves the prior local row's PK.
        if let Some(pk) = preserved_pk.as_ref() {
            local_item.id = pk.clone();
        }
        // CopyPaste-cm0u: the envelope's pin state is authoritative (it travels
        // with the item now). The pin LWW already won above (this is the
        // TakeRemote branch), so apply the sender's pinned/pin_order directly.
        local_item.pinned = env.pinned;
        local_item.pin_order = env.pin_order;

        let write_res = if preserved_pk.is_some() {
            replace_cloud_item_by_item_id(db, &local_item)
        } else {
            insert_item(db, &local_item).map_err(anyhow::Error::from)
        };
        match write_res {
            Ok(()) => {
                stored += 1;
                tracing::info!("relay-sync: ingested remote item (id={})", local_item.id);
            }
            Err(e) => tracing::warn!("relay-sync: store failed: {e}"),
        }
    }

    // Byte-cap prune after ingest (long-offline backfill safety) — same policy
    // as the Supabase poll path.
    if stored > 0 {
        let max_bytes = storage_quota_bytes.min(i64::MAX as u64) as i64;
        match prune_to_cap(db, max_bytes) {
            Ok(0) => {}
            Ok(n) => tracing::debug!("relay-sync: byte-pruned {n} rows after ingest"),
            Err(e) => tracing::warn!("relay-sync: prune_to_cap failed: {e}"),
        }
    }

    (wm, stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{get_item_by_item_id, SyncKey};

    use crate::relay::testutil::{
        envelope_to_pull, make_local_text_item, make_pinned_pull, make_pull_item,
        make_tombstone_pull, open_mem_db, skey,
    };
    use crate::relay::types::RelayEnvelope;

    /// receive ingests a relay item via insert_item with LWW, and a re-pull of
    /// the SAME item (self-echo / equal lamport) is a no-op. Watermark advances.
    #[test]
    fn ingest_inserts_then_dedups_with_lww() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([9u8; 32]);
        let sync_bytes = skey("ingest-lww-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);

        // Build a wire item by encrypting a text payload through the cloud crypto.
        let plaintext = b"ingest me";
        let item_id = "item-ingest-1";
        let pull = make_pull_item(1, item_id, plaintext, &sync_key, 10, 2000);

        let g = db.blocking_lock();
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull),
            Watermark::default(),
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored1, 1, "first ingest inserts the row");
        assert_eq!(wm1.wall, 2000);
        assert_eq!(wm1.id, 1);
        // The row is present and decodes through the production path.
        let got = get_item_by_item_id(&g, item_id)
            .expect("query")
            .expect("row present");
        assert_eq!(got.lamport_ts, 10);

        // Re-pull the SAME item with equal lamport, equal wall_time, and equal
        // origin (a genuine self-echo of a row we pushed) → LWW no-op.
        // CopyPaste-ayvs: the total order now tie-breaks on wall_time then
        // origin, so a true echo must match ALL three keys (a higher wall_time
        // would legitimately win — that is the convergence fix, not a regression).
        let pull2 = make_pull_item(2, item_id, plaintext, &sync_key, 10, 2000);
        let (wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull2),
            wm1,
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored2, 0, "equal lamport+wall+origin echo is a no-op");
        // Watermark still advances past the seen row (id) so we don't re-fetch it.
        assert_eq!(wm2.wall, 2000);
        assert_eq!(wm2.id, 2);

        // A strictly-newer lamport for the same item_id wins LWW (replace).
        let pull3 = make_pull_item(3, item_id, b"edited", &sync_key, 11, 2002);
        let (_wm3, stored3) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull3),
            wm2,
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored3, 1, "newer lamport replaces in place");
    }

    // ── CopyPaste-cm0u: delete + pin propagate over the relay envelope ────────

    /// A delete envelope round-trips: build_content_b64 on a tombstone produces
    /// a `deleted=true` / empty-ct envelope (no decrypt of NULL content), and
    /// ingest applies it as a local soft-delete on a previously-live item.
    #[test]
    fn relay_tombstone_round_trip_soft_deletes_local() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([4u8; 32]);
        let sync_bytes = skey("relay-tombstone-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        // First ingest a live item (lamport 10).
        let item_id = "item-del-1";
        let live = make_pull_item(1, item_id, b"to be deleted", &sync_key, 10, 1000);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&live),
            Watermark::default(),
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored1, 1, "live item inserted");
        assert!(
            !get_item_by_item_id(&g, item_id).unwrap().unwrap().deleted,
            "item starts live"
        );

        // Now ingest a tombstone (lamport 11 > 10) — must soft-delete locally.
        let tomb = make_tombstone_pull(2, item_id, 11, 2000);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&tomb),
            wm1,
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored2, 1, "tombstone applied");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "relay tombstone must soft-delete the local item"
        );
        assert!(row.content.is_none(), "tombstone wipes content");
    }

    /// Pin state propagates: a pinned envelope ingests as a pinned local row.
    #[test]
    fn relay_pin_round_trip_sets_pinned_local() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([8u8; 32]);
        let sync_bytes = skey("relay-pin-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-pin-1";
        let pinned = make_pinned_pull(1, item_id, b"pin me", &sync_key, 5, 1000, 2.0);
        let (_wm, stored) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pinned),
            Watermark::default(),
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored, 1, "pinned item inserted");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(row.pinned, "relay must carry pinned=true");
        assert_eq!(row.pin_order, Some(2.0), "relay must carry pin_order");
    }

    // ── CopyPaste-ayvs: transport tie-break parity (relay == P2P resolve) ─────

    /// On EQUAL lamport, relay `ingest_page_blocking` must converge to the SAME
    /// winner as the P2P `merge::resolve` (lamport -> wall_time ->
    /// origin_device_id). Drive both with identical inputs and assert they agree
    /// for both tie-break outcomes (remote-wins and local-wins on device id).
    #[test]
    fn relay_equal_lamport_tie_break_matches_p2p_resolve() {
        use base64::Engine as _;
        use copypaste_core::{encrypt_for_cloud, insert_item};
        use copypaste_sync::merge::{resolve, MergeOutcome};
        use copypaste_sync::protocol::WireItem;

        // Helper: build a P2P WireItem mirroring a relay envelope's keys.
        fn wire(item_id: &str, lamport: i64, wall: i64, origin: &str) -> WireItem {
            WireItem {
                id: item_id.to_owned(),
                item_id: item_id.to_owned(),
                content_type: "text".to_owned(),
                content: Some(vec![1, 2, 3]),
                content_nonce: Some(vec![0u8; 24]),
                blob_ref: None,
                is_sensitive: false,
                lamport_ts: lamport,
                wall_time: wall,
                expires_at: None,
                app_bundle_id: None,
                origin_device_id: origin.to_owned(),
                key_version: 2,
                file_name: None,
                mime: None,
                deleted: false,
                pinned: false,
                pin_order: None,
            }
        }

        // Two cases: remote origin "zzz" (> local) must win; "aaa" (< local) loses.
        for (remote_origin, remote_should_win) in [("zzz", true), ("aaa", false)] {
            let db = open_mem_db();
            let local_key = zeroize::Zeroizing::new([2u8; 32]);
            let sync_bytes = skey("relay-parity-pass");
            let sync_key = SyncKey::from_bytes(sync_bytes);
            let g = db.blocking_lock();

            let item_id = "item-parity";
            // Seed a LOCAL item: lamport 5, wall 1000, origin "mmm".
            let mut seed = make_local_text_item(item_id, b"local-content", &local_key, 5, 1000);
            seed.origin_device_id = "mmm".to_owned();
            insert_item(&g, &seed).unwrap();

            // P2P decision via resolve on identical keys.
            let remote_wire = wire(item_id, 5, 1000, remote_origin);
            let p2p_take_remote = matches!(resolve(&seed, &remote_wire), MergeOutcome::TakeRemote);
            assert_eq!(
                p2p_take_remote, remote_should_win,
                "sanity: resolve decision for origin={remote_origin}"
            );

            // Relay decision: ingest an equal-lamport envelope with the same keys.
            let env = RelayEnvelope {
                item_id: item_id.to_owned(),
                lamport_ts: 5,
                ct_b64: base64::engine::general_purpose::STANDARD
                    .encode(encrypt_for_cloud(&sync_key, item_id, b"remote-content").unwrap()),
                deleted: false,
                pinned: false,
                pin_order: None,
                wall_time: 1000,
                origin_device_id: remote_origin.to_owned(),
            };
            let pull = envelope_to_pull(1, "text", &env, 1000);
            let (_wm, stored) = ingest_page_blocking(
                &g,
                &local_key,
                &sync_bytes,
                std::slice::from_ref(&pull),
                Watermark::default(),
                u64::MAX,
                copypaste_core::config::MAX_DECODED_IMAGE_MB,
            );
            let relay_took_remote = stored == 1;
            assert_eq!(
                relay_took_remote, p2p_take_remote,
                "relay ingest must converge to the SAME winner as P2P resolve \
                 (origin={remote_origin}): relay={relay_took_remote}, p2p={p2p_take_remote}"
            );
            // Confirm the stored row's origin matches the chosen winner.
            let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
            let expected_origin = if remote_should_win {
                remote_origin
            } else {
                "mmm"
            };
            assert_eq!(
                row.origin_device_id, expected_origin,
                "winning origin must persist for deterministic future tie-breaks"
            );
        }
    }

    // ── CopyPaste-bfiu: delete-before-create over relay must not resurrect ────

    /// A tombstone for an UNKNOWN item_id inserts a tombstone row; a later
    /// out-of-order create with a LOWER lamport then loses LWW and the item
    /// stays deleted.
    #[test]
    fn relay_delete_before_create_does_not_resurrect() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([3u8; 32]);
        let sync_bytes = skey("relay-dbc-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-race-1";
        // Delete arrives FIRST (lamport 20) for an item we have never seen.
        let tomb = make_tombstone_pull(1, item_id, 20, 2000);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&tomb),
            Watermark::default(),
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored1, 1, "tombstone inserted for unknown item");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "unknown-item tombstone must persist as deleted"
        );

        // Create arrives LATER with a LOWER lamport (10 < 20) — must lose LWW.
        let create = make_pull_item(2, item_id, b"resurrected?", &sync_key, 10, 1000);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&create),
            wm1,
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored2, 0, "late lower-lamport create must NOT resurrect");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "item must stay deleted after the racing create"
        );
    }

    // ── dtq3: additive multi-transport dedup ─────────────────────────────────

    /// When the SAME `item_id` arrives via TWO independent transports (relay +
    /// Supabase / cloud) the consumer-side LWW guard must ensure exactly ONE DB
    /// row is written — no double-count, no duplicate content.
    ///
    /// This test simulates the scenario by calling `ingest_page_blocking` twice
    /// for the same `item_id` (same lamport, same wall_time, same origin), which
    /// models a peer that receives the item from both relay and Supabase.  The
    /// second call must be a complete no-op: `stored == 0` and the DB still has
    /// exactly one row for that `item_id`.
    #[test]
    fn both_transports_deliver_same_item_inserts_exactly_once() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([0xBBu8; 32]);
        let sync_bytes = skey("dual-transport-dedup-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-dual-transport-1";
        let plaintext = b"hello from both transports";

        // --- Transport 1 (relay): first delivery ---
        let relay_pull = make_pull_item(1, item_id, plaintext, &sync_key, 7, 1500);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&relay_pull),
            Watermark::default(),
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(stored1, 1, "first transport delivery must insert the row");

        // Confirm exactly one row in DB with the correct lamport.
        let row_after_first = get_item_by_item_id(&g, item_id)
            .expect("query ok")
            .expect("row must exist after first transport");
        assert_eq!(row_after_first.lamport_ts, 7);

        // --- Transport 2 (cloud/Supabase, modelled as another relay call with
        // the SAME item_id, lamport, wall_time, and origin): second delivery ---
        // Use a different relay `id` (id=2) to avoid watermark dedup; the
        // envelope `item_id` is identical — this is what makes it a cross-transport
        // duplicate.  The ingest path keys on envelope `item_id`, not relay row `id`.
        let cloud_pull = make_pull_item(2, item_id, plaintext, &sync_key, 7, 1500);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&cloud_pull),
            wm1,
            u64::MAX,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        );
        assert_eq!(
            stored2, 0,
            "second transport delivery of the same item_id must be a LWW no-op (stored==0)"
        );

        // Confirm the DB still has EXACTLY one row for this item_id.
        let row_after_second = get_item_by_item_id(&g, item_id)
            .expect("query ok")
            .expect("row must still exist after second transport");
        assert_eq!(
            row_after_second.lamport_ts, 7,
            "lamport must be unchanged — row not double-written"
        );
        // There must not be a second row with a different PK carrying the same item_id.
        // `get_item_by_item_id` returns the UNIQUE row (item_id has a UNIQUE index),
        // so the fact that it returns Some without UNIQUE conflict is proof enough.
        // Additionally verify the content is intact (not corrupted by a partial re-write).
        assert!(
            row_after_second.content.is_some(),
            "content must be intact after dedup no-op"
        );
    }
}
