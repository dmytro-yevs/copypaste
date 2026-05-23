-- CopyPaste — Row-Level Security for `public.clipboard_items`
--
-- Run AFTER `schema.sql`.
--
-- Design decision (documented below):
-- ----------------------------------------------------------------------------
-- We pivot RLS on `user_id` (FK to `auth.users.id`), NOT on `device_id`.
--
-- Why:
--   * `device_id` is a UUID the daemon generates locally — it has no
--     relationship to `auth.uid()`, so we could only enforce equality by
--     storing the device UUID inside the JWT (custom claim) or by a
--     join table. Both add operational weight.
--   * `user_id` is supplied automatically (via `default auth.uid()`) and
--     can be matched directly against `auth.uid()` in the RLS predicate.
--     This is the simpler approach and matches the Supabase paved path.
--
-- Trade-off:
--   * One Supabase user account = one CopyPaste "trust circle". Every
--     device that signs into the same account can read/write every
--     other device's items for that account. This is the model the
--     daemon already assumes (single-user, multi-device sync).
--   * If multi-tenant per-device isolation is ever required, switch to
--     `device_id = (auth.jwt() ->> 'device_id')` with a custom JWT claim
--     and drop `user_id`.
-- ----------------------------------------------------------------------------

-- ── Enable RLS ────────────────────────────────────────────────────────────────

alter table public.clipboard_items enable row level security;
alter table public.clipboard_items force row level security;

-- ── Policies ──────────────────────────────────────────────────────────────────

-- Drop-and-recreate so this script is idempotent.
drop policy if exists clipboard_items_select_own on public.clipboard_items;
drop policy if exists clipboard_items_insert_own on public.clipboard_items;
drop policy if exists clipboard_items_update_own on public.clipboard_items;
drop policy if exists clipboard_items_delete_own on public.clipboard_items;

-- SELECT: a signed-in user only sees their own rows.
create policy clipboard_items_select_own
    on public.clipboard_items
    for select
    to authenticated
    using (user_id = auth.uid());

-- INSERT: the inserted row must be owned by the inserting user.
--   `with check` runs on NEW; `default user_id := auth.uid()` (set below)
--   means clients can omit `user_id` entirely.
create policy clipboard_items_insert_own
    on public.clipboard_items
    for insert
    to authenticated
    with check (user_id = auth.uid());

-- UPDATE: must match on both old and new row.
create policy clipboard_items_update_own
    on public.clipboard_items
    for update
    to authenticated
    using      (user_id = auth.uid())
    with check (user_id = auth.uid());

-- DELETE: only owner can delete.
create policy clipboard_items_delete_own
    on public.clipboard_items
    for delete
    to authenticated
    using (user_id = auth.uid());

-- ── Default owner ─────────────────────────────────────────────────────────────
--
-- Lets clients `insert into clipboard_items (device_id, content_type, ...)`
-- without ever spelling out `user_id`. The default fires before the RLS
-- `with check`, so the row is auto-tagged with the caller's uid.

alter table public.clipboard_items
    alter column user_id set default auth.uid();

-- ── Anon role hardening ───────────────────────────────────────────────────────
--
-- Postgres' default ACL grants ALL to PUBLIC. Strip everything from the
-- `anon` (unauthenticated) role so a missing JWT can't even attempt a read.
-- (The `authenticated` role still has table-level privileges via RLS.)

revoke all on public.clipboard_items from anon;
grant  select, insert, update, delete on public.clipboard_items to authenticated;
