-- ---------------------------------------------------------------------------
-- The minimum of Supabase that the migrations and the assertions depend on,
-- for running them against a stock PostgreSQL server.
--
-- This exists because `supabase start` needs Docker, and the schema and the
-- policies are worth verifying even where Docker is not available. It stubs
-- *identity* only — the roles, `auth.users`, and `auth.uid()`. It does not
-- stand in for GoTrue, PostgREST or Realtime, so the checks it enables are
-- "the SQL is right", never "the client works". See `supabase/dev/smoke.sh`
-- for the other half.
-- ---------------------------------------------------------------------------

do $$
begin
    if not exists (select 1 from pg_roles where rolname = 'anon') then
        create role anon nologin noinherit;
    end if;
    if not exists (select 1 from pg_roles where rolname = 'authenticated') then
        create role authenticated nologin noinherit;
    end if;
    if not exists (select 1 from pg_roles where rolname = 'service_role') then
        create role service_role nologin noinherit bypassrls;
    end if;
end
$$;

grant usage on schema public to anon, authenticated, service_role;

create schema if not exists auth;

-- Only the columns anything here touches. Real GoTrue has ~30 more.
create table if not exists auth.users (
    id         uuid primary key,
    email      text unique,
    created_at timestamptz not null default now()
);

-- Byte-for-byte the shape Supabase ships: both spellings of the claim setting,
-- `stable`, and a NULL rather than an error when no claims are set. The
-- policies are only as trustworthy as this function is faithful.
create or replace function auth.uid() returns uuid
    language sql stable
    as $$
    select coalesce(
        nullif(current_setting('request.jwt.claim.sub', true), ''),
        (nullif(current_setting('request.jwt.claims', true), '')::jsonb ->> 'sub')
    )::uuid
$$;

grant usage on schema auth to anon, authenticated, service_role;
grant execute on function auth.uid() to anon, authenticated, service_role;
