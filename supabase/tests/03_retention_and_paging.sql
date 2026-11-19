-- ---------------------------------------------------------------------------
-- Retention behaviour, and the plan the pull query actually gets.
--
-- Transactional and rolled back, like 02, but this one needs a fixture role
-- that bypasses forced RLS and can write `inserted_at` directly because the
-- whole point of that column is that no client can.
--
--     psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f supabase/tests/03_retention_and_paging.sql
-- ---------------------------------------------------------------------------

\set ON_ERROR_STOP on

do $$
begin
    if not (select rolsuper or rolbypassrls from pg_roles where rolname = current_user) then
        raise exception
            'this file needs a BYPASSRLS fixture session: `inserted_at` is deliberately not '
            'writable by client roles, and forced RLS applies to the table owner too';
    end if;
    if not exists (select 1 from auth.users where id = '11111111-1111-4111-8111-111111111111') then
        raise exception 'the development accounts are missing: run `supabase db reset` or supabase/dev/verify-schema.sh';
    end if;
end
$$;

begin;

-- --- the pull query's plan --------------------------------------------------

insert into public.clipboard_items
    (user_id, item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id,
     signature, updated_at)
select
    case when i % 2 = 0
         then '11111111-1111-4111-8111-111111111111'::uuid
         else '22222222-2222-4222-8222-222222222222'::uuid end,
    'row-' || lpad(i::text, 6, '0'),
    'Y2lwaGVy', 'bm9uY2U=', 'text',
    -- Many rows share a millisecond on purpose: that is the burst the compound
    -- (created_at, item_id) ordering exists to page through (INV-N1, AT-24).
    1700000000000 + (i / 25),
    false,
    'device-a',
    'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=',
    -- And many share a position, as every row of one upsert does. Stamped in
    -- the opposite order to `created_at`, so the two orders cannot be
    -- mistaken for each other.
    timestamptz '2026-01-01 00:00:00Z' + make_interval(secs => (10000 - i) / 25)
from generate_series(1, 10000) as i;

analyze public.clipboard_items;

do $$
declare
    line    record;
    plan    text;
    shape   text[];
    -- Both cursor shapes `SupabaseRest::fetch_since` can send, byte for byte,
    -- over the server's position; then the same two over the device's stamp,
    -- which clients older than the position cursor still send. Each is the
    -- filter, the order and the index that has to serve them, or the compound
    -- keyset buys correctness at the price of a sort on every page.
    queries text[][] := array[
        -- select=<columns>&updated_at=gte.<position>
        -- &order=updated_at.asc,item_id.asc&limit=100
        array[$q$ where updated_at >= '2026-01-01T00:03:20.000000Z' $q$,
              'updated_at asc, item_id asc', 'clipboard_items_user_updated_idx'],
        -- select=<columns>
        -- &or=(updated_at.gt.P,and(updated_at.eq.P,item_id.gt.ID))
        -- &order=updated_at.asc,item_id.asc&limit=100
        array[$q$ where (updated_at > '2026-01-01T00:03:20.000000Z'
                         or (updated_at = '2026-01-01T00:03:20.000000Z'
                             and item_id > 'row-005001')) $q$,
              'updated_at asc, item_id asc', 'clipboard_items_user_updated_idx'],
        array[$q$ where created_at >= 1700000000200 $q$,
              'created_at asc, item_id asc', 'clipboard_items_user_created_idx'],
        array[$q$ where (created_at > 1700000000200
                         or (created_at = 1700000000200 and item_id > 'row-005001')) $q$,
              'created_at asc, item_id asc', 'clipboard_items_user_created_idx']
    ];
begin
    perform set_config(
        'request.jwt.claims',
        '{"sub":"11111111-1111-4111-8111-111111111111","role":"authenticated"}',
        true);
    set local role authenticated;

    foreach shape slice 1 in array queries loop
        plan := '';
        for line in execute
            'explain (costs off) select item_id, ciphertext, nonce, content_type,'
            || ' created_at, deleted, origin_device_id, signature, updated_at'
            || ' from public.clipboard_items '
            || shape[1]
            || ' order by ' || shape[2] || ' limit 100'
        loop
            plan := plan || line."QUERY PLAN" || E'\n';
        end loop;

        if plan not like '%' || shape[3] || '%' then
            raise exception
                E'the pull query does not use %.\n%\n%', shape[3], shape[1], plan;
        end if;
        if plan like '%Seq Scan%' then
            raise exception E'the pull query falls back to a sequential scan.\n%\n%',
                shape[1], plan;
        end if;
        if plan like '%Sort%' then
            -- A sort here means the index is being read for the filter and the
            -- ordering is being redone, which is the cost the third index
            -- column exists to avoid.
            raise exception
                E'the pull query sorts instead of reading the index in order.\n%\n%',
                shape[1], plan;
        end if;
    end loop;

    reset role;
end
$$;

-- A version written late under an old device stamp is still read after the
-- rows the server took before it. Driven through the client's own write — the
-- merge-duplicates upsert, as `authenticated` — onto a row that already sits
-- far behind the cursor: only `touch_updated_at` can move it, so without the
-- trigger the row stays where the fixture put it and this fails.
do $$
declare
    last_seen record;
    next_id   text;
begin
    select updated_at, item_id into last_seen
      from public.clipboard_items
     where user_id = '11111111-1111-4111-8111-111111111111'
     order by updated_at desc, item_id desc limit 1;

    perform set_config(
        'request.jwt.claims',
        '{"sub":"11111111-1111-4111-8111-111111111111","role":"authenticated"}',
        true);
    set local role authenticated;

    insert into public.clipboard_items
        (item_id, ciphertext, nonce, content_type, created_at, deleted,
         origin_device_id, signature)
    values ('row-009998', 'bGF0ZQ==', 'bm9uY2U=', 'text', 1, false, 'device-a',
            'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=')
    on conflict (user_id, item_id) do update
       set ciphertext = excluded.ciphertext, nonce = excluded.nonce,
           content_type = excluded.content_type, created_at = excluded.created_at,
           deleted = excluded.deleted, origin_device_id = excluded.origin_device_id,
           signature = excluded.signature;

    select item_id into next_id
      from public.clipboard_items
     where (updated_at > last_seen.updated_at
            or (updated_at = last_seen.updated_at and item_id > last_seen.item_id))
     order by updated_at asc, item_id asc limit 1;
    reset role;

    if next_id is distinct from 'row-009998' then
        raise exception
            'a version stamped behind the cursor by its device was not placed after the row read before it: %',
            next_id;
    end if;
end
$$;

-- The keyset at the resolution the client keeps: one microsecond apart is a
-- different position, the same microsecond is paged by `item_id`.
insert into public.clipboard_items
    (user_id, item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id,
     signature, updated_at)
select '11111111-1111-4111-8111-111111111111', id, 'Y2lwaGVy', 'bm9uY2U=', 'text', 1, false,
       'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=', placed
  from (values
        ('tie-0', timestamptz '2999-06-01T00:00:00.000499Z'),
        ('tie-a', timestamptz '2999-06-01T00:00:00.000500Z'),
        ('tie-b', timestamptz '2999-06-01T00:00:00.000500Z'),
        ('tie-c', timestamptz '2999-06-01T00:00:00.000501Z')) as rows (id, placed);

do $$
declare
    listed text[];
begin
    perform set_config(
        'request.jwt.claims',
        '{"sub":"11111111-1111-4111-8111-111111111111","role":"authenticated"}',
        true);
    set local role authenticated;

    select array_agg(item_id order by updated_at, item_id) into listed
      from public.clipboard_items
     where updated_at >= '2999-06-01T00:00:00.000500Z';
    if listed is distinct from array['tie-a', 'tie-b', 'tie-c'] then
        raise exception 'the inclusive bound is not exact to the microsecond: %', listed;
    end if;

    select array_agg(item_id order by updated_at, item_id) into listed
      from public.clipboard_items
     where (updated_at > '2999-06-01T00:00:00.000500Z'
            or (updated_at = '2999-06-01T00:00:00.000500Z' and item_id > 'tie-a'));
    if listed is distinct from array['tie-b', 'tie-c'] then
        raise exception 'the keyset does not page a shared position by item_id: %', listed;
    end if;

    reset role;
end
$$;

rollback;

-- --- retention ---------------------------------------------------------------

begin;

-- Tighten the policy for the test only; rolled back with everything else.
update private.retention_policy set ttl_hours = 24, max_rows_per_user = 3 where id;

insert into public.clipboard_items
    (user_id, item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id,
     signature, inserted_at)
values
    -- Old by the server's clock, and stamped a decade into the future by the
    -- client's. Sorting eviction on the client value lets an intra-account
    -- writer forge a low sort key, escape eviction and displace legitimate
    -- items (`CopyPaste-1uqb`, manifest 05 §5.1 row 4a).
    ('11111111-1111-4111-8111-111111111111', 'forged-old', 'Y3Q=', 'bm8=', 'text',
     4102444800000, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=', now() - interval '48 hours'),
    -- Fresh by the server's clock, ancient by the client's: must survive.
    ('11111111-1111-4111-8111-111111111111', 'fresh', 'Y3Q=', 'bm8=', 'text',
     1, false, 'device-a', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=', now()),
    ('22222222-2222-4222-8222-222222222222', 'bob-old', 'Y3Q=', 'bm8=', 'text',
     1700000000000, false, 'device-b', 'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=', now() - interval '30 hours');

do $$
declare
    result record;
begin
    select * into result from private.enforce_retention();

    if exists (select 1 from public.clipboard_items where item_id = 'forged-old') then
        raise exception 'a row with a forged created_at escaped the TTL';
    end if;
    if exists (select 1 from public.clipboard_items where item_id = 'bob-old') then
        raise exception 'the TTL did not reach a second account''s rows';
    end if;
    if not exists (select 1 from public.clipboard_items where item_id = 'fresh') then
        raise exception 'the TTL evicted a row that was inserted moments ago';
    end if;
    if result.ttl_deleted <> 2 then
        raise exception 'the job reported % TTL deletions, expected 2', result.ttl_deleted;
    end if;
end
$$;

-- The per-account cap, with `inserted_at` and `created_at` in opposite orders
-- so that only one of them can be the one being honoured.
insert into public.clipboard_items
    (user_id, item_id, ciphertext, nonce, content_type, created_at, deleted, origin_device_id,
     signature, inserted_at)
select
    '11111111-1111-4111-8111-111111111111',
    'cap-' || i,
    'Y3Q=', 'bm8=', 'text',
    2000000000000 - i,                     -- newest by the client's clock first
    false, 'device-a',
    'c2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDAwMDAwMDAwMDA=',
    now() - make_interval(mins => 60 - i)  -- newest by the server's clock last
from generate_series(1, 6) as i;

do $$
declare
    survivors text[];
begin
    perform private.enforce_retention();

    select array_agg(item_id order by item_id)
      into survivors
      from public.clipboard_items
     where user_id = '11111111-1111-4111-8111-111111111111';

    -- The 3 newest by `inserted_at` are `fresh` (inserted at `now()`), then
    -- cap-6 and cap-5. By `created_at` the winners would have been cap-1, cap-2
    -- and cap-3, so this distinguishes the two orderings rather than merely
    -- counting rows.
    if survivors is distinct from array['cap-5', 'cap-6', 'fresh'] then
        raise exception 'the cap kept the wrong rows: %', survivors;
    end if;

    if (select count(*) from public.clipboard_items
         where user_id = '22222222-2222-4222-8222-222222222222') <> 0 then
        raise exception 'the cap is not partitioned by account';
    end if;
end
$$;

rollback;

\echo '03_retention_and_paging: ok'
