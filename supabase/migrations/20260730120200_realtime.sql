-- ---------------------------------------------------------------------------
-- Realtime.
--
-- What the client subscribes to (`crates/copypaste-cloud/src/realtime/`):
--
--     topic:  realtime:clipboard_items
--     config: postgres_changes [{ event: "*", schema: "public",
--                                 table: "clipboard_items",
--                                 filter: "user_id=eq.<uuid>" }]
--
-- Realtime is an accelerator and never the source of truth: `postgres_changes`
-- is at-most-once and replays nothing that happened while the socket was down.
-- The cursor poll is the correctness mechanism (manifest 05 row 9a, §4.8).
-- Nothing in this file changes that, and no configuration here can.
-- ---------------------------------------------------------------------------

-- Replica identity stays DEFAULT (the primary key) rather than FULL.
--
-- FULL would put the whole old row — ciphertext included — into the WAL and
-- into every UPDATE event's `old_record`, doubling the bytes for a payload the
-- client never reads: `frame.rs` uses `record`/`new` for INSERT and UPDATE and
-- only ever reads `item_id` out of an old record.
alter table public.clipboard_items replica identity default;

do $do$
declare
    published text;
begin
    if not exists (select 1 from pg_publication where pubname = 'supabase_realtime') then
        -- Fresh/self-hosted database. On a Supabase project the platform has
        -- already created this publication.
        create publication supabase_realtime with (publish = 'insert, update');
    else
        select pubinsert::int::text || pubupdate::int::text || pubdelete::int::text
          into published
          from pg_publication where pubname = 'supabase_realtime';
        if published <> '110' then
            begin
                alter publication supabase_realtime set (publish = 'insert, update');
            exception when insufficient_privilege then
                raise warning
                    'could not restrict the supabase_realtime publication to insert,update. '
                    'DELETE events are not filtered by row-level security, so leaving them '
                    'published leaks one account''s delete activity to every other subscriber. '
                    'Run as the publication owner: '
                    'alter publication supabase_realtime set (publish = ''insert, update'');';
            end;
        end if;
    end if;

    if not exists (
        select 1 from pg_publication_tables
         where pubname = 'supabase_realtime'
           and schemaname = 'public'
           and tablename = 'clipboard_items'
    ) then
        alter publication supabase_realtime add table public.clipboard_items;
    end if;
end
$do$;

-- ---------------------------------------------------------------------------
-- Why DELETE is not published. Three reasons, any one of which is sufficient.
--
-- 1. Security. Realtime cannot evaluate RLS for a DELETE: with the default
--    replica identity the WAL carries only the primary key, so `user_id` is not
--    there to check a policy or a `user_id=eq.<uuid>` filter against. Delete
--    events therefore reach subscribers that RLS would have excluded. The rows
--    are ciphertext, but "account X deleted item Y at time T" is exactly the
--    metadata this design tries not to spill sideways.
--
-- 2. It would be noise the client reports as a fault. `frame.rs` treats a
--    DELETE whose `old_record` carries no `item_id` as a protocol error —
--    correctly, because guessing an id there would delete the wrong row — and
--    surfaces it to the subscriber. Every retention deletion would produce one.
--
-- 3. There is nothing for the client to learn from it. A user-initiated delete
--    travels as a *tombstone*: an ordinary row version with `deleted = true`
--    and the payload wiped, which arrives as an UPDATE (manifest 05 §3.5). The
--    only real DELETEs are the retention job's, and a row aged out of the cloud
--    must not be removed locally — local history has its own, longer, TTL.
--
-- The client's `RealtimeEvent::Delete` branch is consequently unreachable
-- against this deployment. It stays as the correct handling of an out-of-band
-- removal; it is not a path this schema feeds.
-- ---------------------------------------------------------------------------

-- ---------------------------------------------------------------------------
-- Two limits worth knowing, neither of which this file can raise:
--
-- * Realtime drops changes whose WAL record exceeds its configured maximum
--   (`max_record_bytes`, 1 MiB by default). A large image will not arrive as an
--   event — the poll picks it up on the next tick. This is the ordinary case of
--   "Realtime is an accelerator", not a failure.
-- * Realtime applies RLS per subscriber per change. The `user_id=eq.<uuid>`
--   filter in the join is what keeps that cheap, and it is mandatory on the
--   client side for that reason as well as the security one.
-- ---------------------------------------------------------------------------
