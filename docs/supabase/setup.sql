-- CopyPaste — one-shot Supabase provisioning script.
--
-- This is the concatenation of `schema.sql` + `rls-policies.sql` in the
-- correct order, so cloud sync can be provisioned with a SINGLE paste into
-- the Supabase SQL Editor instead of two separate runs.
--
-- It is also embedded into the CLI: `copypaste cloud setup-sql` prints this
-- exact text. The CLI and this file are kept in sync via `include_str!`.
--
-- The script is fully idempotent — safe to run more than once.
--
-- USAGE
--   1. Supabase dashboard → SQL Editor → New query.
--   2. Paste this entire file (or run `copypaste cloud setup-sql | pbcopy`).
--   3. Click Run. Expect "Success. No rows returned."
--   4. Add `clipboard_items` to the `supabase_realtime` publication
--      (Database → Replication) — DDL alone cannot toggle that.
--
-- After this, run `copypaste cloud test` to confirm connectivity.

-- ════════════════════════════════════════════════════════════════════════════
-- PART 1 — SCHEMA
-- ════════════════════════════════════════════════════════════════════════════

-- ── Extensions ────────────────────────────────────────────────────────────────

create extension if not exists "pgcrypto";   -- gen_random_uuid()

-- ── Table ─────────────────────────────────────────────────────────────────────

create table if not exists public.clipboard_items (
    -- Identity
    id                uuid           primary key default gen_random_uuid(),
    item_id           uuid           not null,

    -- Ownership (RLS pivots on this column)
    user_id           uuid           not null references auth.users (id) on delete cascade,
    device_id         text           not null,

    -- Content (encrypted client-side)
    content_type      text           not null,
    payload_ct        bytea,                     -- ChaCha20-Poly1305 ciphertext
    content_nonce     bytea,                     -- 24-byte XChaCha20 nonce
    content_hash      bytea,                     -- BLAKE3-256 of plaintext (32 bytes)
    blob_ref          text,                      -- optional large-blob CAS pointer
    is_sensitive      boolean        not null default false,

    -- LWW / CRDT metadata
    lamport_ts        bigint         not null,
    wall_time         bigint         not null,   -- Unix milliseconds
    expires_at        bigint,                    -- Unix milliseconds, nullable

    -- Provenance
    app_bundle_id     text,

    -- Server bookkeeping
    created_at        timestamptz    not null default now(),
    updated_at        timestamptz    not null default now()
);

comment on table  public.clipboard_items                  is 'Encrypted clipboard history, one row per item, replicated via Supabase Realtime.';
comment on column public.clipboard_items.user_id          is 'Owner — RLS scopes every operation to auth.uid().';
comment on column public.clipboard_items.device_id        is 'Origin device UUID (mirrors WireItem.origin_device_id).';
comment on column public.clipboard_items.payload_ct       is 'ChaCha20-Poly1305 ciphertext of the clipboard payload. Server never sees plaintext.';
comment on column public.clipboard_items.content_nonce    is '24-byte XChaCha20-Poly1305 nonce. Must be unique per (user_id, key).';
comment on column public.clipboard_items.content_hash     is 'BLAKE3-256 hash of plaintext for client-side dedup. Server cannot verify.';
comment on column public.clipboard_items.lamport_ts       is 'Lamport clock at the time of last write — drives LWW conflict resolution.';
comment on column public.clipboard_items.wall_time        is 'Wall-clock time (Unix ms) — tiebreaker when lamport_ts is equal.';

-- ── Indexes ───────────────────────────────────────────────────────────────────

create index if not exists clipboard_items_device_lamport_idx
    on public.clipboard_items (device_id, lamport_ts desc);

create index if not exists clipboard_items_user_created_idx
    on public.clipboard_items (user_id, created_at desc);

create index if not exists clipboard_items_expires_idx
    on public.clipboard_items (expires_at)
    where expires_at is not null;

create index if not exists clipboard_items_user_content_hash_idx
    on public.clipboard_items (user_id, content_hash)
    where content_hash is not null;

-- ── updated_at trigger ────────────────────────────────────────────────────────

create or replace function public.set_updated_at()
returns trigger
language plpgsql
as $$
begin
    new.updated_at := now();
    return new;
end;
$$;

drop trigger if exists clipboard_items_set_updated_at on public.clipboard_items;
create trigger clipboard_items_set_updated_at
    before update on public.clipboard_items
    for each row
    execute function public.set_updated_at();

-- ════════════════════════════════════════════════════════════════════════════
-- PART 2 — ROW-LEVEL SECURITY
-- ════════════════════════════════════════════════════════════════════════════
--
-- RLS pivots on `user_id` (FK to auth.users.id), NOT device_id: one Supabase
-- account = one CopyPaste trust circle. Every device that signs into the same
-- account reads/writes every other device's items for that account.

-- ── Enable RLS ────────────────────────────────────────────────────────────────

alter table public.clipboard_items enable row level security;
alter table public.clipboard_items force row level security;

-- ── Policies (drop-and-recreate so this script is idempotent) ──────────────────

drop policy if exists clipboard_items_select_own on public.clipboard_items;
drop policy if exists clipboard_items_insert_own on public.clipboard_items;
drop policy if exists clipboard_items_update_own on public.clipboard_items;
drop policy if exists clipboard_items_delete_own on public.clipboard_items;

create policy clipboard_items_select_own
    on public.clipboard_items
    for select
    to authenticated
    using (user_id = auth.uid());

create policy clipboard_items_insert_own
    on public.clipboard_items
    for insert
    to authenticated
    with check (user_id = auth.uid());

create policy clipboard_items_update_own
    on public.clipboard_items
    for update
    to authenticated
    using      (user_id = auth.uid())
    with check (user_id = auth.uid());

create policy clipboard_items_delete_own
    on public.clipboard_items
    for delete
    to authenticated
    using (user_id = auth.uid());

-- ── Default owner ─────────────────────────────────────────────────────────────
-- Lets clients insert without spelling out user_id; the default fires before
-- the RLS `with check`, auto-tagging the row with the caller's uid.

alter table public.clipboard_items
    alter column user_id set default auth.uid();

-- ── Anon role hardening ───────────────────────────────────────────────────────
-- Strip everything from the unauthenticated `anon` role so a missing JWT
-- can't even attempt a read.

revoke all on public.clipboard_items from anon;
grant  select, insert, update, delete on public.clipboard_items to authenticated;
