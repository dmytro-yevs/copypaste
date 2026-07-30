-- ---------------------------------------------------------------------------
-- clipboard_items: the one table copypaste-cloud talks to.
--
-- The contract is `crates/copypaste-cloud/src/rest/mod.rs`. Column names, the
-- `select=` list and the page ordering are fixed by that module; this file is
-- the deployment side of the same contract and must not drift from it.
--
-- The server never sees plaintext. `ciphertext` is sealed on the device with
-- XChaCha20-Poly1305 under an Argon2id key derived from a passphrase that never
-- leaves it. There is deliberately no content column, no plaintext column, no
-- search index over content and no server-side key anywhere in this schema.
-- ---------------------------------------------------------------------------

create table public.clipboard_items (
    -- Row PK. Per-device, never used for cross-device comparison (manifest 05
    -- R-ID-1). The client neither sends nor selects it.
    id                uuid primary key default gen_random_uuid(),

    -- RLS pivot. The default fires before the `with check`, so clients never
    -- send this column and the policy is what proves they could not have lied
    -- about it. One account is one trust circle (manifest 05 §4.3).
    user_id           uuid not null default auth.uid()
                          references auth.users (id) on delete cascade,

    -- CRDT identity: stable across devices, the upsert conflict key.
    item_id           text not null,

    -- Sealed on the client, base64 in a `text` column. Deliberately not
    -- `bytea`: assigning a bare base64 string to bytea stores the ASCII bytes
    -- of the base64 text and reads back as `\x…` hex, which is what made cloud
    -- download fail outright in v1 (manifest 05 §4.5, AT-45). A text column has
    -- one encoding and it is the one the client wrote.
    -- Empty on a tombstone written by the upsert path, NULL on one written by
    -- the PATCH path; both spellings are what the client actually sends.
    ciphertext        text,
    nonce             text,

    content_type      text not null,

    -- Version wall clock, ms since epoch, client-supplied. This is the value
    -- the poll cursor pages on, so every writer restamps it on every mutation
    -- (`SupabaseRest::fetch_since`). Not the row's birth time — see
    -- `inserted_at` for that.
    created_at        bigint not null,

    -- Tombstone flag. Always sent explicitly, including `false`: omitting it
    -- lets the column default win on a merge-duplicates upsert and resurrects a
    -- deleted item (manifest 05 T-5).
    deleted           boolean not null default false,

    origin_device_id  text not null,

    -- HMAC-SHA256, base64, over every column above, under a key HKDF-derived
    -- from the sync key (`crates/copypaste-cloud/src/crypto/sign.rs`).
    --
    -- This database cannot produce it and cannot check it, and that is the
    -- point: encryption hides the content, but the columns the merge orders on
    -- travel in the clear because this table pages on them. Without a signature
    -- anything that can write here — including this service — can stamp a
    -- version that outranks a device's real one, or a tombstone that deletes an
    -- item everywhere (manifest 05 §5.3). Clients verify before merging and
    -- refuse what does not verify.
    --
    -- It is a MAC of the *ciphertext*, never of the plaintext: a plaintext hash
    -- stored beside the ciphertext would hand this server an equality oracle
    -- over content, which is the one thing the design does not give it.
    --
    -- `not null` so a row that was never signed cannot be written at all.
    signature         text not null,

    -- Server-assigned, for retention. The retention job orders on this and
    -- never on the client-supplied `created_at`, or a device with a forged
    -- clock escapes eviction (manifest 05 §5.1 row 4a). `authenticated` has no
    -- column privilege on it — see the grants in the RLS migration, which are
    -- what actually makes "server-assigned" true.
    inserted_at       timestamptz not null default now(),
    updated_at        timestamptz not null default now(),

    -- The client restricts `item_id` to `[A-Za-z0-9_-]` so it can be placed in
    -- a PostgREST `in.(…)` filter without quoting rules deciding what the
    -- filter means (`rest::item::validate_item_id`). Enforced here too: an id
    -- containing a comma, injected by anything that reaches the REST API
    -- directly, would otherwise sit in the table until some other device
    -- fetched it and tried to build a filter from it.
    constraint clipboard_items_item_id_charset
        check (item_id ~ '^[A-Za-z0-9_-]{1,128}$'),

    -- T-4: a tombstone must never carry a stale payload. This mirrors
    -- `CloudItem::validate` exactly, as a backstop for a writer that is not
    -- this client.
    constraint clipboard_items_tombstone_has_no_payload
        check (
            not deleted
            or (coalesce(ciphertext, '') = '' and coalesce(nonce, '') = '')
        ),

    -- The other half of the same validation: a live row with no payload is a
    -- row every device will fail to open. Fail loudly at write time instead.
    constraint clipboard_items_live_has_payload
        check (deleted or (ciphertext is not null and ciphertext <> '')),

    -- Lower bound on the version clock. The client clamps negatives on read
    -- (`CopyPaste-psx7`); refusing them on write means the clamp is not the
    -- only thing standing between a hostile stamp and the merge.
    constraint clipboard_items_created_at_non_negative
        check (created_at >= 0),

    -- Bounded metadata. These columns are plaintext, so they are also the only
    -- place an account holder could stash bulk data in the clear.
    constraint clipboard_items_content_type_bounded
        check (length(content_type) between 1 and 128),
    constraint clipboard_items_origin_device_id_bounded
        check (length(origin_device_id) between 1 and 128),

    -- Base64 and bounded. 44 characters today (32 bytes of HMAC-SHA256); the
    -- bound is looser than that so a longer MAC is a client change rather than
    -- a migration, and tight enough that the column cannot become somewhere to
    -- park bulk plaintext.
    constraint clipboard_items_signature_bounded
        check (signature ~ '^[A-Za-z0-9+/=]{16,128}$'),

    -- Storage backstop, not the product limit. The per-item cap the user should
    -- meet is client-side (10 MiB image/file, 8 MiB text — manifest 05 §5.1
    -- row 13) so the error is a local one rather than an opaque backend
    -- rejection. 16 MiB of base64 is ~12 MiB of ciphertext: above every
    -- client-side limit, below anything that would hurt the table.
    constraint clipboard_items_payload_bounded
        check (octet_length(coalesce(ciphertext, '')) <= 16 * 1024 * 1024)
);

comment on table public.clipboard_items is
    'Ciphertext and metadata only. The server cannot decrypt any row: content is '
    'sealed client-side under a passphrase-derived key that never leaves the device.';
comment on column public.clipboard_items.item_id is
    'Cross-device CRDT identity and the upsert conflict key (with user_id).';
comment on column public.clipboard_items.created_at is
    'Client-supplied version clock in ms. The poll cursor pages on this. Never used for retention.';
comment on column public.clipboard_items.inserted_at is
    'Server-assigned. The only ordering the retention job is allowed to trust.';

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------

-- The upsert conflict target. PostgREST needs a unique index to resolve
-- `on_conflict`; without it *every* write fails, not just a replay — PostgREST
-- 12.2.3 answers 400 with `42P10` on the first request, measured by
-- `supabase/dev/postgrest-harness.sh` with this index dropped. Scoped to
-- (user_id, item_id) rather than item_id alone because this is a shared table
-- (manifest 05 §4.2, AT-52).
create unique index clipboard_items_user_item_uidx
    on public.clipboard_items (user_id, item_id);

-- The read path. `fetch_since` sends
--     created_at=gte.<cursor>&order=created_at.asc,item_id.asc&limit=<n>
-- with `user_id = auth.uid()` supplied by RLS, so the index that serves it is
-- (user_id, created_at, item_id) — equality on the leading column, range on the
-- second, and the third supplying the tie-break the ordering needs. Without it
-- every pull is a sequential scan plus a sort of the whole account.
--
-- All three columns ascend. `rest/mod.rs` describes this index as
-- "(created_at asc, item_id desc)" in prose, which does not match the `order=`
-- the same module sends two screens further down (`created_at.asc,item_id.asc`).
-- The code is right and the prose is stale; matching the prose would cost an
-- incremental sort on every page.
create index clipboard_items_user_created_idx
    on public.clipboard_items (user_id, created_at, item_id);

-- Retention TTL probe. In steady state the expired tail is small, so an index
-- scan beats the sequential scan the delete would otherwise do every run. The
-- per-account row cap in the same job is a whole-table window pass and is not
-- served by an index at all — see the note in the retention migration.
create index clipboard_items_inserted_at_idx
    on public.clipboard_items (inserted_at);

-- ---------------------------------------------------------------------------
-- updated_at
-- ---------------------------------------------------------------------------
--
-- Trigger rather than a column default so it tracks updates, and so that
-- `authenticated` needing no UPDATE privilege on the column is enough to make
-- it untamperable: a trigger assigning to NEW is not subject to the column
-- privileges checked against the statement's target list.

create or replace function public.touch_updated_at() returns trigger
    language plpgsql
    set search_path = ''
    as $$
begin
    new.updated_at = now();
    -- Server-assigned means server-assigned: an UPDATE cannot move a row's
    -- position in the retention order even if some future grant made the
    -- column writable.
    new.inserted_at = old.inserted_at;
    return new;
end;
$$;

create trigger clipboard_items_touch_updated_at
    before update on public.clipboard_items
    for each row execute function public.touch_updated_at();
