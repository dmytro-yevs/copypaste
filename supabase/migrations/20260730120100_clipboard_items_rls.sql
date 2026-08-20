-- ---------------------------------------------------------------------------
-- Row-level security.
--
-- This is the second layer, not the first. Rows are sealed client-side, so a
-- misconfigured policy exposes ciphertext rather than plaintext. That is a
-- reason to get this right anyway: metadata alone (device ids, sizes, content
-- types, timestamps, delete activity) is a real disclosure, and "the other
-- layer will save us" is how both layers end up broken.
--
-- Shape of the model: RLS pivots on `user_id`, not on a device. Every device
-- signed into an account reads every other device's rows — one account, one
-- trust circle. That is what the whole design assumes (manifest 05 §4.3).
-- ---------------------------------------------------------------------------

alter table public.clipboard_items enable row level security;
-- `force` so the table owner is policed too. Without it, any SECURITY DEFINER
-- function owned by the table owner silently bypasses every policy below — and
-- this deployment has such a function (the retention job), which is why that
-- job gets an explicit, named policy instead of an implicit exemption.
alter table public.clipboard_items force row level security;

-- Postgres' default ACL grants ALL on new objects to PUBLIC, which `anon`
-- inherits. Newer Supabase images also grant table-level INSERT/UPDATE to
-- `authenticated` before this migration runs. Revoke every client role before
-- the column grants below, or server-assigned columns stay writable.
revoke all on public.clipboard_items from anon, authenticated, public;

-- ---------------------------------------------------------------------------
-- Privileges: what the role may touch at all, before any policy runs.
--
-- RLS decides *which rows*; column privileges decide *which columns*. The split
-- matters here for one specific reason: `inserted_at` is the only ordering the
-- retention job trusts (manifest 05 §5.1 row 4a), and a blanket
-- `grant update on clipboard_items` would let any account holder restamp it and
-- escape eviction — the same forgery the relay's `wall_time` sort suffered,
-- moved to a new column. `id`, `user_id`, `inserted_at` and `updated_at` are
-- therefore not writable by the client at all.
-- ---------------------------------------------------------------------------

-- SELECT is whole-table on purpose: Realtime evaluates RLS for the subscribed
-- role over the whole record, and a column-level restriction here buys nothing
-- (it is the account's own metadata) while risking a broken subscription.
grant select on public.clipboard_items to authenticated;

grant insert (item_id, ciphertext, nonce, content_type, created_at, deleted,
              origin_device_id, signature)
    on public.clipboard_items to authenticated;

-- The same eight columns: PostgREST's merge-duplicates upsert issues
-- `insert … on conflict do update set` over exactly the columns in the body.
-- `signature` is in both lists because it is written with every row and must be
-- replaced with every row: a merge that updated the metadata and kept the old
-- signature would leave a row no device can accept.
grant update (item_id, ciphertext, nonce, content_type, created_at, deleted,
              origin_device_id, signature)
    on public.clipboard_items to authenticated;

-- Granted although the client never uses it: deletes travel as tombstones
-- (`SupabaseRest::tombstone` PATCHes a flag, it does not DELETE), because a row
-- that is simply gone tells an offline device nothing and its own copy would be
-- re-uploaded. The privilege exists so a user can purge their own account.
grant delete on public.clipboard_items to authenticated;

-- ---------------------------------------------------------------------------
-- Policies. All four scope on `user_id = auth.uid()`, all `to authenticated`.
-- `anon` gets no policy and no privilege: a request bearing only the
-- publishable key is refused outright, which is what makes the anon-key code
-- path dead weight rather than a fallback (manifest 05 §7.4).
-- ---------------------------------------------------------------------------

create policy clipboard_items_select on public.clipboard_items
    for select to authenticated
    using (user_id = (select auth.uid()));

create policy clipboard_items_insert on public.clipboard_items
    for insert to authenticated
    with check (user_id = (select auth.uid()));

-- Both clauses. `using` decides which rows may be targeted, `with check`
-- decides what they may be updated to; `using` alone would let a row be
-- rewritten into another account.
create policy clipboard_items_update on public.clipboard_items
    for update to authenticated
    using (user_id = (select auth.uid()))
    with check (user_id = (select auth.uid()));

create policy clipboard_items_delete on public.clipboard_items
    for delete to authenticated
    using (user_id = (select auth.uid()));

-- The `(select auth.uid())` wrapping is not decoration: it makes the call an
-- InitPlan evaluated once per query instead of once per row, which is the
-- difference between an index scan and a per-row function call on a page of
-- 200 rows. The predicate is identical either way.
