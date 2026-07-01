//! Passphrase re-encryption (CopyPaste-vvsf): bulk migration of cloud items
//! to a new encryption key.

use super::client::RestClient;
use super::error::RestResult;
use crate::models::CloudClipboardRow;

impl RestClient {
    /// Re-encrypt all cloud items after a passphrase change.
    ///
    /// # CopyPaste-vvsf — passphrase change re-encryption
    ///
    /// When the user changes their passphrase the encryption key changes. Any
    /// items already uploaded to Supabase are still encrypted under the OLD key
    /// and will become unreadable on other devices (or after a reinstall) that
    /// only know the new key.
    ///
    /// This method:
    /// 1. Fetches all rows visible to the current user.
    /// 2. Applies the caller-supplied `reencrypt` closure to each row's
    ///    `payload_ct`, transforming ciphertext encrypted under the old key to
    ///    ciphertext under the new key.
    /// 3. Upserts each transformed row back to Supabase.
    ///
    /// # Closure contract
    ///
    /// `reencrypt(old_ct: &str) -> Result<String, E>` must:
    /// - Accept the existing `payload_ct` value (PostgREST `\x<hex>` form).
    /// - Return the new ciphertext in the same `\x<hex>` form.
    /// - Return an error to abort processing of that row (the error is logged
    ///   and the row is skipped; processing continues with the remaining rows).
    ///
    /// Tombstone rows (`deleted = true`, `payload_ct = None`) are skipped —
    /// tombstones carry no ciphertext to re-encrypt.
    ///
    /// # Atomicity
    ///
    /// There is no cross-row transaction; re-encryption is opportunistic.
    /// If the process is interrupted, some rows will remain under the old key.
    /// The daemon must call this method again (or the user must re-authenticate)
    /// to complete the migration. The caller is responsible for sequencing with
    /// the regular sync loop (e.g. pausing syncs during re-encryption).
    ///
    /// # Return value
    ///
    /// Returns `(success_count, skip_count, error_count)`:
    /// - `success_count` — rows successfully re-encrypted and upserted.
    /// - `skip_count` — rows skipped (tombstones or rows where `reencrypt`
    ///   returned an error).
    /// - `error_count` — rows where the upsert itself failed.
    pub async fn reencrypt_all_cloud_items<F, E>(
        &self,
        reencrypt: F,
    ) -> RestResult<(usize, usize, usize)>
    where
        // CopyPaste-vvsf: closure receives `(item_id, old_ciphertext_b64)` so the
        // caller can use the item_id as AEAD AAD when decrypting and re-encrypting.
        // Both `encrypt_for_cloud` and `decrypt_from_cloud` require `item_id` as
        // the AAD binding; without it, a round-trip through XChaCha20-Poly1305 with
        // the correct item_id AAD is impossible from inside the closure.
        F: Fn(&str, &str) -> Result<String, E>,
        E: std::fmt::Display,
    {
        let rows = self.list_cloud_items().await?;

        let mut success = 0usize;
        let mut skipped = 0usize;
        let mut errors = 0usize;

        for row in rows {
            // Skip tombstones — no ciphertext to re-encrypt.
            let old_ct = match &row.payload_ct {
                None => {
                    // Tombstone: skip.
                    tracing::debug!(
                        item_id = %row.item_id,
                        "reencrypt_all_cloud_items: skipping tombstone row"
                    );
                    skipped += 1;
                    continue;
                }
                Some(ct) => ct.clone(),
            };

            // Apply the re-encryption closure, passing item_id for AAD binding.
            let new_ct = match reencrypt(&row.item_id, &old_ct) {
                Ok(ct) => ct,
                Err(e) => {
                    tracing::warn!(
                        item_id = %row.item_id,
                        error = %e,
                        "reencrypt_all_cloud_items: closure failed; skipping row"
                    );
                    skipped += 1;
                    continue;
                }
            };

            // Build the updated row (same metadata, new ciphertext).
            let updated = CloudClipboardRow {
                payload_ct: Some(new_ct),
                ..row
            };

            match self.replace_cloud_item_by_item_id(&updated).await {
                Ok(()) => {
                    success += 1;
                }
                Err(e) => {
                    tracing::error!(
                        item_id = %updated.item_id,
                        error = %e,
                        "reencrypt_all_cloud_items: upsert failed"
                    );
                    errors += 1;
                }
            }
        }

        Ok((success, skipped, errors))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::test_support::{live_row, tombstone_row};

    /// `reencrypt_all_cloud_items` must skip tombstone rows (no payload_ct) and
    /// report them in `skip_count`, not `success_count` or `error_count`.
    ///
    /// This test uses mockito to return one live row and one tombstone.
    #[tokio::test]
    #[serial_test::serial]
    async fn reencrypt_skips_tombstones_and_processes_live_rows() {
        use mockito::mock;

        let list_body =
            serde_json::to_string(&vec![live_row(), tombstone_row()]).expect("serialize rows");

        // Mock GET for list_cloud_items.
        let _list_mock = mock("GET", "/rest/v1/clipboard_items?order=lamport_ts.asc")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&list_body)
            .create();

        // Mock POST for the upsert of the live row.
        let _upsert_mock = mock("POST", "/rest/v1/clipboard_items?on_conflict=item_id")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();

        let client = RestClient::new(mockito::server_url(), "anon", "tok");

        let (success, skipped, errors) = client
            .reencrypt_all_cloud_items(|_item_id, old_ct| -> Result<String, String> {
                // Simple "re-encryption": swap hex prefix (item_id available for AAD binding)
                Ok(format!("\\xnew{}", &old_ct[2..]))
            })
            .await
            .expect("reencrypt should succeed");

        assert_eq!(success, 1, "one live row should be re-encrypted");
        assert_eq!(skipped, 1, "one tombstone should be skipped");
        assert_eq!(errors, 0, "no errors expected");
    }

    /// When the re-encryption closure returns an error, the row must be skipped
    /// (counted in `skip_count`), not in `error_count`.
    #[tokio::test]
    #[serial_test::serial]
    async fn reencrypt_skips_rows_where_closure_errors() {
        use mockito::mock;

        let list_body = serde_json::to_string(&vec![live_row()]).expect("serialize rows");

        let _list_mock = mock("GET", "/rest/v1/clipboard_items?order=lamport_ts.asc")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&list_body)
            .create();

        // No upsert mock — it must not be called.

        let client = RestClient::new(mockito::server_url(), "anon", "tok");

        let (success, skipped, errors) = client
            .reencrypt_all_cloud_items(|_item_id, _ct| -> Result<String, String> {
                Err("decryption failed: bad key".into())
            })
            .await
            .expect("reencrypt should not fail at the outer level");

        assert_eq!(success, 0, "closure error must not count as success");
        assert_eq!(skipped, 1, "closure-errored row must count as skipped");
        assert_eq!(errors, 0, "upsert error counter must be 0");
    }

    /// CopyPaste-vvsf: the closure must receive the row's `item_id` so the
    /// caller can bind it as AEAD AAD when calling `decrypt_from_cloud` /
    /// `encrypt_for_cloud`.  Verify that the closure receives the correct
    /// item_id from the live row (not an empty string or the tombstone's id).
    #[tokio::test]
    #[serial_test::serial]
    async fn reencrypt_closure_receives_item_id_for_aad() {
        use mockito::mock;
        use std::sync::Mutex;

        let live = live_row();
        let expected_item_id = live.item_id.clone();

        let list_body =
            serde_json::to_string(&vec![live, tombstone_row()]).expect("serialize rows");

        let _list_mock = mock("GET", "/rest/v1/clipboard_items?order=lamport_ts.asc")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&list_body)
            .create();

        // Mock upsert for the live row.
        let _upsert_mock = mock("POST", "/rest/v1/clipboard_items?on_conflict=item_id")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();

        let client = RestClient::new(mockito::server_url(), "anon", "tok");

        // Capture which item_ids the closure was called with.
        let seen_ids: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let seen_ids_ref = &seen_ids;

        let (success, skipped, _errors) = client
            .reencrypt_all_cloud_items(|item_id, old_ct| -> Result<String, String> {
                seen_ids_ref.lock().unwrap().push(item_id.to_owned());
                // Identity re-encryption (same ciphertext).
                Ok(old_ct.to_owned())
            })
            .await
            .expect("reencrypt should succeed");

        let seen = seen_ids.into_inner().unwrap();
        assert_eq!(
            seen.len(),
            1,
            "closure called once (tombstone must be skipped)"
        );
        assert_eq!(
            seen[0], expected_item_id,
            "closure must receive the live row's item_id for AAD binding (CopyPaste-vvsf)"
        );
        assert_eq!(success, 1, "live row must be re-encrypted");
        assert_eq!(skipped, 1, "tombstone must be skipped");
    }
}
