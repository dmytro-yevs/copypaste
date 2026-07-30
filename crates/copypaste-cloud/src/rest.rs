//! PostgREST access to the one table this crate uses, `clipboard_items`.
//!
//! # What the deployment must look like
//!
//! The SQL below is the contract this module codes against. It is written out
//! here rather than in a `docs/` file that drifts, because two things in it are
//! load-bearing for code in this file: the unique index (without it the upsert
//! conflict target does not resolve and every replay fails at runtime), and the
//! `user_id` default (without it the client would have to spell out `user_id`,
//! which the RLS `with check` would then have to be trusted to police).
//!
//! ```sql
//! -- ------------------------------------------------------------------
//! -- Table
//! -- ------------------------------------------------------------------
//! create table public.clipboard_items (
//!     id                uuid primary key default gen_random_uuid(),
//!     -- RLS pivot. The default fires before the `with check`, so clients
//!     -- never send this column.
//!     user_id           uuid not null default auth.uid()
//!                           references auth.users (id) on delete cascade,
//!     -- CRDT identity: stable across devices, the upsert conflict key.
//!     item_id           text not null,
//!     -- Sealed on the client. base64 in a `text` column, deliberately not
//!     -- `bytea`: assigning a bare base64 string to a bytea column stores the
//!     -- ASCII bytes of the base64 text and reads back as `\x…` hex, which is
//!     -- what made cloud download fail outright in v1 (port manifest 05 §4.5).
//!     -- A text column has one encoding, and it is the one the client wrote.
//!     -- NULL on a tombstone.
//!     ciphertext        text,
//!     nonce             text,
//!     content_type      text not null,
//!     -- Version wall clock, ms since epoch, client-supplied. This is the
//!     -- value the poll cursor pages on, so every writer must restamp it on
//!     -- every mutation (see `SupabaseRest::fetch_since`).
//!     created_at        bigint not null,
//!     -- Tombstone flag. Always sent explicitly, including `false`: omitting
//!     -- it lets the column default win on a merge-duplicates upsert and
//!     -- resurrects a deleted item (manifest T-5).
//!     deleted           boolean not null default false,
//!     origin_device_id  text not null,
//!     -- Server-assigned, for retention. A retention job must order on this,
//!     -- never on the client-supplied `created_at`, or a device with a forged
//!     -- clock can escape eviction (manifest §5.1 row 4a).
//!     inserted_at       timestamptz not null default now(),
//!     updated_at        timestamptz not null default now()
//! );
//!
//! -- The upsert conflict target. PostgREST needs a unique index to resolve
//! -- `on_conflict`; without this, a replayed batch is a 409, not a no-op.
//! -- Scoped to (user_id, item_id) rather than item_id alone because this is a
//! -- shared table (manifest §4.2).
//! create unique index clipboard_items_user_item_uidx
//!     on public.clipboard_items (user_id, item_id);
//!
//! -- The read path: `user_id` from RLS, then the (created_at desc, item_id
//! -- desc) keyset this module orders on.
//! create index clipboard_items_user_created_idx
//!     on public.clipboard_items (user_id, created_at desc, item_id desc);
//!
//! create or replace function public.touch_updated_at() returns trigger
//!     language plpgsql as $$
//! begin
//!     new.updated_at = now();
//!     return new;
//! end;
//! $$;
//!
//! create trigger clipboard_items_touch_updated_at
//!     before update on public.clipboard_items
//!     for each row execute function public.touch_updated_at();
//!
//! -- ------------------------------------------------------------------
//! -- Row-level security — the second layer. The first is that `ciphertext`
//! -- is opaque: a misconfigured policy exposes rows that are still
//! -- unreadable. That is why this is defence in depth and not the defence.
//! -- ------------------------------------------------------------------
//! alter table public.clipboard_items enable row level security;
//! -- `force` so the table owner is policed too; without it a definer-context
//! -- function bypasses every policy below.
//! alter table public.clipboard_items force row level security;
//!
//! -- Postgres' default ACL grants ALL to PUBLIC, which the `anon` role
//! -- inherits. Revoke before granting, or the publishable key alone can read.
//! revoke all on public.clipboard_items from anon, public;
//! grant select, insert, update, delete on public.clipboard_items to authenticated;
//!
//! create policy clipboard_items_select on public.clipboard_items
//!     for select to authenticated
//!     using (user_id = auth.uid());
//!
//! create policy clipboard_items_insert on public.clipboard_items
//!     for insert to authenticated
//!     with check (user_id = auth.uid());
//!
//! -- Both clauses: `using` decides which rows may be targeted, `with check`
//! -- decides what they may be updated to. Only `using` would let a row be
//! -- rewritten into another account.
//! create policy clipboard_items_update on public.clipboard_items
//!     for update to authenticated
//!     using (user_id = auth.uid())
//!     with check (user_id = auth.uid());
//!
//! create policy clipboard_items_delete on public.clipboard_items
//!     for delete to authenticated
//!     using (user_id = auth.uid());
//! ```
//!
//! One account is one trust circle: RLS pivots on `user_id`, not on a device,
//! so every device signed into the account reads every other device's rows.
//! That is the model the whole design assumes.
//!
//! # What this module does not do
//!
//! It does not merge. It fetches a page, writes a batch, and reports what the
//! server said. Ordering, tie-breaks, tombstone precedence and the download
//! watermark belong to the sync engine; this file is the transport it uses.
//!
//! # Error policy
//!
//! * `401` is [`RestError::Unauthorized`] and is **never retried here**. The
//!   caller refreshes the token once and retries once; a client that retried a
//!   401 on its own would spin forever against a refresh that keeps handing
//!   back a dead token (manifest AT-36).
//! * `429` is surfaced with its `Retry-After` rather than slept on, for the
//!   same reason: the caller owns the schedule.
//! * `5xx` and network faults retry under the crate's single `backoff` policy.
//! * No error variant can hold a filesystem path or a token — the only free
//!   text any of them carries is a `&'static str` written in this file.

use std::fmt;
use std::time::Duration;

use backoff::backoff::Backoff;
use backoff::future::retry;
use backoff::{Error as Retryable, ExponentialBackoff};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::auth::{now_ms, retry_after_secs, transient_backoff};
use crate::CloudConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The one table.
pub const TABLE: &str = "clipboard_items";

/// Explicit column list. Naming the columns rather than `select=*` means a
/// column added to the table later cannot change what this client parses.
pub const SELECT_COLUMNS: &str =
    "item_id,ciphertext,nonce,content_type,created_at,deleted,origin_device_id";

/// Upper bound on a page, whatever the caller asks for.
///
/// A page is a bounded unit of work and a bounded allocation; an unbounded
/// `limit` from a caller would be neither.
pub const MAX_PAGE_LIMIT: u32 = 200;

/// Rows per upsert request.
///
/// Batches keep the round-trip count down; chunking keeps any one request
/// inside the backend's body limit and keeps a retry cheap.
pub const UPSERT_CHUNK: usize = 100;

/// Item ids per tombstone request. Smaller than [`UPSERT_CHUNK`] because these
/// go into the query string, which has a much lower practical ceiling.
pub const TOMBSTONE_CHUNK: usize = 50;

/// The conflict target, and the unique index it needs.
///
/// The manifest's §4.2 gap: uniqueness is scoped to `(user_id, item_id)` on a
/// shared table, so `on_conflict` must name both columns. `user_id` is not in
/// the request body — the column default `auth.uid()` fills it before the
/// conflict is inferred, which is exactly why the default has to exist.
pub const CONFLICT_TARGET: &str = "user_id,item_id";

/// Per-request timeout. Applied per request so there is no fallible client
/// builder and therefore no "fall back to a client with no timeout" branch.
pub const REST_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

/// One row as it travels to and from the backend.
///
/// `Debug` is derived: every field here is metadata or ciphertext, and none of
/// it is a secret the backend does not already hold. Plaintext never reaches
/// this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudItem {
    /// Stable across devices; the upsert conflict key. Restricted to
    /// `[A-Za-z0-9_-]` so it can be placed in a PostgREST filter without
    /// quoting rules deciding what the filter means — see [`RestError::InvalidItem`].
    pub item_id: String,
    /// base64. The server cannot read this. Empty on a tombstone.
    pub ciphertext: String,
    /// base64.
    pub nonce: String,
    pub content_type: String,
    /// Version wall clock, ms since epoch. **Not the row's birth time**: it is
    /// the timestamp the poll cursor pages on, so a writer must restamp it on
    /// every mutation. A tombstone that kept the original creation time would
    /// sort below the watermark of every device that already saw the item, and
    /// the deletion would never propagate.
    pub created_at: i64,
    /// Tombstone flag. Always serialised, including `false` (manifest T-5).
    pub deleted: bool,
    pub origin_device_id: String,
}

impl CloudItem {
    /// A live row, from already-sealed bytes.
    ///
    /// The base64 encoding happens here so no caller has to pick an alphabet:
    /// one encoder, matching the one [`CloudItem::ciphertext_bytes`] decodes
    /// with.
    pub fn sealed(
        item_id: impl Into<String>,
        ciphertext: &[u8],
        nonce: &[u8],
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: BASE64.encode(ciphertext),
            nonce: BASE64.encode(nonce),
            content_type: content_type.into(),
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// A live row from a payload that is *already* base64, which is the shape
    /// [`crate::crypto::encrypt_row`] hands back.
    ///
    /// Note the argument order: `ciphertext` first, then `nonce`. The crypto
    /// side returns the pair the other way round, and a swap here would be
    /// silent — every row would encrypt fine and fail to open on the peer.
    pub fn from_sealed_b64(
        item_id: impl Into<String>,
        ciphertext_b64: impl Into<String>,
        nonce_b64: impl Into<String>,
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: ciphertext_b64.into(),
            nonce: nonce_b64.into(),
            content_type: content_type.into(),
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// A tombstone: a version of the item with the payload gone.
    ///
    /// Carrying no ciphertext is a hard precondition, not a nicety — a
    /// tombstone that shipped a stale payload would leak content the user
    /// deleted (manifest T-4).
    pub fn tombstone(
        item_id: impl Into<String>,
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: String::new(),
            nonce: String::new(),
            content_type: content_type.into(),
            created_at,
            deleted: true,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// Decode the sealed payload.
    pub fn ciphertext_bytes(&self) -> Result<Vec<u8>, RestError> {
        BASE64
            .decode(&self.ciphertext)
            .map_err(|_| RestError::Malformed)
    }

    /// Decode the nonce.
    pub fn nonce_bytes(&self) -> Result<Vec<u8>, RestError> {
        BASE64.decode(&self.nonce).map_err(|_| RestError::Malformed)
    }

    /// Client-side preconditions, checked before anything is sent.
    fn validate(&self) -> Result<(), RestError> {
        validate_item_id(&self.item_id)?;
        if self.deleted && !self.ciphertext.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "a tombstone must not carry ciphertext",
            });
        }
        if !self.deleted && self.ciphertext.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "a live item must carry ciphertext",
            });
        }
        if self.content_type.is_empty() || self.origin_device_id.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "content_type and origin_device_id are required",
            });
        }
        Ok(())
    }
}

/// An `item_id` goes into a PostgREST filter (`item_id=in.(…)`), where quoting
/// and commas are syntax. Rather than inventing an escaping scheme, restrict
/// the identifier to characters that have no meaning there — uuids, which is
/// what these are, fit comfortably.
fn validate_item_id(item_id: &str) -> Result<(), RestError> {
    if item_id.is_empty() {
        return Err(RestError::InvalidItem {
            reason: "item_id must not be empty",
        });
    }
    let ok = item_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(RestError::InvalidItem {
            reason: "item_id must be [A-Za-z0-9_-]",
        })
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a PostgREST call failed.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    /// `401` — the access token was refused.
    ///
    /// Distinct on purpose: this is the caller's cue to refresh **once** and
    /// retry **once**. It is never retried inside this module.
    #[error("the access token was refused")]
    Unauthorized,

    /// `403` — authenticated, but row-level security refused. Refreshing the
    /// token cannot help; the deployment or the row is wrong.
    #[error("the account is not allowed to touch these rows")]
    Forbidden,

    /// `429`, with `Retry-After` in its delta-seconds form when present.
    #[error("the backend is rate limiting this client")]
    RateLimited { retry_after_secs: Option<u64> },

    /// The request never got an answer. The URL is stripped before the error is
    /// kept.
    #[error("could not reach the sync backend")]
    Network(#[source] reqwest::Error),

    /// `5xx`. Transient: retried under the shared policy before it surfaces.
    #[error("the sync backend returned status {status}")]
    Server { status: u16 },

    /// A `4xx` that is not 401, 403 or 429 — most usefully a `409`, which means
    /// the unique index behind [`CONFLICT_TARGET`] is missing from the
    /// deployment.
    #[error("the sync backend rejected the request (status {status})")]
    Rejected { status: u16 },

    /// A 2xx whose body was not the shape PostgREST promises, or a row whose
    /// base64 does not decode.
    #[error("the sync backend returned an unexpected response")]
    Malformed,

    /// A client-side precondition failed and nothing was sent. `reason` is a
    /// `&'static str` written in this file, so it cannot carry a path, a token
    /// or user content.
    #[error("item rejected before sending: {reason}")]
    InvalidItem { reason: &'static str },
}

impl RestError {
    fn from_reqwest(err: reqwest::Error) -> Self {
        RestError::Network(err.without_url())
    }

    fn is_transient(&self) -> bool {
        matches!(self, RestError::Network(_) | RestError::Server { .. })
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

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
    /// The order is `(created_at desc, item_id desc)` — a compound key with no
    /// ties, so paging is deterministic. `limit` is clamped to
    /// [`MAX_PAGE_LIMIT`]; a full page means "there may be more", and the
    /// caller should drain rather than wait for the next tick.
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
            ("order", "created_at.desc,item_id.desc".to_string()),
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
    /// is refused outright by the policies above.
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
/// Values are known to be `[A-Za-z0-9_-]` by [`validate_item_id`], so there is
/// no quoting to get wrong.
fn in_list(item_ids: &[String]) -> String {
    format!("in.({})", item_ids.join(","))
}

/// Map a response to its body, or to the error its status means.
async fn classify(response: Response) -> Result<String, RestError> {
    let status = response.status();
    if status.is_success() {
        return response.text().await.map_err(|_| RestError::Malformed);
    }

    let retry_after = retry_after_secs(response.headers());
    let code = status.as_u16();
    let detail = truncate_body(&response.text().await.unwrap_or_default());

    match status {
        StatusCode::UNAUTHORIZED => {
            tracing::debug!(status = code, "access token refused");
            Err(RestError::Unauthorized)
        }
        StatusCode::FORBIDDEN => {
            tracing::warn!(status = code, detail = %detail, "row-level security refused the request");
            Err(RestError::Forbidden)
        }
        StatusCode::TOO_MANY_REQUESTS => {
            tracing::warn!(status = code, retry_after_secs = ?retry_after, "rate limited");
            Err(RestError::RateLimited {
                retry_after_secs: retry_after,
            })
        }
        StatusCode::CONFLICT => {
            tracing::error!(
                status = code,
                detail = %detail,
                "upsert conflict did not resolve — the deployment is probably missing the \
                 unique index on (user_id, item_id)"
            );
            Err(RestError::Rejected { status: code })
        }
        _ if status.is_server_error() => {
            tracing::warn!(status = code, detail = %detail, "sync backend faulted");
            Err(RestError::Server { status: code })
        }
        _ => {
            tracing::warn!(status = code, detail = %detail, "sync backend rejected the request");
            Err(RestError::Rejected { status: code })
        }
    }
}

/// A short snippet of an error body, for logs only. PostgREST error bodies are
/// JSON with `message`/`details`/`hint`, but a proxy in front of it can return
/// anything, so this does not assume a shape.
fn truncate_body(body: &str) -> String {
    const MAX: usize = 200;
    let body = body.trim();
    if body.is_empty() {
        return "<empty body>".to_string();
    }
    if body.len() <= MAX {
        return body.to_string();
    }
    let mut end = MAX;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &body[..end])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::stub::{Reply, Stub};
    use backoff::ExponentialBackoffBuilder;

    const ANON: &str = "anon-key-abc";
    const TOKEN: &str = "user-access-token";

    fn fast_retry() -> ExponentialBackoff {
        ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(1))
            .with_max_interval(Duration::from_millis(2))
            .with_max_elapsed_time(Some(Duration::from_millis(10)))
            .build()
    }

    fn client(stub: &Stub) -> SupabaseRest {
        SupabaseRest::new(CloudConfig {
            url: stub.base_url.clone(),
            anon_key: ANON.to_string(),
        })
        .with_retry_policy(fast_retry())
    }

    fn item(id: &str) -> CloudItem {
        CloudItem::sealed(
            id,
            b"sealed-bytes",
            b"nonce12",
            "text",
            1_700_000_000_000,
            "device-a",
        )
    }

    /// Split a captured `?a=b&c=d` target into pairs, percent-decoded enough
    /// for assertions to read naturally.
    fn query_pairs(target: &str) -> Vec<(String, String)> {
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        query
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (percent_decode(k), percent_decode(v))
            })
            .collect()
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.replace('+', " ").into_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let Ok(byte) = u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"),
                    16,
                ) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).to_string()
    }

    fn value_of(pairs: &[(String, String)], key: &str) -> String {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("no `{key}` in query: {pairs:?}"))
    }

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
        assert_eq!(value_of(&pairs, "order"), "created_at.desc,item_id.desc");
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

    // -- status handling ----------------------------------------------------

    #[tokio::test]
    async fn a_401_is_distinct_and_is_not_retried() {
        let stub = Stub::start(vec![Reply::json(401, r#"{"message":"JWT expired"}"#)]).await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(
            stub.request_count(),
            1,
            "the caller refreshes once and retries once; retrying here would spin"
        );
    }

    #[tokio::test]
    async fn a_401_on_the_write_path_is_the_same_error() {
        let stub = Stub::start(vec![Reply::json(401, "{}")]).await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(stub.request_count(), 1);
    }

    #[tokio::test]
    async fn a_403_is_not_confused_with_a_401() {
        let stub = Stub::start(vec![Reply::json(403, r#"{"message":"RLS"}"#)]).await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        assert!(
            matches!(err, RestError::Forbidden),
            "refreshing a token cannot fix a policy refusal: {err:?}"
        );
    }

    #[tokio::test]
    async fn a_429_carries_retry_after_and_is_not_slept_on_here() {
        let stub = Stub::start(vec![Reply::json(429, "{}").with_header("retry-after", "17")]).await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        match err {
            RestError::RateLimited { retry_after_secs } => assert_eq!(retry_after_secs, Some(17)),
            other => panic!("expected RateLimited, got {other:?}"),
        }
        assert_eq!(stub.request_count(), 1);
    }

    #[tokio::test]
    async fn a_429_without_a_header_still_says_rate_limited() {
        let stub = Stub::start(vec![Reply::json(429, "{}")]).await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                RestError::RateLimited {
                    retry_after_secs: None
                }
            ),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_5xx_is_retried_and_then_succeeds() {
        let stub = Stub::start(vec![Reply::text(503, "down"), Reply::json(200, "[]")]).await;
        let rows = client(&stub)
            .fetch_since(TOKEN, 0, 10)
            .await
            .expect("fetch");
        assert!(rows.is_empty());
        assert_eq!(stub.request_count(), 2);
    }

    #[tokio::test]
    async fn a_persistent_5xx_gives_up_with_the_status() {
        let stub = Stub::start(vec![Reply::text(500, "boom")]).await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        assert!(matches!(err, RestError::Server { status: 500 }), "{err:?}");
        assert!(stub.request_count() > 1);
    }

    #[tokio::test]
    async fn a_409_says_the_conflict_target_did_not_resolve() {
        let stub = Stub::start(vec![Reply::json(
            409,
            r#"{"code":"23505","message":"duplicate key value violates unique constraint"}"#,
        )])
        .await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, RestError::Rejected { status: 409 }),
            "{err:?}"
        );
        assert_eq!(stub.request_count(), 1, "a 409 is not transient");
    }

    #[tokio::test]
    async fn a_partial_batch_stops_at_the_first_failing_chunk() {
        let stub = Stub::start(vec![Reply::empty(201), Reply::json(401, "{}")]).await;
        let items: Vec<CloudItem> = (0..150).map(|i| item(&format!("id-{i}"))).collect();
        let err = client(&stub).upsert(TOKEN, &items).await.unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(stub.request_count(), 2);
    }

    // -- base64 -------------------------------------------------------------

    #[test]
    fn sealed_bytes_round_trip_through_base64() {
        let ciphertext: Vec<u8> = (0u8..=255).collect();
        let nonce = [7u8; 12];
        let row = CloudItem::sealed("a1", &ciphertext, &nonce, "image", 1, "device-a");

        assert_eq!(row.ciphertext_bytes().expect("decode"), ciphertext);
        assert_eq!(row.nonce_bytes().expect("decode"), nonce);
        assert!(!row.deleted);
        // Standard alphabet with padding — one encoder for the whole crate.
        assert_eq!(row.nonce, BASE64.encode(nonce));
    }

    #[test]
    fn junk_base64_decodes_to_malformed_rather_than_panicking() {
        let mut row = CloudItem::sealed("a1", b"x", b"y", "text", 1, "device-a");
        row.ciphertext = "not base64 !!".to_string();
        assert!(matches!(row.ciphertext_bytes(), Err(RestError::Malformed)));
    }

    #[test]
    fn a_row_can_be_built_from_payloads_that_are_already_base64() {
        let sealed = CloudItem::sealed("a1", b"ct", b"nc", "text", 9, "device-a");
        let same = CloudItem::from_sealed_b64(
            "a1",
            BASE64.encode(b"ct"),
            BASE64.encode(b"nc"),
            "text",
            9,
            "device-a",
        );
        assert_eq!(sealed, same, "the two constructors must agree");
        same.validate().expect("valid row");
    }

    #[test]
    fn a_constructed_tombstone_carries_no_payload() {
        let row = CloudItem::tombstone("a1", "text", 5, "device-a");
        assert!(row.deleted);
        assert!(row.ciphertext.is_empty());
        assert!(row.nonce.is_empty());
        row.validate().expect("a tombstone is a valid row to send");
    }

    #[test]
    fn a_row_serialises_with_exactly_the_columns_the_table_has() {
        let json = serde_json::to_value(item("a1")).expect("serialise");
        let object = json.as_object().expect("object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "ciphertext",
                "content_type",
                "created_at",
                "deleted",
                "item_id",
                "nonce",
                "origin_device_id",
            ]
        );
        let mut selected: Vec<&str> = SELECT_COLUMNS.split(',').collect();
        selected.sort_unstable();
        assert_eq!(keys, selected, "what we write is what we read back");
    }

    // -- no paths, no tokens ------------------------------------------------

    #[tokio::test]
    async fn errors_carry_neither_a_path_nor_a_token() {
        let stub = Stub::start(vec![Reply::json(
            401,
            r#"{"message":"failed at /Users/dmytro/copypaste.db with token user-access-token"}"#,
        )])
        .await;
        let err = client(&stub).fetch_since(TOKEN, 0, 10).await.unwrap_err();
        let rendered = format!("{err} {err:?}");
        assert!(!rendered.contains("/Users/"), "path leaked: {rendered}");
        assert!(!rendered.contains(TOKEN), "token leaked: {rendered}");
    }

    #[tokio::test]
    async fn a_network_failure_reports_no_url() {
        let rest = SupabaseRest::new(CloudConfig {
            url: "http://127.0.0.1:1".into(),
            anon_key: ANON.into(),
        })
        .with_retry_policy(fast_retry());
        let err = rest.fetch_since(TOKEN, 0, 10).await.unwrap_err();
        assert!(matches!(err, RestError::Network(_)), "{err:?}");
        assert!(!format!("{err}").contains('/'));
    }

    #[test]
    fn debug_for_the_client_shows_no_key_material() {
        let rest = SupabaseRest::new(CloudConfig {
            url: "https://project.supabase.co".into(),
            anon_key: "anon-secret-looking-key".into(),
        });
        assert!(!format!("{rest:?}").contains("anon-secret-looking-key"));
    }

    #[test]
    fn an_error_body_is_truncated_for_the_log() {
        let long = "y".repeat(4000);
        let detail = truncate_body(&long);
        assert!(detail.len() <= 204);
        assert!(detail.ends_with('…'));
        assert_eq!(truncate_body("  "), "<empty body>");
    }
}
