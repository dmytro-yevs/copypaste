//! PostgREST access to the one table this crate uses, `clipboard_items`.
//!
//! # What the deployment must look like
//!
//! The SQL below is the contract this module codes against. It is written out
//! here rather than in a `docs/` file that drifts, because two things in it are
//! load-bearing for code in this module: the unique index (without it the upsert
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
//! -- The read path: `user_id` from RLS, then the (created_at asc, item_id
//! -- desc) keyset this module orders on.
//! create index clipboard_items_user_created_idx
//!     on public.clipboard_items (user_id, created_at, item_id);
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
//! watermark belong to the sync engine; this module is the transport it uses.
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
//!   text any of them carries is a `&'static str` written in this module.
//!
//! The constants below stay in this file, next to the SQL they describe: each
//! one is a name for something in that schema, and moving them away from it is
//! how a column list and a `select=` drift apart.

use std::time::Duration;

pub mod client;
pub mod error;
pub mod item;

#[cfg(test)]
mod testkit;

pub use client::SupabaseRest;
pub use error::RestError;
pub use item::CloudItem;

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
