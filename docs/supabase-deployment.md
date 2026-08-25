# The Supabase deployment

Everything the server side of cloud sync is: one table, its policies, its
Realtime configuration, and a retention job. It lives in `supabase/` and is
applied with the Supabase CLI.

The client is `crates/copypaste-cloud`. Its release integration test speaks to
a disposable local Supabase project through the same GoTrue, PostgREST, and
Realtime clients the product uses. `crates/copypaste-cloud/src/rest/mod.rs`
carries the SQL contract this deployment implements.

```
supabase/
  config.toml                     local stack: ports, auth, which schemas PostgREST exposes
  migrations/
    …120000_clipboard_items.sql   the table, its constraints, its indexes, the timestamp trigger
    …120100_clipboard_items_rls.sql   grants, revokes, four policies
    …120200_realtime.sql          the publication, and why DELETE is not in it
    …120300_retention.sql         TTL + per-account cap, the role that may run it, the schedule
  seed.sql                        two local accounts (no rows — see below)
  tests/                          assertions, plain SQL, runnable by psql anywhere
    real-supabase.sh              disposable full-stack release gate
  dev/verify-schema.sh            applies everything to a throwaway cluster and runs the assertions
  dev/smoke.sh                    the same round trip through PostgREST and GoTrue
  dev/postgrest-harness.sh        runs smoke.sh against a real PostgREST, without Docker
```

## The server cannot decrypt anything

Rows are sealed on the device with XChaCha20-Poly1305 under an Argon2id key
derived from a passphrase that never leaves it. There is no content column, no
`content_hash`, no index over content, and no key material anywhere in this
schema. `tests/01_schema_audit.sql` asserts that as a property rather than
leaving it to review: it fails if a column named `content`/`plaintext`/`body`/…
appears, if any column is a `tsvector`, or if any index mentions `to_tsvector`.

`seed.sql` seeds accounts but no clipboard rows, for the same reason — this
database has no way to manufacture a row a client could open.

RLS is the second layer. A misconfigured policy here exposes ciphertext, which
is a reason to get it right anyway: the metadata (device ids, content types,
payload sizes, timing, delete activity) is a real disclosure on its own.

## What was verified, and what was not

| | Status |
|---|---|
| Migrations apply cleanly, in order, from empty | **verified** — `supabase/dev/verify-schema.sh` |
| Account isolation, the constraints, the grants, the trigger | **verified** — `tests/01`–`02` |
| The pull query gets an index scan with no sort, in **both** cursor shapes | **verified** — `tests/03`, 10,000 rows, many of them sharing a millisecond |
| Retention evicts on the server clock, not the client's | **verified** — `tests/03` |
| The assertions fail when the schema is wrong | **verified** — 20 mutations (RLS off, force off, policy widened, index dropped, `inserted_at` made writable, retention re-pointed at `created_at`, DELETE re-published, …), each detected by at least one suite. One is worth knowing: widening the UPDATE policy to `using (true)` is caught by the static audit but *not* by the behavioural one, because the SELECT policy still hides the row from the `where` clause. Defence in depth, and a reason to keep both suites. |
| PostgREST resolves `on_conflict=user_id,item_id` with no `user_id` in the body | **verified** — `supabase/dev/postgrest-harness.sh`, PostgREST 12.2.3 |
| The client's `select=`, `order=`, `created_at=gte.` and keyset query strings are accepted verbatim | **verified** — same run |
| A row with no `signature` is refused by the deployment, not just by the client | **verified** — same run |
| The publishable key on its own reaches nothing | **verified** — same run |
| GoTrue sign-in, Realtime, encrypted convergence | **never verified.** `tests/real-supabase.sh` covers them and blocks publish, but it landed after `v2.0.0-alpha.5` and has not run once. Nothing here has ever spoken to an account service. |
| The rows above, end to end through the real platform rather than a harness | **never verified** — same gate. Each has narrower coverage listed above; none of it involves GoTrue or the platform's own PostgREST configuration. |
| `supabase start` / `supabase db reset` | **never verified** — same gate |

The ordinary CI schema job remains the fast PostgreSQL-only check. The release
workflow additionally starts the official local stack from empty, runs all SQL
assertions, then runs `crates/copypaste-cloud/tests/real_supabase.rs`. Publishing
depends on that job, so a failed platform contract cannot produce a release.

**The upsert assumption held.** `?on_conflict=user_id,item_id` resolves with no
`user_id` in the request body: the column default fills it before the conflict
is inferred, exactly as designed. The negative control matters as much — with
the unique index dropped, the *first* upsert fails **400** (`42P10`). The client
classifies it `Permanent` and the round fails loudly.

`scripts/cloud-stub.py` is a different thing and is not evidence about this
deployment: it answers the client's HTTP shapes in memory, with no Postgres, no
RLS and no JWT verification, and it is permissive where the real service is
strict. It proves the client is wired up; this directory is what it will be
wired up *to*.

## Local development

```bash
supabase start                      # needs Docker
supabase db reset                   # migrations + seed
supabase/dev/smoke.sh               # round trip through the API
supabase/tests/real-supabase.sh      # full release gate; starts and stops the stack
psql "$(supabase status -o json | jq -r .DB_URL)" -f supabase/tests/01_schema_audit.sql
```

Without Docker:

```bash
supabase/dev/verify-schema.sh       # SQL, policies, plans: a PostgreSQL server only
PGRST_BIN=./postgrest \
  supabase/dev/postgrest-harness.sh # + the API surface: a postgrest binary too
```

The two seeded accounts are `dev-a@example.test` and `dev-b@example.test`, both
with the password `copypaste-dev`. Two, not one, so that "device A cannot see
device B" is exercisable end to end and not only in SQL.

## Deploying

```bash
supabase link --project-ref <ref>
supabase db push
psql "$PROD_URL" -f supabase/tests/01_schema_audit.sql    # read-only, safe here
```

Then, in the project:

1. **Enable `pg_cron`.** The retention migration creates the function
   unconditionally and schedules it only if the extension is present; if it is
   not, the migration emits a `WARNING` and rows accumulate forever. Confirm
   with `select jobname, schedule, active from cron.job;`.
2. **Require email confirmation.** `config.toml` disables it for the local
   stack; leaving it disabled in production lets an unverified address hold an
   account, and the account password is what gates access to the ciphertext.
3. **Check the publication.** `supabase_realtime` must publish `insert, update`
   and not `delete` — see [Realtime](#realtime). The migration will warn rather
   than fail if it could not change it.

The anon key ships in the binary. That is intended: it gets a request past the
API gateway and nothing else, because `anon` holds no privilege and no policy on
this table (`tests/02` asserts the refusal).

## The table

Exactly the shape `rest/mod.rs` codes against: eight columns the client writes
and reads (`item_id`, `ciphertext`, `nonce`, `content_type`, `created_at`,
`deleted`, `origin_device_id`, `signature`), plus `id`, `user_id`,
`inserted_at`, `updated_at`, which the client never sends.

Four things are load-bearing and easy to lose in a refactor:

- **`ciphertext` is `text`, not `bytea`.** Assigning a bare base64 string to a
  `bytea` column stores the ASCII bytes of the base64 text and reads back as
  `\x…` hex. Manifest 05 §4.5 and AT-45 require an end-to-end text round trip.
- **The unique index on `(user_id, item_id)`** is what PostgREST resolves
  `on_conflict` through. Without it every write fails 400 (`42P10`) — measured,
  not assumed; see the verification table above.
- **`signature` is `not null`.** It is an HMAC over every other client column
  under a key derived from the sync key, and this database can neither produce
  nor check it. That is the point: the columns the merge orders on travel in the
  clear, so without a signature anything that can write here — including this
  service — can stamp a version that outranks a device's real one, or a
  tombstone that deletes an item everywhere (manifest 05 §5.3). It is a MAC of
  the *ciphertext*, never of the plaintext: a plaintext hash stored beside the
  ciphertext would be an equality oracle over content.
- **`(user_id, created_at, item_id)`** is the pull query's index: equality on
  the RLS pivot, range on the cursor, and the third column supplying the
  tie-break the ordering needs so the page comes back in order without a sort.
  `tests/03` asserts the plan, not just the index's existence — with 10,000 rows
  it fails if the plan contains a `Seq Scan` or a `Sort`.

`created_at` is a client-supplied version clock in milliseconds, restamped on
every mutation, and it is what the cursor pages on. `inserted_at` is
server-assigned and is the only ordering retention is allowed to trust.

## Row-level security

Four policies, all `to authenticated`, all `user_id = auth.uid()`: SELECT
`using`, INSERT `with check`, UPDATE **both**, DELETE `using`. RLS is enabled
*and* forced. `anon` and `PUBLIC` are revoked before anything is granted.
`user_id` defaults to `auth.uid()` so the client never spells it out.

Two additions beyond the contract, both about the same attack:

- **Column-level INSERT/UPDATE grants.** `authenticated` may write only the
  eight client columns. `id`, `user_id`, `inserted_at` and `updated_at` are not
  writable at all. Without this, any account holder could restamp `inserted_at`
  and escape eviction (manifest 05 §5.1 row 4a, `CopyPaste-1uqb`).
- **The timestamp trigger restores `inserted_at`** on every update, so the
  privilege system is not the only thing holding that column still.

RLS pivots on `user_id`, not on a device, so **one account is one trust circle**:
every device signed in reads every other device's rows.

The retention job is the only cross-account reader, and because RLS is *forced*
it cannot inherit an exemption from owning the table: it runs `SECURITY DEFINER`
as a dedicated `copypaste_retention` role that has exactly two policies (SELECT
and DELETE, `using (true)`) and no INSERT or UPDATE anywhere. It can remove
rows; it can never write one.

## Quota and TTL

Cloud storage is bounded to **500 items per account for 24 hours**. The policy
lives in `private.retention_policy` (one row, `ttl_hours = 24`,
`max_rows_per_user = 500`) and is enforced by
`private.enforce_retention()` on an hourly `pg_cron` schedule.

Where each limit lives, and why:

| Limit | Home | Reasoning |
|---|---|---|
| 24 h item TTL | **Server**, `pg_cron` | Client-side deletion is explicitly ruled out by §5.2: a device offline for a month would delete rows the others still need. An RLS predicate (`inserted_at > now() - 24h`) was considered and rejected — it hides rows without reclaiming storage, and it would change what a client sees in the middle of a drain. |
| 500 rows per account | **Server**, same job | Same reasoning, plus rule 4a: the prune must order on a server-assigned value. Ranking is `row_number() over (partition by user_id order by inserted_at desc)`. |
| Prune by server clock, never the client's | **Server**, three ways | The job reads `inserted_at`; `authenticated` holds no column privilege on it; the trigger restores it on update. `tests/03` inserts a row stamped a decade into the future by the client and asserts it is still evicted. |
| Per-item size (10 MiB image/file, 8 MiB text) | **Client** (`sync::push`), with a server backstop | Manifest verdict for row 13: enforce before upload so the user gets a clear local error rather than an opaque backend rejection. The server's `check (octet_length(ciphertext) <= 16 MiB)` sits above every client-side limit; it bounds abuse, it is not the product limit. The client measures the **plaintext**, before sealing, so the number it reports is the one the user can see. |
| HTTP rate limiting | **Platform** | Supabase enforces its own; the client honours `429` + `Retry-After` in delta-seconds (`rest/error.rs`, `sync/retry.rs`). |
| Page size | **Client**, ceiling on the server | The client clamps to 200 (`MAX_PAGE_LIMIT`); PostgREST's `max_rows` is 1000. That ordering matters — if `max_rows` were ever set *below* the client's limit, PostgREST would silently truncate pages and "a short page means caught up" would stop being true (manifest row 8). |
| Local history cap | **Client**, unchanged | And local eviction must not move the download watermark (INV-N5). |

Consequences worth stating plainly, because they are user-visible:

- A device offline for more than 24 hours misses whatever aged out. Local
  history is the durable copy; the cloud table is a transit buffer.
- A row evicted by retention is **not** re-uploaded: push only sends local
  changes newer than the watermark. It is gone from the cloud for good.
- A new device signing in sees at most the retention window, never the full
  history of the account.
- Retention deletions produce no Realtime events (see below) and must never
  delete anything locally.

Changing the policy is an `update private.retention_policy set …`, not a
migration. Setting either column to `NULL` disables that half — which is
manifest 05 §5.2's third option, and it is a decision to write into the privacy
documentation, not a default to drift into.

## Realtime

The client joins `realtime:clipboard_items` with
`postgres_changes [{event: "*", schema: "public", table: "clipboard_items",
filter: "user_id=eq.<uuid>"}]`. The table is added to the `supabase_realtime`
publication, replica identity stays DEFAULT, and **the publication does not
publish DELETE**. Three reasons, any one sufficient:

1. Realtime cannot apply RLS to a delete. With the default replica identity the
   WAL carries only the primary key, so `user_id` is not there to check a policy
   or the subscriber's filter against, and delete events reach subscribers RLS
   would have excluded. Ciphertext is not at risk; "account X deleted item Y at
   time T" is.
2. The client would report each one as a fault. `frame.rs` treats a DELETE whose
   `old_record` has no `item_id` as a protocol error — correctly, since guessing
   there would delete the wrong row — and every retention deletion would produce
   one.
3. There is nothing to learn from it. A user's delete travels as a *tombstone*,
   an ordinary row version with `deleted = true` and the payload wiped, which
   arrives as an UPDATE. The only real DELETEs are retention's, and a row aged
   out of the cloud must not be removed locally.

Replica identity is deliberately **not** `FULL`: it would put the whole previous
row, ciphertext included, into the WAL and into every UPDATE event, for a field
the client never reads.

Realtime remains an accelerator and never the source of truth. It is
at-most-once and replays nothing missed while the socket was down; the cursor
poll is the correctness mechanism (manifest row 9a — "the single most important
item in this table"). Nothing in this deployment can change that, and one thing
in it depends on it: rows larger than Realtime's per-record limit (1 MiB by
default) are not delivered as events at all, and are picked up by the next poll.

## Compound cursor pagination

INV-N1 requires a keyset over a total order with no ties, and AT-24 is its
regression test. A single inclusive `created_at=gte.<cursor>` bound avoids
skipping rows at the boundary millisecond, but it cannot advance when more than
one page shares that millisecond: every pull would return the same first page by
`item_id`, leaving the remaining rows unreachable.

The client carries both halves and sends the compound form once it knows them,
against the deployment index:

```
or=(created_at.gt.M,and(created_at.eq.M,item_id.gt.ID))
&order=created_at.asc,item_id.asc
```

Two details are worth keeping straight, because they look contradictory:

- **The bare millisecond bound stays inclusive.** A strict `gt` is only safe on
  the *pair*, which is unique. On the millisecond alone it drops the boundary
  rows.
- **The tie-break may be absent**, and then the inclusive form is what goes out.
  A page is still drained by the pair within one round, so the stall is gone
  from the drain; a source that does not persist the second half re-offers the
  boundary millisecond on the next round, which is free. `supabase/tests/03`'s
  plan assertion covers both shapes — same index, same ordering.

## Deployment additions to the contract in `rest/mod.rs`

The deployment adds the following constraints without changing the client's
column names, types, `select=` list, conflict target, `user_id` default, or four
policies:

| Addition | Why |
|---|---|
| `check` constraints mirroring `CloudItem::validate` (tombstone carries no payload, live row carries one, `item_id` charset, `created_at >= 0`) | The same rules the client already enforces, as a backstop for a writer that is not this client. Manifest 05 §5.3 is explicit that account compromise implies write access. |
| Column-level INSERT/UPDATE grants | Retention-order forgery — see [Row-level security](#row-level-security). |
| Trigger also restores `inserted_at` | Same. |
| `(select auth.uid())` instead of bare `auth.uid()` in the policies | Identical predicate, evaluated once per query instead of once per row. |
| An index on `inserted_at` | The TTL delete's probe. |
| The publication excludes DELETE | See [Realtime](#realtime). |
| Metadata length caps (`content_type`, `origin_device_id`, 128 chars) | These columns are plaintext, and so are the only place bulk data could be stashed in the clear. |

## What is not here

- **Pin state does not cross this transport.** Manifest 05 T-6 and §3.6 treat
  `pinned`/`pin_order` as ordinary LWW fields that must travel; `CloudItem` has no
  such columns, so the table has none. The divergence is recorded in manifest
  05 §3.6, together with the thing it is entangled with: the daemon refuses a
  remote delete of a pinned row *because* pin state does not sync, so carrying
  pin state means revisiting that refusal in the same change. Neither half moves
  alone.
- **No storage bucket.** Large payloads go in the row; if that changes, the size
  backstop and Realtime's per-record limit are the two constraints to revisit.
- **No `service_role` usage.** Nothing here needs a key that bypasses RLS, and
  the retention job deliberately uses a least-privilege role instead.
