# 05 — Sync Correctness & Backend Protocols

> **Port manifest, implementation-independent.** Harvested from the pre-rewrite
> tree (P2P `copypaste-sync` + `sync_orch`, Supabase `cloud/` + `copypaste-supabase`,
> and the custom `copypaste-relay`). Everything here is a *contract*, not an API.
> Old `file:line` citations are provenance, not a porting instruction. Paths are
> relative to the repo root; where a section is already scoped to one crate the
> citations are shortened (e.g. `merge.rs:28` inside §3 means
> `crates/copypaste-sync/src/merge.rs`).
>
> STATUS: complete.
>
> **Read §5.4 first if the question is "is dropping the relay safe?"**

---

## 1. Purpose & scope

### 1.1 What this document is for

The rewrite is library-first and **drops the custom relay server in favour of
Supabase**. Three years of bug-driven refinement went into the *convergence
rules*, not into the transport code. This manifest extracts those rules so they
can be re-implemented from scratch without re-discovering the same bugs.

### 1.2 The domain in one paragraph

A user has N devices. Each device has a local SQLite history of *clipboard
items*. An item is identified across devices by a stable `item_id`. Item content
is **opaque ciphertext** encrypted client-side under a per-account sync key — the
backend never sees plaintext and cannot compare, hash, or order content.
Therefore **all conflict resolution operates on metadata only**. Any two devices
that have seen the same set of writes MUST hold the same winning version.

### 1.3 Two identifier spaces (source of the worst historical bug)

| Field | Scope | Stability | Role |
|---|---|---|---|
| `item_id` | cross-device | stable for the life of the logical item | **CRDT identity.** All dedup, LWW lookup, and upsert conflict targets key on this. Bound into the AEAD AAD. |
| `id` (row PK) | per-device | freshly generated (`uuid_v4`) on every device | local row PK only. FTS / pins / copy-item all key on it. |
| `origin_device_id` | cross-device | stable per originating peer | final LWW tie-break |

> **Rule R-ID-1.** Never use the row PK for cross-device comparison, dedup,
> lookup, or tie-break. Two separate historical bugs came from doing so:
> - `sync_orch` looked up incoming items by `wire.id` instead of `wire.item_id`,
>   so the local row was never found, every item was treated as new, and the
>   INSERT hit the `idx_clipboard_item_id` UNIQUE constraint — updates were
>   silently dropped (`crates/copypaste-daemon/src/sync_orch/merge/mod.rs:144-151`).
> - the LWW final tie-break compared `remote.origin_device_id` against
>   `local.id` — two different identifier spaces. Row PKs are random per write,
>   device ids are stable, so the winner was nondeterministic and peers diverged
>   (`crates/copypaste-sync/src/merge.rs:42-54`, "merge.rs:39 BUG"; fixed by adding
>   `ClipboardItem::origin_device_id` in schema v3).

> **Rule R-ID-2.** On an LWW replace, **preserve the local row PK**. Adopting the
> remote's `id` orphans FTS rows, pins, and any local reference. Both the P2P and
> cloud paths implement this as `preserved_pk`
> (`sync_orch/merge/mod.rs:159-163, 313-317`; `cloud/poll/ingest.rs:106-108, 304-309`).

### 1.4 Scope boundary

In scope: LWW ordering, Lamport stamping, tombstones, idempotency/replay,
watermark pagination, Supabase auth/REST/Realtime protocol behaviour, relay
feature parity, poll cadence.

Out of scope: encryption scheme, key derivation/rotation, pairing/mTLS, blob
chunking, UI. (Those live in sibling manifests.)

---

## 2. Invariants (MUST hold)

### Convergence

- **INV-C1 (total order).** The comparison used to pick a winner between two
  versions of the same `item_id` MUST be a **strict total order on metadata
  only**, identical on every device and on every transport. Content is opaque;
  it can never be an input to the decision.
- **INV-C2 (one comparator, all transports).** P2P, Supabase, and any future
  transport MUST route through the *same* comparator function. Historically they
  did not: P2P used the full 3-key order while cloud and relay used a bare
  `remote_lamport <= local → keep`. On **equal** lamport that always kept the
  local copy — decided by arrival locality, not by the data — so two devices with
  the same `item_id` at equal lamport and different content each kept their own
  copy and **never converged**. (`CopyPaste-ayvs`; fix = the shared `remote_wins`
  primitive, `crates/copypaste-sync/src/merge.rs:66-112`.)
- **INV-C3 (order-independence).** Applying a set of remote versions in any
  arrival order MUST yield the same final state. This follows from INV-C1 given
  the merge is a pure `max` under the total order.
- **INV-C4 (identity).** Convergence is per `item_id`, never per row PK (R-ID-1).

### Idempotency

- **INV-I1 (re-delivery is a no-op).** Re-delivering a version already applied
  MUST change nothing. This is *free* given INV-C1: a re-delivered version
  compares equal on all three keys, and the comparator's final step is a strict
  `>` — equal loses, so the local copy is kept.
- **INV-I2 (self-echo is a no-op).** A transport where a device both publishes to
  and subscribes from the same stream will receive its own writes back. That MUST
  be absorbed by INV-I1, not by a special "did I send this?" filter. (This is
  exactly how the relay's shared-account inbox worked:
  `crates/copypaste-daemon/src/relay/mod.rs:32-35`.)
- **INV-I3 (multi-transport delivery).** If the same `item_id` is delivered over
  two transports simultaneously, exactly one row must exist locally afterwards.
  Again a consequence of INV-C1 + item_id dedup (`daemon/src/relay/mod.rs:44-51`).
- **INV-I4 (cursor advance ≠ apply).** A cursor/watermark MUST advance past every
  row that was *readable*, including rows that were skipped (LWW loser,
  undecryptable, duplicate). Otherwise the same unreadable row is re-fetched
  forever. (`cloud/poll/ingest.rs:458-466`; relay `receive/ingest.rs:50-57`.)

### No data loss

- **INV-N1 (no silent skip on pagination).** A cursor MUST be a compound keyset
  over a **total order with no ties**, not over a millisecond timestamp alone.
  See §4.4 — a `wall_time`-only strict `gt` cursor permanently loses every row
  sharing the boundary millisecond once a burst fills a page.
- **INV-N2 (delete wins, and cannot be resurrected).** See §3.5.
- **INV-N3 (undecryptable ≠ delete).** A version that cannot be decrypted (wrong
  key, missing key, tampered blob) MUST be skipped with a warning and MUST NOT
  overwrite or delete the local copy, and MUST NOT be persisted as a partial
  "poison row" (a row with no nonce / no blob reference that every downstream
  consumer then rejects). (`CopyPaste-jww` / `CopyPaste-5y4`,
  `sync_orch/merge/mod.rs:286-301`.)
- **INV-N4 (atomic replace).** Replacing a local version MUST be atomic across
  (delete old row, insert new row, update the full-text index). A non-atomic
  delete-then-insert loses the row if the insert fails.
  (`sync_orch/merge/mod.rs:371-381`, `replace_item_atomic`.)
- **INV-N5 (retention prune must not move the cursor).** Locally evicting old
  rows to honour a storage cap MUST NOT move the download watermark backwards.
  The watermark is stored independently of the item rows precisely for this
  reason (`cloud/poll/ingest.rs:475-491`).
- **INV-N6 (fail-closed auth).** If configured credentials fail to authenticate,
  sync MUST abort rather than silently downgrading to a lower-privilege
  (anonymous) scope — a downgrade masks credential rotation, misconfiguration, or
  an active attack (`crates/copypaste-daemon/src/cloud/auth.rs:42-53`).

---

## 3. Merge / ordering algorithm

### 3.1 The total order

Compare two versions of the *same* `item_id`. Larger wins. Keys in priority
order:

1. `lamport_ts` (i64)
2. `wall_time` (Unix ms, i64)
3. `origin_device_id` (lexicographic byte order on the string)

Ties on all three ⇒ **keep local** (the final comparison is a strict `>`). This
makes the operation idempotent (INV-I1) and makes re-delivery free.

```
function remote_wins(local{lamport, wall, origin}, remote{lamport, wall, origin}) -> bool:
    if remote.lamport != local.lamport: return remote.lamport > local.lamport
    if remote.wall    != local.wall:    return remote.wall    > local.wall
    return remote.origin > local.origin        # strict: equal -> keep local
```

Reference: `crates/copypaste-sync/src/merge.rs:28-64` (`resolve`, object form) and
`:97-112` (`remote_wins`, three-scalar form for transports that decode from
different wire shapes). A property test asserts the two agree across the whole
3×3×3 decision space (`merge.rs:650-687`).

> **Do NOT re-introduce two comparators.** If the rewrite needs both an
> object-shaped and a scalar-shaped entry point, one must be defined in terms of
> the other, and the equivalence must be property-tested.

### 3.2 Why `wall_time` before `origin_device_id`

`origin_device_id` alone would be deterministic but arbitrary — one device would
always win ties, which is a *stable* but *wrong-feeling* result. Wall time is a
best-effort human-intent approximation; `origin_device_id` exists purely to make
the last step total.

### 3.3 Lamport stamping rule

Every **local** mutation (capture, re-copy/promote, pin, unpin, reorder, delete)
stamps:

```
next_lamport_ts(prev_lamport, now_ms) = max(prev_lamport + 1, now_ms)
```

(`crates/copypaste-core/src/storage/items/types.rs:65-67`, saturating add.)

This single formula gives **both** properties the order needs:

- **monotonic** — strictly greater than the row's own previous value, so two
  edits inside the same wall-clock millisecond still order correctly;
- **time-ordered** — at least `now_ms`, so across devices the newest *writer*
  wins under lamport-first LWW, and `wall_time` / `origin_device_id` only break
  exact ties.

> **Bug `CopyPaste-ojhe` (must not recur).** Before unification the daemon
> stamped `lamport_ts` with **three colliding conventions in the same i64
> column**: fresh capture = `0`, recopy/promote = `now_ms` (~1.75e12), pin/delete
> = `existing + 1` (a small counter). A stale recopy therefore permanently
> outranked a newer pin/delete: **pins were silently overwritten and deletes were
> resurrected.** (`types.rs:38-64`; regression test
> `merge.rs:728-753 unified_pin_delete_beats_older_recopy`.)
>
> **Rule:** exactly one value space for `lamport_ts`. Every writer uses
> `next_lamport_ts`. No exceptions, including "internal" mutations like pin
> ordering.

Backward compatibility of the formula: a legacy row at `lamport_ts = 0` (or a
peer emitting a small counter) is deterministically dominated by any fresh
`now_ms`-based write, and loses to any strictly larger future value — so
newest-writer-wins holds with **no migration**.

### 3.4 Clock-skew handling

Only two guards existed, and both are worth porting:

- **Lower bound, at the decode boundary.** `lamport_ts` and `wall_time` are i64
  on the wire but semantically non-negative. A hostile or buggy peer sending
  `lamport_ts = -1` would, when cast to u64 for a Lamport clock, become
  `u64::MAX` and **win every comparison forever**. Both fields are clamped to
  `>= 0` during deserialization — not in the ingest loop — so *every* ingress
  path is covered regardless of call site (`CopyPaste-psx7`;
  `crates/copypaste-sync/src/protocol.rs:47-127, 236-256`).
- **Upper bound, dynamic.** The engine additionally rejected values above a
  ceiling computed relative to the local clock. The decode-boundary clamp is
  deliberately lower-bound-only so the ceiling can stay transport-specific
  (`protocol.rs:98-103, 639-641`).

> **Rule R-CLK-1.** Validate at the *decode boundary*, not at the *use site*. A
> validation that lives only in one consumer is a validation that a future
> second consumer will silently skip.
>
> **Rule R-CLK-2.** No wall-clock skew *correction* is attempted, and none should
> be. The design deliberately tolerates skew: `lamport_ts` (monotonic per row) is
> the primary key and `wall_time` is only a tie-break, so a device with a wrong
> clock can win ties it "shouldn't" but cannot corrupt causality or resurrect
> deletes.

Saturation: the standalone `LamportClock` type saturates at `u64::MAX` with a
once-per-process warning rather than panicking
(`crates/copypaste-sync/src/clock.rs:38-80`). Note this type was **not** on the
daemon production path — the daemon stamps via `next_lamport_ts` and resolves via
the comparator; the clock type survived only for the P2P session protocol
(`clock.rs:1-7`). *Do not port the clock type without a live consumer.*

### 3.5 Tombstones and delete-wins

A delete is **not** a row removal — it is a *version* of the item with
`deleted = true` and content wiped. It therefore participates in normal LWW.

Rules:

- **T-1 (tombstone is a normal version).** The comparator does not know about
  `deleted`. A tombstone beats a live version iff it wins the 3-key order; a live
  version beats an older tombstone iff it wins. This falls out for free because
  the delete was stamped with `next_lamport_ts` and so is strictly newer than the
  version it deleted. (`merge.rs:574-599`.)
- **T-2 (tombstone must propagate its flag).** `deleted` MUST be carried on the
  wire and MUST be persisted on the receiver. A winning tombstone lands as
  `deleted = true` with content NULL, wiping the payload on that device
  (`merge.rs:143-145`).
- **T-3 (delete-before-create: insert a tombstone for an UNKNOWN item).** This is
  the subtle one. If a delete arrives for an `item_id` the device has never seen
  (out-of-order delivery, or the device was offline when the item was created and
  came back after it was deleted), the device MUST **persist a tombstone row
  anyway**. Otherwise a later-arriving create has nothing to lose LWW against and
  **resurrects the item**. (`CopyPaste-bfiu`; implemented identically in all three
  transports: `sync_orch/merge/mod.rs:202-223`, `cloud/poll/ingest.rs:166-192`,
  `relay/receive/ingest.rs:138-155`.)
- **T-4 (tombstone carries no ciphertext).** A tombstone MUST send `payload = NULL`
  — never a stale ciphertext. Enforced client-side as a hard precondition
  (`crates/copypaste-supabase/src/rest/write.rs:89-104`: reject a "tombstone" whose
  payload is set).
- **T-5 (upsert must always send `deleted` explicitly).** On an upsert that
  merges duplicates, **omitting** the `deleted` column lets the server fall back
  to the column default (`false`) and **resurrects a tombstoned item**. `deleted`
  must always be present in the payload, including `deleted: false` for live rows.
  (`CopyPaste-kgs7`, `rest/write.rs:14-25, 123-148`.)
- **T-6 (pin/order columns follow the same rule).** `pinned` must always be sent
  (including `false`), and `pin_order` must serialise as explicit `null` rather
  than being omitted — omission cannot clear a previously-set cloud value.
  (`CopyPaste-vqm0`, `rest/write.rs:27-32, 179-193`.)

### 3.6 What travels with a version

The winning version replaces the loser wholesale. Fields that MUST be carried
and restored, each with a bug behind it:

| Field | Why it must travel |
|---|---|
| `origin_device_id` | tie-break determinism across relay hops. Preserve the *original* origin; do not restamp with the forwarding device. Only stamp the local id when the field is empty (legacy rows). (`merge.rs:157-205`) |
| `key_version` | the ciphertext + AAD were produced under this key generation. Hard-coding a version on receive made **every synced item undecryptable** on the receiver (AEAD auth failure). Absent-on-wire must default to the *current* version, not `1` — defaulting to 1 resurrects the bug. (`merge.rs:135-141`, `protocol.rs:184-199`) |
| `deleted` | T-2 |
| `pinned`, `pin_order` | pin and reorder are ordinary LWW operations, not local-only state. **Trust the wire value; do not OR-merge with local state** — the writer bumped `lamport_ts` before broadcasting, so the wire only wins when it is causally later, which is exactly when its pin state should apply. (`sync_orch/merge/mod.rs:319-325`) |
| `is_sensitive` | must be **recomputed on the receiver from the decrypted plaintext**, not trusted from the wire. Historically receive always set it `false`, so a password synced from another device bypassed the auto-wipe TTL. (`CopyPaste-kcf`, `sync_orch/merge/mod.rs:327-341`) |
| file `name` / `mime` | for file items these live inside the at-rest blob metadata, which the outbound re-key path clears. They must be lifted onto the wire before that happens or the receiver falls back to a literal `"file"` name. (`merge.rs:171-179`) |

Local-only, must NOT travel: the row PK (R-ID-2), the image thumbnail
(capture-time derived, backfilled locally — `merge.rs:150-153`), `is_synced`.

> **v2 divergence, decided 2026-07-30: pin state does not travel over the cloud
> transport.** `CloudItem` has no `pinned`/`pin_order`, the Supabase table has no
> such columns, and T-6 therefore has nothing to apply to on this path. Pinning
> is a local decision that syncs, if at all, over P2P.
>
> This is a divergence and not an oversight, so it is written here rather than
> left implicit for a third time. What it costs: pin a row on device A and it
> stays unpinned on device B until a P2P round carries it, and the pin/unpin is
> not an LWW *version* on the cloud path at all — a cloud row that wins the merge
> does not carry pin state with it, so the receiver keeps its own.
>
> What it entangles: the daemon **refuses a remote delete of a pinned row**
> precisely because pin state does not sync — without that refusal, a device that
> cannot see the pin would delete a row the user had pinned, and data loss is the
> worst outcome (`CLAUDE.md` rule 4). The two decisions stand or fall together.
> Carrying pin state means revisiting that refusal in the same change: it is
> three fields (`pinned`, `pin_order` as explicit `null` per T-6, and both on
> `LocalItem`), a column pair and a migration, and a `CloudSource` that can read
> and write them — none of it hard, all of it across two crates and the
> deployment. Until that happens, neither half may be changed alone.

### 3.7 Merge pseudocode (transport-agnostic)

```
apply_remote_versions(versions):
    for v in versions:
        clamp_nonnegative(v.lamport_ts, v.wall_time)          # R-CLK-1

        local = lookup_by_item_id(v.item_id)                  # R-ID-1

        if local exists and not remote_wins(local, v):
            skip                                              # INV-I1 / INV-I2
            advance_cursor(v); continue                       # INV-I4

        preserved_pk = local?.row_pk                          # R-ID-2

        if v.deleted:                                         # tombstone path
            if local exists: soft_delete(preserved_pk, v.lamport_ts, v.wall_time)
            else:            insert_tombstone(v.item_id, v.lamport_ts,
                                              v.wall_time, v.origin_device_id)   # T-3
            advance_cursor(v); continue

        plaintext = decrypt(v.payload)                        # opaque until here
        if decrypt failed:
            warn (never log payload or key)
            advance_cursor(v); continue                       # INV-N3, INV-I4

        row = build_local_row(v, plaintext)
        row.row_pk       = preserved_pk ?? new_pk
        row.is_sensitive = detect_sensitive(plaintext)        # CopyPaste-kcf

        atomically { delete_old_row; insert row; reindex_fts } # INV-N4
        advance_cursor(v)

    if anything_written: prune_to_storage_cap()               # INV-N5-safe
```

**Concurrency note worth porting.** The historical implementation ran this in
three phases so the expensive CPU work never held the database lock:
phase 1 = LWW resolve + tombstone writes (lock held), phase 2 = decrypt / image
decode+re-encode (**lock released**), phase 3 = writes + prune (lock re-acquired).
Holding the DB mutex across a PNG decode was the single cause of "one tiny image
stalls every database writer" (`sync_orch/merge/mod.rs:118-131, 240-252, 353-357`).

---

## 4. Backend protocol requirements (Supabase side)

### 4.1 Table

One table, `clipboard_items`, one row per logical item per account.
(`docs/supabase/schema.sql`.)

| Column | Type | Notes |
|---|---|---|
| `id` | uuid PK | row PK; per-device (see R-ID-1) |
| `item_id` | uuid NOT NULL | **CRDT identity — needs a UNIQUE constraint, see §4.2** |
| `user_id` | uuid NOT NULL → `auth.users(id)` ON DELETE CASCADE, `DEFAULT auth.uid()` | RLS pivot |
| `device_id` | text NOT NULL | = `origin_device_id` on the wire; note the **name mismatch** |
| `content_type` | text NOT NULL | `text` \| `image` \| `file` |
| `payload_ct` | bytea | ciphertext; NULL for tombstones |
| `content_nonce` | bytea | |
| `content_hash` | bytea | SHA-256 of plaintext, client-side dedup only; server cannot verify |
| `blob_ref` | text | large-blob pointer |
| `is_sensitive` | bool NOT NULL DEFAULT false | (recomputed on receive anyway — §3.6) |
| `lamport_ts` | bigint NOT NULL | LWW key 1 |
| `wall_time` | bigint NOT NULL | LWW key 2, Unix ms |
| `expires_at` | bigint | Unix ms, nullable |
| `app_bundle_id` | text | |
| `deleted` | bool NOT NULL DEFAULT false | tombstone flag (§3.5) |
| `pinned` | bool NOT NULL DEFAULT false | |
| `pin_order` | double precision | fractional ordering |
| `created_at` / `updated_at` | timestamptz | `updated_at` maintained by a BEFORE UPDATE trigger |

Indexes that earned their place:
`(user_id, created_at desc)` (owner scan / Realtime hot path);
`(expires_at) where expires_at is not null` (TTL probe);
`(user_id, content_hash) where content_hash is not null` (dedup).

> **New requirement for the rewrite:** the poll query orders by
> `(wall_time asc, id asc)` — that pair needs an index
> (`(user_id, wall_time, id)`), which the old schema did **not** have.

### 4.2 ⚠️ Known schema gap to fix in the rewrite

The client upserts with PostgREST `?on_conflict=item_id` +
`Prefer: resolution=merge-duplicates` (`rest/write.rs:36-58`), but **the
published schema declares no UNIQUE constraint or unique index on `item_id`**
(`docs/supabase/schema.sql:34` — `item_id uuid not null` and nothing else;
`docs/supabase/setup.sql:34` likewise). PostgREST requires a unique index to
resolve `on_conflict`. Add:

```sql
create unique index clipboard_items_user_item_uidx
    on public.clipboard_items (user_id, item_id);
```

Scoping the uniqueness to `(user_id, item_id)` rather than `item_id` alone is the
right call for a shared table; PostgREST's `on_conflict` must then name both
columns.

### 4.3 RLS expectations

(`docs/supabase/rls-policies.sql`.)

- `alter table … enable row level security;` **and** `force row level security;`
- Four policies, all `to authenticated`, all predicated on `user_id = auth.uid()`
  — SELECT `using`, INSERT `with check`, UPDATE **both** `using` and `with check`,
  DELETE `using`.
- `alter column user_id set default auth.uid()` so clients never spell out
  `user_id`; the default fires before the `with check`.
- `revoke all … from anon;` then
  `grant select, insert, update, delete … to authenticated;` — Postgres' default
  ACL grants ALL to PUBLIC, so an unrevoked `anon` can attempt reads with only the
  publishable key.

**Documented trade-off to carry forward:** RLS pivots on `user_id`, not
`device_id`, because `device_id` is a locally-generated UUID with no relationship
to `auth.uid()`; enforcing it would require a custom JWT claim or a join table.
Consequence: **one account = one trust circle**; every device signed into the
account can read every other device's items. That is the model the whole design
assumes.

**Client-side consequence (important):** because RLS grants only the
`authenticated` role, requests bearing only the anonymous/publishable key are
rejected outright. The anon-key code path in the old client was therefore dead
weight in practice (`cloud/auth.rs:26-30`).

### 4.4 Read path: keyset pagination

Query shape (`cloud/poll/cursor.rs:31, 57-82`):

```
GET /rest/v1/clipboard_items
    ?select=<explicit column list>
    &order=wall_time.asc,id.asc
    &limit=20
    &or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))
```

> **INV-N1, the watermark bug, in full.** The original query was
> `order=wall_time.desc&limit=20` with **no lower bound**, so every tick
> re-fetched the same newest 20 rows: older history never downloaded at all, and
> anything beyond 20 rows between ticks was lost forever
> (`cloud/poll/loop_task.rs:68-82`).
>
> The first fix — `wall_time`-only cursor with strict `gt` and `asc` order — had a
> second, subtler failure: `wall_time` is *millisecond* granularity, so a burst of
> ≥ `limit` rows sharing the same maximum millisecond was fatal. One tick fetched
> `limit` of them, advanced the watermark to that millisecond, and the next tick's
> strict `gt` filtered out the remaining same-millisecond rows **forever** —
> silent download data loss (`cursor.rs:36-56`).
>
> The correct fix is a **compound keyset** over `(wall_time, id)`, a tuple with no
> ties, expressed as "a later millisecond, OR the same millisecond with a larger
> id". The relay's inbox pull independently rediscovered the same requirement
> (`crates/copypaste-relay/src/state/inbox/pull.rs:34-49`).

Cursor rules:

- **Cold start** (persisted watermark has a `wall` but no `id`, e.g. restored from
  an older `wall`-only setting): use an **inclusive `gte`** on the boundary
  millisecond so no boundary row is skipped, and rely on `item_id` dedup to absorb
  the re-offered rows (`cursor.rs:66-73`).
- **Seeding**: `max(persisted_watermark, local MAX(wall_time))`; either source
  missing contributes `0` (`cursor.rs:88-108`).
- **Persistence**: written inside the same lock/transaction as the ingest so a
  crash cannot lose it; a persist failure only costs re-pagination
  (`cursor.rs:113-120`, `poll/ingest.rs:507-515`).
- **Advance on every readable row** (INV-I4), including LWW losers and
  undecryptable rows.
- **Never regress**: the new cursor is seeded from the old one, so `max` semantics
  make it monotonic (`poll/ingest.rs:446-448, 532-536`).
- **Burst drain**: when a batch comes back exactly `limit` rows, re-poll
  immediately instead of waiting the interval — otherwise a multi-device burst
  drains at one page per interval (`poll/loop_task.rs:193-196, 230-234`).
- **Stall guard**: if a *full* batch produced **no** cursor advance (every row
  unparseable), break the drain loop instead of spinning on the same window
  forever (`poll/loop_task.rs:236-247`).

### 4.5 Write path: upsert

- `POST /rest/v1/clipboard_items?on_conflict=item_id` with
  `Prefer: resolution=merge-duplicates` ⇒ `INSERT … ON CONFLICT (item_id) DO
  UPDATE SET …`. Accept 200 / 201 / 204 as success (`rest/write.rs:39-78`).
- Always send `deleted`, `pinned`, `pin_order` (T-5, T-6).
- Omit `user_id` — the column default `auth.uid()` fills it and RLS `with check`
  enforces it (`cloud/ingest.rs:41-42`).
- **`bytea` encoding gotcha.** `payload_ct` is `bytea`. Assigning a *bare base64
  string* is accepted by Postgres but stores the **literal ASCII bytes of the
  base64 text**, and PostgREST then returns bytea on read in hex output form
  (`\x…`) — so the read path's base64 decode failed and cloud **download never
  worked at all**. Send the canonical hex input form `\x<hex>`; decode
  symmetrically on read. (`cloud/ingest.rs:60-74, 99-105`.)

> **Live gap in the old daemon:** the daemon's own push path posted to
> `/rest/v1/clipboard_items` with **no** `on_conflict` parameter
> (`cloud/push/transport.rs:49-58`), while the library's `RestClient` used the
> proper upsert. Re-pushing an item (e.g. after a pin change) therefore collided
> on the PK, was classified `Permanent`, and was dropped. **The rewrite must use
> one upsert path everywhere.**

### 4.6 Auth flows (GoTrue)

Endpoints and shapes actually exercised (`crates/copypaste-supabase/src/auth.rs`):

| Operation | Request |
|---|---|
| password sign-in | `POST /auth/v1/token?grant_type=password`, header `apikey: <anon>`, body `{email, password}` |
| refresh | `POST /auth/v1/token?grant_type=refresh_token`, header `apikey: <anon>`, body `{refresh_token}` |
| sign-out | `POST /auth/v1/logout`, `apikey` + `Authorization: Bearer <access>`; success = 204 or 200 |
| user profile | `GET /auth/v1/user`, `apikey` + bearer |

Session handling:

- `expires_at = now + expires_in`, computed client-side with a **saturating add** —
  a hostile/huge `expires_in` that wraps would make the token look already-expired
  and stall auth forever (`auth.rs:415-431`).
- Rotated refresh tokens are saved back into the session store on every refresh
  (`auth.rs:180-190`).
- `Debug` for the session type must redact both tokens (`auth.rs:499-530`).
- Emails must be redacted in logs (`a***@example.com`; anything without a usable
  `@` collapses to `<redacted>`) (`auth.rs:74-86`).

#### 4.6.1 The `invalid_grant` ambiguity — how it was disambiguated

**The problem.** GoTrue returns HTTP `400`/`422` with the OAuth error code
`invalid_grant` for *both* "bad email/password" (password grant) and "expired or
revoked refresh token" (refresh grant). The error body is therefore **not** a
reliable discriminator — and the two failures need completely different recovery
(prompt the user for credentials vs. silently re-authenticate).

**The fix.** Thread the **grant kind** through the request helper and decide on
that, never on the body:

```
post_json(url, body, grant):
    ...
    if status in {400, 422}:
        return match grant:
            Refresh  -> InvalidRefreshToken(message)
            Password -> InvalidCredentials(message)
    return GoTrue{status, message}
```

(`auth.rs:53-58` — the `GrantKind` enum exists solely for this; `auth.rs:344-383`.)
The doc comment is explicit: *"The grant kind is authoritative … we must NOT guess
from the body here."*

**Error-body decoding.** Read the raw text first, try the structured GoTrue
envelope, and if that yields nothing useful fall back to a **200-byte truncated
raw snippet** — so a 502 HTML gateway page is diagnosable instead of collapsing to
"unknown error" (`auth.rs:391-411`; tests at `:540-581`).

#### 4.6.2 Token refresh strategy

Two independent mechanisms, both needed:

**(a) Proactive background refresh** (`auth.rs:276-331`)

- Refresh `REFRESH_MARGIN_SECS = 60` before expiry.
- Sleep after a success = `max(expires_in - 60, MIN_REFRESH_INTERVAL_SECS = 5)`.
  The **floor is load-bearing**: without it a short-lived token
  (`expires_in <= margin`) yields a zero sleep and an unthrottled loop hammering
  GoTrue (`auth.rs:17-20, 64-68`).
- No session yet ⇒ idle-poll every 10 s (this is *not* a failure retry and must
  not be folded into the backoff).
- On refresh **failure**, exponential backoff 30 s → 300 s cap, reset on any
  success (`CopyPaste-vgpy` / `CopyPaste-8ebg.59`, `auth.rs:22-34, 262-275`).
- 30 s HTTP timeout on all auth calls; without it a stalled GoTrue endpoint blocks
  the refresh loop forever (`CopyPaste-8ebg.49`, `auth.rs:36-48`).

**(b) Reactive 401 recovery** (`cloud/auth.rs:101-133`)

Ordered fallback, and the order matters:

1. If a session is stored → try the **refresh-token grant** first. Cheap, and it
   avoids re-sending the password on every 401.
2. Refresh grant fails (no session / token expired / revoked) → fall back to a
   **full password sign-in**, so a long-lived daemon recovers after the refresh
   token itself ages out.
3. Nothing configured → the anon key (dead in practice under the RLS above).

Every path updates a shared `signed_in` flag — `true` on a fresh token from either
grant, `false` when re-auth fails — so status UI stops claiming the daemon is
signed in after auth dies ("BUG 2", `cloud/auth.rs:96-100`). A test pins the
priority explicitly: on 401 the refresh grant is used and the password endpoint is
asserted **never hit** (`cloud/auth.rs:316-436`).

#### 4.6.3 401 / 429 handling on data requests

Identical policy on **both** read and write paths (`cloud/push/transport.rs:112-199`,
`cloud/poll/transport.rs:87-140`):

| Status | Action |
|---|---|
| 2xx | success |
| **401** | refresh the shared bearer, retry **exactly once**. A single-shot guard is mandatory: a refresh that itself returns a still-401 token would otherwise spin forever. Second 401 ⇒ hard error. |
| **429** | honour `Retry-After` (**delta-seconds form only**; the HTTP-date form is deliberately unsupported — Supabase emits integer seconds and date parsing buys nothing), clamped to the max backoff; fall back to the current backoff when the header is absent. Also single-shot, so a server stuck on 429 cannot pin the loop. |
| 5xx / network | transient: exponential backoff 1 s → 30 s, hard cap of 4 attempts |
| other 4xx | permanent: give up on this item |

> The 401-on-**read** case was originally folded into the generic error bucket, so
> an expired token permanently stalled *downloads* while uploads kept working
> (`poll/transport.rs:37-40`). Both directions need the same treatment.
>
> Likewise 429-on-read was originally generic-failed, so the client ignored the
> server's guidance and waited a full poll interval instead
> (`poll/transport.rs:62-68`).

### 4.7 Realtime (Phoenix channel over WebSocket)

**Wire format.** Every frame is a 5-element JSON array:
`[join_ref, ref, topic, event, payload]`
(`crates/copypaste-supabase/src/protocol.rs:10-56`).

- Parse refs defensively: a **numeric** ref must map to *absent*, not to the empty
  string. Mapping it to `Some("")` meant a reply's ref never matched the
  heartbeat's ref and heartbeat replies were silently dropped
  (`CopyPaste-crh3.97`, `protocol.rs:45-55`).
- Reject frames that are not exactly 5 elements.

**Connection.** `wss://<project>/realtime/v1/websocket`; the publishable/anon key
goes in a **request header**, not the URL query string (`CopyPaste-lnjm`,
`realtime/session.rs:28-33`).

**Join payload** (`realtime/join.rs:39-51`):

```json
["1","1","realtime:clipboard_items","phx_join",{
  "config": {
    "access_token": "<user JWT>",
    "postgres_changes": [{
      "event":  "*",
      "schema": "public",
      "table":  "clipboard_items",
      "filter": "user_id=eq.<user uuid>"
    }]
  }
}]
```

Three non-negotiables:

1. **`access_token` = the current user JWT**, re-read at *every* reconnect. A JWT
   captured once at client construction goes stale and the channel silently
   re-joins with a dead token (`session.rs:54-57, 75`).
2. **`event: "*"`, never `"INSERT"`.** INSERT-only silently drops every
   cross-device UPDATE and DELETE — i.e. every pin, unpin, reorder, and tombstone
   (`join.rs:22-25`).
3. **The `user_id=eq.<uuid>` filter is mandatory** and a missing `user_id` is a
   **hard session error**, not a silently-omitted filter. Without it the Realtime
   server can place cross-user rows into the event stream before server-side RLS
   applies, leaking data on a permissive or misconfigured deployment. Defense in
   depth: the filter does not replace RLS, it backs it up
   (`CopyPaste-nr2y`, `join.rs:11-20`, `session.rs:65-74`).

**Heartbeat** (`session.rs:95-125`, `protocol.rs:69-78`):
`[null, "<ref>", "phoenix", "heartbeat", {}]` on a fixed interval (30 s default),
with a monotonically increasing ref counter starting at 2 (ref 1 is the join).

> **Bound the heartbeat write.** On a half-open socket `send` can stall
> indefinitely, silently starving heartbeats until the ~60 s server-side timeout
> kills the connection. Treat a write that does not complete within one heartbeat
> interval as a disconnect (`session.rs:163-183`).

**Event routing** (`realtime/dispatch.rs:81-139`):

| Event | Handling |
|---|---|
| `phx_reply` with `status == "ok"` | **join confirmed** — fire a one-permit notification. Downstream gates "realtime is live" on *this*, not on socket-open (see §4.8). Must use a store-a-permit primitive so a reply arriving before the waiter registers is not lost (`dispatch.rs:92-99`). |
| `phx_reply` non-ok | warn; do **not** signal joined |
| `phx_error` | log with a **redacted** payload |
| `phx_close` | server closed the channel |
| `postgres_changes` | parse and forward |
| anything else | trace only |

**Change payload parsing** (`protocol.rs:118-149`): the event lives under
`payload.data`; accept `record`/`new` and `old_record`/`old` spellings; accept a
lowercase `type`; default the table name.

**Log hygiene.** A raw frame can embed clipboard ciphertext and metadata. On a
parse failure log **length + a 16-byte hex prefix only**, never the frame
(`dispatch.rs:29-46`). Error payloads go through a redactor
(`dispatch.rs:105-111`). The WS URL is scrubbed before logging
(`reconnect.rs:70`).

**Reconnect** (`realtime/reconnect.rs:43-119`):

- Exponential backoff, initial → max, driven by one shared scheduler.
- A session that ran **at least as long as `max_backoff`** counts as *stable*:
  reset the schedule to the initial delay. Otherwise a healthy server that blips
  once an hour keeps an ever-growing backoff.
- A `ConnectError` (pre-join failure) always advances the backoff.
- Clear the `running` flag from an **RAII guard**, not at the bottom of the loop:
  a panic or abort mid-loop otherwise leaves the flag `true` forever and the
  handle lies about a dead worker (`reconnect.rs:19-36`).
- Graceful shutdown sends `phx_leave` then a Close frame (`session.rs:186-202`).

**TLS.** SPKI certificate pinning was supported for `wss://`, with a plain path
for loopback `ws://` in dev (`CopyPaste-qkao`, `session.rs:35-38`,
`realtime/realtime_tls.rs`). Carry or consciously drop — see §7.

### 4.8 Poll cadence and the realtime/poll relationship

Realtime is an **accelerator**, never the source of truth. HTTP polling is always
running as the backstop; only its *interval* changes.

| State | Interval | Source |
|---|---|---|
| WS channel join confirmed | 60 s (catch-up safety net) | `cloud/poll/mod.rs:44` |
| WS down / never connected | 10 s (sole download path) | `cloud/poll/mod.rs:49` |

> **v2 divergence, decided 2026-07-30, in the first row only.** v2's cadence is a
> ladder rather than two fixed intervals: it starts at 5 s, doubles while nothing
> happens, and snaps back to 5 s on any change — so it is *faster* than this table
> whenever anything is going on, and slower only after a long quiet spell. The
> ceiling with a confirmed channel is **300 s, not 60 s**, for battery on a phone.
>
> That is only defensible because of something v1 did not have: a reconnected
> channel emits `RealtimeEvent::Resubscribed`, which forces a round immediately.
> The window this ceiling bounds is therefore "an event the server never sent"
> (a row over Realtime's 1 MiB per-record limit, or one dropped by a full
> subscriber queue), not "everything that happened while the socket was down".
>
> The second row is **not** diverged from: with no confirmed channel the ceiling
> is 10 s, and that is also what a caller gets by default until it reports a
> join. The failure this closes is a real one — a driver whose caller never wired
> Realtime up would otherwise put two idle devices ten minutes apart end to end
> while its own comment claimed the push channel was carrying the latency.

- The fast/slow switch is gated on **channel-join confirmation** (`phx_reply ok`),
  **not** on socket-open. A socket that is open but whose channel never joined
  delivers nothing, and backing off on it would silently halve the sync rate
  (`dispatch.rs:76-80`).
- The slow interval was lowered 120 s → 60 s to halve the worst-case missed-event
  window at negligible HTTP cost (`poll/mod.rs:38-44`).
- Missed ticks use **skip**, not burst: if one poll round runs long, resume on the
  next aligned tick rather than firing the backlog back-to-back and hammering the
  backend right after recovery (`poll/loop_task.rs:60-65`).
- Changing the interval recreates the ticker and **consumes the immediate first
  tick**, so a period change does not cause a double poll (`loop_task.rs:115-121`).

### 4.9 Upload queue behaviour

(`cloud/push/loop_task.rs`.) Rules worth preserving:

- **Bounded in-memory retry queue**; failed items are re-enqueued with their
  **already-computed ciphertext** so a retry never re-encrypts (`:162-166`).
- **Drain the backlog before accepting new work**, so recovery is observable and
  old items are not starved by a steady stream of new captures (`:283-285`).
- **Startup backlog sweep** of everything not yet marked synced — otherwise only
  *future* captures ever reach the backend (`:168-197`). Mark rows synced on
  success so restarts don't re-upload (`:81-85`).
- **Re-sweep on the key-absent → key-present edge.** If the daemon starts before
  the user enters the sync passphrase, the startup sweep is a no-op; without an
  edge-triggered re-sweep the entire pre-passphrase history is stranded until each
  item is manually re-copied ("BUG C2", `:174-177, 224-244`).
- **Periodic drain of the broadcast channel during outages.** While the retry loop
  is busy, nothing reads the broadcast channel; pins, deletes, and new captures
  accumulate in the ring buffer and are **silently dropped** when it overflows.
  Add the receive as an extra arm in the backoff select **and** a periodic drain
  tick (`CopyPaste-1t38`, `:204-212, 320-403`).
- Park each received item in the queue **before** any network await, so a shutdown
  between dequeue and push leaves it visible rather than lost (`:219-222`).
- Prefer shutdown over receive in the select (`biased`) so a burst cannot starve
  teardown (`:409-419`).
- Per-request HTTP timeout on every client; do **not** fall back to a
  no-timeout client if the builder fails (`CopyPaste-16vr`, `:144-152`).

---

## 5. Relay feature-parity checklist

### 5.0 What the relay actually was

Read this before the table or the verdicts will look wrong.

The relay was **not** a per-device message broker. All of an account's devices
derived **one shared inbox id** from the sync key
(`derive_relay_inbox_id(sync_key)`) and **co-registered** it, each receiving an
*independent* bearer token. Every device pushed to and pulled from that single
inbox; a device's own writes echoed back and were absorbed by LWW
(`crates/copypaste-daemon/src/relay/mod.rs:8-35`;
`crates/copypaste-relay/tests/fanout_multi_device.rs:6-14`).

Structurally that is **the same shape as one Supabase table row-scoped by
`user_id`** — with two differences that matter:

- the relay inbox was an **append-only queue** (monotonic per-inbox `id`,
  duplicates allowed, no update); Supabase is a **table keyed on `item_id`**
  (upsert, one row per logical item). The Supabase shape is *better* for
  convergence — there is no second copy of an item to reconcile.
- the relay was **account-less and zero-knowledge**: it saw an opaque
  HKDF-derived inbox id and ciphertext, and nothing else. Supabase sees an email
  address, device ids, content types, sizes, timestamps and pin state.

### 5.1 The table

| # | Relay guarantee | Where it lived | Supabase | Verdict |
|---|---|---|---|---|
| 1 | **Shared-account fan-out** — any co-registered token can push; every token reads every item | `relay/mod.rs:8-35`, `tests/fanout_multi_device.rs` | One table + RLS `user_id = auth.uid()`; every signed-in device reads all rows | ✅ **Covered, and simpler.** Upsert-on-`item_id` removes the duplicate-row class entirely. |
| 2 | **Independent per-device credentials on one inbox (R1a)** — revoking one device does not affect the others | `state/registration.rs`, `tests/integration.rs:203-272` | Each device holds its own GoTrue session + refresh token; sign-out revokes one | ✅ **Covered.** |
| 3 | **Distinct per-device inboxes (legacy fan-out mode)** — sender must push to each inbox; relay never broadcasts across ids | `tests/fanout_multi_device.rs:12-14` | n/a | ⚪ **Drop.** The daemon never used it; it was a leftover of the pre-shared-inbox design. Nothing breaks. |
| 4 | **Per-inbox quota, 500 items** (config default; tier cap 1000; effective = `min(hard cap, tier)`) with **silent prune of the oldest** | `config.rs:27,99`, `state/inbox/push.rs:161-185`, `quota.rs:30-35` | ❌ nothing server-side | ⚠️ **Not covered — must be replaced.** See 5.2. |
| 4a | **Prune by server-assigned `id`, never by client `wall_time`** | `CopyPaste-1uqb`, `state/inbox/push.rs:161-170` | n/a | ⚠️ **Carry the *rule* into whatever replaces the quota.** The inbox was sorted by client-supplied `wall_time`, so an intra-account attacker could forge a low `wall_time` to sort their item near the front, escape eviction, and displace legitimate items. Any retention job must order on a **server-assigned** value (`created_at`), never on `wall_time`. |
| 5 | **24 h item TTL** — deliberately far shorter than the 30-day local history TTL; the relay is an *ephemeral transit buffer* (ADR-009) | `config.rs:15-23,97`, `state/eviction.rs:107-140`, `config.rs:340-357` | ❌ nothing deletes; `expires_at` column exists and is indexed but is never acted on | ⚠️ **Not covered — privacy regression.** See 5.2. |
| 6 | **Keyset cursor pagination** `?since=<wall>&since_id=<id>`, ordered, no ties | `state/inbox/pull.rs:34-49` | PostgREST `or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))` + `order=wall_time.asc,id.asc&limit=` | ✅ **Covered** (§4.4) — and both systems independently arrived at the same compound-keyset requirement, which is strong evidence it is load-bearing. Needs the `(user_id, wall_time, id)` index (§4.1). |
| 7 | **`Relay-Watermark: <wall>,<id>` response header** — lets a client interrupted mid-drain (e.g. by a 401-forced re-registration) recover the confirmed cursor instead of discarding progress | `CopyPaste-tspz`, `routes/items.rs:214-231, 283-302` | ❌ no equivalent header | ⚪ **Drop — equivalent client-side.** The value is just the last row of the page; the client computes it. On an *empty* page the relay echoed the request cursor back, which the client already holds. **Provided** the client persists its cursor after each page (it does — §4.4). No loss. |
| 8 | **`Relay-Has-More: true\|false` header** — disambiguates "short page = inbox exhausted" from "short page = byte-budget truncated mid-page" | `CopyPaste-8ebg.58`, `routes/items.rs:232-240`, `state/inbox/pull.rs:59-72` | ❌ no equivalent (PostgREST offers `Prefer: count=exact` → `Content-Range`, which is a different and more expensive thing) | ⚪ **Drop, but understand *why* it is safe.** The header existed only because the relay imposed a **byte budget** on a page (`MAX_PULL_BYTES_BUDGET`) to stop a caller forcing multi-GiB of cloning under a global mutex. PostgREST has no such budget, so `rows.len() < limit` is once again an unambiguous "caught up". **If the rewrite ever adds a size-based page cap, this ambiguity returns and the signal must come back.** |
| 9 | **At-least-once delivery with a polling backstop** — items persist until TTL/quota, no ack, clients replay from a cursor; SSE is additive only | `state/inbox/pull.rs`, `routes/items.rs:305-340` | Rows persist until deleted; poll is the same cursor replay | ✅ **Covered, strictly stronger** (no TTL to race). |
| 9a | **Push channel is at-most-once and must never be trusted alone** — relay SSE could miss events across a disconnect, so poll was mandatory | `routes/items.rs:305-340`; Android client keeps a catch-up poll (`RelaySubscriptionClient.kt:62`) | Supabase Realtime `postgres_changes` has the *same* property: no replay of events missed while disconnected | 🔴 **Covered only if you keep the poll loop.** This is the single most important item in this table. Realtime is an *accelerator*; the cursor poll is the correctness mechanism (§4.8). Deleting the poll loop "because we have Realtime now" reintroduces silent data loss on every reconnect. |
| 10 | **Proof-of-possession registration** — `pop = HMAC-SHA256(sync_key, prefix‖inbox_id)`, exactly 32 bytes; first registration stores it, co-registration must match under a **constant-time** compare; mismatch → generic `401` so there is no registration oracle | `CopyPaste-n2l` / `CopyPaste-crh3.89` / `CopyPaste-crh3.12`, `state/registration.rs:20-61`, `tests/pop_verification.rs` | GoTrue account auth + RLS on `user_id` | ⚠️ **Structurally replaced, but it is a different secret.** See 5.3. |
| 11 | **Per-(IP, device_id) registration rate limit**, keyed on the *tuple* so the limiter cannot leak "this device id is known" across IPs | `CopyPaste-…HIGH#5`, `state/registration.rs:66-120` | Supabase platform auth rate limits | 🟡 **Roughly covered** by GoTrue's own sign-in throttling; no per-item-id analogue is needed because there is no id-guessing attack surface left. |
| 12 | **Per-account device cap** (5 free / 10 pro) | `quota.rs:19-25` | ❌ none | ⚪ **Drop** unless it is a product requirement. It was a billing lever, not a correctness one. |
| 13 | **Per-item size cap** — 10 MiB image/file, 8 MiB text, `413` on exceed | `quota.rs:37-56`, `tests/integration.rs:501-576` | Supabase has its own body limits, but they differ and the error shape differs | 🟡 **Move client-side.** Enforce before upload so the user gets a clear local error instead of an opaque backend rejection. Keep the split limits (they exist so a text item that stores locally is not rejected on upload). |
| 14 | **Per-IP / per-device HTTP rate limiting with `Retry-After`** | `routes/mod.rs:29-42` | Supabase enforces its own; client must honour `429` + `Retry-After` | ✅ **Covered server-side; client obligation already specified** (§4.6.3). |
| 15 | **Inactive-device reaping** — device records with an empty inbox and stale `last_seen` are removed; reap on `last_seen`, **never** on `registered_at` (an actively-polling device with an empty inbox would otherwise be locked out) | `state/eviction.rs:16-63` | n/a — no device registry | ⚪ **Drop.** The rule is only recorded here because it is a good example of "liveness must be measured by activity, not by creation time" if any device registry reappears. |
| 16 | **Zero-knowledge, account-less server** — the relay knew an opaque HKDF inbox id and ciphertext. No email, no account | `relay/mod.rs:61-66` | Supabase knows: account email, `device_id`, `content_type`, payload sizes, `wall_time`, `created_at`, `pinned`, `deleted` | 🔴 **Genuine metadata-privacy regression.** Content stays E2E encrypted; the *metadata* surface grows substantially, and sync now **requires an account**. Must be stated plainly in the privacy docs — see 5.4. |
| 17 | Operational scaffolding: SQLite write-through + deferred write retry queue, supervised background tasks, mutex-poison survival, Prometheus metrics, governor cleanup, TLS/proxy-header handling, connection caps | `db.rs`, `retry.rs`, `supervise.rs`, `governor_cleanup.rs`, `api/metrics.rs`, `config.rs` | Supabase's problem | ✅ **Drop entirely — this is the payoff for dropping the relay.** Roughly 5 000 lines of server code and its test suite disappear. |

Legend: ✅ covered · 🟡 partially covered / needs a client-side move · ⚪ consciously dropped, nothing breaks · ⚠️ not covered, needs a deliberate replacement · 🔴 not covered, and something real is lost.

### 5.2 The two gaps that need an actual decision: quota and TTL

These are the only two rows where "Supabase covers it" is **false** and the
consequence is not merely cosmetic.

The relay held **≤ 500 items per account for ≤ 24 hours**. That was not a
limitation — it was the design (`config.rs:15-23`, and a test that *asserts* the
relay TTL is shorter than the local history TTL so nobody "fixes" it upward:
`config.rs:340-357`). Two properties fell out of it:

1. **Bounded server storage**, hence bounded cost and bounded blast radius.
2. **The server forgets.** A ciphertext exfiltrated from the relay is at most a
   day old, and there is no long-term corpus to attack offline later.

Supabase, as configured today, keeps every row forever.

What actually breaks:

- **Correctness: nothing.** Retention is not a convergence property. Local
  history is capped independently by `prune_to_cap` against
  `storage_quota_bytes`, and — crucially — **local eviction does not move the
  download watermark** (INV-N5), so a device that prunes locally does not
  re-download (`cloud/poll/ingest.rs:475-491`).
- **Cost and performance:** unbounded row growth on the `user_id` hot path.
- **Privacy:** the "server forgets within a day" property is gone.

Options, in preference order:

1. **`pg_cron` retention job** — `delete from clipboard_items where created_at <
   now() - interval '…'`. Order on `created_at` (server-assigned), **never** on
   `wall_time` (client-supplied — rule 4a). Restores both properties. Also
   finally makes the existing `expires_at` column + its partial index mean
   something.
2. **Per-account row cap** enforced by the same job (keep newest N by
   `created_at`).
3. **Accept unbounded retention** — then say so explicitly in the privacy
   documentation and remove the "ephemeral" language.

Do **not** implement retention client-side by issuing DELETEs: a device that has
been offline for a month would delete rows another device still needs.

### 5.3 What proof-of-possession actually bought, and what replaces it

The PoP existed to close one specific attack: the inbox id is *derived from the
sync key*, so it is secret — but it is also transmitted on every request, so it
could leak (logs, a proxy, a compromised device). Someone who learned **only the
inbox id** could otherwise co-register against it and siphon the account's
ciphertext. Requiring a correct `HMAC-SHA256(sync_key, …)` means the attacker
must hold the sync key itself. The relay could never *verify* a first
registration (it does not know the sync key), so first-use stores the PoP and
every later co-registration is checked against it, in constant time, with a
generic `401` on mismatch (`state/registration.rs:20-61`).

Under Supabase the equivalent gate is: **you must be able to sign in to the
account.** That is a strictly better *access-control* story — a password plus a
server-side check beats a shared-secret HMAC.

But note precisely what changed: **the secret being proven is different.**

| | Relay | Supabase |
|---|---|---|
| Secret that gates access to the ciphertext | the **sync key** (passphrase-derived) | the **account password** |
| Secret that decrypts the ciphertext | the sync key | the sync key |
| Are they the same secret? | yes | **no** |

Consequence to record in the threat model: an attacker who compromises the
Supabase account but **not** the sync passphrase can now (a) read all ciphertext
and all metadata, and (b) **write rows into the account**. Under the relay they
could not reach the inbox at all. E2E confidentiality of *content* is unchanged —
they cannot decrypt — but the write capability is new: they can inject rows that
every device will fetch, attempt to decrypt, and (correctly) discard per INV-N3.
An injected row with a huge `lamport_ts` could also *outrank* legitimate versions
of an existing `item_id` and effectively censor it, since the merge cannot
distinguish a forged version from a real one.

> **Mitigation worth designing in:** bind the ciphertext's AAD to `item_id` (this
> already happens) **and** consider signing the LWW metadata under the sync key so
> a row whose metadata was not produced by a sync-key holder can be rejected
> before it participates in the merge. This is the one security capability the
> relay had that Supabase does not, and it is cheap to add client-side.

### 5.4 Summary verdict

**Dropping the relay is safe for correctness.** Every convergence,
idempotency, and no-data-loss guarantee in §2 either carries over unchanged or is
strengthened by Supabase's upsert-on-`item_id` model.

Three things must be handled deliberately, not by omission:

1. **Keep the polling backstop** (row 9a). Realtime does not replay missed
   events. This is the only item that can silently reintroduce data loss.
2. **Add a retention job** (row 4/5) or explicitly accept and document
   unbounded server-side retention.
3. **Record the threat-model change** (row 10/16): account password now gates
   ciphertext access, metadata visibility increases, sync requires an account,
   and metadata forgery becomes possible without the sync key.

Everything else is either covered, cheaply replicated client-side, or was never
used.

---

## 6. Acceptance tests to re-create

Grouped by the invariant they defend. These are behaviours, not ports — the old
test names are given so the original can be consulted.

### 6.1 Concurrent-edit convergence

| Test | Assertion | Old |
|---|---|---|
| **AT-1 two replicas converge** | Two replicas apply the same set of versions in *different orders* and end byte-identical. | `copypaste-sync/tests/crdt.rs:119 merge_convergent_two_replicas_yield_identical_state` |
| **AT-2 commutativity** | Two independent operations applied in either order yield the same state. | `crdt.rs:197 commutative_two_ops_independent_of_order` |
| **AT-3 higher lamport wins** | Simultaneous edits to one `item_id`: higher `lamport_ts` wins even with a *lower* `wall_time`. | `conflicts.rs:115`, `merge.rs:329` |
| **AT-4 wall-time tie-break** | Equal `lamport_ts` → higher `wall_time` wins. | `conflicts.rs:149`, `merge.rs:345` |
| **AT-5 device-id tie-break + drift guard** | Equal `lamport_ts` *and* `wall_time` → lexicographically larger `origin_device_id` wins, **and both peers pick the same winner**. Must assert the *symmetric* case (local device id larger ⇒ local wins). | `conflicts.rs:186`, `crdt.rs:312`, `crdt.rs:348`, `merge.rs:361` |
| **AT-6 exact tie keeps local** | All three keys equal ⇒ no change (this is what makes re-delivery free). | `merge.rs:373` |
| **AT-7 no oscillation** | Three-way merge across three replicas reaches a fixed point; no ping-pong. | `conflicts.rs:236 three_way_merge_no_oscillation` |
| **AT-8 causality preserved** | Lamport ordering survives a merge round. | `crdt.rs:219` |
| **AT-9 comparator equivalence (property)** | The object-shaped and scalar-shaped comparators agree over the **entire** decision space (all combinations of lower/equal/higher on each of the three keys). This is the guard against `CopyPaste-ayvs` recurring. | `merge.rs:650 remote_wins_matches_resolve_across_decision_space` |
| **AT-10 equal-lamport tie is actually broken** | Explicitly assert the case the old cloud/relay comparator got wrong: equal lamport, equal wall, remote device id larger ⇒ **remote wins**. | `merge.rs:693` |
| **AT-11 unified lamport space** | A pin/delete stamped after a re-copy strictly outranks it. Encodes `CopyPaste-ojhe`. | `merge.rs:728 unified_pin_delete_beats_older_recopy` |

### 6.2 Replay / idempotency safety

| Test | Assertion | Old |
|---|---|---|
| **AT-12 apply twice = apply once** | Re-applying an identical version changes nothing. | `crdt.rs:173 idempotent_apply_same_event_twice_no_change` |
| **AT-13 self-echo** | A device pushes an item, then receives it back from the backend; exactly one local row, unchanged. | relay `mod.rs:32-35` (documented; assert it directly) |
| **AT-14 two transports, one row** | The same `item_id` delivered over Realtime **and** the poll path in the same window ⇒ exactly one row. | `both_transports_deliver_same_item_inserts_exactly_once` (`relay/mod.rs:44-51`) |
| **AT-15 within-session replay guard** | Same `(item_id, lamport_ts)` twice ⇒ second dropped. Same `item_id` with a **higher** `lamport_ts` ⇒ **admitted** (it is a CRDT update, not a replay). Guard is bounded and evicts oldest-first. | `copypaste-sync/src/inbox/replay_guard.rs:116-187` |
| **AT-16 upsert does not resurrect** | Upsert a live row over a tombstoned `item_id` with a *lower* lamport ⇒ still deleted. Then assert the payload always carries `deleted` explicitly. | `rest/write.rs:123-148` (`CopyPaste-kgs7`) |
| **AT-17 hostile timestamps** | `lamport_ts: -42`, `wall_time: -999` ⇒ both clamped to 0 **at deserialization**, including when nested inside a batch message. Positive values pass through untouched. | `protocol.rs:618, 643, 668` (`CopyPaste-psx7`) |

### 6.3 Delete semantics

| Test | Assertion | Old |
|---|---|---|
| **AT-18 tombstone beats older live** | Higher-lamport tombstone replaces a live local item and wipes content. | `merge.rs:581` |
| **AT-19 live cannot beat newer tombstone** | Lower-lamport live version with a *higher* wall time does **not** resurrect. | `merge.rs:593` |
| **AT-20 delete-before-create** | Delete for an unknown `item_id` ⇒ a tombstone row is persisted; a subsequently-arriving create for that `item_id` loses LWW and the item stays deleted. Must be asserted **per transport**. | `sync_orch/merge/tests.rs:87`, `relay/receive/ingest.rs:498-541` (`CopyPaste-bfiu`) |
| **AT-21 delete vs concurrent update** | Deterministic resolution, same on both replicas. | `conflicts.rs:279` |
| **AT-22 tombstone leaks no ciphertext** | A tombstone with a payload set is rejected before it can be sent. | `rest/write.rs:217` |

### 6.4 Offline → online catch-up (pagination / no-loss)

| Test | Assertion | Old |
|---|---|---|
| **AT-23 forward pagination, > limit rows** | More rows exist than one page: **all** are eventually fetched, in order, with no gaps. | `cloud/tests.rs:1096 poll_forward_pagination_does_not_skip_when_more_than_limit_arrive` |
| **AT-24 same-millisecond burst** | ≥ `limit` rows sharing **one** `wall_time`: all are fetched via the compound keyset. **This is the regression test for the worst silent-data-loss bug in the codebase.** | `cloud/tests.rs:1248 poll_fetches_all_rows_sharing_one_wall_time_via_keyset_cursor` |
| **AT-25 watermark advances, no refetch** | Already-ingested rows are not re-requested on the next tick. | `cloud/tests.rs:951` |
| **AT-26 watermark survives restart** | Persist → reload ⇒ resume from the cursor, not from zero. Missing file and malformed file both degrade to zero without panicking. | `relay/watermark.rs:96, 126, 143`; `cloud/tests.rs:819` |
| **AT-27 watermark seeding** | Startup watermark = `max(persisted, local MAX(wall_time))`. | `cloud/tests.rs:819 load_poll_watermark_takes_max_of_persisted_and_local` |
| **AT-28 cursor advances past unusable rows** | An undecryptable / duplicate / LWW-loser row still advances the cursor, so it is never re-requested (INV-I4). | `cloud/poll/ingest.rs:458-466` |
| **AT-29 no-advance stall guard** | A *full* page in which no row yields cursor progress breaks the burst-drain loop instead of spinning. | `poll/loop_task.rs:236-247` |
| **AT-30 LWW replace preserves the local PK** | A remote win replaces in place; the local row PK, FTS entry and pin state still resolve. | `cloud/tests.rs:1348 poll_lww_replaces_existing_item_id_preserving_local_pk`; `sync_orch/merge/tests.rs:140` |
| **AT-31 backfill respects the local cap** | A long-offline device that pulls thousands of rows converges to `storage_quota_bytes` — **and the watermark does not move backwards** as a result (INV-N5). | `cloud/poll/ingest.rs:475-505` |
| **AT-32 pre-passphrase history uploads** | Start with no sync passphrase, capture items, then set the passphrase ⇒ the backlog sweep runs on that edge and the history uploads. | "BUG C2", `cloud/push/loop_task.rs:224-244` |
| **AT-33 mutations survive an outage** | Backend down; pin / delete / capture events are emitted; they are not lost to broadcast-ring overflow and are all delivered once the backend returns. | `CopyPaste-1t38`, `push/loop_task.rs:320-403` |

### 6.5 Backend protocol conformance

| Test | Assertion | Old |
|---|---|---|
| **AT-34 401 → refresh → retry once** | First request 401s, refresh succeeds, retry succeeds, and the **refreshed** token is installed in the shared bearer. Applies to **both** read and write paths. | `cloud/auth.rs:324`, `cloud/tests.rs:393` |
| **AT-35 401 prefers the refresh grant** | With a stored session, a 401 uses `grant_type=refresh_token` and the password endpoint is asserted **never hit**. | `cloud/auth.rs:316-436` |
| **AT-36 401 twice ⇒ hard error** | A refresh that yields a still-401 token does not loop. | `push/transport.rs:160-162` |
| **AT-37 `invalid_grant` disambiguation** | A `400`+`invalid_grant` on the **password** grant surfaces "bad credentials"; the identical body on the **refresh** grant surfaces "bad refresh token". | `auth_integration.rs:sign_in_invalid_credentials…` / `refresh_session_invalid_token…`; `auth.rs:368-377` |
| **AT-38 non-JSON error body preserved** | A 502 HTML page yields a diagnosable truncated snippet, not "unknown error". | `auth.rs:542` |
| **AT-39 429 honours `Retry-After`** | Integer-seconds `Retry-After` is slept, clamped to max backoff; a missing header falls back to backoff; single-shot so a permanent 429 cannot pin the loop. Both read and write paths. | `cloud/tests.rs:460`, `poll/transport.rs:121-136` |
| **AT-40 auth fails closed** | Configured credentials that fail ⇒ sync aborts; the anon key is **never** substituted and never appears in the error. | `cloud/auth.rs:161-218` |
| **AT-41 signed-in flag tracks reality** | Successful bearer resolution ⇒ `true`; failed refresh ⇒ `false`. | `cloud/auth.rs:266, 297` |
| **AT-42 token redaction** | Session `Debug` shows neither access nor refresh token; emails are masked; `expires_at` remains visible. | `auth.rs:489, 500`; `session_store.rs` |
| **AT-43 `expires_at` saturates** | A hostile `expires_in` near the integer max does not wrap into "already expired". | `auth.rs:465` |
| **AT-44 refresh sleep is floored** | `expires_in <= margin` never yields a zero sleep. | `auth.rs:439` |
| **AT-45 bytea round-trip** | An item pushed and then polled back decrypts to the original plaintext — this is the end-to-end guard for the `\x<hex>` encoding bug. | `cloud/bytea_e2e.rs`, `cloud/tests.rs:869` |
| **AT-46 Phoenix wire format** | 5-element array; numeric refs ⇒ absent (not empty string); wrong arity rejected; heartbeat and join round-trip. | `protocol.rs:159-236` |
| **AT-47 join payload** | Contains the JWT at `/config/access_token`, registers `event:"*"` (asserting `"INSERT"` is **absent**), and always carries `filter: user_id=eq.<uuid>`. A missing `user_id` is a hard session error. | `join.rs:59-120`, `session.rs:65-74` |
| **AT-48 join-confirmed gating** | `phx_reply` with `status:"ok"` fires the joined signal; a non-ok reply does **not**. The poll interval only slows after the signal. | `dispatch.rs:225, 265` |
| **AT-49 reconnect backoff** | Exponential doubling, capped; reset after a session that ran ≥ max backoff; connect errors always advance. | `reconnect.rs:152, 179`; `reconnect_backoff.rs` |
| **AT-50 payload never logged** | A parse failure logs length + a 16-byte prefix only; error payloads are redacted; the WS URL is scrubbed. | `dispatch.rs:29-46`, `cloud/tests.rs:716` |
| **AT-51 RLS policy audit (static)** | Assert against the SQL text itself: RLS enabled **and** forced; every policy scopes on `user_id = auth.uid()`; `anon` privileges revoked; `user_id` default is `auth.uid()`. Cheap, and it caught real drift. | `copypaste-supabase/tests/rls_policies.rs` |
| **AT-52 upsert conflict target resolves** | *New.* Assert the unique index on `(user_id, item_id)` exists — otherwise `on_conflict` fails at runtime (§4.2). | — (gap) |

### 6.6 Crypto-adjacent tests that guard sync correctness

| Test | Assertion | Old |
|---|---|---|
| **AT-53 key_version round-trip** | Encrypt under version *v*, send, receive, and decrypt through the **production read path**. Guards against the "every synced item is undecryptable" regression. | `merge.rs:471 wire_round_trip_preserves_key_version_so_receiver_can_decrypt` |
| **AT-54 undecryptable item is skipped, not stored** | A sync-key-wrapped item that cannot be decrypted is dropped, not persisted as a poison row; the sender re-sends after the key arrives. | `CopyPaste-jww`/`5y4`, `sync_orch/merge/mod.rs:286-301` |
| **AT-55 sensitive re-detection on receive** | An inbound item whose plaintext looks sensitive gets `is_sensitive` set locally, so auto-wipe TTL applies. | `CopyPaste-kcf`, `sync_orch/merge/tests.rs:192` |
| **AT-56 sensitive items never leave the device** | Neither the catch-up read nor the backlog sweep enqueues a sensitive item. | `CopyPaste-20yw`, `catchup.rs:141`, `cloud/backlog.rs:302` |

---

## 7. Known-unjustified complexity we should NOT port

Ordered roughly by how much code disappears.

### 7.1 The entire relay server

~5 000 lines of server plus a large integration suite: SQLite write-through, a
deferred-write retry queue (`CopyPaste-k4py` / `crh3.70`), supervised background
tasks (`CopyPaste-bp3o`), mutex-poison recovery on every handler, governor
cleanup tasks, Prometheus metrics, proxy-header trust configuration
(`CopyPaste-hzmb`), SSE connection caps (`CopyPaste-h7i8`), page byte budgets,
tier quotas, device reaping. All of it exists to make a bespoke server
production-safe. **Deleting it is the point of the rewrite.** §5 records the four
behaviours worth carrying forward; everything else goes.

### 7.2 Two comparators for one total order

`resolve` (object-shaped) and `remote_wins` (scalar-shaped) implement the same
order twice, kept honest by a property test. That property test only exists
because the duplication does. **One comparator.** If two shapes are genuinely
needed, define one in terms of the other.

### 7.3 The `LamportClock` type

`tick` / `observe` / saturation handling / a once-per-process warning, plus ~15
unit tests — and its own doc comment says it is **not on the production path**
(`copypaste-sync/src/clock.rs:1-7`). The daemon stamps via `next_lamport_ts` and
resolves via the comparator. It survived "for the session protocol + its tests".
**Do not port it without a live consumer.** Related: the P2P handshake carries a
`clock` field that nothing reads (`protocol.rs:352-358`).

### 7.4 The anonymous-key code path

Bearer resolution has a whole branch for "no email/password ⇒ use the anon key",
and its own doc comment admits the RLS policies grant only `authenticated`, so
those requests are **rejected outright** (`cloud/auth.rs:26-30, 55-59`). Dead
branch, dead error-handling, dead tests. Sync requires an account: make that a
type-level fact, not a runtime fallback.

### 7.5 Two divergent write paths

The daemon POSTs without `on_conflict` (`cloud/push/transport.rs:49-58`); the
library `RestClient` upserts with `on_conflict=item_id`
(`rest/write.rs:49-58`). They disagree, and the daemon's version is the broken
one (§4.5). **One upsert, one code path.**

### 7.6 Dead columns and dead wire fields

- **`content_hash`** — schema comment says the server cannot verify it; the merge
  path explicitly sets it to `None` on every received item
  (`merge.rs:130`). Carries an index. Either use it for dedup or drop it.
- **`content_nonce` as a separate cloud column** — the cloud payload is a
  self-framing `nonce‖ciphertext` blob, so the column is not part of the cloud
  round-trip.
- **`blob_ref` on the wire** — the outbound re-key path *clears* it, which is
  precisely why `file_name`/`mime` had to be lifted onto the wire separately
  (`merge.rs:171-179`). The design is fighting itself; re-think file metadata as
  a first-class wire field rather than a JSON blob that gets stripped.

### 7.7 Legacy-compatibility paths with no legacy to be compatible with

A from-scratch rewrite has no old peers.

- Version-gated decode of **two** relay wire formats, V1 `base64(JSON{…})` and V2
  `base64(0x01‖len‖meta‖ct)` (`CopyPaste-crh3.69`, `relay/receive/ingest.rs:59-63`).
- The `wall_time`-only cursor fallback when `since_id` is absent
  (`state/inbox/pull.rs:50-56`; `cursor.rs:66-73`). Always use the compound keyset.
- `serde(default)` back-compat on ~8 wire fields for "peers on an older build".
- The "cloud row pre-dates the pin columns" fallback branch
  (`cloud/poll/ingest.rs:316-326`).
- `key_version` defaulting for peers that predate the field (`protocol.rs:198`) —
  keep the *field*, drop the *default*.

### 7.8 Feature-gated code with no production caller

`Tier::Pro` is `#[cfg(any(test, feature = "quota-tiers"))]` and its own comment
says live registration always stores `Tier::Free` (`quota.rs:9-15`).
`check_history_quota` has no production caller and is `#[allow(dead_code)]`
(`quota.rs:106-120`). `push_item` (the self-decoding wrapper) is a test-only
helper living in production code (`state/inbox/push.rs:17-24`). Three
`#[allow(dead_code)]` annotations that each say "intentional: no production
caller". **If it has no caller, it has no reason to exist.**

### 7.9 Outright dead code in the merge path

The auto-apply block computes `local_latest_wt` with the query
`SELECT COALESCE(MAX(wall_time),0) FROM clipboard_items WHERE origin_device_id =
'' OR 1=1` — a tautological `WHERE` — then immediately discards it
(`let _ = local_latest_wt;`) and uses a second, near-identical query
(`sync_orch/merge/mod.rs:449-468`). Two queries, one used, one a no-op with a
nonsense predicate.

### 7.10 Feature bleed through the merge signature

`merge_incoming_with_crypto` threads an `AutoApplyCtx` of three `Arc`s
(a change-count sentinel, a key, a config handle) purely so the merge can write
the winning item to the system clipboard — which forces a type alias just to
dodge a clippy lint (`sync_orch/merge/mod.rs:93-106`) and puts platform
pasteboard concerns inside the CRDT merge. **Merge should return what changed;
let a subscriber decide whether to touch the clipboard.**

### 7.11 The `#[allow(clippy::too_many_arguments)]` epidemic

`poll_once` takes 13 parameters, `realtime_loop` 13, `push_loop` 12,
`start_relay` 12, `attempt_push_and_record` 14. Every one is annotated with a
suppression and a comment explaining that the parameters are "independent
runtime slices". They are not — they are a context object that was never
created. In a library-first rewrite this is the difference between a usable API
and an unusable one.

### 7.12 Micro-optimisations that outgrew their benefit

- `local_to_wire` **and** `local_to_wire_owned` — a borrowing and a by-value
  variant of the same conversion, with a test asserting they agree
  (`CopyPaste-ux2i`, `merge.rs:207-250, 424-449`). Real motivation (avoiding a
  multi-MB blob copy), but the right fix is one by-value API.
- `decoded_len_padded_b64` — an O(1) base64 length calculator so the quota check
  avoids allocating, in a path that then decodes anyway
  (`CopyPaste-crh3.72`, `routes/items.rs:92-118`). Dies with the relay.
- The burst-drain shutdown probe implemented as
  `timeout(Duration::from_millis(0), shutdown.notified())`
  (`poll/loop_task.rs:249-256`) — a zero-duration timeout used as a non-blocking
  poll. Use a proper try-recv primitive.

### 7.13 Decide deliberately: SPKI pinning for Supabase Realtime

Certificate pinning (`CopyPaste-qkao`, `realtime/realtime_tls.rs`, ~433 lines
plus a hand-rolled DER reader) made sense against a self-hosted relay whose
certificate you control. Pinning a **third-party managed** endpoint means a
hard outage every time the provider rotates its chain, with no way to ship a fix
to already-installed clients in time. Either drop it, or pair it with a
remotely-updatable pin set and a documented fail-open policy. Do not port it by
inertia.

### 7.14 One thing that looks like complexity but is not — keep it

The three-phase merge (lock → **unlock** for CPU-heavy decrypt/decode → relock
for writes, `sync_orch/merge/mod.rs:118-131, 240-252, 353-357`) reads as
over-engineering and is not. Holding the database lock across an image
decode+re-encode stalled every other writer. Keep the shape; it will be much
cleaner without the `spawn_blocking` + `blocking_lock` gymnastics that the old
async/sync boundary forced.
