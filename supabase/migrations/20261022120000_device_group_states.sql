-- Membership must survive clipboard retention and history deletion. The row
-- holds an individually signed roster encrypted under the existing sync key.
create table public.device_group_states (
    user_id uuid not null default auth.uid() references auth.users(id) on delete cascade,
    device_id text not null check (length(device_id) between 1 and 128 and device_id ~ '^[A-Za-z0-9_-]+$'),
    ciphertext text not null check (length(ciphertext) between 1 and 1048600),
    nonce text not null check (length(nonce) = 32),
    ciphertext_digest text not null check (ciphertext_digest ~ '^[0-9a-f]{64}$'),
    primary key (user_id, device_id)
);
alter table public.device_group_states enable row level security;
alter table public.device_group_states force row level security;
revoke all on public.device_group_states from public, anon, authenticated;
grant select on public.device_group_states to authenticated;
grant insert(device_id, ciphertext, nonce), update(device_id, ciphertext, nonce)
    on public.device_group_states to authenticated;
create policy device_group_states_select on public.device_group_states
    for select to authenticated using (user_id = (select auth.uid()));
create policy device_group_states_insert on public.device_group_states
    for insert to authenticated with check (user_id = (select auth.uid()));
create policy device_group_states_update on public.device_group_states
    for update to authenticated using (user_id = (select auth.uid()))
    with check (user_id = (select auth.uid()));

-- A nonce is client-controlled, so it cannot detect an account-only writer
-- changing ciphertext while preserving the nonce. This catalog token is always
-- recomputed by the database and is excluded from client column write grants.
create function public.hash_device_group_state() returns trigger
language plpgsql set search_path = pg_catalog as $$
begin
    new.ciphertext_digest := encode(sha256(convert_to(new.nonce || new.ciphertext, 'UTF8')), 'hex');
    return new;
end;
$$;
revoke all on function public.hash_device_group_state() from public, anon, authenticated;
create trigger hash_device_group_state before insert or update on public.device_group_states
    for each row execute function public.hash_device_group_state();

-- The quota includes retired writers. Never evict a removal record to admit
-- another writer. Serialize inserts per account so concurrent devices cannot
-- bypass the same bound enforced by the client.
create function public.bound_device_group_states() returns trigger
language plpgsql set search_path = pg_catalog, public as $$
begin
    perform pg_advisory_xact_lock(hashtextextended(new.user_id::text, 0));
    if not exists(select 1 from public.device_group_states
        where user_id = new.user_id and device_id = new.device_id)
       and (select count(*) from public.device_group_states where user_id = new.user_id) >= 4096 then
        raise exception 'Device group state quota exceeded';
    end if;
    return new;
end;
$$;
revoke all on function public.bound_device_group_states() from public, anon, authenticated;
create trigger bound_device_group_states before insert on public.device_group_states
    for each row execute function public.bound_device_group_states();

do $$ begin
    if exists(select 1 from pg_publication where pubname = 'supabase_realtime') then
        alter publication supabase_realtime add table public.device_group_states;
    end if;
end $$;
