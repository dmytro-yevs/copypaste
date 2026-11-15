-- Space key envelopes. Each row seals one key epoch for one device (or, as
-- recipient 'recovery', under the sync passphrase), or tells a removed device
-- about its removal under the key it still holds. The server sees only
-- device ids, epoch numbers and ciphertext. Rows are immutable: a sender
-- inserts and later removes its own older epochs.
create table public.space_key_envelopes (
    user_id uuid not null default auth.uid() references auth.users(id) on delete cascade,
    epoch bigint not null check (epoch between 0 and 9007199254740991),
    sender text not null check (sender ~ '^[A-Za-z0-9_-]{1,128}$'),
    recipient text not null check (recipient ~ '^[A-Za-z0-9_-]{1,128}$'),
    nonce text not null check (length(nonce) between 32 and 48 and nonce ~ '^[A-Za-z0-9+/=]+$'),
    -- A removal notice carries a whole signed roster, as roster rows do.
    ciphertext text not null check (length(ciphertext) between 1 and 1048600),
    primary key (user_id, epoch, sender, recipient)
);
alter table public.space_key_envelopes enable row level security;
alter table public.space_key_envelopes force row level security;
revoke all on public.space_key_envelopes from public, anon, authenticated;
grant select, delete on public.space_key_envelopes to authenticated;
grant insert(epoch, sender, recipient, nonce, ciphertext)
    on public.space_key_envelopes to authenticated;
create policy space_key_envelopes_select on public.space_key_envelopes
    for select to authenticated using (user_id = (select auth.uid()));
create policy space_key_envelopes_insert on public.space_key_envelopes
    for insert to authenticated with check (user_id = (select auth.uid()));
create policy space_key_envelopes_delete on public.space_key_envelopes
    for delete to authenticated using (user_id = (select auth.uid()));

-- Bounded per account. Serialized so concurrent devices cannot pass the
-- bound together.
create function public.bound_space_key_envelopes() returns trigger
language plpgsql set search_path = pg_catalog, public as $$
begin
    perform pg_advisory_xact_lock(hashtextextended('space:' || new.user_id::text, 0));
    if (select count(*) from public.space_key_envelopes where user_id = new.user_id) >= 2048 then
        raise exception 'Space key envelope quota exceeded';
    end if;
    return new;
end;
$$;
revoke all on function public.bound_space_key_envelopes() from public, anon, authenticated;
create trigger bound_space_key_envelopes before insert on public.space_key_envelopes
    for each row execute function public.bound_space_key_envelopes();
