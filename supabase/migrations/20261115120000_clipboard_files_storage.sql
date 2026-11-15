-- ---------------------------------------------------------------------------
-- clipboard-files: encrypted file chunks for devices that are not online
-- together (Copy mode). The client contract is
-- `crates/copypaste-cloud/src/storage.rs` and `crypto/file.rs`.
--
-- The server sees ciphertext and opaque names only:
-- `{user id}/{object id}/{index}`, where the object id is an HMAC of the item
-- id under a key derived from the sync key, and index 00000 is the sealed
-- manifest. Objects follow the 24-hour row TTL: the uploading device deletes
-- them (Supabase rejects direct SQL deletes from `storage.objects`).
-- ---------------------------------------------------------------------------

-- 4 MiB of plaintext per object plus the nonce and tag.
insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('clipboard-files', 'clipboard-files', false, 4194368, array['application/octet-stream'])
on conflict (id) do update set
    public = excluded.public,
    file_size_limit = excluded.file_size_limit,
    allowed_mime_types = excluded.allowed_mime_types;

-- Counts one account's file objects. A policy on `storage.objects` cannot
-- query the same table under RLS without recursing, so this runs as its owner.
-- Its own schema: `private` stays closed to client roles.
create schema if not exists clipboard_files;
revoke all on schema clipboard_files from public;
create or replace function clipboard_files.object_count(account uuid) returns bigint
    language sql stable security definer
    set search_path = ''
    as $$
    select count(*)
    from storage.objects
    where bucket_id = 'clipboard-files'
      and (storage.foldername(name))[1] = account::text
$$;
revoke all on function clipboard_files.object_count(uuid) from public, anon;
grant usage on schema clipboard_files to authenticated;
grant execute on function clipboard_files.object_count(uuid) to authenticated;

-- Each account reads, writes and deletes only under its own folder. There is
-- no update policy: objects are immutable, so an upsert is refused.
create policy clipboard_files_select on storage.objects
    for select to authenticated
    using (
        bucket_id = 'clipboard-files'
        and (storage.foldername(name))[1] = (select auth.uid())::text
    );

-- The name shape keeps plaintext out of object names, and the object count
-- caps an account at about 2 GiB (512 objects of at most 4 MiB).
create policy clipboard_files_insert on storage.objects
    for insert to authenticated
    with check (
        bucket_id = 'clipboard-files'
        and (storage.foldername(name))[1] = (select auth.uid())::text
        and name ~ '^[0-9a-f-]{36}/[0-9a-f]{32}/[0-9]{5}$'
        and clipboard_files.object_count((select auth.uid())) < 512
    );

create policy clipboard_files_delete on storage.objects
    for delete to authenticated
    using (
        bucket_id = 'clipboard-files'
        and (storage.foldername(name))[1] = (select auth.uid())::text
    );
