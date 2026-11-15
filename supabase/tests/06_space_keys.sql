-- Space key envelopes are per account, immutable and bounded.
\set ON_ERROR_STOP on
begin;
set local role authenticated;
set local request.jwt.claim.sub = '11111111-1111-4111-8111-111111111111';
insert into public.space_key_envelopes(epoch,sender,recipient,nonce,ciphertext)
values(1,'mac','android',repeat('a',48),'sealed'),
      (1,'mac','recovery',repeat('A',32),'sealed');
insert into public.space_key_envelopes(epoch,sender,recipient,nonce,ciphertext)
values(1,'mac','android',repeat('a',48),'again')
on conflict(user_id,epoch,sender,recipient) do nothing;
do $$ begin
    if (select count(*) from public.space_key_envelopes) <> 2
       or (select ciphertext from public.space_key_envelopes where recipient='android') <> 'sealed' then
        raise exception 'Envelope inserts were not idempotent';
    end if;
    begin
        update public.space_key_envelopes set ciphertext='tampered';
        raise exception 'Envelopes were mutable';
    exception when insufficient_privilege then null;
    end;
    begin
        insert into public.space_key_envelopes(user_id,epoch,sender,recipient,nonce,ciphertext)
        values('22222222-2222-4222-8222-222222222222',1,'mac','x',repeat('a',48),'c');
        raise exception 'Envelope account ownership was writable';
    exception when insufficient_privilege then null;
    end;
    begin
        insert into public.space_key_envelopes(epoch,sender,recipient,nonce,ciphertext)
        values(2,'mac','bad id',repeat('a',48),'c');
        raise exception 'An invalid recipient was accepted';
    exception when check_violation then null;
    end;
end $$;
set local request.jwt.claim.sub = '22222222-2222-4222-8222-222222222222';
do $$ begin
    if exists(select 1 from public.space_key_envelopes) then
        raise exception 'Envelopes leaked across accounts';
    end if;
end $$;
-- Another account's delete reaches nothing.
delete from public.space_key_envelopes;
set local request.jwt.claim.sub = '11111111-1111-4111-8111-111111111111';
delete from public.space_key_envelopes where sender='mac' and epoch < 1;
do $$ begin
    if (select count(*) from public.space_key_envelopes) <> 2 then
        raise exception 'Another account or a current epoch was deleted';
    end if;
end $$;
delete from public.space_key_envelopes where sender='mac' and epoch < 2;
do $$ begin
    if exists(select 1 from public.space_key_envelopes) then
        raise exception 'The owner could not remove its old epochs';
    end if;
end $$;
set local role anon;
do $$ begin
    begin
        perform 1 from public.space_key_envelopes;
        raise exception 'Anonymous envelope reads were allowed';
    exception when insufficient_privilege then null;
    end;
end $$;
reset role;
do $$ begin
    if not exists(select 1 from pg_class where oid='public.space_key_envelopes'::regclass
                  and relrowsecurity and relforcerowsecurity) then
        raise exception 'Envelope RLS is not forced';
    end if;
end $$;
rollback;
\echo '06_space_keys: ok'
