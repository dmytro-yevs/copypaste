-- ---------------------------------------------------------------------------
-- Retention: the TTL and the per-account row cap.
--
-- The v1 relay held at most 500 items per account for at most 24 hours
-- (manifest 05 §5.1 rows 4, 4a, 5 and §5.2). Those numbers were not a
-- limitation, they were the design: bounded storage, and a server that forgets.
-- Supabase as shipped keeps every row forever, so both properties have to be
-- rebuilt here or consciously abandoned. This file rebuilds them, with the v1
-- values as the defaults.
--
-- Why the server and not the client: a device that has been offline for a month
-- must not be the thing that decides what other devices still need. Client-side
-- DELETEs would do exactly that (manifest 05 §5.2, explicit).
--
-- Why not `expires_at` + a partial index, which is what v1's schema had: that
-- column was never acted on by anything. A column nobody reads is worse than no
-- column, so v2 has none and the job reads `inserted_at` instead.
-- ---------------------------------------------------------------------------

create schema if not exists private;

-- Not exposed through PostgREST (`config.toml` lists only `public` and
-- `graphql_public`), and not reachable by the API roles either.
revoke all on schema private from public;
revoke usage on schema private from anon, authenticated;

-- ---------------------------------------------------------------------------
-- The knobs, as a one-row table rather than as literals in the function body,
-- so that changing retention is an audited UPDATE rather than a migration — and
-- so the current policy is queryable when someone asks "how long do we keep
-- this?".
-- ---------------------------------------------------------------------------

create table private.retention_policy (
    id                  boolean primary key default true check (id),
    -- Delete rows older than this many hours, by `inserted_at`.
    -- NULL disables the TTL — which is a decision to state in the privacy
    -- documentation, not a default to drift into (manifest 05 §5.2 option 3).
    ttl_hours           integer check (ttl_hours is null or ttl_hours > 0),
    -- Keep at most this many rows per account, newest first by `inserted_at`.
    -- NULL disables the cap.
    max_rows_per_user   integer check (max_rows_per_user is null or max_rows_per_user > 0),
    updated_at          timestamptz not null default now()
);

comment on table private.retention_policy is
    'Single row. The cloud copy is a transit buffer, not an archive: local history '
    'has its own, longer, TTL and is the durable copy.';

insert into private.retention_policy (id, ttl_hours, max_rows_per_user)
values (true, 24, 500);

-- Deny by default like every other table here, even though nothing user-facing
-- can reach this schema.
alter table private.retention_policy enable row level security;
alter table private.retention_policy force row level security;

-- ---------------------------------------------------------------------------
-- The one role allowed to see across accounts.
--
-- `force row level security` on `clipboard_items` policed the table owner
-- deliberately, so the retention job cannot quietly inherit an exemption; it
-- needs a policy, and a policy names a role. A dedicated `nologin` role means
-- that exemption is a single greppable line rather than something folded into
-- an admin role that has many other reasons to exist.
-- ---------------------------------------------------------------------------

do $do$
begin
    if not exists (select 1 from pg_roles where rolname = 'copypaste_retention') then
        create role copypaste_retention nologin;
    end if;
    -- Ownership of the function below can only be handed to a role we are a
    -- member of. Superusers are exempt, a CREATEROLE role in PG16 gets the
    -- membership implicitly, and everything else needs this line.
    execute format('grant copypaste_retention to %I', current_user);
end
$do$;

grant usage on schema private to copypaste_retention;
grant select on private.retention_policy to copypaste_retention;
grant select, delete on public.clipboard_items to copypaste_retention;

create policy retention_policy_read on private.retention_policy
    for select to copypaste_retention using (true);

-- The cross-account exemption, in full. SELECT because the cap has to rank rows
-- per account; DELETE because that is the job. No INSERT and no UPDATE: this
-- role can remove rows and can never write one, so a compromise of it cannot
-- inject a version into anybody's history.
create policy clipboard_items_retention_select on public.clipboard_items
    for select to copypaste_retention using (true);

create policy clipboard_items_retention_delete on public.clipboard_items
    for delete to copypaste_retention using (true);

-- ---------------------------------------------------------------------------
-- The job.
-- ---------------------------------------------------------------------------

create or replace function private.enforce_retention()
    returns table (ttl_deleted bigint, cap_deleted bigint)
    language plpgsql
    security definer
    set search_path = ''
    as $$
declare
    ttl_h   integer;
    cap     integer;
    n_ttl   bigint := 0;
    n_cap   bigint := 0;
begin
    select ttl_hours, max_rows_per_user
      into ttl_h, cap
      from private.retention_policy
     where id;

    if ttl_h is not null then
        -- `inserted_at`, never `created_at`. `created_at` is client-supplied,
        -- and the relay learned the hard way that sorting eviction on a
        -- client-supplied clock lets an intra-account writer forge a value,
        -- escape eviction, and displace legitimate items (`CopyPaste-1uqb`,
        -- manifest 05 §5.1 row 4a). `inserted_at` is defaulted by the server
        -- and `authenticated` holds no column privilege on it.
        with gone as (
            delete from public.clipboard_items
             where inserted_at < now() - make_interval(hours => ttl_h)
            returning 1
        )
        select count(*) into n_ttl from gone;
    end if;

    if cap is not null then
        -- Whole-table pass: ranking every account's rows is a sequential scan
        -- plus a sort by construction, and no index avoids that. It is cheap
        -- while the table is small and is the first thing to reshape (per-user
        -- batches, driven off a candidate list) if it ever is not.
        with ranked as (
            select id,
                   row_number() over (
                       partition by user_id
                       order by inserted_at desc, id desc
                   ) as rn
              from public.clipboard_items
        ),
        gone as (
            delete from public.clipboard_items c
             using ranked r
             where c.id = r.id
               and r.rn > cap
            returning 1
        )
        select count(*) into n_cap from gone;
    end if;

    return query select n_ttl, n_cap;
end;
$$;

grant create on schema private to copypaste_retention;
alter function private.enforce_retention() owner to copypaste_retention;
revoke create on schema private from copypaste_retention;

-- SECURITY DEFINER functions are executable by PUBLIC by default. This one
-- deletes rows across every account.
revoke all on function private.enforce_retention() from public;

do $do$
begin
    execute format('grant execute on function private.enforce_retention() to %I', current_user);
end
$do$;

-- ---------------------------------------------------------------------------
-- Scheduling.
--
-- pg_cron is available on Supabase but has to be enabled, and a migration that
-- hard-fails on a project where it is not is worse than one that says so. The
-- function is created either way, so retention can be run by hand or by any
-- external scheduler while that is sorted out.
--
-- Verify after deploying:  select jobname, schedule, active from cron.job;
-- ---------------------------------------------------------------------------

do $do$
begin
    if not exists (select 1 from pg_extension where extname = 'pg_cron')
       and exists (select 1 from pg_available_extensions where name = 'pg_cron') then
        create extension pg_cron;
    end if;

    if exists (select 1 from pg_extension where extname = 'pg_cron') then
        if exists (select 1 from cron.job where jobname = 'copypaste-retention') then
            perform cron.unschedule('copypaste-retention');
        end if;
        -- Hourly, off the hour: a 24 h TTL does not need minute precision, and
        -- an off-peak minute keeps the whole-table cap pass away from the
        -- top-of-hour crowd.
        perform cron.schedule(
            'copypaste-retention',
            '17 * * * *',
            $cron$select private.enforce_retention()$cron$
        );
    else
        raise warning
            'pg_cron is not installed: retention is NOT scheduled and rows will accumulate '
            'forever. Enable the pg_cron extension and re-run this migration, or schedule '
            'select private.enforce_retention() externally.';
    end if;
end
$do$;
