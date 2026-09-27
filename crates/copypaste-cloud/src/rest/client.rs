//! The two requests, and the one place both auth headers are attached.
//!
//! Every query-shape decision that the cursor depends on is here — the
//! inclusive `gte`, the ascending compound order, the clamped `limit` — and
//! each carries the bug it prevents.

use std::fmt;

use super::error::{classify, RestError};
use super::item::{validate_item_id, CloudItem};
use super::{CONFLICT_TARGET, MAX_PAGE_LIMIT, REST_TIMEOUT, SELECT_COLUMNS, TABLE, UPSERT_CHUNK};
use crate::auth::transient_backoff;
use crate::CloudConfig;
use backon::{ExponentialBuilder, Retryable};
use reqwest::Client;

/// PostgREST client for one Supabase project.
#[derive(Clone)]
pub struct SupabaseRest {
    config: CloudConfig,
    http: Client,
    retry: ExponentialBuilder,
}

impl fmt::Debug for SupabaseRest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupabaseRest")
            .field("url", &self.config.url())
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
    pub fn with_retry_policy(mut self, policy: ExponentialBuilder) -> Self {
        self.retry = policy;
        self
    }

    /// One page of rows, oldest first, positioned by the keyset cursor.
    ///
    /// The cursor has two halves, and which of them the caller holds decides
    /// the bound this sends:
    ///
    /// * `after_item_id = None` — the caller knows only the millisecond, so the
    ///   bound is **inclusive** (`created_at=gte.…`). A strict `gt` on a
    ///   millisecond alone silently drops every row that shares the boundary
    ///   millisecond with the last row of the previous page (manifest §4.4,
    ///   INV-N1). Re-offered boundary rows are free to absorb, because applying
    ///   a version already applied is a no-op (INV-I1).
    /// * `after_item_id = Some(id)` — the caller knows the full position, so the
    ///   bound is the compound keyset
    ///   `or=(created_at.gt.M,and(created_at.eq.M,item_id.gt.ID))`: everything
    ///   strictly after the pair `(M, ID)` in the page order. Exclusive is
    ///   correct *here* and only here, because `(created_at, item_id)` is a
    ///   total order with no ties, so "strictly after the last row I applied"
    ///   cannot skip a row it has not seen.
    ///
    /// The millisecond-only form is what stalls once more than one page of rows
    /// shares a single `created_at`: the watermark cannot advance past that
    /// millisecond, so the same first page comes back forever and the rows
    /// behind it never download (manifest §5.1 row 6, INV-N1, AT-24). The
    /// compound form advances through every row in the total order.
    ///
    /// The order is **ascending** — `(created_at asc, item_id asc)` — and it is
    /// the order the keyset is expressed in; the deployment's
    /// `(user_id, created_at, item_id)` index serves both.
    ///
    /// Ascending is load-bearing, not a preference. A forward cursor cannot
    /// drain a newest-first page: take the newest `limit` rows and advance past
    /// them and everything older is skipped forever; do not advance and the
    /// same page is returned on every tick. The failure is invisible in steady
    /// state, where the backlog is smaller than one page, and appears only for
    /// a device that has been offline long enough to matter — which is exactly
    /// when history matters most. Manifest 05 §4.4 records the required forward
    /// order and AT-23 exercises a backlog larger than one page.
    ///
    /// `limit` is clamped to [`MAX_PAGE_LIMIT`]; a full page means "there may
    /// be more", and the caller should drain rather than wait for the next tick.
    pub async fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        after_item_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CloudItem>, RestError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        let since_ms = since_ms.max(0);
        let url = self.table_url();
        let mut query = vec![
            ("select", SELECT_COLUMNS.to_string()),
            ("order", "created_at.asc,item_id.asc".to_string()),
            ("limit", limit.to_string()),
        ];
        match after_item_id {
            Some(item_id) => {
                validate_item_id(item_id)?;
                query.push(("or", keyset_after(since_ms, item_id)));
            }
            None => query.push(("created_at", format!("gte.{since_ms}"))),
        }

        let body = self
            .send(token, || {
                self.http
                    .get(url.clone())
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
                    .post(url.clone())
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

    fn table_url(&self) -> url::Url {
        let path = format!("/rest/v1/{TABLE}");
        self.config.endpoint(&path)
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
        let build = &build;
        let attempt = || async move {
            let sent = build()
                .timeout(REST_TIMEOUT)
                .header("apikey", self.config.anon_key())
                .bearer_auth(token)
                .send()
                .await;

            match sent {
                Ok(response) => classify(response).await,
                Err(err) => Err(RestError::from_reqwest(err)),
            }
        };

        attempt
            .retry(self.retry)
            .when(RestError::is_transient)
            .await
    }
}

/// PostgREST's spelling of "strictly after the pair `(created_at, item_id)`".
///
/// The `and(…)` arm is what makes the bound safe to make exclusive: without it
/// a strict `gt` on the millisecond alone loses every row sharing it, and an
/// inclusive bound on the millisecond alone cannot get past a millisecond that
/// holds more than one page (INV-N1).
///
/// `item_id` is `[A-Za-z0-9_-]` by [`validate_item_id`], checked by the caller,
/// so it cannot close the parenthesis or add a disjunct.
fn keyset_after(created_at: i64, item_id: &str) -> String {
    format!("(created_at.gt.{created_at},and(created_at.eq.{created_at},item_id.gt.{item_id}))")
}

// Tests

#[cfg(test)]
mod tests {
    use super::super::testkit::{client, item, key, query_pairs, value_of, ANON, TOKEN};
    use super::*;
    use crate::auth::stub::{header as request_header, json as request_json, Reply, Stub};
    use url::Url;
    use wiremock::matchers::{header as header_match, headers, method, path, query_param};
    use wiremock::Mock;

    // -- fetch_since --------------------------------------------------------

    #[test]
    fn table_url_join_preserves_an_ipv6_authority() {
        let rest = SupabaseRest::new(
            CloudConfig::new(
                "https://[2001:db8::1]:8443/nested%20base/?stale=true#old",
                ANON,
            )
            .unwrap(),
        );
        assert_eq!(
            rest.table_url().as_str(),
            "https://[2001:db8::1]:8443/rest/v1/clipboard_items"
        );
    }

    #[test]
    fn captured_query_decoding_handles_form_escapes() {
        let url = Url::parse(
            "http://[::1]:54321/rest/v1/clipboard_items?filter=a%2Bb+c&unicode=%E2%9C%93",
        )
        .expect("valid captured URL");
        let pairs = query_pairs(&url);
        assert_eq!(value_of(&pairs, "filter"), "a+b c");
        assert_eq!(value_of(&pairs, "unicode"), "✓");
    }

    #[tokio::test]
    async fn fetch_since_asks_for_an_inclusive_bound_and_a_total_order() {
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(query_param("select", SELECT_COLUMNS))
            .and(query_param("created_at", "gte.1700000000000"))
            .and(query_param("order", "created_at.asc,item_id.asc"))
            .and(query_param("limit", "20"));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, 1_700_000_000_000, None, 20)
            .await
            .expect("fetch");

        let request = stub.only_request().await;
        assert_eq!(request.method.as_str(), "GET");
        assert_eq!(request.url.path(), "/rest/v1/clipboard_items");

        let pairs = query_pairs(&request.url);
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
    async fn a_known_tie_break_becomes_a_compound_keyset() {
        // INV-N1 / AT-24. The millisecond alone cannot be paged past once one
        // millisecond holds more than a page of rows; the pair can.
        let keyset =
            "(created_at.gt.1700000000000,and(created_at.eq.1700000000000,item_id.gt.item-42))";
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(query_param("or", keyset))
            .and(query_param("order", "created_at.asc,item_id.asc"));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, 1_700_000_000_000, Some("item-42"), 20)
            .await
            .expect("fetch");

        let request = stub.only_request().await;
        let pairs = query_pairs(&request.url);
        assert_eq!(value_of(&pairs, "or"), keyset);
        assert!(
            !pairs.iter().any(|(k, _)| k == "created_at"),
            "the keyset replaces the bare bound rather than joining it: {pairs:?}"
        );
        assert_eq!(value_of(&pairs, "order"), "created_at.asc,item_id.asc");
    }

    #[tokio::test]
    async fn a_hostile_tie_break_cannot_rewrite_the_filter() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 0).await;
        let err = client(&stub)
            .fetch_since(TOKEN, 1, Some("a),or=(deleted.eq.false"), 10)
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(stub.request_count().await, 0);
    }

    #[tokio::test]
    async fn fetch_since_sends_both_the_apikey_and_the_user_token() {
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(header_match("apikey", ANON))
            .and(header_match("authorization", format!("Bearer {TOKEN}")));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .expect("fetch");

        let request = stub.only_request().await;
        assert_eq!(request_header(&request, "apikey"), Some(ANON));
        assert_eq!(
            request_header(&request, "authorization"),
            Some(format!("Bearer {TOKEN}").as_str()),
            "the user JWT, not the anon key, is what RLS pivots on"
        );
    }

    #[tokio::test]
    async fn the_page_size_is_bounded_in_both_directions() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, 0, None, 100_000)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "limit"),
            MAX_PAGE_LIMIT.to_string()
        );

        let stub = Stub::start(vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, 0, None, 0)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "limit"),
            "1"
        );
    }

    #[tokio::test]
    async fn a_negative_watermark_is_floored_at_zero() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .fetch_since(TOKEN, -42, None, 10)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "created_at"),
            "gte.0"
        );
    }

    #[tokio::test]
    async fn a_page_round_trips_into_rows() {
        let body = r#"[
            {"item_id":"a1","ciphertext":"c2VhbGVk","nonce":"bm9uY2U=",
             "content_type":"text","created_at":1700000000001,"deleted":false,
             "origin_device_id":"device-b","signature":"c2ln"},
            {"item_id":"a2","ciphertext":"","nonce":"",
             "content_type":"text","created_at":1700000000000,"deleted":true,
             "origin_device_id":"device-c"}
        ]"#;
        let stub = Stub::start(vec![Reply::json(200, body)], 1).await;
        let rows = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .expect("fetch");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].item_id, "a1");
        assert_eq!(rows[0].ciphertext_bytes().expect("base64"), b"sealed");
        assert_eq!(rows[0].nonce_bytes().expect("base64"), b"nonce");
        assert_eq!(rows[0].signature, "c2ln");
        assert!(rows[1].deleted, "a tombstone must survive the round trip");
        assert!(rows[1].ciphertext.is_empty());
        assert!(
            rows[1].signature.is_empty(),
            "an absent signature must decode as absent, not fail the page"
        );
    }

    #[tokio::test]
    async fn a_body_that_is_not_a_row_array_is_malformed() {
        let stub = Stub::start(vec![Reply::json(200, r#"{"message":"nope"}"#)], 1).await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::Malformed), "{err:?}");
    }

    // -- upsert -------------------------------------------------------------

    #[tokio::test]
    async fn upsert_names_the_conflict_target_and_asks_for_a_merge() {
        let mock = Mock::given(method("POST"))
            .and(path("/rest/v1/clipboard_items"))
            .and(query_param("on_conflict", CONFLICT_TARGET))
            .and(headers(
                "prefer",
                vec!["resolution=merge-duplicates", "return=minimal"],
            ));
        let stub = Stub::start_matching(mock, vec![Reply::empty(201)], 1).await;
        let written = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .expect("upsert");
        assert_eq!(written, 1);

        let request = stub.only_request().await;
        assert_eq!(request.method.as_str(), "POST");
        let pairs = query_pairs(&request.url);
        assert_eq!(value_of(&pairs, "on_conflict"), CONFLICT_TARGET);
        let prefer = request_header(&request, "prefer").expect("Prefer header");
        assert!(
            prefer.contains("resolution=merge-duplicates"),
            "without merge-duplicates a replayed batch is a 409, not a no-op: {prefer}"
        );
    }

    #[tokio::test]
    async fn an_upsert_body_is_an_array_that_always_states_deleted() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine as _;

        let stub = Stub::start(vec![Reply::empty(201)], 1).await;
        let mut live = item("a1");
        live.deleted = false;
        client(&stub).upsert(TOKEN, &[live]).await.expect("upsert");

        let body = request_json(&stub.only_request().await);
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
        let stub = Stub::start(vec![Reply::empty(201)], 3).await;
        let items: Vec<CloudItem> = (0..250).map(|i| item(&format!("id-{i}"))).collect();
        let written = client(&stub).upsert(TOKEN, &items).await.expect("upsert");

        assert_eq!(written, 250);
        let requests = stub.requests().await;
        assert_eq!(requests.len(), 3, "250 rows at {UPSERT_CHUNK}/request");
        let sizes: Vec<usize> = requests
            .iter()
            .map(|request| request_json(request).as_array().expect("array").len())
            .collect();
        assert_eq!(sizes, vec![UPSERT_CHUNK, UPSERT_CHUNK, 50]);
    }

    #[tokio::test]
    async fn a_batch_that_fits_in_one_chunk_is_one_request() {
        let stub = Stub::start(vec![Reply::empty(201)], 1).await;
        let items: Vec<CloudItem> = (0..UPSERT_CHUNK)
            .map(|i| item(&format!("id-{i}")))
            .collect();
        client(&stub).upsert(TOKEN, &items).await.expect("upsert");
        assert_eq!(stub.request_count().await, 1);
    }

    #[tokio::test]
    async fn an_empty_batch_touches_the_network_at_all() {
        let stub = Stub::start(vec![Reply::empty(201)], 0).await;
        assert_eq!(client(&stub).upsert(TOKEN, &[]).await.expect("upsert"), 0);
        assert_eq!(stub.request_count().await, 0);
    }

    #[tokio::test]
    async fn a_tombstone_carrying_ciphertext_is_refused_before_anything_is_sent() {
        let stub = Stub::start(vec![Reply::empty(201)], 0).await;
        let mut poisoned = item("a1");
        poisoned.deleted = true; // ciphertext still set

        let err = client(&stub)
            .upsert(TOKEN, &[item("a2"), poisoned])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(
            stub.request_count().await,
            0,
            "the whole batch is validated before any of it is written"
        );
    }

    #[tokio::test]
    async fn a_tombstone_travels_through_the_same_upsert_as_a_live_row() {
        // One write path (manifest 05 §7.5), and here it is also the only one
        // that can carry a signature: a PATCH does not hold `content_type` or
        // `origin_device_id`, so it could not sign what the merge reads.
        let stub = Stub::start(vec![Reply::empty(201)], 1).await;
        let mut dead = CloudItem::tombstone("a1", "text", 1_700_000_000_002, "device-a");
        dead.sign(&key());

        client(&stub).upsert(TOKEN, &[dead]).await.expect("upsert");

        let request = stub.only_request().await;
        assert_eq!(
            request.method.as_str(),
            "POST",
            "a delete is a row version, not a PATCH"
        );
        let body = request_json(&request);
        let row = &body[0];
        assert_eq!(row["deleted"], true);
        assert_eq!(row["ciphertext"], "");
        assert!(
            row["signature"].as_str().is_some_and(|s| !s.is_empty()),
            "an unsigned tombstone is the most destructive forgery there is"
        );
    }

    #[tokio::test]
    async fn an_unsigned_row_never_reaches_the_network() {
        let stub = Stub::start(vec![Reply::empty(201)], 0).await;
        let unsigned = CloudItem::sealed("a1", b"ct", b"nc", "text", 1, "device-a");
        let err = client(&stub).upsert(TOKEN, &[unsigned]).await.unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(stub.request_count().await, 0);
    }

    #[tokio::test]
    async fn a_live_item_with_no_ciphertext_is_refused() {
        let stub = Stub::start(vec![Reply::empty(201)], 0).await;
        let mut empty = item("a1");
        empty.ciphertext = String::new();
        let err = client(&stub).upsert(TOKEN, &[empty]).await.unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn an_item_id_that_could_change_a_filter_is_refused() {
        let stub = Stub::start(vec![Reply::empty(201)], 0).await;
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
        let stub = Stub::start(vec![Reply::empty(201), Reply::json(401, "{}")], 2).await;
        let items: Vec<CloudItem> = (0..150).map(|i| item(&format!("id-{i}"))).collect();
        let err = client(&stub).upsert(TOKEN, &items).await.unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(stub.request_count().await, 2);
    }

    #[test]
    fn debug_for_the_client_shows_no_key_material() {
        let rest = SupabaseRest::new(
            CloudConfig::new("https://project.supabase.co", "anon-secret-looking-key").unwrap(),
        );
        assert!(!format!("{rest:?}").contains("anon-secret-looking-key"));
    }
}
