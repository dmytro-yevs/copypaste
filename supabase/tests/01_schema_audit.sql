-- ---------------------------------------------------------------------------
-- Static audit: assertions about the schema text itself, no writes.
--
-- Safe to run against any deployment, including production:
--     psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f supabase/tests/01_schema_audit.sql
--
-- Manifest 05 AT-51 asks for exactly this ("cheap, and it caught real drift")
-- and AT-52 adds the unique index the upsert needs. Plain SQL rather than
-- pgTAP: these run identically under `psql` against a stock PostgreSQL server,
-- the local stack, and a hosted project, and adding a test-only extension to
-- get `ok()` and `is()` would buy nothing this file needs.
-- ---------------------------------------------------------------------------

\set ON_ERROR_STOP on
\timing off

-- --- the columns the client writes and reads -------------------------------

do $$
declare
    expected text[] := array[
        'ciphertext', 'content_type', 'created_at', 'deleted',
        'item_id', 'nonce', 'origin_device_id'
    ];
    found text[];
begin
    -- `rest::SELECT_COLUMNS`, sorted. If this fails, either the table or that
    -- constant moved and the other did not.
    select array_agg(column_name order by column_name)
      into found
      from information_schema.columns
     where table_schema = 'public' and table_name = 'clipboard_items'
       and column_name = any (expected);

    if found is distinct from expected then
        raise exception 'clipboard_items is missing columns the client selects: expected %, found %',
            expected, found;
    end if;
end
$$;

do $$
declare
    bad text;
begin
    -- The security property, asserted rather than assumed: there is no column
    -- that could hold plaintext, and no full-text index over content anywhere.
    select string_agg(format('%I.%I', table_name, column_name), ', ')
      into bad
      from information_schema.columns
     where table_schema = 'public'
       and (column_name in ('content', 'plaintext', 'body', 'text', 'preview', 'content_hash')
            or data_type = 'tsvector');
    if bad is not null then
        raise exception 'a column that could hold plaintext or a content index exists: %', bad;
    end if;

    select string_agg(indexname, ', ') into bad
      from pg_indexes
     where schemaname = 'public' and indexdef ilike '%to_tsvector%';
    if bad is not null then
        raise exception 'a full-text index over content exists: %', bad;
    end if;
end
$$;

-- --- indexes ---------------------------------------------------------------

do $$
begin
    -- AT-52. PostgREST resolves `on_conflict=user_id,item_id` through this
    -- index; without it every replayed batch is a 409 instead of a no-op.
    if not exists (
        select 1 from pg_indexes
         where schemaname = 'public' and tablename = 'clipboard_items'
           and indexdef ilike 'CREATE UNIQUE INDEX%(user_id, item_id)'
    ) then
        raise exception 'the unique index on (user_id, item_id) is missing: the upsert conflict target will not resolve';
    end if;

    -- The cursor's index. `fetch_since` filters `created_at=gte.…` under an
    -- RLS-supplied `user_id` and orders `created_at.asc,item_id.asc`.
    if not exists (
        select 1 from pg_indexes
         where schemaname = 'public' and tablename = 'clipboard_items'
           and indexdef ilike '%(user_id, created_at, item_id)'
    ) then
        raise exception 'the (user_id, created_at, item_id) index is missing: every pull becomes a sequential scan';
    end if;
end
$$;

-- --- row-level security ----------------------------------------------------

do $$
declare
    unprotected text;
begin
    -- Deny by default, everywhere, not only on the table we remembered.
    select string_agg(format('%I.%I', schemaname, tablename), ', ')
      into unprotected
      from pg_tables
     where schemaname in ('public', 'private')
       and not exists (
           select 1 from pg_class c
             join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = schemaname and c.relname = tablename
              and c.relrowsecurity and c.relforcerowsecurity
       );
    if unprotected is not null then
        raise exception 'these tables do not have row-level security enabled AND forced: %', unprotected;
    end if;
end
$$;

do $$
declare
    p record;
    n integer := 0;
begin
    for p in
        select policyname, cmd, roles, qual, with_check
          from pg_policies
         where schemaname = 'public' and tablename = 'clipboard_items'
           and 'authenticated' = any (roles)
    loop
        n := n + 1;
        if coalesce(p.qual, '') !~ 'auth\.uid\(\)'
           and coalesce(p.with_check, '') !~ 'auth\.uid\(\)' then
            raise exception 'policy % does not scope on auth.uid()', p.policyname;
        end if;
        -- `using` alone on an UPDATE would let a row be rewritten into another
        -- account.
        if p.cmd = 'UPDATE' and (p.qual is null or p.with_check is null) then
            raise exception 'the UPDATE policy needs both `using` and `with check`';
        end if;
    end loop;

    if n <> 4 then
        raise exception 'expected 4 policies for `authenticated` (select/insert/update/delete), found %', n;
    end if;
end
$$;

do $$
declare
    granted text;
begin
    -- `anon` holds the publishable key. It must reach nothing.
    select string_agg(distinct privilege_type, ', ')
      into granted
      from information_schema.role_table_grants
     where table_schema = 'public' and table_name = 'clipboard_items'
       and grantee in ('anon', 'PUBLIC');
    if granted is not null then
        raise exception 'anon/PUBLIC still hold privileges on clipboard_items: %', granted;
    end if;

    select string_agg(distinct privilege_type, ', ')
      into granted
      from information_schema.role_column_grants
     where table_schema = 'public' and table_name = 'clipboard_items'
       and grantee in ('anon', 'PUBLIC');
    if granted is not null then
        raise exception 'anon/PUBLIC still hold column privileges on clipboard_items: %', granted;
    end if;
end
$$;

do $$
declare
    writable text;
begin
    -- Server-assigned columns must not be writable by the client, or the
    -- retention order can be forged (manifest 05 §5.1 row 4a).
    select string_agg(distinct format('%s(%s)', privilege_type, column_name), ', ')
      into writable
      from information_schema.role_column_grants
     where table_schema = 'public' and table_name = 'clipboard_items'
       and grantee = 'authenticated'
       and privilege_type in ('INSERT', 'UPDATE')
       and column_name in ('id', 'user_id', 'inserted_at', 'updated_at');
    if writable is not null then
        raise exception 'authenticated can write server-assigned columns: %', writable;
    end if;
end
$$;

do $$
declare
    default_expr text;
begin
    select column_default into default_expr
      from information_schema.columns
     where table_schema = 'public' and table_name = 'clipboard_items'
       and column_name = 'user_id';
    -- The default is what lets the client omit `user_id`; the `with check` is
    -- what makes omitting it safe. Both, or neither works.
    if coalesce(default_expr, '') !~ 'auth\.uid\(\)' then
        raise exception 'user_id has no auth.uid() default (found %)', default_expr;
    end if;
end
$$;

-- --- realtime --------------------------------------------------------------

do $$
declare
    pub record;
begin
    select * into pub from pg_publication where pubname = 'supabase_realtime';
    if not found then
        raise exception 'the supabase_realtime publication does not exist';
    end if;
    if not exists (
        select 1 from pg_publication_tables
         where pubname = 'supabase_realtime' and schemaname = 'public'
           and tablename = 'clipboard_items'
    ) then
        raise exception 'clipboard_items is not published to Realtime: a second device only learns of a write on the next poll';
    end if;
    if not (pub.pubinsert and pub.pubupdate) then
        raise exception 'the publication does not publish inserts and updates';
    end if;
    if pub.pubdelete then
        raise exception 'the publication publishes DELETE: Realtime cannot apply RLS to a delete, so one account''s delete activity reaches every other subscriber';
    end if;
end
$$;

-- --- retention -------------------------------------------------------------

do $$
declare
    f record;
begin
    select p.prosecdef, r.rolname into f
      from pg_proc p
      join pg_namespace n on n.oid = p.pronamespace
      join pg_roles r on r.oid = p.proowner
     where n.nspname = 'private' and p.proname = 'enforce_retention';
    if not found then
        raise exception 'private.enforce_retention() is missing: nothing evicts anything';
    end if;
    if not f.prosecdef then
        raise exception 'private.enforce_retention() is not SECURITY DEFINER and will be blocked by forced RLS';
    end if;
    if f.rolname <> 'copypaste_retention' then
        raise exception 'private.enforce_retention() is owned by % rather than the least-privilege role', f.rolname;
    end if;

    if exists (
        select 1 from information_schema.role_routine_grants
         where specific_schema = 'private' and routine_name = 'enforce_retention'
           and grantee in ('anon', 'authenticated', 'PUBLIC')
    ) then
        raise exception 'an API role can execute the cross-account delete function';
    end if;
end
$$;

\echo '01_schema_audit: ok'
