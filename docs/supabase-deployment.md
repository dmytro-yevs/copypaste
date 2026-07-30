# The Supabase deployment

Everything the server side of cloud sync is: one table, its policies, its
Realtime configuration, and a retention job. It lives in `supabase/` and is
applied with the Supabase CLI.

The client is `crates/copypaste-cloud` — 7,500 lines, 156 tests, and until now
not one of them has spoken to a real Supabase project. `crates/copypaste-cloud/src/rest/mod.rs`
carries the SQL contract in its module docs; this deployment is built from that
contract, and every place it deviates is listed in [Deviations](#deviations-from-the-contract-in-restmodrs).

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
  dev/verify-schema.sh            applies everything to a throwaway cluster and runs the assertions
  dev/smoke.sh                    the same round trip through PostgREST and GoTrue
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
| PostgREST resolves `on_conflict=user_id,item_id` with no `user_id` in the body | **not verified** |
| GoTrue password sign-in against the seeded accounts | **not verified** |
| Realtime delivers `postgres_changes` for the client's join frame | **not verified** |
| `supabase start` / `supabase db reset` | **not verified** |

The container this was built in has a Docker client but no daemon and no
Supabase CLI, so nothing that needs a container was run. What *was* run is a
stock PostgreSQL 16 server with `auth.uid()`, `auth.users` and the API roles
stubbed (`supabase/dev/harness/00-supabase-stubs.sql`) — enough to prove the SQL
and the policies, and not enough to prove anything about PostgREST, GoTrue or
Realtime. `supabase/dev/smoke.sh` covers that half and is marked unverified at
the top of the file.

**Run `smoke.sh` before trusting the upsert.** If PostgREST will not resolve the
conflict target without `user_id` in the request body, every replayed batch is a
409 and push breaks in a way no test in this repository can currently see.

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
psql "$(supabase status -o json | jq -r .DB_URL)" -f supabase/tests/01_schema_audit.sql
```

Without Docker:

```bash
supabase/dev/verify-schema.sh       # needs a PostgreSQL server installation only
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

Exactly the shape `rest/mod.rs` codes against: seven columns the client writes
and reads (`item_id`, `ciphertext`, `nonce`, `content_type`, `created_at`,
`deleted`, `origin_device_id`), plus `id`, `user_id`, `inserted_at`,
`updated_at`, which the client never sends.

Three things are load-bearing and easy to lose in a refactor:

- **`ciphertext` is `text`, not `bytea`.** Assigning a bare base64 string to a
  `bytea` column stores the ASCII bytes of the base64 text and reads back as
  `\x…` hex. That is manifest 05 §4.5 — the bug that made cloud download fail
  outright in v1.
- **The unique index on `(user_id, item_id)`** is what PostgREST resolves
  `on_conflict` through. Without it the upsert is a 409 and every replay fails.
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
  seven client columns. `id`, `user_id`, `inserted_at` and `updated_at` are not
  writable at all. Without this, any account holder could restamp `inserted_at`
  and escape eviction — the forgery of manifest 05 §5.1 row 4a, moved from v1's
  `wall_time` to v2's retention column.
- **The timestamp trigger restores `inserted_at`** on every update, so the
  privilege system is not the only thing holding that column still.

The model is unchanged from v1's: RLS pivots on `user_id`, not on a device, so
**one account is one trust circle** — every device signed in reads every other
device's rows.

The retention job is the only cross-account reader, and because RLS is *forced*
it cannot inherit an exemption from owning the table: it runs `SECURITY DEFINER`
as a dedicated `copypaste_retention` role that has exactly two policies (SELECT
and DELETE, `using (true)`) and no INSERT or UPDATE anywhere. It can remove
rows; it can never write one.

## Quota and TTL

The v1 relay held **≤ 500 items per account for ≤ 24 hours**. Manifest 05 §5.2
is explicit that this was the design and not a limitation: bounded storage, and
a server that forgets within a day. Supabase as shipped keeps every row forever,
so both properties are rebuilt here, with v1's numbers as the defaults, in
`private.retention_policy` (one row, `ttl_hours = 24`, `max_rows_per_user =
500`) driven by `private.enforce_retention()` on an hourly `pg_cron` schedule.

Where each of v1's limits now lives, and why:

| Limit | Home | Reasoning |
|---|---|---|
| 24 h item TTL | **Server**, `pg_cron` | Client-side deletion is explicitly ruled out by §5.2: a device offline for a month would delete rows the others still need. An RLS predicate (`inserted_at > now() - 24h`) was considered and rejected — it hides rows without reclaiming storage, and it would change what a client sees in the middle of a drain. |
| 500 rows per account | **Server**, same job | Same reasoning, plus rule 4a: the prune must order on a server-assigned value. Ranking is `row_number() over (partition by user_id order by inserted_at desc)`. |
| Prune by server clock, never the client's | **Server**, three ways | The job reads `inserted_at`; `authenticated` holds no column privilege on it; the trigger restores it on update. `tests/03` inserts a row stamped a decade into the future by the client and asserts it is still evicted. |
| Per-item size (10 MiB image/file, 8 MiB text) | **Client** (`sync::push`), with a server backstop | Manifest verdict for row 13: enforce before upload so the user gets a clear local error rather than an opaque backend rejection. The server's `check (octet_length(ciphertext) <= 16 MiB)` sits above every client-side limit; it bounds abuse, it is not the product limit. The client measures the **plaintext**, before sealing, so the number it reports is the one the user can see. |
| Per-account device cap (5 / 10) | **Dropped** | A billing lever, not a correctness one (manifest row 12). |
| HTTP rate limiting | **Platform** | Supabase enforces its own; the client honours `429` + `Retry-After` in delta-seconds (`rest/error.rs`, `sync/retry.rs`). |
| Page size | **Client**, ceiling on the server | The client clamps to 200 (`MAX_PAGE_LIMIT`); PostgREST's `max_rows` is 1000. That ordering matters — if `max_rows` were ever set *below* the client's limit, PostgREST would silently truncate pages and "a short page means caught up" would stop being true (manifest row 8). |
| Local history cap | **Client**, unchanged | And local eviction must not move the download watermark (INV-N5). |

Consequences worth stating plainly, because they are user-visible:

- A device offline for more than 24 hours misses whatever aged out. Local
  history is the durable copy; the cloud table is a transit buffer. This is v1's
  behaviour, not a regression.
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

## Relay → Supabase parity check

Manifest 05 §5.1, row by row, against what now exists. "Covered" here means
covered *by something that is written down and runnable*, not covered in
principle.

| # | Relay guarantee | Where it lives now | Status |
|---|---|---|---|
| 1 | Shared-account fan-out | `clipboard_items` + the four policies | ✅ Covered. Asserted in `tests/02`. |
| 2 | Independent per-device credentials on one account | GoTrue sessions; each device holds its own refresh token, sign-out revokes one | ✅ Covered by the platform. Nothing owed server-side. |
| 3 | Distinct per-device inboxes | — | ⚪ Dropped, as the manifest recommends. |
| 4 | Per-account quota, 500 items, silent prune of the oldest | `private.retention_policy.max_rows_per_user` + `enforce_retention()` | ✅ **Gap closed.** Asserted in `tests/03`. Needs `pg_cron` enabled on the project. |
| 4a | Prune on a server-assigned value, never a client clock | `inserted_at`: read by the job, not writable by `authenticated`, restored by the trigger | ✅ Covered, three independent ways, one test. |
| 5 | 24 h item TTL | `private.retention_policy.ttl_hours` | ✅ **Gap closed**, same job, same caveat about `pg_cron`. |
| 6 | Keyset cursor pagination with no ties | Client keyset `or=(created_at.gt.M,and(created_at.eq.M,item_id.gt.ID))` + the `(user_id, created_at, item_id)` index | ✅ **Gap closed, client-side.** See the note below for what it was and what it now is. |
| 7 | `Relay-Watermark` response header | Client computes it from the last row of the page | ⚪ Dropped, no loss. |
| 8 | `Relay-Has-More` header | Not needed: PostgREST has no byte budget, so a short page is unambiguously "caught up" | ⚪ Dropped — **conditional on `max_rows` (1000) staying above the client's page limit (200)**, which is now written down in `config.toml`. |
| 9 | At-least-once delivery, replay from a cursor | Rows persist until retention; poll replays from the watermark | ✅ Covered, and now bounded by our own TTL exactly as the relay's was. |
| 9a | Push channel is at-most-once; the poll is mandatory | `sync::pull` + `sync::cadence`; Realtime only resets the interval | ✅ Covered — and nothing in this deployment tempts anyone to remove the poll. Retention deletes are invisible to Realtime by design, which makes the poll the only path for them, which is correct. |
| 10 | Proof-of-possession registration | GoTrue account auth + RLS | ⚠️ Structurally replaced, and **the secret being proven is different**. An attacker with the account password but not the sync passphrase can read all ciphertext and metadata and *write rows*. Bounded here by: the AEAD's AAD binding `item_id` (client), `created_at >= 0` and the charset check (server), the future-skew refusal (client), and `inserted_at` being unforgeable so an injected row cannot escape eviction. **Still owed:** signing the LWW metadata under the sync key, which is the manifest's own suggested mitigation and the one capability the relay had that this does not. It is stated plainly in [cloud-privacy](cloud-privacy.md) rather than left to a table nobody reads, and it needs a column here, so it is a deployment change as well as a client one. |
| 11 | Per-(IP, device) registration rate limit | GoTrue sign-in throttling | 🟡 Roughly covered by the platform. No per-item-id analogue is needed. |
| 12 | Per-account device cap | — | ⚪ Dropped. Billing lever. |
| 13 | Per-item size cap, split by type | Client caps in `sync::push` (8 MiB text, 10 MiB otherwise), server `check` at 16 MiB base64 as the backstop | ✅ **Covered.** Measured on the plaintext, before sealing, so the number is the one the user can see; an item over the cap is withheld and counted (`skipped_too_large`), never deleted and never sent for the backend to refuse. |
| 14 | Per-IP rate limiting with `Retry-After` | Platform + the client's 429 handling | ✅ Covered. |
| 15 | Inactive-device reaping | — | ⚪ Dropped. No device registry exists. |
| 16 | Zero-knowledge, account-less server | — | 🔴 **Genuine regression, unchanged.** Content stays end-to-end encrypted; the metadata surface grows and sync now requires an account. What this deployment stores per row: `user_id`, `item_id`, ciphertext **length**, `content_type`, `origin_device_id`, `created_at`, `deleted`, `inserted_at`, `updated_at` — plus the account email in GoTrue. Retention shortens the window to 24 h, which limits but does not remove it. **Written down now**, in [cloud-privacy](cloud-privacy.md) (manifest §5.4 item 3). The metadata surface itself is unchanged: documenting a regression does not remove it. |
| 17 | Operational scaffolding (write-behind cache, retry queue, supervisors, metrics, connection caps) | Supabase's problem | ✅ Dropped entirely. This is the payoff. |

### Row 6, in full — what it was, and what it is now

v1's cursor was a compound keyset: `or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))`.
INV-N1 states the requirement as a keyset over a total order with no ties,
"not over a millisecond timestamp alone", and AT-24 is its regression test.

v2's client sent a single inclusive bound — `created_at=gte.<cursor>`. The
inclusive bound fixed the worse half of the v1 bug: no row sharing the boundary
millisecond is skipped, because re-offering a row is free and skipping it is not.
But the cursor carried only the millisecond, so if **more than one page of rows
shared a single `created_at`** (100 rows, `PULL_PAGE_LIMIT`), the watermark could
not advance past that millisecond: every pull re-fetched the same first 100 of
them by `item_id` and the rest were never reached.

The client now carries both halves and sends the compound form once it knows
them, against the index this deployment already had:

```
or=(created_at.gt.M,and(created_at.eq.M,item_id.gt.ID))
&order=created_at.asc,item_id.asc
```

Two details are worth keeping straight, because they look contradictory:

- **The bare millisecond bound stays inclusive.** A strict `gt` is only safe on
  the *pair*, which is unique. On the millisecond alone it drops the boundary
  row, which is the original v1 bug.
- **The tie-break may be absent**, and then the inclusive form is what goes out.
  A page is still drained by the pair within one round, so the stall is gone
  from the drain; a source that does not persist the second half re-offers the
  boundary millisecond on the next round, which is free. `supabase/tests/03`'s
  plan assertion covers both shapes — same index, same ordering.

## Deviations from the contract in `rest/mod.rs`

Everything the client depends on is unchanged: column names and types, the
`select=` list, the conflict target, the `user_id` default, the four policies.
The additions:

| Deviation | Why |
|---|---|
| `check` constraints mirroring `CloudItem::validate` (tombstone carries no payload, live row carries one, `item_id` charset, `created_at >= 0`) | The same rules the client already enforces, as a backstop for a writer that is not this client. Manifest 05 §5.3 is explicit that account compromise implies write access. |
| Column-level INSERT/UPDATE grants | Retention-order forgery — see [Row-level security](#row-level-security). |
| Trigger also restores `inserted_at` | Same. |
| `(select auth.uid())` instead of bare `auth.uid()` in the policies | Identical predicate, evaluated once per query instead of once per row. |
| An index on `inserted_at` | The TTL delete's probe. |
| The publication excludes DELETE | See [Realtime](#realtime). |
| Metadata length caps (`content_type`, `origin_device_id`, 128 chars) | These columns are plaintext, and so are the only place bulk data could be stashed in the clear. |

`rest/mod.rs` used to describe the read index as `(created_at asc, item_id
desc)` while `fetch_since` sent `order=created_at.asc,item_id.asc` and
`sync::pull::sort_page` sorted ascending on both. The code was consistent and the
comment was stale; the comment now says `asc` on both, which is what the index
here is. Building the index the old comment described would have cost an
incremental sort on every page.

## What is not here

- **Pin state does not cross this transport, and that is now a decision rather
  than a silence.** Manifest 05 T-6 and §3.6 treat `pinned`/`pin_order` as
  ordinary LWW fields that must travel; v2's `CloudItem` has no such columns, so
  the table has none. The divergence is recorded in manifest 05 §3.6, together
  with the thing it is entangled with: the daemon refuses a remote delete of a
  pinned row *because* pin state does not sync, so carrying pin state means
  revisiting that refusal in the same change. Neither half moves alone.
- **No `expires_at`, no `content_hash`, no `blob_ref`, no `is_sensitive`, no
  `app_bundle_id`.** All are in v1's schema; §7.6 lists the first three as dead
  columns, `is_sensitive` is recomputed on the receiver from plaintext the
  server never has, and sensitive items never leave the device at all.
- **No storage bucket.** Large payloads go in the row; if that changes, the size
  backstop and Realtime's per-record limit are the two constraints to revisit.
- **No `service_role` usage.** Nothing here needs a key that bypasses RLS, and
  the retention job deliberately uses a least-privilege role instead.
