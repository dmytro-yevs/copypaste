//! The three requests, and the one place both auth headers are attached.
//!
//! Every query-shape decision that the cursor depends on is here — the
//! inclusive `gte`, the ascending compound order, the clamped `limit` — and
//! each carries the bug it prevents.

use std::fmt;

use backoff::backoff::Backoff;
use backoff::future::retry;
use backoff::{Error as Retryable, ExponentialBackoff};
use reqwest::Client;

use super::error::{classify, RestError};
use super::item::{validate_item_id, CloudItem};
use super::{
    CONFLICT_TARGET, MAX_PAGE_LIMIT, REST_TIMEOUT, SELECT_COLUMNS, TABLE, TOMBSTONE_CHUNK,
    UPSERT_CHUNK,
};
use crate::auth::{now_ms, transient_backoff};
use crate::CloudConfig;

/// PostgREST client for one Supabase project.
#[derive(Clone)]
pub struct SupabaseRest {
    config: CloudConfig,
    http: Client,
    retry: ExponentialBackoff,
}

impl fmt::Debug for SupabaseRest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupabaseRest")
            .field("url", &self.config.url)
            .finish_non_exhaustive()
    }
}

impl SupabaseRest {
    pub fn new(config: CloudConfig) -> Self {
        Self {
            config,
            http: Client::new(),
            retry: transient_backoff(),
        }
    }

    /// Replace the retry policy — for tests, and for a caller with its own
    /// idea of how long a transient failure may take.
    #[must_use]
    pub fn with_retry_policy(mut self, policy: ExponentialBackoff) -> Self {
        self.retry = policy;
        self
    }

    /// Newest-first page of rows whose version timestamp is at or after
    /// `since_ms`.
    ///
    /// The bound is **inclusive** (`gte`), not `gt`: `created_at` has
    /// millisecond granularity, and a strict `gt` silently drops every row that
    /// shares the boundary millisecond with the last row of the previous page —
    /// the worst silent-data-loss bug in v1 (manifest §4.4, INV-N1). Re-offered
    /// boundary rows are free to absorb, because applying a version already
    /// applied is a no-op (INV-I1).
    ///
    /// The order is **ascending** — `(created_at asc, item_id asc)` — a compound
    /// key with no ties, so paging is deterministic.
    ///
    /// Ascending is load-bearing, not a preference. A forward cursor cannot
    /// drain a newest-first page: take the newest `limit` rows and advance past
    /// them and everything older is skipped forever; do not advance and the
    /// same page is returned on every tick. The failure is invisible in steady
    /// state, where the backlog is smaller than one page, and appears only for
    /// a device that has been offline long enough to matter — which is exactly
    /// when history matters most. Manifest 05 §4.4 records this as a shipped v1
    /// bug: `order=wall_time.desc&limit=20` re-fetched the same newest twenty
    /// rows and older history never downloaded at all.
    ///
    /// `limit` is clamped to [`MAX_PAGE_LIMIT`]; a full page means "there may
    /// be more", and the caller should drain rather than wait for the next tick.
    pub async fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        limit: u32,
    ) -> Result<Vec<CloudItem>, RestError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        let url = self.table_url();
        let query = [
            ("select", SELECT_COLUMNS.to_string()),
            ("created_at", format!("gte.{}", since_ms.max(0))),
            ("order", "created_at.asc,item_id.asc".to_string()),
            ("limit", limit.to_string()),
        ];

        let body = self
            .send(token, || {
                self.http
                    .get(&url)
                    .query(&query)
                    .header("Accept", "application/json")
            })
            .await?;

        let items: Vec<CloudItem> = serde_json::from_str(&body).map_err(|_| {
            tracing::warn!("the row page was not the expected JSON array");
            RestError::Malformed
        })?;
        tracing::debug!(rows = items.len(), since_ms, "fetched a page");
        Ok(items)
    }

    /// Write `items`, keyed on the conflict target.
    ///
    /// Idempotent: `on_conflict` plus `Prefer: resolution=merge-duplicates`
    /// makes a replayed batch an update to the same values rather than a
    /// duplicate-key failure, which is what lets an interrupted push simply be
    /// re-sent. Large batches are chunked into [`UPSERT_CHUNK`] rows per
    /// request.
    ///
    /// Every field is sent on every row, including `deleted: false`. Omitting
    /// `deleted` lets the column default win on a merge and resurrects a
    /// tombstoned item (manifest T-5).
    ///
    /// Returns the number of rows accepted. Preconditions are checked for the
    /// whole batch before anything is sent, so a bad row cannot leave half a
    /// batch written.
    pub async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<usize, RestError> {
        for item in items {
            item.validate()?;
        }
        if items.is_empty() {
            return Ok(0);
        }

        let url = self.table_url();
        let mut written = 0usize;
        for chunk in items.chunks(UPSERT_CHUNK) {
            let payload = serde_json::to_value(chunk).map_err(|_| RestError::Malformed)?;
            self.send(token, || {
                self.http
                    .post(&url)
                    .query(&[("on_conflict", CONFLICT_TARGET)])
                    .header("Prefer", "resolution=merge-duplicates,return=minimal")
                    .json(&payload)
            })
            .await?;
            written += chunk.len();
        }

        tracing::debug!(rows = written, "upserted");
        Ok(written)
    }

    /// Mark items deleted, rather than removing the rows.
    ///
    /// A delete has to be a *version* of the item, not the absence of one: a
    /// device that is offline now and syncs next week must be told the item
    /// died, and a row that is simply gone tells it nothing — its own copy
    /// would be re-uploaded and the item would come back.
    ///
    /// The payload wipes `ciphertext` and `nonce` (manifest T-4) and restamps
    /// `created_at` to now, because that column is what the poll cursor pages
    /// on: leaving the original creation time would hide the deletion below the
    /// watermark of every device that already has the item.
    ///
    /// Prefer [`SupabaseRest::upsert`] with a full [`CloudItem::tombstone`]
    /// when the caller holds the item's merge metadata; this is the id-only
    /// path.
    pub async fn tombstone(&self, token: &str, item_ids: &[String]) -> Result<usize, RestError> {
        for item_id in item_ids {
            validate_item_id(item_id)?;
        }
        if item_ids.is_empty() {
            return Ok(0);
        }

        let url = self.table_url();
        let payload = serde_json::json!({
            "deleted": true,
            "ciphertext": serde_json::Value::Null,
            "nonce": serde_json::Value::Null,
            "created_at": now_ms(),
        });

        let mut tombstoned = 0usize;
        for chunk in item_ids.chunks(TOMBSTONE_CHUNK) {
            let filter = in_list(chunk);
            self.send(token, || {
                self.http
                    .patch(&url)
                    .query(&[("item_id", filter.as_str())])
                    .header("Prefer", "return=minimal")
                    .json(&payload)
            })
            .await?;
            tombstoned += chunk.len();
        }

        tracing::debug!(rows = tombstoned, "tombstoned");
        Ok(tombstoned)
    }

    fn table_url(&self) -> String {
        format!(
            "{}/rest/v1/{}",
            self.config.url.trim_end_matches('/'),
            TABLE
        )
    }

    /// Send a prepared request under the shared retry policy.
    ///
    /// `build` is called once per attempt because a `reqwest::RequestBuilder`
    /// is consumed by sending it. Both auth headers are added *here*, so no
    /// call site can forget one: the `apikey` gets the request past the API
    /// gateway, and the user's JWT is what RLS pivots on — the anon key alone
    /// is refused outright by the policies in the module docs.
    async fn send<F>(&self, token: &str, build: F) -> Result<String, RestError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut policy = self.retry.clone();
        policy.reset();
        let build = &build;

        retry(policy, move || async move {
            let sent = build()
                .timeout(REST_TIMEOUT)
                .header("apikey", self.config.anon_key.as_str())
                .bearer_auth(token)
                .send()
                .await;

            let err = match sent {
                Ok(response) => match classify(response).await {
                    Ok(body) => return Ok(body),
                    Err(err) => err,
                },
                Err(err) => RestError::from_reqwest(err),
            };

            if err.is_transient() {
                Err(Retryable::transient(err))
            } else {
                Err(Retryable::permanent(err))
            }
        })
        .await
    }
}

/// PostgREST's `in.(…)` filter value.
///
/// Values are known to be `[A-Za-z0-9_-]` by
/// [`validate_item_id`](super::item::validate_item_id), so there is no quoting
/// to get wrong.
fn in_list(item_ids: &[String]) -> String {
    format!("in.({})", item_ids.join(","))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::testkit::{client, item, query_pairs, value_of, ANON, TOKEN};
    use super::*;
    use crate::auth::stub::{Reply, Stub};

    // -- fetch_since --------------------------------------------------------

    #[tokio::test]
    async fn fetch_since_asks_for_an_inclusive_bound_and_a_total_order() {
        let stub = Stub::start(vec![Reply::json(200, "[]")]).await;
        client(&stub)
            .fetch_since(TOKEN, 1_700_000_000_000, 20)
            .await
            .expect("fetch");

        let request = stub.only_request();
        assert_eq!(request.method, "GET");
        assert!(request.target.starts_with("/rest/v1/clipboard_items?"));

        let pairs = query_pairs(&request.target);
        assert_eq!(value_of(&pairs, "select"), SELECT_COLUMNS);
        assert_eq!(
            value_of(&pairs, "created_at"),
            "gte.1700000000000",
            "a strict `gt` loses every row in the boundary millisecond"
        );
        // Ascending: a forward cursor cannot drain a newest-first page.
        assert_eq!(value_of(&pairs, "order"), "created_at.asc,item_id.asc");
        assert_eq!(value_of(&pairs, "limit"), "20");
    }

    #[tokio::test]
    async fn fetch_since_sends_both_the_apikey_and_the_user_token() {
        let stub = Stub::start(vec![Reply::json(200, "[]")]).await;
        client(&stub)
            .fetch_since(TOKEN, 0, 10)
            .await
            .expect("fetch");

        let request = stub.only_request();
        assert_eq!(request.header("apikey"), Some(ANON));
        assert_eq!(
            request.header("authorization"),
            Some(format!("Bearer {TOKEN}").as_str()),
            "the user JWT, not the anon key, is what RLS pivots on"
        );
    }

    #[tokio::test]
    async fn the_page_size_is_bounded_in_both_directions() {
        let stub = Stub::start(vec![Reply::json(200, "[]")]).await;
        client(&stub)
            .fetch_since(TOKEN, 0, 100_000)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().target), "limit"),
            MAX_PAGE_LIMIT.to_string()
        );

        let stub = Stub::start(vec![Reply::json(200, "[]")]).await;
        client(&stub).fetch_since(TOKEN, 0, 0).await.expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().target), "limit"),
            "1"
        );
    }

    #[tokio::test]
    async fn a_negative_watermark_is_floored_at_zero() {
        let stub = Stub::start(vec![Reply::json(200, "[]")]).await;
        client(&stub)
            .fetch_since(TOKEN, -42, 10)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().target), "created_at"),
            "gte.0"
        );
    }

    #[tokio::test]
    async fn a_page_round_trips_into_rows() {
        let body = r#"[
            {"item_id":"a1","ciphertext":"c2VhbGVk","nonce":"bm9uY2U=",
             "content_type":"text","created_at":1700000000001,"deleted":false,
             "origin_device_id":"device-b"},
            {"item_id":"a2","ciphertext":"","nonce":"",
             "content_type":"text","created_at":1700000000000,"deleted":true,
             "origin_device_id":"device-c"}
        ]"#;
        let stub = Stub::start(vec![Reply::json(200, body)]).await;
        let rows = client(&stub)
            .fetch_since(TOKEN, 0, 10)
            .await
            .expect("fetch");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].item_id, "a1");
        assert_eq!(rows[0].ciphertext_bytes().expect("base64"), b"sealed");
        assert_eq!(rows[0].nonce_bytes().expect("base64"), b"nonce");
        assert!(rows[1].deleted, "a tombstone must survive the round trip");
        assert!(rows[1].ciphertext.is_empty());
    }

    #[tokio::test]
    async fn a_body_that_is_not_a_row_array_is_malformed() {
        let stub = Stub::start(vec![Reply::json(200, r#"{"message":"nope"}"#)]).await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        assert!(matches!(err, RestError::Malformed), "{err:?}");
    }

    // -- upsert -------------------------------------------------------------

    #[tokio::test]
    async fn upsert_names_the_conflict_target_and_asks_for_a_merge() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let written = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .expect("upsert");
        assert_eq!(written, 1);

        let request = stub.only_request();
        assert_eq!(request.method, "POST");
        let pairs = query_pairs(&request.target);
        assert_eq!(value_of(&pairs, "on_conflict"), CONFLICT_TARGET);
        let prefer = request.header("prefer").expect("Prefer header");
        assert!(
            prefer.contains("resolution=merge-duplicates"),
            "without merge-duplicates a replayed batch is a 409, not a no-op: {prefer}"
        );
    }

    #[tokio::test]
    async fn an_upsert_body_is_an_array_that_always_states_deleted() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine as _;

        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let mut live = item("a1");
        live.deleted = false;
        client(&stub).upsert(TOKEN, &[live]).await.expect("upsert");

        let body = stub.only_request().json();
        let rows = body.as_array().expect("an array of rows");
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["item_id"], "a1");
        assert_eq!(row["ciphertext"], BASE64.encode(b"sealed-bytes"));
        assert_eq!(row["nonce"], BASE64.encode(b"nonce12"));
        assert_eq!(row["content_type"], "text");
        assert_eq!(row["created_at"], 1_700_000_000_000i64);
        assert_eq!(
            row["deleted"], false,
            "omitting `deleted` lets the column default resurrect a tombstone"
        );
        assert_eq!(row["origin_device_id"], "device-a");
        assert!(
            row.get("user_id").is_none(),
            "user_id comes from the column default, never from the client"
        );
    }

    #[tokio::test]
    async fn a_large_batch_is_chunked() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let items: Vec<CloudItem> = (0..250).map(|i| item(&format!("id-{i}"))).collect();
        let written = client(&stub).upsert(TOKEN, &items).await.expect("upsert");

        assert_eq!(written, 250);
        let requests = stub.requests();
        assert_eq!(requests.len(), 3, "250 rows at {UPSERT_CHUNK}/request");
        let sizes: Vec<usize> = requests
            .iter()
            .map(|r| r.json().as_array().expect("array").len())
            .collect();
        assert_eq!(sizes, vec![UPSERT_CHUNK, UPSERT_CHUNK, 50]);
    }

    #[tokio::test]
    async fn a_batch_that_fits_in_one_chunk_is_one_request() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let items: Vec<CloudItem> = (0..UPSERT_CHUNK)
            .map(|i| item(&format!("id-{i}")))
            .collect();
        client(&stub).upsert(TOKEN, &items).await.expect("upsert");
        assert_eq!(stub.request_count(), 1);
    }

    #[tokio::test]
    async fn an_empty_batch_touches_the_network_at_all() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        assert_eq!(client(&stub).upsert(TOKEN, &[]).await.expect("upsert"), 0);
        assert_eq!(stub.request_count(), 0);
    }

    #[tokio::test]
    async fn a_tombstone_carrying_ciphertext_is_refused_before_anything_is_sent() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let mut poisoned = item("a1");
        poisoned.deleted = true; // ciphertext still set

        let err = client(&stub)
            .upsert(TOKEN, &[item("a2"), poisoned])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(
            stub.request_count(),
            0,
            "the whole batch is validated before any of it is written"
        );
    }

    #[tokio::test]
    async fn a_live_item_with_no_ciphertext_is_refused() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        let mut empty = item("a1");
        empty.ciphertext = String::new();
        let err = client(&stub).upsert(TOKEN, &[empty]).await.unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn an_item_id_that_could_change_a_filter_is_refused() {
        let stub = Stub::start(vec![Reply::empty(201)]).await;
        for bad in ["a,b", "a)b", "\"quoted\"", "a b", ""] {
            let mut sneaky = item("placeholder");
            sneaky.item_id = bad.to_string();
            let err = client(&stub).upsert(TOKEN, &[sneaky]).await.unwrap_err();
            assert!(
                matches!(err, RestError::InvalidItem { .. }),
                "{bad:?} -> {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_partial_batch_stops_at_the_first_failing_chunk() {
        let stub = Stub::start(vec![Reply::empty(201), Reply::json(401, "{}")]).await;
        let items: Vec<CloudItem> = (0..150).map(|i| item(&format!("id-{i}"))).collect();
        let err = client(&stub).upsert(TOKEN, &items).await.unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(stub.request_count(), 2);
    }

    // -- tombstone ----------------------------------------------------------

    #[tokio::test]
    async fn tombstone_patches_a_flag_rather_than_deleting_the_row() {
        let stub = Stub::start(vec![Reply::empty(204)]).await;
        let before = now_ms();
        let count = client(&stub)
            .tombstone(TOKEN, &["a1".to_string(), "a2".to_string()])
            .await
            .expect("tombstone");
        assert_eq!(count, 2);

        let request = stub.only_request();
        assert_eq!(
            request.method, "PATCH",
            "a hard DELETE would not propagate to a device that is offline"
        );
        assert_eq!(
            value_of(&query_pairs(&request.target), "item_id"),
            "in.(a1,a2)"
        );

        let body = request.json();
        assert_eq!(body["deleted"], true);
        assert!(
            body["ciphertext"].is_null(),
            "a tombstone must wipe the payload"
        );
        assert!(body["nonce"].is_null());
        let restamped = body["created_at"].as_i64().expect("created_at");
        assert!(
            restamped >= before,
            "a tombstone that kept the original timestamp hides below every \
             device's watermark and never propagates"
        );
    }

    #[tokio::test]
    async fn tombstone_chunks_its_id_list() {
        let stub = Stub::start(vec![Reply::empty(204)]).await;
        let ids: Vec<String> = (0..120).map(|i| format!("id-{i}")).collect();
        let count = client(&stub)
            .tombstone(TOKEN, &ids)
            .await
            .expect("tombstone");

        assert_eq!(count, 120);
        let requests = stub.requests();
        assert_eq!(requests.len(), 3, "120 ids at {TOMBSTONE_CHUNK}/request");
        let first = value_of(&query_pairs(&requests[0].target), "item_id");
        assert_eq!(first.matches(',').count(), TOMBSTONE_CHUNK - 1);
    }

    #[tokio::test]
    async fn tombstoning_nothing_sends_nothing() {
        let stub = Stub::start(vec![Reply::empty(204)]).await;
        assert_eq!(client(&stub).tombstone(TOKEN, &[]).await.expect("ok"), 0);
        assert_eq!(stub.request_count(), 0);
    }

    #[tokio::test]
    async fn a_hostile_id_cannot_widen_the_tombstone_filter() {
        let stub = Stub::start(vec![Reply::empty(204)]).await;
        let err = client(&stub)
            .tombstone(TOKEN, &["a1,*".to_string()])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(stub.request_count(), 0);
    }

    #[test]
    fn the_in_filter_has_the_shape_postgrest_expects() {
        assert_eq!(in_list(&["a".into()]), "in.(a)");
        assert_eq!(in_list(&["a".into(), "b".into()]), "in.(a,b)");
    }

    #[test]
    fn debug_for_the_client_shows_no_key_material() {
        let rest = SupabaseRest::new(CloudConfig {
            url: "https://project.supabase.co".into(),
            anon_key: "anon-secret-looking-key".into(),
        });
        assert!(!format!("{rest:?}").contains("anon-secret-looking-key"));
    }
}
