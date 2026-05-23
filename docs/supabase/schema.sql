-- CopyPaste — Supabase schema for `clipboard_items`
--
-- This DDL provisions the single table the `copypaste-supabase` Realtime
-- client subscribes to (`realtime:clipboard_items`). The column layout
-- mirrors `WireItem` in `crates/copypaste-sync/src/protocol.rs`:
--
--   pub struct WireItem {
--       id, item_id, content_type,
--       content (encrypted blob, base64 over JSON wire),
--       content_nonce (24-byte ChaCha20-Poly1305 nonce),
--       blob_ref,
--       is_sensitive,
--       lamport_ts, wall_time, expires_at,
--       app_bundle_id,
--       origin_device_id,
--   }
--
-- Conventions:
--   * `id` is the row PK and matches `WireItem.id`.
--   * `device_id` corresponds to `WireItem.origin_device_id`.
--   * Payload is stored as `bytea` (`payload_ct`) — the daemon encrypts
--     ChaCha20-Poly1305 client-side; the server never sees plaintext.
--   * `content_hash` is a 32-byte BLAKE3 digest of the *plaintext*, used
--     for client-side dedup. Optional (nullable for legacy rows).
--   * `lamport_ts` + `wall_time` give LWW conflict resolution semantics.
--
-- Run this in the Supabase SQL Editor before `rls-policies.sql`.

-- ── Extensions ────────────────────────────────────────────────────────────────

create extension if not exists "pgcrypto";   -- gen_random_uuid()

-- ── Table ─────────────────────────────────────────────────────────────────────

create table if not exists public.clipboard_items (
    -- Identity
    id                uuid           primary key default gen_random_uuid(),
    item_id           uuid           not null,

    -- Ownership (RLS pivots on this column — see rls-policies.sql)
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

-- Primary read path: "give me everything since I last synced from device X".
create index if not exists clipboard_items_device_lamport_idx
    on public.clipboard_items (device_id, lamport_ts desc);

-- Owner scan (Realtime/RLS hot path).
create index if not exists clipboard_items_user_created_idx
    on public.clipboard_items (user_id, created_at desc);

-- TTL cleanup probe.
create index if not exists clipboard_items_expires_idx
    on public.clipboard_items (expires_at)
    where expires_at is not null;

-- Dedup lookups (NULL-friendly — partial index skips legacy rows).
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
