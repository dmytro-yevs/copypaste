-- Encrypted file objects: one account's folder, opaque names, a quota, and no
-- rewrites. Rolled back afterwards.
\set ON_ERROR_STOP on
begin;

do $$
begin
    if (select public from storage.buckets where id = 'clipboard-files') then
        raise exception 'the file bucket is public';
    end if;
end
$$;

set local role authenticated;
set local request.jwt.claim.sub = '11111111-1111-4111-8111-111111111111';
insert into storage.objects (bucket_id, name) values
    ('clipboard-files', '11111111-1111-4111-8111-111111111111/0123456789abcdef0123456789abcdef/00000'),
    ('clipboard-files', '11111111-1111-4111-8111-111111111111/0123456789abcdef0123456789abcdef/00001');

do $$
begin
    begin
        insert into storage.objects (bucket_id, name) values
            ('clipboard-files', '22222222-2222-4222-8222-222222222222/0123456789abcdef0123456789abcdef/00000');
        raise exception 'an object was written into another account''s folder';
    exception when insufficient_privilege then null;
    end;
    begin
        insert into storage.objects (bucket_id, name) values
            ('clipboard-files', '11111111-1111-4111-8111-111111111111/report.pdf/00000');
        raise exception 'a readable object name was accepted';
    exception when insufficient_privilege then null;
    end;
    update storage.objects set name = name || '0';
    if found then
        raise exception 'an object was rewritten';
    end if;
end
$$;

-- The quota counts the account's objects: 510 more fill it, the next fails.
insert into storage.objects (bucket_id, name)
select 'clipboard-files',
       '11111111-1111-4111-8111-111111111111/fedcba9876543210fedcba9876543210/' || lpad(i::text, 5, '0')
from generate_series(1, 510) as i;
do $$
begin
    begin
        insert into storage.objects (bucket_id, name) values
            ('clipboard-files', '11111111-1111-4111-8111-111111111111/fedcba9876543210fedcba9876543210/00511');
        raise exception 'the object quota was not enforced';
    exception when insufficient_privilege then null;
    end;
end
$$;

set local request.jwt.claim.sub = '22222222-2222-4222-8222-222222222222';
do $$
begin
    if exists (select 1 from storage.objects) then
        raise exception 'another account''s objects are visible';
    end if;
    delete from storage.objects;
    if found then
        raise exception 'another account''s objects were deleted';
    end if;
end
$$;

set local request.jwt.claim.sub = '11111111-1111-4111-8111-111111111111';
delete from storage.objects where name like '%/0123456789abcdef0123456789abcdef/%';
do $$
begin
    if (select count(*) from storage.objects) <> 510 then
        raise exception 'the owner could not delete its objects';
    end if;
end
$$;

rollback;
