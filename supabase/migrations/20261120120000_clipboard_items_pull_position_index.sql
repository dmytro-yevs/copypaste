-- ---------------------------------------------------------------------------
-- The pull order is the server's, not a device's.
--
-- Clients page on `(updated_at, item_id)`. `updated_at` is assigned here on
-- every write — the column default on insert, `touch_updated_at` on update —
-- and `authenticated` holds no privilege to write it, so a row uploaded late,
-- or by a device whose clock is wrong, still sorts after every row this
-- database accepted before it. Paging on the client-supplied `created_at`
-- could not promise that: a cursor that had already passed such a row's stamp
-- never met the row.
--
-- Nothing a client needs is granted here. `authenticated` already selects the
-- whole row, so a project that has not applied this migration answers the same
-- query correctly and pays a sort of the account's rows for it. This index is
-- what makes it an in-order scan.
--
-- `clipboard_items_user_created_idx` stays: clients older than this change
-- still page on `(created_at, item_id)`.
-- ---------------------------------------------------------------------------

create index clipboard_items_user_updated_idx
    on public.clipboard_items (user_id, updated_at, item_id);

comment on column public.clipboard_items.updated_at is
    'Server-assigned on every write. The poll cursor pages on this.';
comment on column public.clipboard_items.created_at is
    'Client-supplied version clock in ms. The merge orders on it. Clients older than the position cursor page on it; never used for retention.';
