# Port Manifest 05 — Sync and Backend

This manifest specifies the current v2 convergence and backend contract. P2P
and cloud sync transport the same logical item versions and use the same
content comparator. The cloud backend stores ciphertext and signed ordering
metadata; it never receives plaintext or a derivable content key.

There is one P2P protocol, one Supabase row shape and one current merge model.
No relay framing, endpoint alias, version-selected decoder or transport repair
path belongs to the product.

## 1. Responsibilities

The sync layer owns:

- stable cross-device identity and deterministic conflict resolution;
- tombstone, pin and replay semantics;
- sensitive-content and size gates before egress;
- P2P and cloud transport adaptation;
- cloud authentication, signed row metadata, REST pagination and Realtime;
- independent upload and download cursors;
- bounded retries, adaptive idle cadence and user-visible refusal counts.

Storage owns local row transactions and the current schema. Crypto owns keys,
envelopes, signatures and authentication failures. Pairing owns peer trust. The
UI owns presentation of sync state. Those layers consume sync outcomes without
reimplementing the comparator or cursor rules.

## 2. Identity and one total order

### 2.1 Identifier spaces

| Value | Scope | Rule |
|---|---|---|
| `item_id` | cross-device | Stable identity of one logical item, AEAD AAD input and backend conflict key. |
| local row id | one database | Local references, FTS and pins only; never compared across devices. |
| `origin_device_id` | one version | Device that authored the version; preserved unchanged across hops. |

Incoming replacement preserves the local row id. Replacing it with a remote
database id would orphan the local FTS and pin references.

A local capture may have no origin attached while it remains purely local. The
first egress stamps this device's stable id; every forwarder then preserves that
origin. No receiver guesses an origin from a row id, device name or transport.

### 2.2 Content comparator

For two versions of the same `item_id`, larger wins in this exact order:

1. `created_at` — non-negative Unix milliseconds;
2. `content_hash` — deterministic digest representation;
3. `deleted` — a tombstone sorts above a live version;
4. `origin_device_id` — lexicographic final tie-break.

Equality on all four keeps the local row. One exported function owns this
decision and every P2P/cloud apply path calls it. Planning, fetching and applying
must not carry independent tuple comparisons.

The position of `deleted` is load-bearing. A tombstone preserves the hash of the
item it deletes, so it ties that live version on the first two keys. Consulting
origin first could resurrect a deletion merely because one device id sorts
higher.

This last-write-wins model is intentionally metadata-based. Clipboard entries
are snapshots and ciphertext is opaque, so a structural CRDT cannot merge
inside a value.

### 2.3 Pin order

Pin state has a separate P2P total order because cloud rows do not carry pin
fields. Compare `pin_updated_at`, `origin_device_id`, `pinned`, then total-order
`pin_order`. A content update does not acquire a fresh timestamp merely to move
a pin, and a pin update does not restamp content.

## 3. Convergence and data safety

- **INV-C1:** The comparator is strict, total, deterministic and symmetric.
- **INV-C2:** Every transport delegates to the same comparator.
- **INV-C3:** Applying the same set of versions in any order yields the same
  winner.
- **INV-C4:** Convergence is keyed by `item_id`, never local row id.
- **INV-I1:** Re-delivering an applied version is a no-op.
- **INV-I2:** A device's self-echo is absorbed by equality, not a sender filter.
- **INV-I3:** Simultaneous delivery over P2P and cloud still creates one row.
- **INV-I4:** A readable backend row advances the download cursor even when it
  loses, duplicates or cannot decrypt; otherwise it blocks every row after it.
- **INV-N1:** Pagination uses a compound keyset with no ties.
- **INV-N2:** Tombstones persist and win. A tombstone for an unknown item is
  stored so a delayed create cannot resurrect it.
- **INV-N3:** An undecryptable or wrongly signed row is skipped and counted. It
  never replaces, deletes or creates a partial local row.
- **INV-N4:** Applying a winner and maintaining FTS are one atomic store
  operation.
- **INV-N5:** Local retention never moves a sync cursor backwards.
- **INV-N6:** Authentication failure never downgrades to anonymous access.

Versions stamped more than 24 hours ahead of the local clock are refused by
both P2P and cloud. Refusal skips one version, not the complete round. The two
transport constants have a parity test. Negative timestamps are clamped before
sorting so normalization cannot reorder a page after cursor decisions.

Sensitive live items are neither advertised nor served by P2P and are rejected
again at the cloud egress boundary. A payload-less sensitive tombstone may sync
so deletion still converges. Remote content is run through the local detector;
the sender cannot declare a value safe or inject search text.

An over-limit live value is withheld and reported, never deleted locally. The
content-type owner selects the text or binary upload bound.

## 4. Cloud row contract

### 4.1 Table and trust boundaries

`public.clipboard_items` is keyed per account by `(user_id, item_id)`. Client
rows contain the current ciphertext/nonce, content type, bounded payload
metadata, bounded source-app metadata, version timestamp, tombstone flag,
origin device id and metadata signature. Server-owned insertion/update times
are separate from the client version stamp.

Required database properties:

- forced row-level security scopes every operation to `auth.uid()`;
- `anon` and `PUBLIC` have no row or column privileges;
- authenticated clients cannot write server identity, account ownership or
  retention timestamps;
- `(user_id, item_id)` is the upsert conflict target;
- `(user_id, created_at, item_id)` serves the ascending pull keyset;
- client payload and metadata fields have explicit storage bounds;
- Realtime publishes INSERT and UPDATE, not DELETE;
- retention uses server-assigned `inserted_at`, never forgeable `created_at`;
- a least-privilege retention owner enforces the configured TTL and per-account
  row cap.

Cloud rows are signed over every value that affects content interpretation or
ordering. A holder of account credentials without the sync passphrase may be
able to submit a row, but cannot forge a competing version or tombstone that
passes signature verification.

### 4.2 Tombstones and payloads

A live row carries authenticated ciphertext and nonce. A tombstone carries no
ciphertext, nonce-dependent content or source/payload metadata. `deleted` is
always sent explicitly on upsert; omission must not let a column default
resurrect a tombstone.

File rows require validated bounded `FileMetadata`. Non-file rows reject file
metadata. No transport converts an invalid file row to text.

### 4.3 Retention

Backend retention is an ephemeral transit bound, not a history owner. TTL and
quota eviction order by server-assigned insertion time. A client cannot restamp
that value on update to avoid eviction. Local history retention remains
independent and cannot infer that absence from cloud means deletion.

## 5. Cloud REST and cursors

### 5.1 Download keyset

Rows are read oldest first by `(created_at ASC, item_id ASC)`.

- With both cursor halves, request rows strictly after `(timestamp, item_id)`.
- With only a timestamp, include the boundary millisecond and let idempotent
  apply absorb rows seen again.
- Persist both halves after each completed page.
- A full page drains immediately, subject to the bounded pages-per-round cap.
- A full page that produces no cursor progress stops the drain rather than
  spinning.
- The cursor never advances past a refused far-future row. It may advance past
  a wrongly signed or undecryptable row at or behind the local clock.

A timestamp-only strict bound can lose rows that share its millisecond. A
newest-first page cannot be drained by a forward cursor. Neither query shape is
permitted.

The backend index and client query order are one contract. Changing either
requires a database plan test and a same-timestamp page test.

### 5.2 Upload floor

Upload progress is independent from the download watermark. The upload floor
is a compound `(created_at, item_id)` position over local versions and advances
only after a complete successful round. It advances to the instant the round
began, never to a clock read after the scan, so a concurrent capture is not
skipped.

Any operation that writes a version below the floor schedules that position for
another scan. This includes P2P apply, delete, import and a cloud row that loses
to a local winner. Signing in resets the floor to the beginning so existing
eligible history is offered.

An unreadable local payload cannot stall every later upload and cannot be
silently forgotten. The source advances the main floor, persists a bounded set
of unreadable ids, revisits those ids before later scans and uses a durable
keyset walk for overflow. The reported count is the set verified in the current
walk, not an accumulated estimate.

### 5.3 Writes

All cloud writes use one merge-duplicates upsert path with `item_id` as the
declared conflict target. Validate a complete batch before the first request,
then send bounded chunks. A partial request failure leaves the upload floor
unchanged so replay is safe.

PostgreSQL `bytea` encoding is owned by the REST adapter and round-trips through
one maintained representation. Other modules do not handcraft database wire
strings.

## 6. Authentication and request recovery

### 6.1 Session rules

- Password sign-in, refresh, sign-out and user-profile operations use the
  current GoTrue API shapes.
- Grant kind is explicit. Identical HTTP error envelopes from password and
  refresh operations are classified by the request that was made, never by
  guessing from message text.
- Expiry uses saturating clock arithmetic.
- A rotated refresh token is persisted after every successful refresh.
- Tokens are redacted from `Debug`; email logs use the shared mask.
- An error body is parsed structurally or reduced to a bounded redacted snippet.
- All auth requests have a deadline.

Proactive refresh starts before expiry and uses a minimum interval so a
short-lived token cannot cause a tight loop. Failures use the shared maintained
backoff. Concurrent callers share one token rotation.

On a data-request 401, refresh the stored session once and retry the original
request once. A repeated 401 is a hard failure. A 429 honors bounded
delta-seconds `Retry-After`; absent or invalid guidance uses the shared backoff.
Transient network/5xx retry has a bounded attempt count and jitter. Other 4xx
responses are permanent for that request.

### 6.2 Fail-closed status

Missing credentials, invalid password, revoked refresh token and unauthorised
data access have distinct typed outcomes. Status switches out of signed-in state
when recovery fails. No credential value, bearer URL or raw backend response is
rendered to a user.

## 7. Realtime and cadence

Supabase Realtime uses the maintained Phoenix five-element array envelope. A
frame with another arity is rejected. Refs are parsed by type; a numeric ref is
not coerced to an empty string.

The join payload includes the current user JWT, account-scoped
`postgres_changes` filter and `event: "*"` so insert, update and tombstone
events all wake sync. The JWT is reread on every reconnect. A channel is live
only after the server confirms that the PostgreSQL subscription is ready, not
when the socket opens or merely acknowledges join.

Heartbeats have monotonic refs and a write deadline no longer than the heartbeat
interval. Reconnect uses the shared backoff, resets after a stable session and
clears running state through an RAII guard. Shutdown sends channel leave and a
WebSocket close within a bound.

Logs contain no raw frame, JWT, ciphertext-bearing payload or authenticated
connection URL. Parse failures may report only bounded structural diagnostics.

Realtime is an accelerator; polling remains the source-of-truth backstop:

- start and reset at 5 seconds after activity;
- double while idle;
- cap at 300 seconds only while the database subscription is confirmed;
- cap at 10 seconds whenever the push channel is unavailable;
- wake immediately on change and resubscription;
- skip a scheduled tick when a round is already active;
- queue an explicit user request behind the active round.

One single-flight coordinator owns push plus pull. Retry backoff and idle cadence
remain separate policies: one recovers a failed request, the other reduces idle
wakeups.

## 8. P2P transport parity

P2P summaries include every content-order and pin-order key. Full values are
requested only for remote winners. Applying a page rechecks the current local
row atomically so a concurrent local write cannot be overwritten by a decision
made from a stale planning snapshot.

The Noise-authenticated channel carries bounded typed messages. Pairing trust,
SAS confirmation and revocation remain in the pairing contract; sync does not
add a weaker fallback identity. P2P and cloud share future-skew, sensitivity,
payload-validation and content-merge decisions even though pin transport and
authentication differ.

## 9. Acceptance tests

### 9.1 Comparator and apply

- Exhaustive four-key decision-space tests prove symmetry and exact tuple order.
- Permuting three or more versions produces one winner on every device.
- Equal replay and self-echo do not mutate storage.
- Delete/live timestamp ties choose the tombstone; unknown-item tombstones
  remain durable; delayed creates do not resurrect them.
- Content and pin orders do not restamp one another.
- Replacing a winner preserves the local row id and updates FTS atomically.
- Simultaneous P2P/cloud delivery produces one local item.

### 9.2 Security and refusals

- Live sensitive items produce no P2P summary, fetch response or cloud row.
- Sensitive tombstones sync with no payload and preserve local sensitivity.
- Remote content is redetected locally and never inserts sensitive search text.
- Wrong key, wrong item AAD, modified ciphertext and invalid signature are
  counted skips, never deletes or partial rows.
- A forged far-future row cannot drag the cursor forward or censor honest rows.
- Exact upload-size boundaries pass; one byte over is withheld and reported
  while the local row survives.

### 9.3 Pagination and progress

- More than one page sharing one millisecond drains without loss or duplication.
- Cold-start inclusive boundary and persisted compound cursor both converge.
- A full page drains immediately; a no-progress full page stops.
- Cursor persistence after every page survives interruption and never regresses
  after local retention.
- Download watermark and upload floor can move independently without losing
  local writes.
- A failed upload round leaves the floor unchanged. A successful round excludes
  no capture made while it was running.
- Unreadable local rows remain revisitable beyond the bounded id set and the
  visible count corrects during each durable walk.

### 9.4 Backend and auth

- Schema tests verify exact client columns, constraints, indexes, grants, RLS,
  publication and retention role.
- Cross-account and publishable-key-only access are denied.
- Upsert replay updates one row and always sends an explicit tombstone flag.
- Retention trusts server insertion time and enforces TTL plus account cap.
- Password and refresh failures with the same envelope remain distinct.
- Token rotation is single-flight and persists the returned refresh token.
- 401 retries once, 429 honors bounded guidance, transient failures exhaust one
  shared backoff and permanent 4xx does not retry.

### 9.5 Realtime and orchestration

- Join is not live until subscription-ready; failure never selects the long
  poll ceiling.
- Every reconnect reads the current JWT and account id.
- INSERT, UPDATE and tombstone events wake an immediate round.
- Malformed frames, numeric refs, heartbeat timeout and half-open sockets fail
  without leaking raw data.
- Idle cadence reaches each ceiling, resets on activity and shortens immediately
  when the push channel drops.
- Scheduled work skips an active round while an explicit request queues and
  receives the later outcome.

## 10. Module and dependency rules

Use the maintained HTTP, URL, WebSocket, retry, crypto and serialization
packages already selected by the workspace. The core store adapter owns atomic
apply; `copypaste-p2p::sync` owns the comparator; `copypaste-cloud` owns REST,
Realtime and cloud orchestration. Constants duplicated across an unavoidable
crate boundary require a parity test.

No transport may introduce a second merge tuple, row parser, request retry
ladder, cursor spelling, auth classifier or plaintext-bearing diagnostic path.
