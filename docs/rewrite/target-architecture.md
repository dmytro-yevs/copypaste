# CopyPaste v2 — Library-First Architecture

## The governing rule (this replaces the old norm)

The previous codebase obeyed a "prefer hand-rolling" rule cited in
`copypaste-ipc/src/backoff.rs:29-31` — pointing at a `CLAUDE.md` that **no longer
exists in the repository**. The norm outlived its own documentation and produced
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
| X.509 parsing | 2 hand-written DER walkers | `x509-parser` |
| Cert generation | *(already correct)* | `rcgen` |
| TLS | *(already correct)* | `rustls` |
| Chunked AEAD | custom `CHUNK_FORMAT_V1` framing | `aead::stream` (STREAM) |
| Device pairing | OPAQUE (`opaque-ke`) — augmented PAKE for a symmetric problem | `spake2` (balanced PAKE) |
| Frame codec | byte-scanning partial-JSON parser | `tokio_util::codec::LinesCodec` |
| Task supervision | custom supervisor + 4 interval loops | `tokio-graceful-shutdown` / `JoinSet` |
| Data dirs | duplicated 3× | `directories` |
| Atomic file write | hand-rolled temp+rename | `tempfile` / `atomicwrites` |
| Config | *(already correct)* | `serde` + `toml` |
| Telemetry | 460-LOC PII scrubber duplicating our own detector | `sentry` `before_send`, or omit |
| Secret detection | 40 hand-tuned regexes | ruleset sourced from **gitleaks**, executed via `regex::RegexSet` |

### Rust — IPC

The Unix socket + newline-JSON transport was the **right** minimal call and is not
the problem. The problem was modelling it three times and dispatching 61
stringly-typed methods across 21 files.

**Decision (from manifest 04): keep the JSON protocol. Do NOT adopt `tarpc` or
`jsonrpsee`.** This is the one place where the library-first default is overruled
on evidence:

- `tarpc`'s bincode wire has no TypeScript story, and the Tauri UI reads raw JSON.
- `jsonrpsee` replaces append-only *string* error codes with *numeric* ones,
  destroying the forward-compatibility the CLI depends on (it displays unknown
  codes verbatim via `raw_error_code`).
- Neither framework provides the parts that are actually hard here: readiness
  gating, degraded mode, per-method size caps, the pre-runtime takeover probe.

The real defect was never the protocol — it was modelling it three times. Fixes:

- One crate owns the contract. **Only 3 of ~15 typed DTOs are currently used**;
  the other 12 are correct, documented and completely unreferenced while the
  daemon hand-builds `serde_json::json!` beside a comment describing the DTO.
  Adopt them as-is as the single source of truth.
- Framing via `LinesCodec` (~150 LOC deleted) — but the two-tier, method-aware
  size cap must survive as a custom `Decoder`; it fixed a real RAM-amplification
  bug.
- `PROTOCOL_VERSION` stays `1`. Not a byte changes on the wire.

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
   *write* rows — forging a huge `lamport_ts` to outrank and effectively censor a
   legitimate item. End-to-end confidentiality still holds; metadata integrity
   does not. Mitigation: sign the LWW metadata under the sync key.

### UI

| Concern | v1 | v2 |
|---|---|---|
| Data fetching / polling / cache | ~1000 LOC across 6 hooks | `@tanstack/react-query` |
| Virtual list | prefix-sum + binary search + scroll anchoring | `@tanstack/react-virtual` |
| Dialog / focus trap / scroll lock | hand-rolled | `radix-ui` |
| Popover positioning | partial re-implementation of Floating UI | `@floating-ui/react` |
| Toasts | 198 LOC | `sonner` |
| Styling | bespoke CSS + 979-line reference HTML kept in sync by a parity test | Tailwind + shadcn/ui — **one** source of truth |
| Validation | hand-written validators | `zod` |
| State | *(already correct)* | `zustand` |
| Icons | *(already correct)* | `lucide-react` |

### Android

- `#[uniffi::export]` proc-macros — delete the 979-line `.udl` and its duplicate
  signatures.
- Delete the 366-LOC manual ABI counter; UniFFI already ships contract-version
  checksums.

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
   over `lamport_ts → wall_time → origin_device_id` is correct and complete.
2. **Clipboard access via `objc2`/NSPasteboard.** `arboard` exposes no
   `changeCount` (needed for self-write suppression) and no `org.nspasteboard.*`
   privacy markers (needed to skip password managers). A real capability gap.
3. **Fingerprint-pinning cert verifier.** `rustls` ships no pinning verifier. The
   verifier stays; only the DER parsing inside it moves to `x509-parser`.
4. **SAS pairing state machine.** Security-critical domain logic, not generic
   machinery.
5. **SQLCipher rekey via `sqlcipher_export` + ATTACH + atomic rename.** The
   SQLCipher-recommended crash-safe path; `PRAGMA rekey` was rejected for good
   reason.
6. **Curated secret-detection ruleset.** No maintained Rust crate *is* a ruleset.
   Source the patterns from gitleaks rather than hand-authoring them — but the
   in-process scanner stays.

---

## Non-negotiable constraints carried from v1

- Must open and migrate existing user databases (schema v1..v15).
- Must decrypt data written by v0.4.x — HKDF info strings and AEAD AAD layouts are
  byte-exact contracts.
- Keychain service/account names are fixed strings; changing them orphans keys.
- The port manifests in `docs/rewrite/port-manifest/` are the acceptance criteria.
  A subsystem is not "done" until its manifest's tests pass.
