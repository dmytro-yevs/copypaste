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

-- Supabase Storage keeps file objects as rows in `storage.objects`, with RLS
-- enabled by the platform. Only the columns and helper the policies use.
create schema if not exists storage;
create table if not exists storage.buckets (
    id                 text primary key,
    name               text not null,
    public             boolean not null default false,
    file_size_limit    bigint,
    allowed_mime_types text[]
);
create table if not exists storage.objects (
    id         uuid primary key default gen_random_uuid(),
    bucket_id  text references storage.buckets (id),
    name       text not null,
    owner_id   text default (auth.uid())::text,
    created_at timestamptz not null default now(),
    unique (bucket_id, name)
);
alter table storage.objects enable row level security;
-- Supabase's own definition: every path segment but the last.
create or replace function storage.foldername(name text) returns text[]
    language plpgsql
    as $$
declare
    _parts text[];
begin
    select string_to_array(name, '/') into _parts;
    return _parts[1:array_length(_parts, 1) - 1];
end
$$;
grant usage on schema storage to anon, authenticated, service_role;
grant all on storage.objects to anon, authenticated, service_role;
grant select on storage.buckets to anon, authenticated, service_role;
grant execute on function storage.foldername(text) to anon, authenticated, service_role;
