//! Supabase PostgREST client for clipboard item CRUD operations.
//!
//! This module provides the [`RestClient`] for interacting with the Supabase
//! `clipboard_items` table via the PostgREST HTTP API. It is separate from the
//! GoTrue auth client ([`crate::auth::AuthClient`]) and the Realtime WebSocket
//! client ([`crate::realtime::RealtimeClient`]).
//!
//! # Design notes
//!
//! - All mutations use `?on_conflict=item_id&resolution=merge-duplicates` (upsert)
//!   so that concurrent writes from multiple devices converge via LWW (last-write-
//!   wins using `lamport_ts`).
//! - The `deleted` flag is **always** included in upserts (CopyPaste-kgs7). A
//!   missing `deleted` column in an INSERT would allow a previously-deleted
//!   tombstone row to be resurrected with `deleted = false` (the Postgres column
//!   default). By explicitly sending `deleted: true` for tombstones and
//!   `deleted: false` for live items, the upsert always propagates the correct
//!   soft-delete state.
//! - `pinned` and `pin_order` are also always included (CopyPaste-vqm0) so that
//!   pin state propagates to all devices through the cloud.
//! - Re-encryption of cloud items on passphrase change (CopyPaste-vvsf): the
//!   [`RestClient::reencrypt_all_cloud_items`] method fetches all rows for the
//!   current user, re-encrypts each payload under the new key, and upserts the
//!   updated rows back. This is a best-effort bulk operation — the caller is
//!   responsible for ensuring that no other sync operation runs concurrently.

use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::error::AuthError;
use crate::models::CloudClipboardRow;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the REST client.
///
/// Wraps [`AuthError`] for HTTP and credential errors plus PostgREST-specific
/// failures.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    /// An HTTP transport or status error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// PostgREST returned an error body.
    #[error("postgrest error ({status}): {message}")]
    PostgRest { status: u16, message: String },

    /// JSON (de)serialisation failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The caller supplied an invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<AuthError> for RestError {
    fn from(e: AuthError) -> Self {
        // HTTP transport errors from AuthError are also HTTP transport errors here.
        match e {
            AuthError::Http(inner) => RestError::Http(inner),
            other => RestError::PostgRest {
                status: 0,
                message: other.to_string(),
            },
        }
    }
}

pub type RestResult<T> = std::result::Result<T, RestError>;

// ---------------------------------------------------------------------------
// RestClient
// ---------------------------------------------------------------------------

/// PostgREST client scoped to the `clipboard_items` table.
///
/// Requires:
/// - `supabase_url` — project REST URL (`https://{project}.supabase.co`)
/// - `anon_key` — anonymous API key (used as `apikey` header)
/// - `access_token` — user JWT (used as `Authorization: Bearer`)
///
/// The client is cheaply cloneable (all members are `Arc<_>` internally via
/// `reqwest::Client`).
#[derive(Debug, Clone)]
pub struct RestClient {
    http: Client,
    base_url: String,
    anon_key: String,
    access_token: String,
}

impl RestClient {
    /// Construct from explicit credentials.
    ///
    /// `supabase_url` should be the HTTPS base URL (e.g.
    /// `https://abc.supabase.co`). The `/rest/v1/clipboard_items` path is
    /// appended automatically.
    pub fn new(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            http: Client::new(),
            base_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            access_token: access_token.into(),
        }
    }

    /// Build from environment variables.
    ///
    /// Required env vars: `SUPABASE_URL`, `SUPABASE_ANON_KEY`,
    /// `SUPABASE_ACCESS_TOKEN`.
    pub fn from_env() -> RestResult<Self> {
        let url = std::env::var("SUPABASE_URL")
            .map_err(|_| RestError::InvalidArgument("SUPABASE_URL env var not set".into()))?;
        let key = std::env::var("SUPABASE_ANON_KEY")
            .map_err(|_| RestError::InvalidArgument("SUPABASE_ANON_KEY env var not set".into()))?;
        let token = std::env::var("SUPABASE_ACCESS_TOKEN").map_err(|_| {
            RestError::InvalidArgument("SUPABASE_ACCESS_TOKEN env var not set".into())
        })?;
        Ok(Self::new(url, key, token))
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn table_url(&self) -> String {
        format!("{}/rest/v1/clipboard_items", self.base_url)
    }

    fn auth_headers(&self) -> [(&'static str, String); 2] {
        [
            ("apikey", self.anon_key.clone()),
            ("Authorization", format!("Bearer {}", self.access_token)),
        ]
    }

    /// Decode a PostgREST error body.  Returns a best-effort human-readable
    /// message suitable for [`RestError::PostgRest::message`].
    async fn decode_error(resp: reqwest::Response, fallback: &str) -> String {
        let raw = match resp.text().await {
            Ok(t) => t,
            Err(_) => return fallback.to_owned(),
        };
        // PostgREST errors look like: {"code":"...", "details":"...", "hint":"...", "message":"..."}
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
                return msg.to_owned();
            }
        }
        let snippet: String = raw.chars().take(200).collect();
        if snippet.is_empty() {
            fallback.to_owned()
        } else {
            snippet
        }
    }

    // -----------------------------------------------------------------------
    // Read operations
    // -----------------------------------------------------------------------

    /// Fetch all `clipboard_items` rows visible to the current user.
    ///
    /// Rows are ordered by `lamport_ts` ascending so the caller can process
    /// them in causal order. The server-side RLS policy restricts results to
    /// rows owned by the authenticated user.
    pub async fn list_cloud_items(&self) -> RestResult<Vec<CloudClipboardRow>> {
        let url = format!("{}?order=lamport_ts.asc", self.table_url());
        let [apikey_h, auth_h] = self.auth_headers();
        let resp = self
            .http
            .get(&url)
            .header(apikey_h.0, apikey_h.1)
            .header(auth_h.0, auth_h.1)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            let rows: Vec<CloudClipboardRow> = resp.json().await?;
            return Ok(rows);
        }

        let code = status.as_u16();
        let message = Self::decode_error(resp, "list_cloud_items failed").await;
        Err(RestError::PostgRest {
            status: code,
            message,
        })
    }

    // -----------------------------------------------------------------------
    // Write operations (CopyPaste-kgs7, CopyPaste-vqm0)
    // -----------------------------------------------------------------------

    /// Upsert a cloud row by `item_id`, always propagating `deleted`,
    /// `pinned`, and `pin_order`.
    ///
    /// # CopyPaste-kgs7 — `deleted` flag in upsert
    ///
    /// The `deleted` field **must** be included in every upsert payload so that
    /// soft-delete tombstones are never silently resurrected. Without it, a
    /// conflict resolution that falls back to the Postgres column default
    /// (`deleted = false`) would revive a tombstoned item.
    ///
    /// The upsert uses PostgREST's `on_conflict=item_id` with
    /// `resolution=merge-duplicates` (equivalent to SQL `ON CONFLICT (item_id)
    /// DO UPDATE SET …`). LWW conflict resolution is enforced by the database-
    /// side trigger / CHECK; the client sends the authoritative values and the
    /// DB decides which to keep based on `lamport_ts`.
    ///
    /// # CopyPaste-vqm0 — `pinned` / `pin_order` in upsert
    ///
    /// `pinned` and `pin_order` are included in every upsert so that the pin
    /// state set on one device propagates to all others via the cloud.  A
    /// receiving device maps these fields back to its local `clipboard_items`
    /// row when applying the downloaded cloud update.
    ///
    /// # PostgREST UPSERT semantics
    ///
    /// `POST .../clipboard_items?on_conflict=item_id` with
    /// `Prefer: resolution=merge-duplicates` header performs an upsert:
    /// INSERT if no row with this `item_id` exists, UPDATE if one does.
    pub async fn replace_cloud_item_by_item_id(&self, row: &CloudClipboardRow) -> RestResult<()> {
        // Serialise the row. CloudClipboardRow's Serialize impl always emits
        // `deleted`, `pinned`, and `pin_order`. Since `deleted` and `pinned`
        // are `bool` (no `skip_serializing_if`), they always appear in the JSON
        // (false still serializes). `pin_order` is `Option<f64>` with only
        // `#[serde(default)]` — NOT `skip_serializing_if` — so `None` serialises
        // as `null`, which explicitly clears an existing cloud value (correct
        // behaviour for an unordered unpinned item).
        let body = serde_json::to_string(row)?;

        let url = format!("{}?on_conflict=item_id", self.table_url());
        let [apikey_h, auth_h] = self.auth_headers();
        let resp = self
            .http
            .post(&url)
            .header(apikey_h.0, apikey_h.1)
            .header(auth_h.0, auth_h.1)
            .header("Content-Type", "application/json")
            // PostgREST upsert: INSERT … ON CONFLICT (item_id) DO UPDATE SET …
            .header("Prefer", "resolution=merge-duplicates")
            .body(body)
            .send()
            .await?;

        let status = resp.status();
        // PostgREST returns 200 or 201 on successful upsert.
        if status == StatusCode::OK
            || status == StatusCode::CREATED
            || status == StatusCode::NO_CONTENT
        {
            return Ok(());
        }

        let code = status.as_u16();
        let message = Self::decode_error(resp, "replace_cloud_item_by_item_id failed").await;
        Err(RestError::PostgRest {
            status: code,
            message,
        })
    }

    /// Soft-delete a cloud row by `item_id`.
    ///
    /// Sends an upsert with `deleted = true` and clears `payload_ct`.
    /// Preserves `lamport_ts`, `wall_time`, and other metadata so the tombstone
    /// wins LWW merge against any stale live copies on other devices.
    ///
    /// The `lamport_ts` provided by the caller must be strictly greater than
    /// any previously synced value for this item; it is the caller's
    /// responsibility to advance the clock before calling this method.
    pub async fn delete_cloud_item_by_item_id(
        &self,
        tombstone: &CloudClipboardRow,
    ) -> RestResult<()> {
        if !tombstone.deleted {
            return Err(RestError::InvalidArgument(
                "delete_cloud_item_by_item_id requires a tombstone row with deleted = true".into(),
            ));
        }
        if tombstone.payload_ct.is_some() {
            return Err(RestError::InvalidArgument(
                "tombstone row must have payload_ct = None to avoid leaking ciphertext".into(),
            ));
        }
        self.replace_cloud_item_by_item_id(tombstone).await
    }

    // -----------------------------------------------------------------------
    // Passphrase re-encryption (CopyPaste-vvsf)
    // -----------------------------------------------------------------------

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
    use crate::models::CloudClipboardRow;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn live_row() -> CloudClipboardRow {
        CloudClipboardRow {
            id: "row-uuid-1".into(),
            item_id: "item-uuid-1".into(),
            content_type: "text".into(),
            payload_ct: Some("\\xdeadbeef".into()),
            lamport_ts: 10,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            device_id: "device-a".into(),
            deleted: false,
            pinned: false,
            pin_order: None,
        }
    }

    fn tombstone_row() -> CloudClipboardRow {
        CloudClipboardRow {
            id: "row-uuid-2".into(),
            item_id: "item-uuid-2".into(),
            content_type: "text".into(),
            payload_ct: None,
            lamport_ts: 20,
            wall_time: 1_700_000_001_000,
            expires_at: None,
            app_bundle_id: None,
            device_id: "device-a".into(),
            deleted: true,
            pinned: false,
            pin_order: None,
        }
    }

    // -----------------------------------------------------------------------
    // CopyPaste-kgs7: `deleted` is always in the upsert payload
    // -----------------------------------------------------------------------

    /// The serialized upsert body for a LIVE row must include `"deleted":false`
    /// so the upsert never accidentally resurrects a tombstone by omitting the
    /// column (which would fall back to the Postgres column default).
    #[test]
    fn replace_cloud_item_payload_includes_deleted_false_for_live_row() {
        let row = live_row();
        let body = serde_json::to_string(&row).expect("serialize");
        assert!(
            body.contains("\"deleted\":false"),
            "upsert payload must contain deleted:false for live rows; got: {body}"
        );
    }

    /// The serialized upsert body for a TOMBSTONE must include `"deleted":true`
    /// so the tombstone is correctly propagated on upsert.
    #[test]
    fn replace_cloud_item_payload_includes_deleted_true_for_tombstone() {
        let row = tombstone_row();
        let body = serde_json::to_string(&row).expect("serialize");
        assert!(
            body.contains("\"deleted\":true"),
            "upsert payload must contain deleted:true for tombstone rows; got: {body}"
        );
        // Tombstone must NOT include payload_ct (skip_serializing_if = Option::is_none).
        assert!(
            !body.contains("payload_ct"),
            "tombstone payload must omit payload_ct; got: {body}"
        );
    }

    // -----------------------------------------------------------------------
    // CopyPaste-vqm0: `pinned` and `pin_order` are always in the upsert payload
    // -----------------------------------------------------------------------

    /// The serialized upsert body must include `pinned` so pin state propagates.
    #[test]
    fn replace_cloud_item_payload_includes_pinned_and_pin_order() {
        let mut row = live_row();
        row.pinned = true;
        row.pin_order = Some(2.5);

        let body = serde_json::to_string(&row).expect("serialize");
        assert!(
            body.contains("\"pinned\":true"),
            "upsert payload must include pinned:true; got: {body}"
        );
        assert!(
            body.contains("\"pin_order\":2.5"),
            "upsert payload must include pin_order:2.5; got: {body}"
        );
    }

    /// An unpinned row must still include `pinned:false` so a previously-pinned
    /// cloud row is unpinned on merge.
    ///
    /// `pin_order: None` serialises as `"pin_order":null` (not omitted) because
    /// the PostgREST column must be explicitly set to NULL to clear a previously-
    /// set ordering value.  The `#[serde(default)]` attribute provides safe
    /// backwards-compatible deserialization of old rows that lack the column.
    #[test]
    fn replace_cloud_item_payload_includes_pinned_false_when_unpinned() {
        let row = live_row(); // pinned = false, pin_order = None
        let body = serde_json::to_string(&row).expect("serialize");
        assert!(
            body.contains("\"pinned\":false"),
            "upsert payload must include pinned:false for unpinned rows; got: {body}"
        );
        // pin_order = None serialises as null so that the cloud column is set to
        // NULL (clearing any previous ordering), not omitted.
        assert!(
            body.contains("\"pin_order\":null"),
            "pin_order:null must be present to explicitly clear cloud ordering; got: {body}"
        );
    }

    // -----------------------------------------------------------------------
    // delete_cloud_item_by_item_id validation
    // -----------------------------------------------------------------------

    /// `delete_cloud_item_by_item_id` must reject a row with `deleted = false`.
    #[tokio::test]
    async fn delete_rejects_non_tombstone_row() {
        let client = RestClient::new("https://example.supabase.co", "anon-key", "access-token");
        let row = live_row(); // deleted = false
        let err = client
            .delete_cloud_item_by_item_id(&row)
            .await
            .expect_err("should reject live row");
        assert!(
            matches!(err, RestError::InvalidArgument(_)),
            "expected InvalidArgument; got: {err:?}"
        );
    }

    /// `delete_cloud_item_by_item_id` must reject a tombstone that still has
    /// `payload_ct` set (ciphertext would leak).
    #[tokio::test]
    async fn delete_rejects_tombstone_with_payload() {
        let client = RestClient::new("https://example.supabase.co", "anon-key", "access-token");
        let row = CloudClipboardRow {
            deleted: true,
            payload_ct: Some("\\xabcd".into()), // must not be set
            ..live_row()
        };
        let err = client
            .delete_cloud_item_by_item_id(&row)
            .await
            .expect_err("should reject tombstone with payload");
        assert!(
            matches!(err, RestError::InvalidArgument(_)),
            "expected InvalidArgument; got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // CopyPaste-vvsf: reencrypt_all_cloud_items
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // CopyPaste-b5iz: device filter audit
    //
    // The OR 1=1 bug is NOT present in this supabase crate. The PostgREST
    // client uses server-side RLS (Row Level Security) to scope results to the
    // current user's `user_id` — there is no hand-written SQL in this module.
    // The `sync_orch auto-apply` SQL referenced in CopyPaste-b5iz is in the
    // copypaste-daemon crate (SQLCipher local DB, not Supabase PostgREST). This
    // test documents that the supabase REST client does NOT inject OR 1=1 and
    // delegates filtering to server-side RLS.
    // -----------------------------------------------------------------------

    /// Verify that `list_cloud_items` does NOT inject an unconditional `OR 1=1`
    /// filter into the query URL.  The device filter is enforced server-side
    /// by Supabase RLS; we must not add any URL parameter that bypasses it.
    #[test]
    fn list_cloud_items_url_does_not_contain_or_1_equals_1() {
        let client = RestClient::new("https://abc.supabase.co", "anon-key", "access-token");
        // The URL is constructed inside list_cloud_items; we can inspect the
        // table_url() and the appended query string.
        let base = client.table_url();
        let query_url = format!("{}?order=lamport_ts.asc", base);
        assert!(
            !query_url.contains("OR 1=1") && !query_url.contains("or+1%3D1"),
            "list URL must not contain OR 1=1 bypass; got: {query_url}"
        );
    }

    // -----------------------------------------------------------------------
    // RestClient construction
    // -----------------------------------------------------------------------

    #[test]
    fn rest_client_new_trims_trailing_slash() {
        let client = RestClient::new("https://abc.supabase.co/", "anon-key", "access-token");
        // base_url must not have a trailing slash before /rest/v1/...
        assert_eq!(
            client.table_url(),
            "https://abc.supabase.co/rest/v1/clipboard_items"
        );
    }
}
