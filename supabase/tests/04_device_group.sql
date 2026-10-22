-- Membership shares the account boundary but survives clipboard retention.
\set ON_ERROR_STOP on
begin;
set local role authenticated;
set local request.jwt.claim.sub = '11111111-1111-4111-8111-111111111111';
insert into public.device_group_states(device_id,ciphertext,nonce)
values('mac','ciphertext',repeat('A',32))
on conflict(user_id,device_id) do update set
    device_id=excluded.device_id,ciphertext=excluded.ciphertext,nonce=excluded.nonce;
insert into public.device_group_states(device_id,ciphertext,nonce)
values('mac','updated',repeat('B',32))
on conflict(user_id,device_id) do update set
    device_id=excluded.device_id,ciphertext=excluded.ciphertext,nonce=excluded.nonce;
do $$ declare
    previous_hash text;
begin
    if (select count(*) from public.device_group_states) <> 1
       or (select ciphertext from public.device_group_states where device_id='mac') <> 'updated' then
        raise exception 'Group upsert did not converge on one row';
    end if;
    begin
        update public.device_group_states set user_id='22222222-2222-4222-8222-222222222222';
        raise exception 'Group account ownership was writable';
    exception when insufficient_privilege then null;
    end;
    select ciphertext_digest into previous_hash from public.device_group_states where device_id='mac';
    update public.device_group_states set ciphertext='same-nonce-tamper' where device_id='mac';
    if (select ciphertext_digest from public.device_group_states where device_id='mac') = previous_hash
       or (select ciphertext_digest from public.device_group_states where device_id='mac')
          <> encode(sha256(convert_to(repeat('B',32) || 'same-nonce-tamper', 'UTF8')), 'hex') then
        raise exception 'Group catalog hash did not detect a same-nonce ciphertext update';
    end if;
    begin
        update public.device_group_states set ciphertext_digest=previous_hash where device_id='mac';
        raise exception 'Group catalog hash was writable by an account client';
    exception when insufficient_privilege then null;
    end;
end $$;
set local request.jwt.claim.sub = '22222222-2222-4222-8222-222222222222';
do $$ begin
    if exists(select 1 from public.device_group_states) then
        raise exception 'Group membership leaked across accounts';
    end if;
end $$;
set local role anon;
do $$ begin
    begin
        perform 1 from public.device_group_states;
        raise exception 'Anonymous group reads were allowed';
    exception when insufficient_privilege then null;
    end;
end $$;
reset role;
do $$ begin
    if not exists(select 1 from pg_class where oid='public.device_group_states'::regclass and relrowsecurity and relforcerowsecurity) then
        raise exception 'Group RLS is not forced';
    end if;
    if not exists(select 1 from pg_publication_tables where pubname='supabase_realtime' and tablename='device_group_states') then
        raise exception 'Group changes are missing from Realtime';
    end if;
end $$;
rollback;
\echo '04_device_group: ok'
