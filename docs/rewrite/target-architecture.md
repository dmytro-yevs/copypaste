# CopyPaste v2 — Library-First Architecture

> **Three decisions were taken after this document was first written, and they
> reach into most of it.** Each is marked where it applies rather than edited
> away, because a target architecture that shows no evidence of having changed
> its mind is not being read carefully.
>
> 1. **Backward compatibility with v0.4.x was dropped** (`e148e3c1`, CLAUDE.md
>    rule 3). See "Non-negotiable constraints" at the end — two of the original
>    constraints were retired outright.
> 2. **The peer channel is Noise `NNpsk0`, not TLS** (`4aef3482`). That struck
>    four rows from the stack table and retired one of the earned exceptions.
> 3. **The apps go native — SwiftUI on macOS, Compose on Android**, and v1's
>    visual design is explicitly rejected. That replaced the UI section wholesale
>    and changed what the design-token pipeline emits. The Rust core, daemon,
>    IPC, p2p and cloud crates are untouched by it.

## The governing rule (this replaces the old norm)

The previous codebase obeyed a "prefer hand-rolling" rule cited in its
`copypaste-ipc/src/backoff.rs:29-31` — pointing at a `CLAUDE.md` that **no longer
existed in the repository**. (That path is v1's, on
`archive/v0.4.1-pre-rewrite`; v2's crate of the same name has no backoff file
and no retry code of its own.) The norm outlived its own documentation and produced
the same wheel carved 2–6 times over.

**New rule: a dependency is the default. Hand-rolling requires written
justification in an ADR.** Writing code is the exceptional path, not the safe one.

Three narrow exemptions, each requiring an ADR entry:
1. No maintained crate provides the behaviour (verified, with links).
2. The crate cannot see what we need it to see (e.g. anything operating on
   ciphertext — see "Earned exceptions" below).
3. The crate would pull a second crypto/TLS stack into the tree.

"It's only 40 lines" is **not** a justification. Forty lines × 30 places is how
this happened.

---

## Duplication inventory from the v1 audit (what we are NOT rebuilding)

| Concept | Times implemented in v1 | v2: single source |
|---|---|---|
| retry / backoff | **≥6** (CLI ×2, cloud push, cloud poll, relay push, relay receive) | one crate, one policy |
| rate limiting | **3** (daemon, p2p, relay-registration) | `governor` |
| wire-contract model | **3** (ipc crate DTOs, daemon copy, CLI untyped) | one typed crate |
| Lamport / ordering | **4** | one function |
| ASN.1 / DER parser (by hand) | **2** | `x509-parser` |
| regex secret/PII engine | **2** | one detector |
| hex encoding | ~6 sites | `hex` |
| app data-dir resolution | 3 | `directories` |

Plus dead weight carried in v1: a full HELLO/HAVE/WANT sync engine (~5k LOC incl.
tests) that the daemon never instantiated, an unwired telemetry crate (1.7k LOC),
~15 typed DTOs the CLI never imported, and Tailwind class strings threaded through
props in a project with no Tailwind.

---

## Target stack

### Rust — core / daemon

| Concern | v1 (hand-rolled) | v2 (library) |
|---|---|---|
| DB migrations | custom `user_version` ladder + 3 layers of race guards | `rusqlite_migration` |
| Row mapping | positional column lists, hand-synced in 3 places | `serde_rusqlite` (by name) |
| Connection pool | *(already correct)* | keep `r2d2` + `r2d2_sqlite` |
| SQLCipher | *(inherent — keep)* | `rusqlite` `bundled-sqlcipher` |
| Retry / backoff | 6 implementations | `backoff` (one policy object) |
| Rate limiting | 3 implementations | `governor` |
| X.509 parsing | 2 hand-written DER walkers | ~~`x509-parser`~~ — superseded, see below |
| Cert generation | *(already correct)* | ~~`rcgen`~~ — superseded, see below |
| TLS | *(already correct)* | ~~`rustls`~~ — superseded, see below |
| Peer transport | TLS + a pinning verifier | `snow` — Noise `NNpsk0` |
| Chunked AEAD | custom `CHUNK_FORMAT_V1` framing | `aead::stream` (STREAM) |
| Device pairing | OPAQUE (`opaque-ke`) — augmented PAKE for a symmetric problem | ~~`spake2`~~ → the Noise PSK itself, see below |
| Frame codec | byte-scanning partial-JSON parser | `tokio_util::codec::LinesCodec` |
| Task supervision | custom supervisor + 4 interval loops | `tokio-graceful-shutdown` / `JoinSet` |
| Data dirs | duplicated 3× | `directories` |
| Atomic file write | hand-rolled temp+rename | `tempfile` / `atomicwrites` |
| Config | *(already correct)* | `serde` + `toml` |
| Telemetry | 460-LOC PII scrubber duplicating our own detector | `sentry` `before_send`, or omit |
| Secret detection | 40 hand-tuned regexes | ruleset sourced from **gitleaks**, executed via `regex::RegexSet` |

**The superseded rows.** The peer channel was going to be TLS with a
fingerprint-pinning verifier and a balanced PAKE for pairing. It is Noise
`NNpsk0` via `snow` instead (`4aef3482`): the pairing token is 256 bits from the
OS CSPRNG and *is* the pre-shared key, so possession of it is the
authentication. That removes `rustls`, `rcgen`, the pinning verifier and both
DER walkers — and with them the reason for `x509-parser`. The PAKE went the same
way: a PAKE exists to protect a low-entropy human secret from an offline
dictionary attack, and there is no dictionary against a random 256-bit token.
The rows are struck rather than deleted so the change reads as a decision, which
is what it was.

Two more rows are still ahead of the code rather than behind it: `aead::stream`
chunking is not built (items are sealed single-shot under a size cap), and
`governor` is declared in the workspace manifest with nothing yet calling it.

### Rust — IPC

The Unix socket + newline-JSON transport was the **right** minimal call and is not
the problem. The problem was modelling it three times and dispatching 61
stringly-typed methods across 21 files.

**Decision (from manifest 04): keep the JSON protocol. Do NOT adopt `tarpc` or
`jsonrpsee`.** This is the one place where the library-first default is overruled
on evidence:

- `tarpc`'s bincode wire has no TypeScript story. That argument was written for
  a webview UI; with native apps the reason narrows but does not vanish — the
  CLI, the demo scripts and anything a user pipes into `jq` all read the same
  JSON, and a wire a human can read is worth keeping.
- `jsonrpsee` replaces append-only *string* error codes with *numeric* ones,
  destroying the forward-compatibility the CLI depends on (it displays unknown
  codes verbatim via `raw_error_code`).
- Neither framework provides the parts that are actually hard here: readiness
  gating, degraded mode, per-method size caps, the pre-runtime takeover probe.

The real defect was never the protocol — it was modelling it three times. Fixes:

- One crate owns the contract. In v1, **only 3 of ~15 typed DTOs were used**;
  the other 12 were correct, documented and completely unreferenced while the
  daemon hand-built `serde_json::json!` beside a comment describing the DTO.
  v2's `copypaste-ipc` is that single source of truth: daemon, CLI and every
  other client compile against it, so drift is a build error.
- Framing via `LinesCodec` (~150 LOC deleted) — but the two-tier, method-aware
  size cap must survive as a custom `Decoder`; it fixed a real RAM-amplification
  bug.
- `PROTOCOL_VERSION` is `1`. The original reason — "not a byte changes on the
  wire" — expired with backward compatibility: nothing has to interoperate with
  a v0.4.x client, and manifest 04's wire-compatibility sections are reference
  only now. The version field stays because a local socket still needs a
  handshake that fails loudly when a stale binary is left behind, not because
  the shape is frozen. Changing it is a decision, not a breach.

### Backend

Drop the bespoke relay (12k LOC: write-behind cache, custom retry queue, SSE
fan-out, supervisor, a second rate limiter). **Supabase** is already a working
dependency and provides auth, Postgres, realtime, and RLS.

**Verdict (from manifest 05, 17-row parity table): dropping the relay is safe for
correctness.** The relay was never a per-device broker — every device
co-registered a *single* shared inbox id derived from the sync key, which is
structurally the same shape as one `user_id`-scoped Supabase table. Upsert on
`item_id` is in fact *better*: an append-only queue permits duplicate rows per
logical item; a keyed table cannot.

Three things must be handled deliberately rather than simply omitted:

1. **Keep the poll loop.** Realtime `postgres_changes` is at-most-once, exactly
   like the relay's SSE — there is no replay across a disconnect. The cursor poll
   *is* the correctness mechanism; Realtime is only an accelerator. Deleting it
   "because we have Realtime now" silently reintroduces data loss on every
   reconnect.
2. **Quota (500 items) and TTL (24 h) have no Supabase equivalent.** Correctness
   is unaffected, but the "server forgets within a day" privacy property and the
   cost bound are lost. Restore with a `pg_cron` job ordered on `created_at` —
   **never** on client-supplied `wall_time`, or an intra-account attacker forges
   a low `wall_time` to escape eviction and displace legitimate items.
3. **PoP and account auth prove different secrets.** PoP proved possession of the
   sync key; Supabase proves possession of the account password. An attacker
   holding the account but not the passphrase can now read metadata and, worse,
   *write* rows — forging a far-future ordering key to outrank and effectively
   censor a legitimate item. End-to-end confidentiality still holds; metadata
   integrity does not. Mitigation, **not yet built**: sign the LWW metadata
   under the sync key. What is built is a bound rather than a fix — versions
   stamped implausibly far ahead are refused (`MAX_FUTURE_SKEW_MS`), so a bad
   or hostile clock can win ties it should have lost but cannot win everything
   forever.

   Note that the ordering key here is `created_at`, not the manifest's
   `lamport_ts`: **v2 has no Lamport clock**, deliberately, and both transports
   share one comparator so there is no second ordering to drift. Manifest 05's
   Lamport sections are reference material for the same reason its wire framing
   is.

### UI — native per platform

**Superseded.** This section previously prescribed a webview stack:
`@tanstack/react-query` for fetching and cache, `@tanstack/react-virtual` for the
list, `radix-ui` and `@floating-ui/react` for overlays, `sonner` for toasts,
Tailwind + shadcn/ui for styling, `zod`, `zustand`, `lucide-react`. Those were
the right answers *for a webview app*. The apps are now **native per platform**:
**SwiftUI** on macOS, **Jetpack Compose** on Android, each using its own
platform's tools rather than a shared web layer. A React dependency table has
nothing to say about either.

The Tauri + React history window in `crates/copypaste-ui` is not the product
surface. It stays only until the native macOS app exists, and it should not
acquire new dependencies or new architecture in the meantime — work put into it
is work to be deleted.

What the library-first rule means on native:

| Concern | Where it goes |
|---|---|
| List virtualisation, overlays, focus management, toasts, styling, icons | The platform's own frameworks. SwiftUI `List`/`LazyVStack` and Compose `LazyColumn` already are the library; wrapping them in a house abstraction is the v1 mistake in a new language. |
| Server state — history, status, mutations | The Rust core over the IPC contract. Neither app re-implements polling, caching or merge; the daemon is the authority and `copypaste-ipc` is the one model of the contract. |
| Client state | Platform-idiomatic (`@Observable` / `StateFlow`). Small, and never a second copy of what the daemon owns. |
| Rust ↔ platform binding | `#[uniffi::export]` proc-macros for Android — no hand-written `.udl` and no manual ABI counter, since UniFFI ships contract-version checksums. macOS talks to the daemon over the same Unix socket the CLI uses. |

**The Rust core is unaffected by this decision.** `copypaste-core`,
`copypaste-daemon`, `copypaste-ipc`, `copypaste-p2p` and `copypaste-cloud` are
shared by both apps and change in no way: capture, crypto, storage, detection,
IPC, peer sync and cloud sync all sit below the UI boundary. Going native
changes who draws the pixels, not who owns the data.

**Manifest 06 still binds — its behaviour half.** Scroll anchoring, row heights
reserving the full cap, the 15 accessibility requirements, sensitive content
being absent from the view rather than covered over, no filesystem path in a
user-facing error, and the 73 acceptance tests are requirements of the product
and survive the change of toolkit. Its *visual* half — palette, token values,
scales, `design-reference.html` — is reference only now: v1's look is
deliberately not being carried over. See
[`port-manifest/README.md`](port-manifest/README.md).

### Design tokens

The one-source-compiled-per-platform pipeline in `design/` is still the right
shape, and the reason is unchanged: v1 kept a bespoke CSS system in step with a
979-line reference HTML by way of a hand-written parity test, which is two
sources of truth with a test standing between them. Style Dictionary keeps the
single source.

Two things about it change:

- **The outputs are SwiftUI and Compose**, not CSS custom properties and a
  Tailwind `@theme`. The web outputs remain only for as long as the interim
  Tauri window does.
- **The values are open.** The current tokens are v1's, ported verbatim from
  manifest 06 §8, and v1's design is explicitly rejected. The new visual
  language has not been decided, and it is not decided here — nobody should
  treat the existing values as a default, and re-deriving them under new names
  would be the same outcome by a slower route.

The token *names* were kept deliberately close to the manifest so §8 stayed
readable without a translation step. That reason expires with the values; when
the new design lands, the naming is open too.

### Release

`cargo-release` (version/tag) + `cargo-dist` (binaries, checksums, Homebrew cask
and tap) + `git-cliff` (already used). Custom scripts survive only for
macOS DMG signing/notarisation and the Android APK — the parts cargo-dist
genuinely does not cover.

---

## Earned exceptions — custom code that STAYS

These were audited and confirmed correct. Porting them is mandatory; replacing
them with a library would be a regression.

1. **LWW merge on metadata.** Clipboard content is opaque ciphertext. A structural
   CRDT (`automerge`, `yrs`) cannot operate on values it cannot read. Last-write-wins
   is correct and complete. The shipped order is
   `created_at → content_hash → deleted → origin_device_id`; ranking `deleted`
   above the origin is a deliberate correction to the specification this was
   written against, because a tombstone keeps its item's content hash and so
   ties its own live version on the first two keys — with the origin ranked
   higher, deletions were resurrected on about half of those ties.
2. **Clipboard access via `objc2`/NSPasteboard.** `arboard` exposes no
   `changeCount` (needed for self-write suppression) and no `org.nspasteboard.*`
   privacy markers (needed to skip password managers). A real capability gap.
3. ~~**Fingerprint-pinning cert verifier.**~~ **Retired, not ported.** This was
   an earned exception only while the peer channel was TLS. Noise `NNpsk0`
   authenticates from the pairing PSK, so there is no certificate to pin, no
   trust store, and no DER to parse — the custom code was removed by deleting
   the problem rather than by finding a crate for it, which is the better
   outcome and worth recording as one.
4. **SAS pairing state machine.** Security-critical domain logic, not generic
   machinery. *Not built.* What shipped is a minted pairing code, read out and
   entered on the other device, which is also the PSK; whether a
   short-authentication-string confirmation is still wanted on top of a PSK
   handshake is open, and manifest 06's SAS flow is the reference until it is
   settled.
5. **SQLCipher rekey via `sqlcipher_export` + ATTACH + atomic rename.** The
   SQLCipher-recommended crash-safe path; `PRAGMA rekey` was rejected for good
   reason. *Not built* — there is no rotation path in v2 yet.
6. **Curated secret-detection ruleset.** No maintained Rust crate *is* a ruleset.
   Source the patterns from gitleaks rather than hand-authoring them — but the
   in-process scanner stays.

---

## Non-negotiable constraints

Two of the constraints originally listed here — "must open and migrate existing
user databases (schema v1..v15)" and "must decrypt data written by v0.4.x, HKDF
info strings and AAD layouts are byte-exact contracts" — were **retired by the
decision to drop backward compatibility** (`e148e3c1`, CLAUDE.md rule 3). They
are recorded here rather than deleted, because the reasoning that replaced them
only makes sense against what it replaced. v2 reads nothing written by an
earlier version: no migration ladder, no `key_version` dispatch, no legacy
decoder, one schema and one key derivation.

What binds now:

- **v2 must never open, modify, or misreport a v1 file.** The database is
  `copypaste-v2.db`, a distinct filename, so an old file is never touched and
  survives a downgrade intact. A build that does stumble onto v1 data must say
  so plainly rather than failing with a decryption error that reads like
  corruption.
- **No migration path may be retrofitted casually.** Adding one later is a
  feature to be decided, and it is materially harder once the v1 formats have
  left the tree — which they have.
- **Keychain service and account names are fixed strings.** Not for
  compatibility with v0.4.x, which is gone, but because renaming them orphans
  keys already written by any v2 build (manifest 02, I-10). A frozen-identifier
  test asserts it.
- **The security properties survive the format change.** Fail closed on a wrong
  key or a wrong AAD, never fall back to a plaintext read; the AAD binds item
  identity; key material is zeroized; comparisons are constant-time. Dropping
  the byte layouts retires *what* the bytes are, not *what they must guarantee*.
- **Sensitive items never reach the search index**, and no error string shown to
  a user may contain a filesystem path — the daemon socket path discloses the
  local username.
- **The port manifests in `docs/rewrite/port-manifest/` are the acceptance
  criteria.** A subsystem is not "done" until its manifest's tests pass. Read
  [`port-manifest/README.md`](port-manifest/README.md) first: it records, per
  manifest, which sections stayed binding and which became reference material
  when compatibility was dropped. Behaviour stayed binding; formats did not.
