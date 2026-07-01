//! Write operations for [`super::client::RestClient`] (CopyPaste-kgs7,
//! CopyPaste-vqm0): upsert and tombstone delete of `clipboard_items` rows.

use reqwest::StatusCode;

use super::client::RestClient;
use super::error::{RestError, RestResult};
use crate::models::CloudClipboardRow;

impl RestClient {
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::test_support::{live_row, tombstone_row};

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
}
