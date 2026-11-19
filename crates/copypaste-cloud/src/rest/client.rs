//! The two requests, and the one place both auth headers are attached.
//!
//! Every query-shape decision that the cursor depends on is here — the
//! server-assigned position, the inclusive `gte`, the ascending compound
//! order, the clamped `limit` — and each carries the bug it prevents.

use std::fmt;
use std::future::Future;

use super::body::{read_capped, read_prefix, read_row_page, BodyError, RowPage};
use super::error::{classify, RestError};
use super::item::{format_position, validate_item_id, CloudItem, RowListing};
use super::{
    CONFLICT_TARGET, LISTING_COLUMNS, MAX_PAGE_LIMIT, REST_TIMEOUT, SELECT_COLUMNS, TABLE,
    UPSERT_CHUNK,
};
use crate::auth::transient_backoff;
use crate::{CloudConfig, HttpClient};
use backon::{ExponentialBuilder, Retryable};
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use tokio_util::bytes::Bytes;

/// Reply bytes a write drains before its connection is released.
const WRITE_REPLY_BYTES: usize = 1024;

/// PostgREST client for one Supabase project.
#[derive(Clone)]
pub struct SupabaseRest {
    pub(super) config: CloudConfig,
    pub(super) http: Client,
    retry: ExponentialBuilder,
}

/// The rows as one JSON array, allocated once at its exact length.
fn json_body(rows: &[CloudItem]) -> Result<Vec<u8>, RestError> {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    serde_json::to_writer(&mut count, rows).map_err(|_| RestError::Malformed)?;
    let mut body = Vec::with_capacity(count.0);
    serde_json::to_writer(&mut body, rows).map_err(|_| RestError::Malformed)?;
    Ok(body)
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
        Self::with_client(config, HttpClient::default())
    }

    /// Build a client for `config` that shares `http` with other clients.
    pub fn with_client(config: CloudConfig, http: HttpClient) -> Self {
        Self {
            config,
            http: http.0,
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

    /// One page of a listing in the order the server placed the rows, after a
    /// keyset cursor over `(updated_at, item_id)`. No payload travels.
    ///
    /// `updated_at` is assigned by the database on every write and no client
    /// can set it, so this order is the one rows reached the server in. A
    /// device's own stamp is not: a row uploaded late, or by a device whose
    /// clock is wrong, would sort behind or ahead of cursors that then never
    /// meet it.
    ///
    /// * `after_item_id = None` — the bound is **inclusive** on the position
    ///   (`updated_at=gte.…`).
    /// * `after_item_id = Some(id)` — everything strictly after the pair, as
    ///   `or=(updated_at.gt.P,and(updated_at.eq.P,item_id.gt.ID))`. Every row
    ///   of one upsert shares a position, so the pair is what pages through a
    ///   batch larger than a page (INV-N1, AT-24).
    ///
    /// Ascending is load-bearing: a forward cursor cannot drain a newest-first
    /// page (manifest 05 §4.4, AT-23). `limit` is clamped to
    /// [`MAX_PAGE_LIMIT`]; a full page means "there may be more".
    pub async fn list_since(
        &self,
        token: &str,
        position: i64,
        after_item_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<RowListing>, RestError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        let position = format_position(position);
        let mut query = vec![
            ("select", LISTING_COLUMNS.to_string()),
            ("order", PAGE_ORDER.to_string()),
            ("limit", limit.to_string()),
        ];
        match after_item_id {
            Some(item_id) => {
                validate_item_id(item_id)?;
                query.push(("or", keyset_after(&position, item_id)));
            }
            None => query.push(("updated_at", format!("gte.{position}"))),
        }
        let body = self
            .fetch_capped(
                token,
                &self.table_url(),
                &query,
                limit as usize * LISTING_ROW_BYTES,
            )
            .await?;
        let rows: Vec<RowListing> =
            serde_json::from_slice(&body).map_err(|_| RestError::Malformed)?;
        if rows.len() > limit as usize {
            return Err(RestError::Malformed);
        }
        tracing::debug!(rows = rows.len(), "listed a page");
        Ok(rows)
    }

    /// The rows with these ids, whole, in the listing's order. An id that is
    /// absent from a page that was not cut no longer exists; a page cut at
    /// [`PAGE_BYTES`](super::body::PAGE_BYTES) says nothing of the ids it did
    /// not reach.
    pub async fn fetch_rows(&self, token: &str, item_ids: &[String]) -> Result<RowPage, RestError> {
        if item_ids.is_empty() {
            return Ok(RowPage::default());
        }
        if item_ids.len() > MAX_PAGE_LIMIT as usize {
            return Err(RestError::InvalidItem {
                reason: "more rows were asked for than one page holds",
            });
        }
        for item_id in item_ids {
            validate_item_id(item_id)?;
        }
        let url = self.table_url();
        let query = [
            ("select", SELECT_COLUMNS.to_string()),
            ("item_id", format!("in.({})", item_ids.join(","))),
            ("order", PAGE_ORDER.to_string()),
            ("limit", item_ids.len().to_string()),
        ];
        let page = self
            .send_reading(
                token,
                || {
                    self.http
                        .get(url.clone())
                        .query(&query)
                        .header("Accept", "application/json")
                },
                read_row_page,
            )
            .await?;
        tracing::debug!(rows = page.rows.len(), cut = page.cut, "fetched rows");
        Ok(page)
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
            // Serialized once at its exact size and shared by every attempt: a
            // JSON tree or a growing buffer would copy each base64 ciphertext
            // again.
            let body = Bytes::from(json_body(chunk)?);
            self.send(token, || {
                self.http
                    .post(url.clone())
                    .query(&[("on_conflict", CONFLICT_TARGET)])
                    .header("Prefer", "resolution=merge-duplicates,return=minimal")
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
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
    pub(super) async fn send<F>(&self, token: &str, build: F) -> Result<(), RestError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        // Writes ask for `return=minimal`; whatever comes back is not used.
        self.send_reading(token, build, |response| async move {
            read_prefix(response, WRITE_REPLY_BYTES).await;
            Ok(())
        })
        .await
    }

    /// A GET whose JSON body is refused as soon as it passes `cap`.
    pub(super) async fn fetch_capped(
        &self,
        token: &str,
        url: &url::Url,
        query: &[(&str, String)],
        cap: usize,
    ) -> Result<Vec<u8>, RestError> {
        self.send_reading(
            token,
            || {
                self.http
                    .get(url.clone())
                    .query(query)
                    .header("Accept", "application/json")
            },
            |response| async move {
                read_capped(response, cap)
                    .await
                    .map_err(|error| match error {
                        BodyError::TooLarge => RestError::Malformed,
                        BodyError::Network(error) => RestError::from_reqwest(error),
                    })
            },
        )
        .await
    }

    /// [`SupabaseRest::send`], with the successful body consumed by `read`.
    pub(super) async fn send_reading<F, R, Fut, T>(
        &self,
        token: &str,
        build: F,
        read: R,
    ) -> Result<T, RestError>
    where
        F: Fn() -> reqwest::RequestBuilder,
        R: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = Result<T, RestError>>,
    {
        let build = &build;
        let read = &read;
        let attempt = || async move {
            let sent = build()
                .timeout(REST_TIMEOUT)
                .header("apikey", self.config.anon_key())
                .bearer_auth(token)
                .send()
                .await;

            match sent {
                Ok(response) => read(classify(response).await?).await,
                Err(err) => Err(RestError::from_reqwest(err)),
            }
        };

        attempt
            .retry(self.retry)
            .when(RestError::is_transient)
            .await
    }
}

/// The one order every page is read in.
const PAGE_ORDER: &str = "updated_at.asc,item_id.asc";

/// Room for one listed row: three columns of at most 128 characters, one of
/// them free text that JSON may escape six-fold, two numbers and the keys.
const LISTING_ROW_BYTES: usize = 2 * 1024;

/// PostgREST's spelling of "strictly after the pair `(updated_at, item_id)`".
///
/// `item_id` is `[A-Za-z0-9_-]` by [`validate_item_id`], checked by the caller,
/// and `position` is [`format_position`]'s digits and `-:.TZ`, so neither can
/// close the parenthesis or add a disjunct.
fn keyset_after(position: &str, item_id: &str) -> String {
    format!("(updated_at.gt.{position},and(updated_at.eq.{position},item_id.gt.{item_id}))")
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

    // -- list_since and fetch_rows -------------------------------------------

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

    /// 2023-11-14T22:13:20.000001Z.
    const POSITION: i64 = 1_700_000_000_000_001;

    #[tokio::test]
    async fn a_listing_asks_for_an_inclusive_bound_a_total_order_and_no_payload() {
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(query_param("select", LISTING_COLUMNS))
            .and(query_param("updated_at", "gte.2023-11-14T22:13:20.000001Z"))
            .and(query_param("order", "updated_at.asc,item_id.asc"))
            .and(query_param("limit", "20"));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .list_since(TOKEN, POSITION, None, 20)
            .await
            .expect("fetch");

        let request = stub.only_request().await;
        assert_eq!(request.method.as_str(), "GET");
        assert_eq!(request.url.path(), "/rest/v1/clipboard_items");

        let pairs = query_pairs(&request.url);
        assert_eq!(value_of(&pairs, "select"), LISTING_COLUMNS);
        assert!(
            !LISTING_COLUMNS.contains("ciphertext"),
            "a listing must not carry the payload"
        );
        assert_eq!(
            value_of(&pairs, "updated_at"),
            "gte.2023-11-14T22:13:20.000001Z",
            "a strict `gt` loses every row at the boundary position"
        );
        assert!(
            !pairs.iter().any(|(key, _)| key == "created_at"),
            "a device's own stamp must not bound the page: {pairs:?}"
        );
        // Ascending: a forward cursor cannot drain a newest-first page.
        assert_eq!(value_of(&pairs, "order"), "updated_at.asc,item_id.asc");
        assert_eq!(value_of(&pairs, "limit"), "20");
    }

    #[tokio::test]
    async fn a_known_tie_break_becomes_a_compound_keyset() {
        // INV-N1 / AT-24. Every row of one upsert shares a position, so the
        // position alone cannot be paged past once a batch outgrows a page.
        let keyset = "(updated_at.gt.2023-11-14T22:13:20.000001Z,\
                      and(updated_at.eq.2023-11-14T22:13:20.000001Z,item_id.gt.item-42))";
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(query_param("or", keyset))
            .and(query_param("order", "updated_at.asc,item_id.asc"));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .list_since(TOKEN, POSITION, Some("item-42"), 20)
            .await
            .expect("fetch");

        let request = stub.only_request().await;
        let pairs = query_pairs(&request.url);
        assert_eq!(value_of(&pairs, "or"), keyset);
        assert!(
            !pairs.iter().any(|(k, _)| k == "updated_at"),
            "the keyset replaces the bare bound rather than joining it: {pairs:?}"
        );
        assert_eq!(value_of(&pairs, "order"), "updated_at.asc,item_id.asc");
    }

    #[tokio::test]
    async fn a_hostile_tie_break_cannot_rewrite_the_filter() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 0).await;
        let err = client(&stub)
            .list_since(TOKEN, 1, Some("a),or=(deleted.eq.false"), 10)
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(stub.request_count().await, 0);
    }

    #[tokio::test]
    async fn a_listing_sends_both_the_apikey_and_the_user_token() {
        let mock = Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .and(header_match("apikey", ANON))
            .and(header_match("authorization", format!("Bearer {TOKEN}")));
        let stub = Stub::start_matching(mock, vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .list_since(TOKEN, 0, None, 10)
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
            .list_since(TOKEN, 0, None, 100_000)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "limit"),
            MAX_PAGE_LIMIT.to_string()
        );

        let stub = Stub::start(vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .list_since(TOKEN, 0, None, 0)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "limit"),
            "1"
        );
    }

    #[tokio::test]
    async fn a_negative_position_is_floored_at_the_epoch() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 1).await;
        client(&stub)
            .list_since(TOKEN, -42, None, 10)
            .await
            .expect("fetch");
        assert_eq!(
            value_of(&query_pairs(&stub.only_request().await.url), "updated_at"),
            "gte.1970-01-01T00:00:00.000000Z"
        );
    }

    #[tokio::test]
    async fn rows_are_fetched_by_id_and_round_trip() {
        let body = r#"[
            {"item_id":"a1","ciphertext":"c2VhbGVk","nonce":"bm9uY2U=",
             "content_type":"text","created_at":1700000000001,"deleted":false,
             "origin_device_id":"device-b","signature":"c2ln",
             "updated_at":"2023-11-14T22:13:20.000001+00:00"},
            {"item_id":"a2","ciphertext":"","nonce":"",
             "content_type":"text","created_at":1700000000000,"deleted":true,
             "origin_device_id":"device-c"}
        ]"#;
        let stub = Stub::start(vec![Reply::json(200, body)], 1).await;
        let rows = client(&stub)
            .fetch_rows(TOKEN, &["a1".into(), "a2".into()])
            .await
            .expect("fetch")
            .rows;

        let pairs = query_pairs(&stub.only_request().await.url);
        assert_eq!(value_of(&pairs, "select"), SELECT_COLUMNS);
        assert_eq!(value_of(&pairs, "item_id"), "in.(a1,a2)");
        assert_eq!(value_of(&pairs, "order"), "updated_at.asc,item_id.asc");
        assert_eq!(value_of(&pairs, "limit"), "2");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].item_id, "a1");
        assert_eq!(rows[0].ciphertext_bytes().expect("base64"), b"sealed");
        assert_eq!(rows[0].nonce_bytes().expect("base64"), b"nonce");
        assert_eq!(rows[0].signature, "c2ln");
        assert_eq!(rows[0].position, Some(POSITION));
        assert!(rows[1].deleted, "a tombstone must survive the round trip");
        assert!(rows[1].ciphertext.is_empty());
        assert!(
            rows[1].signature.is_empty(),
            "an absent signature must decode as absent, not fail the page"
        );
    }

    #[tokio::test]
    async fn an_id_that_could_change_the_filter_is_never_asked_for() {
        let stub = Stub::start(vec![Reply::json(200, "[]")], 0).await;
        let err = client(&stub)
            .fetch_rows(TOKEN, &["a1".into(), "b),or=(deleted.eq.false".into()])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::InvalidItem { .. }), "{err:?}");
        assert_eq!(stub.request_count().await, 0);
    }

    #[tokio::test]
    async fn a_listing_reads_what_a_row_claims_and_where_it_was_placed() {
        let body = r#"[{"item_id":"a1","created_at":7,"deleted":false,
            "origin_device_id":"device-b","signature":"c2ln",
            "updated_at":"2023-11-14T22:13:20.000001+00:00"}]"#;
        let stub = Stub::start(vec![Reply::json(200, body)], 1).await;
        let listed = client(&stub)
            .list_since(TOKEN, 0, None, 10)
            .await
            .expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].item_id, "a1");
        assert_eq!(listed[0].created_at, 7);
        assert_eq!(listed[0].position, POSITION);
    }

    #[tokio::test]
    async fn a_position_that_cannot_be_read_fails_the_page() {
        // Skipping such a row would lose it while sync still looked healthy.
        for updated_at in [r#""updated_at":"infinity","#, ""] {
            let body = format!(
                r#"[{{"item_id":"a1","created_at":7,"deleted":false,{updated_at}
                    "origin_device_id":"device-b","signature":"c2ln"}}]"#
            );
            let stub = Stub::start(vec![Reply::json(200, &body)], 1).await;
            let err = client(&stub)
                .list_since(TOKEN, 0, None, 10)
                .await
                .unwrap_err();
            assert!(
                matches!(err, RestError::Malformed),
                "{updated_at:?}: {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_body_that_is_not_a_row_array_is_malformed() {
        let stub = Stub::start(vec![Reply::json(200, r#"{"message":"nope"}"#)], 1).await;
        let err = client(&stub)
            .list_since(TOKEN, 0, None, 10)
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
        let expected_nonce = live.nonce.clone();
        live.deleted = false;
        client(&stub).upsert(TOKEN, &[live]).await.expect("upsert");

        let body = request_json(&stub.only_request().await);
        let rows = body.as_array().expect("an array of rows");
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["item_id"], "a1");
        assert_eq!(row["ciphertext"], BASE64.encode(b"sealed-bytes"));
        assert_eq!(row["nonce"], expected_nonce);
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
        let nonce: [u8; 12] = rand::random();
        let unsigned = CloudItem::sealed("a1", b"ct", &nonce, "text", 1, "device-a");
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
