use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use copypaste_core::{encrypt_for_cloud, ClipboardItem, SyncKey};

use crate::sync_common::{decrypt_item_plaintext_blocking, wrap_and_check_cloud_upload_plaintext};

use super::queue::enqueue_for_retry;

/// Decrypt a clipboard item's local ciphertext, re-encrypt it under the
/// current cloud sync key, and append the result to the retry queue.
///
/// Returns `true` when the item was enqueued, `false` when it was skipped
/// (sensitive item, no sync key, decrypt error, encrypt error).
///
/// This is extracted from the `push_loop` main `select!` branch so the SAME
/// processing pipeline can be reused by the periodic drain path
/// (CopyPaste-1t38): when the retry queue is non-empty and a new broadcast
/// item arrives during the retry-backoff sleep, we must enqueue it
/// immediately rather than let it age in the broadcast ring buffer.
///
/// # Safety / reentrancy
///
/// The caller holds no locks when calling this function — the function takes
/// and immediately releases the `sync_key` lock twice (once for the fast
/// no-key check, once for re-encryption). The intermediate plaintext is
/// zeroized at the end of `decrypt_item_plaintext_blocking`.
pub(super) async fn prepare_and_enqueue_item(
    item: ClipboardItem,
    sync_key: &Arc<Mutex<Option<SyncKey>>>,
    local_key: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    retry_queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
    warned_no_key: &mut bool,
) -> bool {
    // P1-1: sensitive items are NEVER uploaded.
    if item.is_sensitive {
        tracing::debug!(
            "cloud-sync push_loop: skipping sensitive id={} (never uploaded)",
            item.id
        );
        return false;
    }
    // CopyPaste-e89n: tombstone items (soft-deleted) carry no content —
    // push them directly without decrypt/re-encrypt. The server stores
    // `deleted=true, payload_ct=NULL` so receiving devices apply the deletion.
    if item.deleted {
        enqueue_for_retry(retry_queue, item, None);
        return true;
    }
    // Fast no-key skip: if no sync passphrase is set there is nothing to
    // upload. Drop the guard immediately so the lock is not held across the
    // await below.
    {
        let key_guard = sync_key.lock().await;
        if key_guard.is_none() {
            if !*warned_no_key {
                tracing::warn!(
                    "cloud-sync push_loop: no sync passphrase set — \
                     skipping upload (call set_sync_passphrase first)"
                );
                *warned_no_key = true;
            }
            return false;
        }
    }
    // Decrypt on the blocking pool (CPU-bound, potentially multi-MB).
    let (item_back, decrypt_res) =
        decrypt_item_plaintext_blocking(item, zeroize::Zeroizing::new(***local_key)).await;
    let item = match item_back {
        Some(it) => it,
        None => {
            tracing::warn!("cloud-sync push_loop: decrypt task failed; skipping");
            return false;
        }
    };
    let plaintext = match decrypt_res {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "cloud-sync push_loop: failed to decrypt id={} for re-encryption: {e}; skipping",
                item.id
            );
            return false;
        }
    };
    // Re-encrypt for cloud under the single per-account sync key. The bytes are
    // reconstructed into a SyncKey and dropped (zeroized) at the end of this scope.
    let payload_ct_b64 = {
        let write_key = super::super::snapshot_cloud_key_bytes(sync_key).await;
        match write_key {
            None => {
                if !*warned_no_key {
                    tracing::warn!(
                        "cloud-sync push_loop: no sync passphrase set — \
                         skipping upload (call set_sync_passphrase first)"
                    );
                    *warned_no_key = true;
                }
                return false;
            }
            Some(key_bytes) => {
                let key = SyncKey::from_bytes(key_bytes);
                let cloud_plaintext = match wrap_and_check_cloud_upload_plaintext(&item, plaintext)
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("cloud-sync push_loop: skipping id={}: {e}", item.id);
                        return false;
                    }
                };
                match encrypt_for_cloud(&key, &item.item_id, &cloud_plaintext) {
                    Ok(blob) => {
                        use base64::Engine as _;
                        base64::engine::general_purpose::STANDARD.encode(&blob)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "cloud-sync push_loop: cloud encrypt failed for id={}: {e}; skipping",
                            item.id
                        );
                        return false;
                    }
                }
            }
        }
    };
    *warned_no_key = false;
    enqueue_for_retry(retry_queue, item, Some(payload_ct_b64));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::ClipboardItem;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn text_item(lamport: i64) -> ClipboardItem {
        // new_text defaults is_sensitive=false, deleted=false.
        ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], lamport)
    }

    /// CopyPaste-20yw / P1-1: a SENSITIVE item must never be enqueued for cloud
    /// upload — `prepare_and_enqueue_item` returns false and leaves the retry
    /// queue empty, BEFORE any key/decrypt/network work. This is a real guard
    /// test: the positive control below proves removing the guard would let the
    /// item through (the previous coverage was a tautology elsewhere).
    #[tokio::test]
    async fn cloud_push_skips_sensitive_item() {
        let sync_key = Arc::new(Mutex::new(None));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let mut queue = VecDeque::new();
        let mut warned = false;

        let mut item = text_item(1);
        item.is_sensitive = true;
        // Even a sensitive TOMBSTONE must be skipped (the sensitive guard runs
        // before the deleted fast-path).
        item.deleted = true;

        let enqueued =
            prepare_and_enqueue_item(item, &sync_key, &local_key, &mut queue, &mut warned).await;

        assert!(!enqueued, "sensitive item must not be enqueued");
        assert!(
            queue.is_empty(),
            "sensitive item must not enter the retry queue"
        );
    }

    /// Positive control: a NON-sensitive tombstone (deleted) item is enqueued
    /// directly (no key/crypto needed). This proves the zero above is the
    /// sensitive guard at work, not a broken setup — and that removing the guard
    /// would make the sensitive tombstone above take this same path and fail the
    /// assertion.
    #[tokio::test]
    async fn cloud_push_enqueues_non_sensitive_tombstone() {
        let sync_key = Arc::new(Mutex::new(None));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let mut queue = VecDeque::new();
        let mut warned = false;

        let mut item = text_item(2);
        item.is_sensitive = false;
        item.deleted = true;

        let enqueued =
            prepare_and_enqueue_item(item, &sync_key, &local_key, &mut queue, &mut warned).await;

        assert!(enqueued, "non-sensitive tombstone must be enqueued");
        assert_eq!(queue.len(), 1, "tombstone must enter the retry queue");
    }
}
