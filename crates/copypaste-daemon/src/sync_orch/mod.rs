//! Sync orchestrator — wires `copypaste-sync` into the daemon.
//!
//! Responsibilities:
//!
//! 1. Subscribe to the daemon's local `new_item_tx` broadcast channel and
//!    convert each freshly-inserted [`ClipboardItem`] into a [`WireItem`],
//!    forwarding it on `outbound_tx` for the transport layer to deliver.
//! 2. Consume incoming [`WireItem`]s pushed by the transport layer via
//!    `incoming_rx` and merge them into the local SQLite database using the
//!    Last-Write-Wins rules defined in `copypaste-sync::merge`.
//!
//! ## Why channels instead of a Transport trait?
//!
//! The actual peer transports (mTLS-over-TCP from `copypaste-p2p`, the
//! Supabase relay from `cloud.rs`, or a future WebRTC channel) live in
//! sibling modules. We expose two `tokio::sync` channels — outbound and
//! inbound — so the orchestrator stays pure I/O-free merge logic and the
//! tests remain hermetic. The transport layer owns the network side and just
//! forwards bytes through these channels.

pub(crate) mod catchup;
pub(crate) mod merge;
pub(crate) mod pasteboard;
pub(crate) mod poison;
pub(crate) mod rekey;

// ── Public re-exports (keep the flat public surface identical to the old file)

pub use catchup::{catchup_items, catchup_read_raw, rekey_catchup_items};
pub use merge::{merge_incoming, merge_incoming_with_crypto};
pub use poison::{is_poison_wire, sweep_poison_rows};
pub use rekey::{
    rekey_outbound_for_peer, AutoApplyCtx, RekeyOutcome, SyncCrypto, SYNC_MAX_BLOB_BYTES,
};

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use copypaste_core::ClipboardItem;
use copypaste_sync::{merge::local_to_wire_owned, protocol::WireItem};

/// Run the sync orchestrator until both upstream channels close or `shutdown`
/// is cancelled.
///
/// * `db` — shared handle to the local SQLite store.
/// * `new_item_rx` — broadcast receiver from `daemon::run`; carries items
///   produced by the local clipboard monitor.
/// * `incoming_rx` — `mpsc` receiver fed by the transport layer with items
///   received from remote peers.
/// * `outbound_tx` — `mpsc` sender drained by the transport layer to push
///   locally-produced items to peers. A closed receiver is logged and
///   ignored — peers may simply not be connected.
/// * `device_id` — UUID stamped as `origin_device_id` on outgoing items.
/// * `storage_quota_bytes` — byte cap passed to `prune_to_cap` after each
///   successful P2P merge so the local DB stays bounded (mirrors the cloud path).
/// * `auto_apply` — when `Some`, enables the Universal Clipboard feature: a
///   genuinely fresh incoming item (newer than the current local clipboard) is
///   written to NSPasteboard immediately after merge, with the self-write guard
///   armed to prevent re-capture by the poller.
/// * `shutdown` — D2: token cancelled by the daemon on SIGINT/SIGTERM so the
///   orchestrator exits promptly instead of waiting for channels to drain.
///
/// Returns `Ok(())` once both channels close or `shutdown` fires.
// `run` takes: db, new_item_rx, incoming_rx, outbound_tx, device_id, crypto,
// storage_quota_bytes, auto_apply, and shutdown — each a distinct runtime
// dependency; no struct without pulling daemon internals into copypaste-sync.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    db: Arc<Mutex<copypaste_core::Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
    crypto: Option<SyncCrypto>,
    storage_quota_bytes: i64,
    auto_apply: Option<AutoApplyCtx>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    info!(%device_id, has_crypto = crypto.is_some(), "sync orchestrator started");

    let mut local_closed = false;
    let mut incoming_closed = false;

    while !(local_closed && incoming_closed) {
        tokio::select! {
            // D2: exit promptly on daemon-wide shutdown signal.
            _ = shutdown.cancelled() => {
                info!("sync orchestrator: shutdown signal received, stopping");
                break;
            }
            // Local clipboard → forward to transport for fan-out.
            local = new_item_rx.recv(), if !local_closed => {
                match local {
                    Ok(item) => {
                        // tke7 (PG-30): master sync gate — checked on every outbound
                        // item so a runtime set_config toggle takes effect immediately.
                        // Reads from AutoApplyCtx.core_config (shared Arc) when
                        // available; defaults to enabled when ctx is absent (P2P off
                        // anyway, so the gate is moot).
                        let sync_enabled = auto_apply
                            .as_ref()
                            .and_then(|ctx| ctx.core_config.read().ok().map(|g| g.sync_enabled))
                            .unwrap_or(true);
                        if !sync_enabled {
                            debug!(
                                item_id = %item.item_id,
                                "sync_orch: sync_enabled=false; not forwarding to P2P peers"
                            );
                            continue;
                        }

                        // P1-1: honour the "sensitive items are NEVER uploaded" guarantee.
                        // Block P2P transport just like relay and cloud paths.
                        if item.is_sensitive {
                            debug!(
                                item_id = %item.item_id,
                                "sync_orch: skipping sensitive item (never forwarded to P2P peers)"
                            );
                            continue;
                        }
                        // CopyPaste-ux2i: `item` is owned here and unused after the
                        // wire item is built, so move its content blobs instead of
                        // cloning them.
                        let wire = local_to_wire_owned(item, &device_id);
                        // CopyPaste-716: per-peer re-keying now happens in the
                        // transport's fanout_to_peers (p2p.rs) so each peer
                        // receives a blob encrypted under its own pairwise sync
                        // key. Sending the raw at-rest wire here is safe because
                        // the outbound_loop holds a SyncCrypto and re-encrypts
                        // once per peer at send time. When crypto is None (P2P
                        // disabled) the raw ciphertext is forwarded as before.
                        debug!(item_id = %wire.id, "sync_orch: forwarding local item to transport");
                        if let Err(e) = outbound_tx.send(wire).await {
                            // No transport listening — normal when P2P/cloud disabled.
                            debug!("sync_orch: outbound channel closed: {e}");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("sync_orch: broadcast lagged by {n} items");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("sync_orch: local channel closed");
                        local_closed = true;
                    }
                }
            }
            // Incoming peer item → LWW merge into DB.
            incoming = incoming_rx.recv(), if !incoming_closed => {
                match incoming {
                    Some(wire) => {
                        // tke7 (PG-30): gate inbound storage behind sync_enabled.
                        // When sync is off, we accept the wire frame from the
                        // transport layer (to keep the channel alive) but discard
                        // the payload rather than merging it into the local DB.
                        let sync_enabled_inbound = auto_apply
                            .as_ref()
                            .and_then(|ctx| ctx.core_config.read().ok().map(|g| g.sync_enabled))
                            .unwrap_or(true);
                        if !sync_enabled_inbound {
                            debug!(
                                item_id = %wire.id,
                                "sync_orch: sync_enabled=false; discarding inbound P2P item"
                            );
                            continue;
                        }
                        if let Err(e) = merge_incoming_with_crypto(
                            &db,
                            vec![wire],
                            crypto.as_ref(),
                            storage_quota_bytes,
                            auto_apply.as_ref(),
                        ).await {
                            warn!("sync_orch: merge_incoming failed: {e}");
                        }
                    }
                    None => {
                        info!("sync_orch: incoming channel closed");
                        incoming_closed = true;
                    }
                }
            }
        }
    }

    info!("sync orchestrator stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::insert_item;

    // Re-export internal items needed by tests (previously available via
    // `use super::*` on the flat file; now gated behind pub(super) in submodules).
    use super::rekey::{parse_file_name_mime, rekey_inbound, rekey_outbound};

    fn make_db() -> Arc<Mutex<copypaste_core::Database>> {
        Arc::new(Mutex::new(
            copypaste_core::Database::open_in_memory().expect("in-memory DB must open"),
        ))
    }

    fn make_wire(id: &str, lamport: i64, content: u8) -> WireItem {
        WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: id.to_string(),
            item_id: format!("{id}-iid"),
            content_type: "text".to_string(),
            content: Some(vec![content]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: lamport,
            wall_time: 1_700_000_000_000 + lamport,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "remote-device".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        }
    }

    /// P2P Phase 3 (cross-device readability): an item encrypted at rest under
    /// device A's per-device local key must, after `rekey_outbound` (shared key
    /// re-wrap) → `rekey_inbound` (shared key unwrap + re-wrap under device B's
    /// local key), be readable on B via the production read path
    /// (`decrypt_item_by_version`). A and B have DIFFERENT local seeds — the
    /// whole point of the shared sync key.
    #[test]
    fn rekey_round_trip_makes_item_readable_on_peer_with_different_local_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        // Two distinct devices, two distinct local seeds.
        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        assert_ne!(seed_a, seed_b);

        // The shared content sync key both sides persisted at pairing (same 32
        // bytes on both — here a fixed test value).
        let shared = [0x33u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        let item_id = "iid-rekey-001".to_string();
        let plaintext = b"the answer is 42 and it travelled over real P2P";

        // SENDER A: item is stored encrypted under A's v2 local key (exactly as
        // a freshly-captured text item is).
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&copypaste_core::ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        // A's wire item carries A's at-rest ciphertext + nonce.
        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "row-1".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(ct_a),
            content_nonce: Some(nonce_a.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 7,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // A's peers.json holds the shared key (peer = B).
        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // OUTBOUND: re-key under the shared sync key.
        rekey_outbound(&crypto_a, &mut wire);
        assert!(
            wire.content_nonce.is_none(),
            "sync-key-wrapped payload must clear the item nonce (self-framed blob)"
        );
        // The wire content is no longer A's at-rest ciphertext.
        assert_ne!(wire.content.as_deref(), Some(plaintext.as_slice()));

        // RECEIVER B: different local seed, same shared key in its peers.json.
        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        // INBOUND: unwrap with shared key, re-wrap under B's local v2 key.
        let (stored, recovered_pt) =
            rekey_inbound(&crypto_b, wire).expect("B must unwrap the sync-key-wrapped item");
        assert_eq!(
            recovered_pt.as_deref(),
            Some(plaintext.as_slice()),
            "B recovers A's original plaintext"
        );
        assert_eq!(stored.key_version, 2);

        // B's production read path: decrypt the stored row with B's keys.
        let b_v2 = derive_v2(&seed_b);
        let stored_nonce = stored.content_nonce.expect("nonce");
        let mut narr = [0u8; copypaste_core::NONCE_SIZE];
        narr.copy_from_slice(&stored_nonce);
        let read_back = copypaste_core::decrypt_item_by_version(
            stored.key_version,
            copypaste_core::V1Key(&seed_b),
            copypaste_core::V2Key(&b_v2),
            &stored.item_id,
            &narr,
            &stored.content.expect("content"),
        )
        .expect("B read path must decrypt the stored synced item");
        assert_eq!(
            read_back, plaintext,
            "synced item reads back as A's original plaintext on B"
        );
    }

    /// v0.6 image sync: an image stored at rest under device A's local v1 seed
    /// (the chunk-encryption key handle, exactly as `handle_image` uses) must,
    /// after `rekey_outbound` (image arm — reassembles plaintext, re-wraps under
    /// the shared key) → `rekey_inbound` (re-chunks under device B's local key),
    /// decode back to the ORIGINAL PNG bytes on B, with a re-derived
    /// file_id/item_id that converges with A's (deterministic dedup).
    #[test]
    fn image_rekey_round_trip_decodes_back_to_original_png_on_peer() {
        use base64::Engine as _;
        use copypaste_core::{
            chunks_from_blob, chunks_to_blob, decode_image, encode_image_with_limit,
        };
        use tempfile::tempdir;

        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        let shared = [0x33u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        // A real tiny PNG (2x2 RGB). Built via the image crate so encode/decode
        // is exercised end-to-end.
        let png = {
            use image::ImageEncoder;
            let mut buf = Vec::new();
            let raw = vec![0xFFu8; 2 * 2 * 3];
            image::codecs::png::PngEncoder::new(&mut buf)
                .write_image(&raw, 2, 2, image::ExtendedColorType::Rgb8)
                .expect("encode test png");
            buf
        };

        // SENDER A: store the image under A's LOCAL v1 seed (handle_image path).
        let file_id_a = crate::clipboard::image_content_hash(&png);
        let (_meta_a, chunks_a) =
            encode_image_with_limit(&png, &seed_a, &file_id_a, 0, 256).expect("A encode image");
        let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
        let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
        let meta_json_a = format!(r#"{{"file_id":{file_id_a:?}}}"#);

        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "img-row".to_string(),
            item_id: item_id.clone(),
            content_type: "image".to_string(),
            content: Some(blob_a),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: Some(meta_json_a),
            is_sensitive: false,
            lamport_ts: 9,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // OUTBOUND image arm: reassemble + re-wrap under shared key.
        assert_eq!(
            rekey_outbound(&crypto_a, &mut wire),
            RekeyOutcome::Rewrapped
        );
        assert!(wire.content_nonce.is_none(), "wrapped blob clears nonce");
        assert!(wire.blob_ref.is_none(), "blob_ref dropped on the wire");
        assert_eq!(wire.content_type, "image");

        // RECEIVER B: different local seed, same shared key.
        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced image");
        assert!(fts.is_none(), "images carry no FTS plaintext");
        assert_eq!(stored.content_type, "image");
        assert_eq!(
            stored.item_id, item_id,
            "item_id must converge across devices"
        );

        // B decodes its re-chunked blob under B's local seed → original PNG.
        let meta_b = stored.blob_ref.expect("B meta json");
        let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
        assert_eq!(file_id_b, file_id_a, "file_id must converge");
        let b_chunks =
            chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
        let recovered = decode_image(&b_chunks, &seed_b, &file_id_b).expect("B decode image");
        assert_eq!(recovered, png, "B recovers A's exact PNG bytes");
    }

    /// v0.6 file sync: same as the image round-trip but for an arbitrary file
    /// blob (`content_type = "file"`), chunked verbatim (no decode/re-encode).
    #[test]
    fn file_rekey_round_trip_decodes_back_to_original_bytes_on_peer() {
        use base64::Engine as _;
        use copypaste_core::{chunks_from_blob, chunks_to_blob, decode_file, encode_file};
        use tempfile::tempdir;

        let seed_a = [0x44u8; 32];
        let seed_b = [0x55u8; 32];
        let shared = [0x66u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        let raw = b"arbitrary file bytes \x00\x01\x02 over P2P sync".to_vec();

        let file_id_a = crate::clipboard::image_content_hash(&raw);
        let (meta_a, chunks_a) = encode_file(
            &raw,
            "notes.bin",
            "application/octet-stream",
            &seed_a,
            &file_id_a,
            0,
        )
        .expect("A encode file");
        let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
        let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
        let meta_json_a = crate::clipboard::build_file_meta_json(&meta_a);

        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "file-row".to_string(),
            item_id: item_id.clone(),
            content_type: "file".to_string(),
            content: Some(blob_a),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: Some(meta_json_a),
            is_sensitive: false,
            lamport_ts: 11,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        assert_eq!(
            rekey_outbound(&crypto_a, &mut wire),
            RekeyOutcome::Rewrapped
        );
        assert!(wire.content_nonce.is_none());
        assert!(
            wire.blob_ref.is_none(),
            "blob_ref must be cleared on the wire"
        );
        assert_eq!(wire.content_type, "file");
        // #21b: filename + mime must be stamped on the wire before blob_ref is cleared.
        assert_eq!(
            wire.file_name.as_deref(),
            Some("notes.bin"),
            "rekey_blob_outbound must stamp file_name onto the wire"
        );
        assert_eq!(
            wire.mime.as_deref(),
            Some("application/octet-stream"),
            "rekey_blob_outbound must stamp mime onto the wire"
        );

        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced file");
        assert!(fts.is_none());
        assert_eq!(stored.content_type, "file");
        assert_eq!(stored.item_id, item_id, "item_id converges");

        let meta_b = stored.blob_ref.expect("B meta json");
        // #21b: verify that filename + mime survive the full rekey round-trip
        // (outbound stamps wire fields; inbound reads them to rebuild file meta).
        let (recovered_name, recovered_mime) =
            parse_file_name_mime(&meta_b).expect("B meta must carry filename+mime");
        assert_eq!(
            recovered_name, "notes.bin",
            "filename must survive outbound→wire→inbound"
        );
        assert_eq!(
            recovered_mime, "application/octet-stream",
            "mime must survive outbound→wire→inbound"
        );
        let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
        assert_eq!(file_id_b, file_id_a);
        let b_chunks =
            chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
        let recovered = decode_file(&b_chunks, &seed_b, &file_id_b).expect("B decode file");
        assert_eq!(recovered, raw, "B recovers A's exact file bytes");
    }

    /// CopyPaste-716: 3-device topology (A paired with B and C under DIFFERENT
    /// pairwise keys) must produce per-peer ciphertext blobs.
    ///
    /// Before the fix: `rekey_outbound` used `shared_sync_key()` (first peer
    /// only) so fanout to C produced a K_AB-encrypted blob that C could never
    /// decrypt. After the fix: `rekey_outbound_for_peer` uses the per-peer key
    /// from the HashMap cache, so each peer gets a blob it can actually decrypt.
    ///
    /// This test simulates device A sending to peers B and C:
    /// - K_AB = [0x33; 32], K_AC = [0x44; 32] (distinct pairwise keys)
    /// - Fanout to B must produce a blob decryptable under K_AB (not K_AC)
    /// - Fanout to C must produce a blob decryptable under K_AC (not K_AB)
    #[test]
    fn three_device_fanout_uses_per_peer_key_not_first_peer_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, decrypt_from_cloud, derive_v2, encrypt_item_with_aad,
            AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        // Device A's local seed.
        let seed_a = [0x11u8; 32];
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        assert_ne!(k_ab, k_ac, "pairwise keys must be distinct");
        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);
        let fp_b = "bb:bb";
        let fp_c = "cc:cc";

        // Device A's peers.json: two peers with different pairwise keys.
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

        // Build a wire item (A's at-rest ciphertext).
        let item_id = "fanout-716-item".to_string();
        let plaintext = b"the shared secret content for 3-device test";
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&copypaste_core::ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        let wire_template = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "row-716".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(ct_a),
            content_nonce: Some(nonce_a.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // ── Fanout to peer B (should use K_AB) ───────────────────────────────
        let mut wire_for_b = wire_template.clone();
        let outcome_b = rekey_outbound_for_peer(&crypto_a, fp_b, &mut wire_for_b);
        assert_eq!(
            outcome_b,
            RekeyOutcome::Rewrapped,
            "fanout to B must succeed (K_AB present)"
        );
        assert!(
            wire_for_b.content_nonce.is_none(),
            "sync-key-wrapped payload clears item nonce"
        );
        let blob_b = wire_for_b.content.as_ref().unwrap().clone();

        // ── Fanout to peer C (should use K_AC) ───────────────────────────────
        let mut wire_for_c = wire_template.clone();
        let outcome_c = rekey_outbound_for_peer(&crypto_a, fp_c, &mut wire_for_c);
        assert_eq!(
            outcome_c,
            RekeyOutcome::Rewrapped,
            "fanout to C must succeed (K_AC present)"
        );
        let blob_c = wire_for_c.content.as_ref().unwrap().clone();

        // ── Verify: B's blob decrypts under K_AB ─────────────────────────────
        let key_b = copypaste_core::SyncKey::from_bytes(k_ab);
        let decrypted_b =
            decrypt_from_cloud(&key_b, &item_id, &blob_b).expect("blob_b must decrypt under K_AB");
        assert_eq!(
            decrypted_b, plaintext,
            "B recovers A's original plaintext from its blob"
        );

        // ── Verify: C's blob decrypts under K_AC ─────────────────────────────
        let key_c = copypaste_core::SyncKey::from_bytes(k_ac);
        let decrypted_c =
            decrypt_from_cloud(&key_c, &item_id, &blob_c).expect("blob_c must decrypt under K_AC");
        assert_eq!(
            decrypted_c, plaintext,
            "C recovers A's original plaintext from its blob"
        );

        // ── Key isolation: B's blob must NOT decrypt under K_AC ──────────────
        // (This is what was silently broken before the fix: fanout used K_AB for
        // all peers, so C received blob_b which it cannot decrypt.)
        let result_wrong = decrypt_from_cloud(&key_c, &item_id, &blob_b);
        assert!(
            result_wrong.is_err(),
            "blob_b (encrypted under K_AB) must NOT decrypt under K_AC — \
             this would be the CopyPaste-716 bug if it succeeded"
        );

        // ── Key isolation: C's blob must NOT decrypt under K_AB ──────────────
        let result_wrong2 = decrypt_from_cloud(&key_b, &item_id, &blob_c);
        assert!(
            result_wrong2.is_err(),
            "blob_c (encrypted under K_AC) must NOT decrypt under K_AB"
        );

        // ── Blobs differ (per-peer encryption with fresh nonces) ─────────────
        assert_ne!(
            blob_b, blob_c,
            "each peer must receive a distinct (independently encrypted) blob"
        );
    }

    /// CopyPaste-716: catchup_items must use the connecting peer's pairwise
    /// key, not the first peer's key. With 2+ peers, the catch-up set for peer
    /// C must decrypt under K_AC only, not K_AB.
    ///
    /// This is the equivalent catch-up path test for the fanout fix above.
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
        let db = make_db();
        let item_id = "catchup-716-item".to_string();
        let plaintext = b"catchup per-peer key test";
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&copypaste_core::ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        let mut local = copypaste_core::ClipboardItem::new_text(ct_a, nonce_a.to_vec(), 1);
        local.item_id = item_id.clone().into();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let db_guard = db.lock().await;
        // Catch-up for peer B: items must be encrypted under K_AB.
        let items_for_b = catchup_items(&db_guard, "device-A", &crypto_a, fp_b);
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
        let items_for_c = catchup_items(&db_guard, "device-A", &crypto_a, fp_c);
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

    /// W2.2: an incoming WireItem from the transport must be persisted to the
    /// local DB via the LWW merge path.
    #[tokio::test]
    async fn sync_orch_inserts_incoming_wire_item() {
        let db = make_db();

        let (_local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, _outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push one wire item from the "transport".
        let wire = make_wire("new-item", 5, 0xAB);
        incoming_tx.send(wire).await.expect("send incoming");

        // Let the orchestrator merge.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Drop senders so the orchestrator exits.
        drop(incoming_tx);
        drop(_local_tx);
        handle.await.expect("task join");

        let db_guard = db.lock().await;
        let rows = copypaste_core::get_page(&*db_guard, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 1, "incoming item must be persisted");
        assert_eq!(rows[0].id, "new-item");
        assert!(rows[0].is_synced, "item from peer must be marked synced");
        assert_eq!(rows[0].lamport_ts, 5);
    }

    /// W2.2: a locally-produced item arriving on the broadcast channel must
    /// be forwarded to the transport's outbound channel.
    #[tokio::test]
    async fn sync_orch_broadcasts_local_item() {
        let db = make_db();

        let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (_incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push a local item through the broadcast channel.
        let item = ClipboardItem::new_text(vec![0xCC, 0xDD], vec![0u8; 24], 9);
        let item_id = item.id.clone();
        local_tx.send(item).expect("broadcast send");

        // Receive on the transport side.
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(200), outbound_rx.recv())
                .await
                .expect("must receive within 200ms")
                .expect("outbound channel must yield item");

        assert_eq!(received.id, item_id, "wire id must match local id");
        assert_eq!(
            received.origin_device_id, "local-device",
            "origin_device_id must be stamped by the orchestrator"
        );
        assert_eq!(received.lamport_ts, 9);

        // Tear down and join.
        drop(local_tx);
        drop(_incoming_tx);
        handle.await.expect("task join");
    }

    /// Op-propagation (v0.6.1): the wire now carries authoritative `pinned` /
    /// `pin_order`, so on LWW TakeRemote the winning wire's pin state is applied
    /// directly — this is what lets an explicit pin / UNPIN / reorder on one
    /// device converge onto the others. (The previous semantic OR-merged the
    /// local pin and was kept only because the wire had no pin field; that
    /// would silently swallow a remote unpin, which the "sync all operations"
    /// requirement forbids.)
    ///
    /// Here a locally-pinned row receives a newer remote that is unpinned →
    /// the row must become unpinned (remote unpin propagated). The lookup uses
    /// `wire.id`, so the local row's `id` must equal `wire.id` for TakeRemote.
    #[tokio::test]
    async fn merge_incoming_takeremote_propagates_wire_pin_state() {
        let db = make_db();
        // Local row pinned=true; lamport 3 < wire lamport 9 → TakeRemote fires.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 3);
        local.id = "shared-id".to_string().into();
        local.item_id = "shared-id-iid".to_string().into();
        local.pinned = true;
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
            let stored = copypaste_core::get_item_by_id(&*g, "shared-id")
                .unwrap()
                .unwrap();
            assert!(stored.pinned, "setup: local row must be pinned");
        }

        // Newer remote, unpinned (make_wire sets pinned=false, pin_order=None).
        let wire = make_wire("shared-id", 9, 0xFF);
        assert_eq!(wire.id, "shared-id");
        assert!(!wire.pinned, "setup: incoming wire is unpinned");

        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 1, "newer remote must win LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1, "must remain ONE row");
        assert!(
            !rows[0].pinned,
            "remote unpin must propagate on TakeRemote — got pinned={}",
            rows[0].pinned
        );
    }

    /// LWW: a stale wire item (lower lamport) must NOT overwrite the local row.
    /// Identity is matched on the cross-device `item_id`, so the local row and
    /// the wire item share `item_id = "shared-iid"` (the local `id` is distinct,
    /// as it always is across devices).
    #[tokio::test]
    async fn merge_incoming_keeps_local_on_older_remote() {
        let db = make_db();
        // Pre-insert a local row with a higher lamport clock. Its `item_id`
        // matches the incoming wire's so they are recognised as the SAME item.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 50);
        local.id = "shared".to_string().into();
        local.item_id = "shared-iid".to_string().into();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let wire = make_wire("shared", 5, 0xFF); // older; item_id = "shared-iid"
        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 0, "older remote must lose LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, Some(vec![0x11]), "local payload preserved");
    }

    /// CopyPaste-bfiu: a P2P tombstone for an UNKNOWN item_id (delete arrives
    /// before the create, out-of-order over P2P) must persist a tombstone row so
    /// a later lower-lamport create is LWW-rejected and the item stays deleted —
    /// instead of the old behaviour that silently skipped the tombstone and let
    /// the create resurrect the item.
    #[tokio::test]
    async fn merge_incoming_delete_before_create_does_not_resurrect() {
        let db = make_db();

        // 1) DELETE arrives first for an item we have never seen (lamport 20).
        let mut tomb = make_wire("ghost", 20, 0x00);
        tomb.item_id = "ghost-iid".to_string();
        tomb.deleted = true;
        tomb.content = None;
        tomb.content_nonce = None;
        let upserted = merge_incoming(&db, vec![tomb]).await.unwrap();
        assert_eq!(upserted, 1, "tombstone for unknown item must be persisted");

        {
            let g = db.lock().await;
            let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
                .unwrap()
                .expect("tombstone row must exist");
            assert!(
                row.deleted,
                "unknown-item tombstone must persist as deleted"
            );
            // The user-facing list must not show it.
            assert!(
                copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
                "tombstone must not appear in the history list"
            );
        }

        // 2) CREATE arrives later with a LOWER lamport (10 < 20) → loses LWW.
        let mut create = make_wire("ghost", 10, 0xAB);
        create.item_id = "ghost-iid".to_string();
        let upserted2 = merge_incoming(&db, vec![create]).await.unwrap();
        assert_eq!(upserted2, 0, "late lower-lamport create must NOT resurrect");

        let g = db.lock().await;
        let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
            .unwrap()
            .expect("row still present");
        assert!(
            row.deleted,
            "item must stay deleted after the racing create"
        );
        assert!(
            copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
            "item must remain hidden from history"
        );
    }

    /// CRDT identity + local-PK preservation: a TakeRemote (newer lamport) for
    /// an item already present locally under a DIFFERENT row `id` must replace
    /// the content in place while preserving the local primary key — so FTS,
    /// `copy_item`, and pins (all keyed on `id`) keep pointing at the same row.
    #[tokio::test]
    async fn merge_incoming_replaces_by_item_id_preserving_local_pk() {
        let db = make_db();
        // Local row: PK "local-pk", item_id "X", lamport 5.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 5);
        local.id = "local-pk".to_string().into();
        local.item_id = "X".to_string().into();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        // Incoming wire: peer's own PK "peer-pk", SAME item_id "X", newer
        // lamport 9, different content.
        let mut wire = make_wire("peer-pk", 9, 0xFF);
        wire.item_id = "X".to_string();

        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 1, "newer remote must win LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "must remain ONE row (replace, not duplicate)"
        );
        assert_eq!(
            rows[0].id, "local-pk",
            "local primary key must be preserved"
        );
        assert_eq!(rows[0].item_id, "X");
        assert_eq!(rows[0].lamport_ts, 9, "remote (newer) lamport stored");
        assert_eq!(rows[0].content, Some(vec![0xFF]), "remote content stored");
        // The peer's row id must NOT have been adopted.
        assert!(
            copypaste_core::get_item_by_id(&*g, "peer-pk")
                .unwrap()
                .is_none(),
            "peer's row id must not leak into local storage"
        );
    }

    /// CopyPaste-kw2: 3-device topology — inbound item from peer C (encrypted
    /// under K_AC) must be decrypted correctly even when K_AB is the first
    /// entry in the peer key map.
    ///
    /// Before the fix: `rekey_inbound` called `shared_sync_key()` which returned
    /// `values().next()` — an arbitrary HashMap entry.  If K_AB was first, the
    /// item from C (encrypted under K_AC) failed to decrypt and was silently
    /// dropped.
    ///
    /// After the fix: `all_sync_keys()` returns all keys; `rekey_inbound` tries
    /// each until AEAD succeeds, so K_AC is always found regardless of iteration
    /// order.
    #[test]
    fn rekey_inbound_3_device_tries_all_keys() {
        use base64::Engine as _;
        use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud};
        use tempfile::tempdir;

        // Distinct pairwise keys: K_AB and K_AC.
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        assert_ne!(k_ab, k_ac);
        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);

        let fp_b = "bb:bb";
        let fp_c = "cc:cc";
        let seed_b_recv = [0x22u8; 32]; // Device B's local seed (the receiver in this test)

        // Device B's peers.json: both A and C, each with their own pairwise key.
        // B shares K_AB with A and K_BC with C — but for this test B receives
        // a blob from C encrypted under K_AC (the key A and C share).
        // We simulate a direct A→B case: B holds K_AB (to reach A) and K_AC (wrong).
        // The payload we produce is encrypted under K_AC and B should find it by
        // trying all keys.
        //
        // To make the scenario concrete: pretend this device IS A and receives a
        // blob from C (encrypted under K_AC).  A's peer map has K_AB (for B) first
        // and K_AC (for C) second.  The fix ensures K_AC is tried.
        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        std::fs::write(
            &peers_path,
            format!(
                r#"[
                    {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                    {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
                ]"#
            ),
        )
        .unwrap();
        let crypto = SyncCrypto::new(seed_b_recv, peers_path);

        // Build a wire item encrypted under K_AC (as if sent by peer C).
        let item_id = "kw2-test-item".to_string();
        let plaintext = b"secret payload from peer C";
        let key_ac = copypaste_core::SyncKey::from_bytes(k_ac);
        let blob_ac = encrypt_for_cloud(&key_ac, &item_id, plaintext).expect("encrypt under K_AC");

        let wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kw2-row".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(blob_ac),
            content_nonce: None, // sync-key-wrapped (no local nonce)
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-C".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // rekey_inbound must succeed even if K_AB is iterated before K_AC.
        let (stored, fts_plaintext) =
            rekey_inbound(&crypto, wire).expect("must decrypt under K_AC (CopyPaste-kw2 fix)");

        assert_eq!(
            fts_plaintext.as_deref(),
            Some(plaintext.as_slice()),
            "recovered plaintext must match original"
        );
        // The stored row must be re-encrypted under this device's v2 key.
        assert!(
            stored.content_nonce.is_some(),
            "stored row must have a local nonce after rekey"
        );
        assert_eq!(stored.key_version, 2, "stored row must be keyed at v2");

        // Sanity: the wrong key (K_AB) alone would have failed.
        let key_ab = copypaste_core::SyncKey::from_bytes(k_ab);
        let blob_from_stored = stored.content.as_ref().unwrap();
        // The stored ciphertext is under the device's v2 key, not K_AB — just
        // confirm K_AB cannot decrypt the original blob (key isolation).
        // We reconstruct the original blob to check isolation.
        let original_blob = encrypt_for_cloud(&key_ac, &item_id, plaintext).unwrap();
        assert!(
            decrypt_from_cloud(&key_ab, &item_id, &original_blob).is_err(),
            "K_AB must not decrypt a blob encrypted under K_AC — key isolation"
        );
        let _ = blob_from_stored; // silence unused warning
    }

    /// CopyPaste-kcf: inbound items must have `is_sensitive` set from the
    /// decrypted plaintext, so cross-device sensitive items get the auto-wipe TTL.
    ///
    /// Before the fix: `wire_to_local` always set `is_sensitive = false`, so a
    /// password or API key copied on device A and synced to B was stored with
    /// `is_sensitive = false` on B — bypassing the auto-wipe TTL entirely.
    ///
    /// After the fix: `merge_incoming_with_crypto` runs `is_sensitive_for_autowipe`
    /// on the recovered plaintext and stores the correct value.
    #[tokio::test]
    async fn rekey_inbound_sets_is_sensitive_from_plaintext() {
        use base64::Engine as _;
        use copypaste_core::encrypt_for_cloud;
        use tempfile::tempdir;

        // A single pairwise key between two devices.
        let k_shared: [u8; 32] = [0x55u8; 32];
        let k_shared_b64 = base64::engine::general_purpose::STANDARD.encode(k_shared);
        let fp_a = "aa:aa";
        let seed_b = [0x22u8; 32];

        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        std::fs::write(
            &peers_path,
            format!(
                r#"[{{"fingerprint":"{fp_a}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto = SyncCrypto::new(seed_b, peers_path);

        let db = make_db();
        let key_sync = copypaste_core::SyncKey::from_bytes(k_shared);

        // ── Test 1: sensitive plaintext (AWS key) → is_sensitive = true ──────
        let item_id_sensitive = "kcf-sensitive".to_string();
        // A real AWS access key triggers the detector at confidence 0.99.
        let sensitive_plaintext = "AKIAIOSFODNN7EXAMPLE";
        let blob_sensitive = encrypt_for_cloud(
            &key_sync,
            &item_id_sensitive,
            sensitive_plaintext.as_bytes(),
        )
        .expect("encrypt sensitive");

        let wire_sensitive = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kcf-row-sens".to_string(),
            item_id: item_id_sensitive.clone(),
            content_type: "text".to_string(),
            content: Some(blob_sensitive),
            content_nonce: None, // sync-key-wrapped
            blob_ref: None,
            is_sensitive: false, // sender's flag — must be overridden by receiver
            lamport_ts: 1,
            wall_time: 1_700_000_000_001,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        let quota = copypaste_core::AppConfig::default().storage_quota_bytes as i64;
        merge_incoming_with_crypto(&db, vec![wire_sensitive], Some(&crypto), quota, None)
            .await
            .expect("merge sensitive item");

        {
            let g = db.lock().await;
            let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
            assert_eq!(rows.len(), 1, "sensitive item must be stored");
            assert!(
                rows[0].is_sensitive,
                "inbound sensitive item must have is_sensitive=true (CopyPaste-kcf fix); got false"
            );
        }

        // ── Test 2: non-sensitive plaintext → is_sensitive = false ───────────
        let item_id_plain = "kcf-plain".to_string();
        let plain_text = "hello world, nothing secret here";
        let blob_plain = encrypt_for_cloud(&key_sync, &item_id_plain, plain_text.as_bytes())
            .expect("encrypt plain");

        let wire_plain = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kcf-row-plain".to_string(),
            item_id: item_id_plain.clone(),
            content_type: "text".to_string(),
            content: Some(blob_plain),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 2,
            wall_time: 1_700_000_000_002,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        merge_incoming_with_crypto(&db, vec![wire_plain], Some(&crypto), quota, None)
            .await
            .expect("merge plain item");

        {
            let g = db.lock().await;
            let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
            assert_eq!(rows.len(), 2, "both items must be stored");
            // The plain item should have is_sensitive=false.
            let plain_row = rows
                .iter()
                .find(|r| r.item_id == item_id_plain)
                .expect("plain item must be in DB");
            assert!(
                !plain_row.is_sensitive,
                "non-sensitive inbound item must have is_sensitive=false"
            );
        }
    }

    /// e5oe: sweep_poison_rows must also delete the matching clipboard_fts row(s)
    /// in the same transaction so no orphaned searchable plaintext accumulates.
    #[test]
    fn sweep_poison_rows_removes_fts_orphan() {
        use copypaste_core::{insert_item_with_fts, Database};

        let db = Database::open_in_memory().expect("in-memory DB");

        // Insert a text item WITH an FTS row, then corrupt it into a poison row
        // by clearing content_nonce (simulates a sync-key-wrapped blob that was
        // stored before the nonce was applied — exactly the pattern sweep detects).
        let item = copypaste_core::ClipboardItem {
            id: "poison-id".to_string().into(),
            item_id: "poison-item-id".to_string().into(),
            content_type: "text".to_string(),
            content: Some(b"ciphertext without nonce".to_vec()),
            content_nonce: Some(vec![0u8; 24]), // valid on insert …
            blob_ref: None,
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "remote".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        insert_item_with_fts(&db, &item, "secret content text").expect("insert");

        // Manually NULL-out the nonce to turn this into a poison row.
        db.conn()
            .execute(
                "UPDATE clipboard_items SET content_nonce = NULL WHERE id = ?1",
                rusqlite::params!["poison-id"],
            )
            .expect("corrupt nonce");

        // FTS row must be present before the sweep.
        let fts_before: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                rusqlite::params!["poison-id"],
                |r| r.get(0),
            )
            .expect("fts count before");
        assert_eq!(fts_before, 1, "FTS row must exist before sweep");

        let swept = sweep_poison_rows(&db).expect("sweep");
        assert_eq!(swept, 1, "exactly one poison row must be swept");

        // After the sweep the FTS row must also be gone (e5oe fix).
        let fts_after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                rusqlite::params!["poison-id"],
                |r| r.get(0),
            )
            .expect("fts count after");
        assert_eq!(
            fts_after, 0,
            "sweep_poison_rows must delete the FTS row to prevent orphaned searchable plaintext (e5oe)"
        );
    }
}
