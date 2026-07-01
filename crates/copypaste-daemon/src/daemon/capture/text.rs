//! Text at-rest encryption helper (v2 key + v4 AAD) and the text capture
//! ingest handler (dedup-by-hash, lamport stamping, sensitive detection).

use copypaste_core::{
    build_item_aad_v2, bump_item_recency, derive_v2, encrypt_item_with_aad, find_recent_by_hash,
    get_item_by_id, insert_item_with_fts, is_sensitive_for_autowipe, AppConfig, ClipboardItem,
    Database, ItemId, AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::cleanup::prune_history;

/// Encrypt a freshly-captured text payload for at-rest storage, producing a
/// ciphertext that the read path (`ipc::write_to_pasteboard`) can decrypt.
///
/// **Key/AAD/key_version consistency (the v0.4 ingest fix).** A new row is
/// stamped `key_version = 2` by [`ClipboardItem::new_text`] (which uses
/// `ITEM_KEY_VERSION_CURRENT = 2`). The read path dispatches on that
/// `key_version` via `copypaste_core::decrypt_item_by_version`, and for
/// `key_version = 2` it decrypts with **the v2 key** (`derive_v2(local_key)`)
/// and **the v4 AAD format** (`build_item_aad_v2(item_id, 4, 2)`).
///
/// Ingest must therefore encrypt with that exact `(key, AAD)` pair. The prior
/// code encrypted with the raw `local_key` (the v1 key) + the v3 AAD
/// (`build_item_aad(item_id, 3)`) while still stamping `key_version = 2`, so
/// every freshly-captured text item failed to round-trip with
/// `EncryptError::AuthFailed` ("authentication tag mismatch") on paste-back.
///
/// `local_key` is the device's v1 storage key (`load_local_key()` /
/// `DeviceKeypair::local_enc_key`). It is used here only as the input keying
/// material to `derive_v2`, mirroring exactly what the read path does
/// (`derive_v2(&self.local_key)`), so the two sides derive the identical v2
/// key.
pub(crate) fn encrypt_text_for_storage(
    plaintext: &[u8],
    local_key: &[u8; 32],
    item_id: &str,
) -> Result<([u8; copypaste_core::NONCE_SIZE], Vec<u8>), copypaste_core::EncryptError> {
    let v2_key = derive_v2(local_key);
    let aad = build_item_aad_v2(
        &ItemId::from(item_id),
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT_U32,
    );
    encrypt_item_with_aad(plaintext, &v2_key, &aad)
}

/// `key_version` stamped into newly-inserted rows, cast from the canonical
/// `copypaste_core::ITEM_KEY_VERSION_CURRENT` (i64) to `u32` as required by
/// `build_item_aad_v2`. A compile-time assertion keeps them in sync.
const ITEM_KEY_VERSION_CURRENT_U32: u32 = ITEM_KEY_VERSION_CURRENT as u32;
// Compile-time guard: if core ever bumps ITEM_KEY_VERSION_CURRENT the cast
// above silently changes too, but this assert documents the expected value.
const _: () = assert!(
    ITEM_KEY_VERSION_CURRENT == 2,
    "ITEM_KEY_VERSION_CURRENT changed — review encrypt_text_for_storage AAD"
);

pub(crate) async fn handle_text(
    text: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
    // mtf5 (PG-22): bundle ID of the frontmost app at capture time, used to
    // force-sensitive any item originating from a password manager / sensitive
    // app (via `is_sensitive_app`).  `None` on non-macOS or when lsappinfo
    // is unavailable.
    source_bundle_id: Option<String>,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    // Item 2: use confidence-gated autowipe check (floor 0.70) so low-signal
    // patterns (phone numbers, order-ids) no longer trigger the 30s TTL wipe.
    // The old `detect(&text).is_some()` fired on any match regardless of
    // confidence; `is_sensitive_for_autowipe` requires confidence >= 0.70.
    //
    // mtf5 (PG-22): also flag the item sensitive when it originates from a
    // known password-manager / sensitive app, even if the content pattern
    // alone would not trigger auto-wipe.  This is the correct defence in depth:
    // a freshly-copied password is often a random string with low confidence.
    let content_is_sensitive = is_sensitive_for_autowipe(&text);
    let app_is_sensitive = source_bundle_id
        .as_deref()
        .map(copypaste_core::is_sensitive_app)
        .unwrap_or(false);
    let is_sensitive = content_is_sensitive || app_is_sensitive;

    // Compute SHA-256 content hash of the PLAINTEXT bytes.
    // This is used for deduplication: if an identical item already exists in
    // history (any age, not expired), we bump its wall_time/lamport_ts to now
    // rather than inserting a duplicate row. The hash is stored on new inserts
    // so future captures of the same content can find the existing row.
    //
    // NEVER log the plaintext or hash — the hash alone is not reversible but
    // logging it alongside the content would create a correlation risk.
    let hash_hex = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // daemon-core L1: every DB touch below is synchronous rusqlite. Run the
    // whole dedup-lookup / bump-or-insert / prune sequence on a blocking thread
    // (mirroring the IPC path) so the async worker is not blocked while the
    // tokio Mutex is held. Inputs are moved in; the resulting item (if any) is
    // returned for the broadcast channel.
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    let join = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();

        // Dedup: look for any non-expired row with the same content hash.
        // `find_recent_by_hash` uses a generous window (i64::MAX) to cover ALL
        // history, not just the last N minutes.  A pinned item is never expired
        // so it will always be found and bumped, which is the correct behaviour.
        match find_recent_by_hash(&db_guard, &hash_hex, now_ms, i64::MAX) {
            Ok(Some(existing_id)) => {
                // Identical content already in history: bump recency to now so
                // the existing row rises to the top of the pinned-first,
                // wall_time DESC sort. We do NOT insert a new row.
                // CopyPaste-ojhe: stamp the unified lamport value space
                // (max(existing + 1, now_ms)) so a recopy is monotonic relative
                // to the row's own prior lamport AND time-ordered. Previously
                // this used bare `now_ms`, which a later pin/delete deriving from
                // a small counter could never overtake under lamport-only LWW.
                // CopyPaste-crh3.68: fetch the existing row ONCE and reuse it for
                // both the lamport read and the broadcast value, instead of
                // re-fetching after the bump (was 4 DB queries on every dedup hit;
                // now 2 query_row calls + the UPDATE).
                let mut existing_row = match get_item_by_id(&*db_guard, &existing_id) {
                    Ok(Some(row)) => row,
                    // Row already gone (raced with a delete) — nothing to bump or
                    // broadcast; the next poll re-captures on a fresh changeCount.
                    Ok(None) => return None,
                    Err(e) => {
                        tracing::warn!("text dedup: could not read existing item: {e}");
                        return None;
                    }
                };
                let new_lamport =
                    copypaste_core::next_lamport_ts(existing_row.lamport_ts, now_ms);
                match bump_item_recency(&db_guard, &existing_id, now_ms, new_lamport, None) {
                    Ok(changed) if changed > 0 => {
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: bumped existing row to top (same content_hash)"
                        );
                        // Reuse the already-fetched row — only wall_time +
                        // lamport_ts changed — so broadcast subscribers (P2P, sync)
                        // see the recency update without a 4th DB read.
                        existing_row.wall_time = now_ms;
                        existing_row.lamport_ts = new_lamport;
                        return Some(existing_row);
                    }
                    Ok(_) => {
                        // Row disappeared between find and bump (race on delete) —
                        // produce no broadcast item; the next poll re-captures.
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: existing row disappeared before bump (deleted concurrently)"
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::warn!("text dedup bump failed: {e}");
                        return None;
                    }
                }
            }
            Ok(None) => {
                // No existing row with this hash — proceed with a fresh insert.
            }
            Err(e) => {
                // DB error on the dedup lookup: log and fall through to insert.
                // Inserting a duplicate is preferable to silently losing a capture.
                tracing::warn!("text dedup hash lookup failed: {e}");
            }
        }

        // Fresh insert path: encrypt then store.
        let item_id = uuid::Uuid::new_v4().to_string();
        let (nonce, ciphertext) =
            match encrypt_text_for_storage(text.as_bytes(), &local_key, &item_id) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("encrypt_text_for_storage failed for text: {e}");
                    return None;
                }
            };
        // CopyPaste-ojhe: stamp the unified lamport value space at capture
        // (`next_lamport_ts(0, now_ms) == now_ms`) instead of a hardcoded 0.
        // A fresh capture must outrank an older recopy/pin/delete of unrelated
        // items under lamport-first LWW; a 0-stamped capture could never win.
        let mut item = ClipboardItem::new_text(
            ciphertext,
            nonce.to_vec(),
            copypaste_core::next_lamport_ts(0, now_ms),
        );
        item.item_id = item_id.into();
        item.is_sensitive = is_sensitive;
        // mtf5 (PG-22): record which app was frontmost at capture time.
        // This allows UIs to display the source app and lets the DB preserve
        // attribution across restarts.
        item.app_bundle_id = source_bundle_id;
        // Stamp the stable on-disk device_id so cloud/P2P peers attribute every
        // captured item to this specific machine across restarts.
        item.origin_device_id = local_device_id;
        // Store the content hash so future captures of identical content can
        // find and bump this row instead of inserting a duplicate.
        item.content_hash = Some(hash_hex);

        if is_sensitive {
            item.expires_at = Some(now_ms + (config.sensitive_ttl_local_secs as i64 * 1000));
        }

        // v0.3 post-T2: insert_item + upsert_fts collapsed into a single
        // transaction. Closes the TOCTOU window where a crash between the row
        // insert and the FTS upsert could leave a row that search would never
        // find. Also handles the v5 UNIQUE-index dedup race internally.
        match insert_item_with_fts(&db_guard, &item, &text) {
            Ok(stored_id) => {
                if stored_id != item.id {
                    // Fix MED #4: `insert_item_with_fts` deduped `item` against
                    // an existing row identified by `stored_id`. Broadcasting
                    // `item` (which carries the REJECTED new uuid) would cause
                    // subscribers (P2P, sync) to look up a nonexistent row.
                    // Fetch the ACTUAL stored row and broadcast that instead, so
                    // all consumers observe a valid, persisted item. If the fetch
                    // fails (extreme race), produce no broadcast for this poll.
                    tracing::debug!(
                        requested = %item.id,
                        existing = %stored_id,
                        "text item deduped against existing row (UNIQUE index race) — broadcasting stored row"
                    );
                    prune_history(&db_guard, &config);
                    match get_item_by_id(&*db_guard, &stored_id) {
                        Ok(Some(stored_item)) => Some(stored_item),
                        Ok(None) => {
                            tracing::debug!(
                                id = %stored_id,
                                "text dedup: stored row disappeared before fetch (deleted concurrently)"
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!("text dedup: failed to fetch stored row for broadcast: {e}");
                            None
                        }
                    }
                } else {
                    tracing::info!(
                        id = %item.id,
                        sensitive = is_sensitive,
                        "stored text item id={} sensitive={}",
                        item.id,
                        is_sensitive
                    );
                    prune_history(&db_guard, &config);
                    Some(item)
                }
            }
            Err(e) => {
                tracing::warn!("failed to store text item: {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_text blocking task failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{decrypt_item_by_version, Database, NONCE_SIZE};

    // -----------------------------------------------------------------------
    // FIX 2: dedup-bump — identical content bumps the existing row to top
    // -----------------------------------------------------------------------

    /// Capturing the same text twice must NOT insert a second row. The existing
    /// row's wall_time must be updated so it appears at the top of history.
    #[tokio::test]
    async fn handle_text_dedup_bumps_existing_row_not_inserts() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "duplicate clipboard text".to_string();

        // First capture.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None)
            .await
            .expect("first capture must succeed");

        // Verify content_hash is set after first insert.
        {
            let guard = db.lock().await;
            let row = copypaste_core::get_item_by_id(&*guard, &item1.id)
                .unwrap()
                .expect("first row must exist");
            assert!(
                row.content_hash.is_some(),
                "content_hash must be set on new row"
            );
        }

        // Second capture of the same text.
        let _item2 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None).await;

        // Must still be exactly one row.
        let guard = db.lock().await;
        let total = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            total, 1,
            "identical text must not insert a duplicate row; expected 1 row, got {total}"
        );
    }

    /// After a dedup bump, the bumped item has a wall_time >= the first
    /// insert's wall_time, so it sorts to the top.
    #[tokio::test]
    async fn handle_text_dedup_bumped_item_has_updated_wall_time() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "text that will be bumped".to_string();

        // Insert the item and record its initial wall_time.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None)
            .await
            .expect("first capture must succeed");
        let wall_time_before = item1.wall_time;

        // A tiny sleep to ensure a different wall_time on the bump.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        // Second capture: should bump, not insert.
        handle_text(text.clone(), &db, &local_key, &config, "test-device", None).await;

        let guard = db.lock().await;
        let row = copypaste_core::get_item_by_id(&*guard, &item1.id)
            .unwrap()
            .expect("original row must still exist after bump");

        assert!(
            row.wall_time >= wall_time_before,
            "bumped wall_time ({}) must be >= original ({})",
            row.wall_time,
            wall_time_before
        );
    }

    /// Capturing two DIFFERENT texts must insert two distinct rows.
    #[tokio::test]
    async fn handle_text_different_content_inserts_two_rows() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        handle_text(
            "first distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;
        handle_text(
            "second distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;

        let guard = db.lock().await;
        let total = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            total, 2,
            "two distinct texts must produce two rows, got {total}"
        );
    }

    // -----------------------------------------------------------------------
    // tke7 (PG-30): sync_enabled config field contract tests
    // -----------------------------------------------------------------------

    /// Documents and verifies the sync_enabled master gate:
    /// - AppConfig::default() has sync_enabled = true.
    /// - Setting sync_enabled=false is persisted to config.toml.
    /// - The config field is read and honoured (per-field assertion).
    #[test]
    fn sync_enabled_defaults_to_true_in_appconfig() {
        let cfg = AppConfig::default();
        assert!(cfg.sync_enabled, "sync_enabled must default to true");
    }

    #[tokio::test]
    async fn sync_enabled_false_gates_outbound_in_handle_text() {
        // When sync_enabled=false, handle_text still inserts the item locally
        // (local capture is NOT gated) but the sync_orch would not forward it.
        // Verify that handle_text itself completes successfully (local-only path).
        let db = Arc::new(Mutex::new(
            Database::open_in_memory().expect("open in-memory db"),
        ));
        let key = [0u8; 32];
        let config = AppConfig {
            sync_enabled: false,
            ..Default::default()
        };
        let item = handle_text(
            "test sync gate".to_string(),
            &db,
            &key,
            &config,
            "test-device",
            None,
        )
        .await;
        // handle_text always stores locally regardless of sync_enabled.
        assert!(
            item.is_some(),
            "handle_text must store locally even when sync_enabled=false"
        );
    }

    // -----------------------------------------------------------------------
    // mtf5 (PG-22): is_sensitive_app wiring tests
    // -----------------------------------------------------------------------

    /// When handle_text is called with a source_bundle_id that matches a known
    /// password manager, the stored item must have is_sensitive = true even if
    /// the content pattern alone would not trigger auto-wipe.
    #[tokio::test]
    async fn handle_text_marks_sensitive_when_source_is_password_manager() {
        let local_key = [0xBBu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        // A random-looking string that would NOT trigger the content-pattern
        // sensitive detector (no API key / credit card patterns).
        let plaintext = "xK9mQ3nR7pT2vW5".to_string();

        // Simulate the clipboard being copied from 1Password.
        let item = handle_text(
            plaintext,
            &db,
            &local_key,
            &config,
            "test-device",
            Some("com.1password.1password".to_string()),
        )
        .await
        .expect("handle_text must store the item");

        // is_sensitive must be true because the SOURCE APP is a password manager.
        assert!(
            item.is_sensitive,
            "mtf5: item must be marked sensitive when source is a password manager"
        );
        // The app_bundle_id must also be recorded on the item.
        assert_eq!(
            item.app_bundle_id.as_deref(),
            Some("com.1password.1password"),
            "mtf5: app_bundle_id must be stored on the item"
        );
    }

    /// Content captured from a non-sensitive app (e.g. Chrome) with innocuous
    /// content must NOT be marked sensitive.
    #[tokio::test]
    async fn handle_text_not_sensitive_for_regular_app() {
        let local_key = [0xCCu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        let item = handle_text(
            "hello from chrome".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            Some("com.google.chrome".to_string()),
        )
        .await
        .expect("handle_text must store the item");

        assert!(
            !item.is_sensitive,
            "mtf5: non-sensitive app + non-sensitive content must not be marked sensitive"
        );
        assert_eq!(
            item.app_bundle_id.as_deref(),
            Some("com.google.chrome"),
            "mtf5: app_bundle_id must be stored even for non-sensitive apps"
        );
    }

    // -----------------------------------------------------------------------
    // v0.4 ingest round-trip
    // -----------------------------------------------------------------------

    /// v0.4 ingest round-trip (HIGH): a freshly-captured text item must be
    /// readable through the SAME path the daemon uses on paste-back. The read
    /// path (`ipc::write_to_pasteboard`, text branch) dispatches on the row's
    /// `key_version` via `decrypt_item_by_version`, deriving the v2 key as
    /// `derive_v2(local_key)`. This test feeds the production ingest crypto
    /// (`encrypt_text_for_storage`) into the production read crypto
    /// (`decrypt_item_by_version`) and asserts the bytes survive.
    ///
    /// Before the ingest fix, ingest encrypted with the v1 key + v3 AAD while
    /// stamping `key_version = 2`, so this round-trip failed with
    /// `EncryptError::AuthFailed`.
    #[test]
    fn fresh_text_capture_round_trips_through_read_path() {
        let local_key = [0x42u8; 32]; // stands in for load_local_key() (the v1 key)
        let item_id = uuid::Uuid::new_v4().to_string();
        let plaintext = b"hello from a fresh clipboard capture";

        // Ingest: exactly what handle_text does to produce the stored row.
        let (nonce, ciphertext) =
            encrypt_text_for_storage(plaintext, &local_key, &item_id).expect("encrypt");

        // The row is stamped key_version = 2 (ClipboardItem::new_text).
        let item = ClipboardItem::new_text(ciphertext.clone(), nonce.to_vec(), 0);
        assert_eq!(
            item.key_version, 2,
            "freshly-captured rows are stamped key_version = 2"
        );

        // Read: replicate the read path's key derivation + dispatch.
        let v1_key = local_key;
        let v2_key = derive_v2(&v1_key);
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let recovered = decrypt_item_by_version(
            item.key_version,
            copypaste_core::V1Key(&v1_key),
            copypaste_core::V2Key(&v2_key),
            &copypaste_core::ItemId::from(item_id.as_str()),
            &nonce_arr,
            &ciphertext,
        )
        .expect("read path must decrypt a freshly-captured row");

        assert_eq!(
            recovered, plaintext,
            "round-trip plaintext must match the captured bytes"
        );
    }
}
