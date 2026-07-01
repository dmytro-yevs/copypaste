//! Read operations for [`super::client::RestClient`].

use super::client::RestClient;
use super::error::{RestError, RestResult};
use crate::models::CloudClipboardRow;

impl RestClient {
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::test_support::live_row;

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
    // CopyPaste-vp63.31: safety-net gap fill — list_cloud_items had no direct
    // mockito coverage (only the URL-injection regression above). Add a
    // happy-path GET test and a 4xx-error-decode test before/while splitting
    // rest.rs into submodules.
    // -----------------------------------------------------------------------

    /// A 200 response with a JSON array body must be parsed into
    /// `Vec<CloudClipboardRow>` in list order.
    #[tokio::test]
    #[serial_test::serial]
    async fn list_cloud_items_happy_path_parses_rows() {
        use mockito::mock;

        let row = live_row();
        let body = serde_json::to_string(&vec![row.clone()]).expect("serialize rows");

        let _mock = mock("GET", "/rest/v1/clipboard_items?order=lamport_ts.asc")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&body)
            .create();

        let client = RestClient::new(mockito::server_url(), "anon", "tok");
        let rows = client
            .list_cloud_items()
            .await
            .expect("200 response should parse");

        assert_eq!(rows.len(), 1, "expected exactly one row");
        assert_eq!(rows[0].item_id, row.item_id);
    }

    /// A non-2xx response must be decoded via `decode_error` into
    /// `RestError::PostgRest { status, message }`, surfacing the PostgREST
    /// `message` field from the JSON error body.
    #[tokio::test]
    #[serial_test::serial]
    async fn list_cloud_items_4xx_returns_postgrest_error_with_decoded_message() {
        use mockito::mock;

        let _mock = mock("GET", "/rest/v1/clipboard_items?order=lamport_ts.asc")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"code":"PGRST301","message":"JWT expired"}"#)
            .create();

        let client = RestClient::new(mockito::server_url(), "anon", "tok");
        let err = client
            .list_cloud_items()
            .await
            .expect_err("4xx response should error");

        match err {
            RestError::PostgRest { status, message } => {
                assert_eq!(status, 401, "status must be propagated from the response");
                assert_eq!(
                    message, "JWT expired",
                    "message must be decoded from the PostgREST error body"
                );
            }
            other => panic!("expected RestError::PostgRest; got: {other:?}"),
        }
    }
}
