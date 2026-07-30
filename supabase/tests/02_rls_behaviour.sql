-- ---------------------------------------------------------------------------
-- Behavioural checks: what a signed-in account can actually do.
--
-- Everything runs inside one transaction that is rolled back, so the database
-- is unchanged afterwards. Needs the two development accounts (`seed.sql`, or
-- `supabase/dev/verify-schema.sh`), and a connection able to `set role` —
-- which means a local stack or the harness, not a hosted project.
--
--     psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f supabase/tests/02_rls_behaviour.sql
--
-- The static audit in 01 checks that the policies say the right thing. This
-- one checks that saying it has the effect we think it does — the two have come
-- apart before, which is why AT-51 exists.
-- ---------------------------------------------------------------------------

\set ON_ERROR_STOP on
\set alice '11111111-1111-4111-8111-111111111111'
\set bob   '22222222-2222-4222-8222-222222222222'

begin;

do $$
begin
    if not exists (select 1 from auth.users where id in (
        '11111111-1111-4111-8111-111111111111',
        '22222222-2222-4222-8222-222222222222'))
    then
        raise exception 'the development accounts are missing: run `supabase db reset` or supabase/dev/verify-schema.sh';
    end if;
end
$$;

-- --- alice writes ----------------------------------------------------------

set local request.jwt.claims = '{"sub":"11111111-1111-4111-8111-111111111111","role":"authenticated"}';
set local role authenticated;

-- Exactly the column list the client sends: no `user_id`, and `deleted` always
-- explicit (manifest 05 T-5).
insert into public.clipboard_items
    (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
values
    ('alice-1', 'Y2lwaGVy', 'bm9uY2U=', 'text', 1700000000000, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA='),
    ('alice-2', 'Y2lwaGVy', 'bm9uY2U=', 'text', 1700000000001, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=');

do $$
begin
    if (select user_id from public.clipboard_items where item_id = 'alice-1')
       <> '11111111-1111-4111-8111-111111111111' then
        raise exception 'the auth.uid() default did not fill user_id';
    end if;
end
$$;

-- The upsert the client actually issues: PostgREST turns
-- `?on_conflict=user_id,item_id` + `Prefer: resolution=merge-duplicates` into
-- this statement. Replaying it must be a no-op, not a conflict.
insert into public.clipboard_items
    (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
values
    ('alice-1', 'bmV3ZXI=', 'bm9uY2Uy', 'text', 1700000000002, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=')
on conflict (user_id, item_id) do update set
    ciphertext       = excluded.ciphertext,
    nonce            = excluded.nonce,
    content_type     = excluded.content_type,
    created_at       = excluded.created_at,
    deleted          = excluded.deleted,
    origin_device_id = excluded.origin_device_id,
    signature        = excluded.signature;

do $$
begin
    if (select count(*) from public.clipboard_items where item_id = 'alice-1') <> 1 then
        raise exception 'the upsert duplicated a row instead of merging it';
    end if;
    if (select created_at from public.clipboard_items where item_id = 'alice-1') <> 1700000000002 then
        raise exception 'the upsert did not overwrite the merged columns';
    end if;
end
$$;

-- --- alice cannot escape her own account ----------------------------------

do $$
begin
    begin
        insert into public.clipboard_items
            (user_id, item_id, ciphertext, nonce, content_type, created_at, deleted,
             origin_device_id, signature)
        values
            ('22222222-2222-4222-8222-222222222222', 'planted', 'Y3Q=', 'bm8=', 'text', 1, false,
             'd', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=');
        raise exception 'a row was written into another account';
    exception when insufficient_privilege then
        null;  -- either the `with check` or the missing column grant; both are correct
    end;

    begin
        update public.clipboard_items
           set user_id = '22222222-2222-4222-8222-222222222222'
         where item_id = 'alice-1';
        raise exception 'an existing row was moved into another account';
    exception when insufficient_privilege then
        null;
    end;
end
$$;

-- --- the server-assigned columns are server-assigned ------------------------

do $$
begin
    -- The eviction-order forgery of manifest 05 §5.1 row 4a, attempted against
    -- v2's column instead of v1's.
    begin
        update public.clipboard_items
           set inserted_at = now() + interval '10 years'
         where item_id = 'alice-1';
        raise exception 'a client restamped inserted_at and escaped retention';
    exception when insufficient_privilege then
        null;
    end;

    begin
        update public.clipboard_items set id = gen_random_uuid() where item_id = 'alice-1';
        raise exception 'a client rewrote the row primary key';
    exception when insufficient_privilege then
        null;
    end;
end
$$;

-- --- the tombstone rules ---------------------------------------------------

do $$
begin
    -- T-4: a tombstone must never carry a payload.
    begin
        insert into public.clipboard_items
            (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
        values ('leaky', 'c3RhbGU=', 'bm8=', 'text', 5, true, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=');
        raise exception 'a tombstone carrying ciphertext was accepted';
    exception when check_violation then
        null;
    end;

    -- A live row with no payload is equally refused.
    begin
        insert into public.clipboard_items
            (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
        values ('empty', '', '', 'text', 5, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=');
        raise exception 'a live row with no ciphertext was accepted';
    exception when check_violation then
        null;
    end;

    -- An id that would change the meaning of a PostgREST `in.(…)` filter.
    begin
        insert into public.clipboard_items
            (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
        values ('a1,*', 'Y3Q=', 'bm8=', 'text', 5, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=');
        raise exception 'an item_id outside [A-Za-z0-9_-] was accepted';
    exception when check_violation then
        null;
    end;
end
$$;

-- A delete: an ordinary upsert of a version with `deleted = true`, no payload,
-- and `created_at` restamped so the deletion sorts above every device's
-- watermark. There is no id-only PATCH path any more — a partial write cannot
-- carry a signature over the columns it does not send.
insert into public.clipboard_items
    (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
values
    ('alice-2', '', '', 'text', 1700000009999, true, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=')
on conflict (user_id, item_id) do update set
    ciphertext       = excluded.ciphertext,
    nonce            = excluded.nonce,
    content_type     = excluded.content_type,
    created_at       = excluded.created_at,
    deleted          = excluded.deleted,
    origin_device_id = excluded.origin_device_id,
    signature        = excluded.signature;

do $$
begin
    if not (select deleted from public.clipboard_items where item_id = 'alice-2') then
        raise exception 'the tombstone upsert did not apply';
    end if;
    if (select coalesce(ciphertext, '') from public.clipboard_items where item_id = 'alice-2') <> '' then
        raise exception 'the tombstone kept a payload';
    end if;
end
$$;

-- --- an unsigned row cannot be written at all ------------------------------

do $$
begin
    -- The deployment half of manifest 05 §5.3. The client refuses unsigned rows
    -- on the way out and refuses them again on the way in; this is the third
    -- point, and the only one an attacker with the account password has to get
    -- past before the row exists at all.
    begin
        insert into public.clipboard_items
            (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id)
        values ('unsigned', 'Y3Q=', 'bm8=', 'text', 5, false, 'device-a');
        raise exception 'a row with no signature was accepted';
    exception when not_null_violation then
        null;
    end;

    begin
        insert into public.clipboard_items
            (item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id, signature)
        values ('junk-signature', 'Y3Q=', 'bm8=', 'text', 5, false, 'device-a', 'not base64!!');
        raise exception 'a signature outside the base64 alphabet was accepted';
    exception when check_violation then
        null;
    end;
end
$$;

-- --- the timestamp trigger --------------------------------------------------
--
-- Checked from the session role rather than as `authenticated`, because
-- `authenticated` cannot name these columns in an UPDATE at all. The point is
-- that the trigger, not the privilege system, is the second line: it rewrites
-- `updated_at` and restores `inserted_at` whoever the writer is.

reset role;

do $$
declare
    kept timestamptz;
    touched timestamptz;
begin
    select inserted_at into kept from public.clipboard_items where item_id = 'alice-1';

    update public.clipboard_items
       set updated_at  = timestamptz '2000-01-01 00:00:00Z',
           inserted_at = timestamptz '2000-01-01 00:00:00Z',
           created_at  = created_at + 1
     where item_id = 'alice-1';

    select updated_at into touched from public.clipboard_items where item_id = 'alice-1';
    if touched < now() - interval '1 minute' then
        raise exception 'the updated_at trigger did not overwrite a supplied value';
    end if;
    if (select inserted_at from public.clipboard_items where item_id = 'alice-1') <> kept then
        raise exception 'inserted_at moved on update';
    end if;
end
$$;

set local request.jwt.claims = '{"sub":"11111111-1111-4111-8111-111111111111","role":"authenticated"}';
set local role authenticated;

-- --- bob sees nothing of alice's -------------------------------------------

reset role;
set local request.jwt.claims = '{"sub":"22222222-2222-4222-8222-222222222222","role":"authenticated"}';
set local role authenticated;

do $$
declare
    n integer;
begin
    if (select count(*) from public.clipboard_items) <> 0 then
        raise exception 'one account can read another account''s rows';
    end if;

    update public.clipboard_items set deleted = true where item_id = 'alice-1';
    get diagnostics n = row_count;
    if n <> 0 then
        raise exception 'one account can tombstone another account''s rows';
    end if;

    delete from public.clipboard_items where item_id = 'alice-1';
    get diagnostics n = row_count;
    if n <> 0 then
        raise exception 'one account can delete another account''s rows';
    end if;
end
$$;

-- --- the publishable key alone reaches nothing -----------------------------

reset role;
set local role anon;

do $$
begin
    begin
        perform 1 from public.clipboard_items;
        raise exception 'anon can read clipboard_items with only the publishable key';
    exception when insufficient_privilege then
        null;
    end;
end
$$;

-- --- a session with no claims at all ---------------------------------------

reset role;
set local request.jwt.claims = '';
set local role authenticated;

do $$
begin
    -- `auth.uid()` is NULL here, and `user_id = null` is never true. The
    -- interesting failure would be the opposite: a policy that lets a
    -- claimless session match every row.
    if (select count(*) from public.clipboard_items) <> 0 then
        raise exception 'a session carrying no subject claim can read rows';
    end if;
end
$$;

reset role;
rollback;

\echo '02_rls_behaviour: ok'
