# CopyPaste v2 — library-first architecture

Which maintained crate does each job, and the short list of custom code that
stays. The governing rule is [CLAUDE.md](../../CLAUDE.md) rule 1 and is not
restated here.

Where a row disagrees with the tree, the tree is right — this is the target, not
an inventory. `Cargo.toml`'s dependency table carries the same choices with the
version pins and the RustSec reasoning attached.

## Core and daemon

| Concern | v1 (hand-rolled) | v2 |
|---|---|---|
| DB schema | `user_version` ladder + three layers of race guards | one transactional fresh schema; exact validation on every existing file |
| Row mapping | positional column lists, hand-synced in three places | `rusqlite`'s `row.get("name")`, by name |
| Connection pool | *(already correct)* | `r2d2` + `r2d2_sqlite` |
| SQLCipher | *(inherent)* | `rusqlite` `bundled-sqlcipher` |
| Retry / backoff | six implementations | one policy object |
| Rate limiting | three implementations | *(not built)* |
| Peer transport | TLS + a pinning verifier + two DER walkers | `snow` — Noise `NNpsk0` |
| Device pairing | OPAQUE — an augmented PAKE for a symmetric problem | the Noise PSK itself |
| Chunked AEAD | custom `CHUNK_FORMAT_V1` framing | `aead::stream` — *not built; items are sealed single-shot under a size cap* |
| Frame codec | byte-scanning partial-JSON parser | `tokio_util::codec::LinesCodec` |
| Task supervision | custom supervisor + four interval loops | `JoinSet` |
| Data dirs | duplicated three times | `directories` |
| Atomic file write | hand-rolled temp+rename | `tempfile` |
| Telemetry | a 460-line PII scrubber duplicating our own detector | *(omitted)* |
| Secret detection | 40 hand-tuned regexes | ruleset sourced from **gitleaks**, executed through `regex::RegexSet` |

**Why there is no TLS row.** The peer channel was going to be TLS with a
fingerprint-pinning verifier and a balanced PAKE for pairing. Noise `NNpsk0`
removes all of it: the pairing token is 256 bits from the OS CSPRNG and *is* the
pre-shared key, so possession is the authentication. No certificate means no
`rustls`, no `rcgen`, no pinning verifier, no DER parser and no `x509-parser`. A
PAKE exists to protect a low-entropy human secret from an offline dictionary
attack, and there is no dictionary against a random 256-bit token.

## IPC

**Keep the newline-JSON protocol over the Unix socket. Do not adopt `tarpc` or
`jsonrpsee`.** This is the one place the library-first default is overruled, on
three pieces of evidence:

- `tarpc`'s bincode wire has no TypeScript story, and the CLI, the demo scripts
  and anything piped into `jq` all read the same JSON. A wire a human can read
  is worth keeping.
- `jsonrpsee` replaces append-only *string* error codes with *numeric* ones,
  destroying the forward compatibility the CLI depends on: it displays unknown
  codes verbatim via `raw_error_code`.
- Neither framework provides the parts that are actually hard here — readiness
  gating, degraded mode, per-method size caps, the pre-runtime takeover probe.

The defect in v1 was never the protocol; it was modelling it three times and
dispatching 61 stringly-typed methods across 21 files, with only 3 of ~15 typed
DTOs ever imported. `copypaste-ipc` is the single source: daemon, CLI and the
Tauri bridge compile against it, so drift is a build error.

Two consequences worth stating:

- `LinesCodec` replaced ~150 lines of framing, but the two-tier, method-aware
  size cap survives as a custom `Decoder` — it fixed a real RAM-amplification
  bug.
- `PROTOCOL_VERSION` is `2`, and changing it is a decision rather than a breach.
  Nothing has to interoperate with a prior-product client; the field stays
  because a local socket needs a handshake that fails loudly when a stale
  binary is left behind.

## Backend

Drop v1's bespoke relay — 12k lines of write-behind cache, custom retry queue,
SSE fan-out, supervisor and a second rate limiter. **Supabase** provides auth,
Postgres, Realtime and RLS.

Manifest 05's 17-row parity table finds this safe for correctness: the relay was
never a per-device broker, since every device co-registered a *single* shared
inbox id derived from the sync key — structurally the same shape as one
`user_id`-scoped table. Upsert on `item_id` is strictly better, because an
append-only queue permits duplicate rows per logical item and a keyed table
cannot.

Three things must be handled deliberately rather than omitted:

1. **Keep the poll loop.** Realtime `postgres_changes` is at-most-once, exactly
   like the relay's SSE: there is no replay across a disconnect. The cursor poll
   *is* the correctness mechanism and Realtime is an accelerator. Deleting it
   "because we have Realtime now" reintroduces data loss on every reconnect.
2. **Quota and TTL have no Supabase equivalent.** Correctness is unaffected, but
   the "server forgets within a day" property and the cost bound are lost.
   Restored by a `pg_cron` job ordered on `created_at` — **never** on
   client-supplied `wall_time`, or an intra-account attacker forges a low
   `wall_time` to escape eviction and displace legitimate items.
3. **PoP and account auth prove different secrets.** PoP proved possession of
   the sync key; Supabase proves possession of the account password. An attacker
   holding the account but not the passphrase can read metadata and *write* rows
   — forging a far-future ordering key to outrank and effectively censor a
   legitimate item. End-to-end confidentiality would hold; metadata integrity
   would not. So the LWW metadata is signed under a key derived from the sync
   passphrase (`cloud/src/crypto/sign.rs`), verified before a row reaches the
   merge, and a row that does not verify is refused rather than ranked. Two
   bounds sit under it: versions stamped implausibly far ahead are refused
   (`MAX_FUTURE_SKEW_MS`), and the ciphertext is inside the signature, so real
   content cannot be spliced onto another version's stamp.
   [`../cloud-privacy.md`](../cloud-privacy.md) is the disclosure page.

The ordering key is `created_at`, not manifest 05's `lamport_ts`: **v2 has no
Lamport clock**, deliberately, and both transports share one comparator so there
is no second ordering to drift.

## UI

One Tauri v2 + React app on macOS, Android and Windows
([ADR-0002](../adr/0002-one-cross-platform-app.md)); `crates/copypaste-ui` is
the product surface. The bridge is one `Backend` trait with two implementations
chosen by a compile-time alias
([ADR-0003](../adr/0003-one-command-surface-two-backends.md)).

| Concern | Where it goes |
|---|---|
| List virtualisation, accessible reorder, overlays, focus management, styling, icons | The React ecosystem — `@tanstack/react-virtual`, `@dnd-kit/react`, Radix, Tailwind v4, shadcn/ui, `lucide-react`. Wrapping any of them in a house abstraction is the v1 mistake in a new language. |
| Server state — history, status, mutations | The Rust core over the IPC contract, via `@tanstack/react-query`. The app re-implements no polling, caching or merge; the daemon is the authority. |
| Client state | `zustand`, and never a second copy of what the daemon owns. |
| Menu bar, popover, global hotkey, launch at login | Tauri's own tray and window APIs, `tauri-plugin-global-shortcut`, `tauri-plugin-autostart`. The one thing `shell/` adds is a *policy*: refusing the shortcuts that would cost an Accessibility grant, which no upstream crate knows to do. |

`@dnd-kit/react` adds a pre-1.0 API and its DOM/state packages to the frontend
audit surface. That cost is accepted because the maintained React 19 adapter
owns pointer, touch, keyboard, auto-scroll and screen-reader drag behaviour,
including virtual lists; custom gesture state would duplicate all of it.

**The Rust core is unaffected by the UI decision.** `copypaste-core`,
`copypaste-daemon`, `copypaste-ipc`, `copypaste-p2p` and `copypaste-cloud` sit
below the boundary and are shared by both targets.

**Manifest 06 still binds — its behaviour half.** Scroll anchoring, row heights
reserving the full cap, the 15 accessibility requirements, sensitive content
being absent from the view rather than covered over, no filesystem path in a
user-facing error, and the 73 acceptance tests. Its *visual* half — palette,
token values, scales, `design-reference.html` — is reference material: v1's look
is not carried over.

## Design tokens

One DTCG source in `design/tokens/`, compiled by Style Dictionary to CSS custom
properties and a Tailwind `@theme`. v1 kept a bespoke CSS system in step with a
979-line reference HTML by way of a hand-written parity test, which is two
sources of truth with a test between them.

The visual system is decided — shadcn/ui on Tailwind v4, zinc base in OKLCH —
and the accessibility numbers are measured rather than asserted: `npm run check`
composites every pair across both themes and all six accents and gates the
build. [`design/README.md`](../../design/README.md) has the comparison that
was made and the three shadcn defaults that failed the contract.

## Release

Hand-written: `.github/workflows/release.yml` plus `scripts/release/`.
**`cargo-dist` was evaluated and rejected on capability, not preference** — it
packages tarballs of plain binaries, does not build `.app` bundles or DMGs
(axodotdev/cargo-dist#24, open since 2022), and its Homebrew installer emits a
formula with no cask support, so it cannot express `app "CopyPaste.app"` or the
postflight [ADR-0001](../adr/0001-macos-distribution-without-a-developer-id.md)
requires. It would cover the CLI tarball and nothing else, splitting the release
across two systems that each want to own the GitHub Release. The full rationale
is at the top of `release.yml`; revisit if #24 is ever implemented.

## Earned exceptions — custom code that stays

Audited and confirmed correct. Replacing these with a library would be a
regression.

1. **LWW merge on metadata.** Clipboard content is opaque ciphertext, so a
   structural CRDT (`automerge`, `yrs`) cannot operate on values it cannot read.
   The order is `created_at → content_hash → deleted → origin_device_id`.
   Ranking `deleted` above the origin is deliberate: a tombstone keeps its
   item's content hash and so ties its own live version on the first two keys —
   with the origin ranked higher, deletions were resurrected on about half of
   those ties.
2. **Clipboard access via `objc2`/NSPasteboard.** `arboard` exposes no
   `changeCount` (needed for self-write suppression) and no `org.nspasteboard.*`
   privacy markers (needed to skip password managers). A real capability gap.
3. **SAS pairing state machine.** Security-critical domain logic, not generic
   machinery. *Not built.* What ships is a minted pairing code, read out and
   entered on the other device, which is also the PSK. Whether a
   short-authentication-string confirmation is wanted on top of a PSK handshake
   is open; manifest 06's SAS flow is the reference until it is settled.
4. **SQLCipher rekey via `sqlcipher_export` + ATTACH + atomic rename.** The
   SQLCipher-recommended crash-safe path; `PRAGMA rekey` was rejected for good
   reason. *Not built* — v2 has no rotation path.
5. **Curated secret-detection ruleset.** No maintained Rust crate *is* a
   ruleset. Source the patterns from gitleaks; the in-process scanner stays.

The fingerprint-pinning cert verifier was an earned exception only while the
peer channel was TLS. Noise removed the problem rather than the code, which is
the better outcome.

## Constraints

- **v2 opens only its own database filename.** Never open, migrate, or probe
  prior-product files. No `LegacyDatabase`, encounter detection, or special
  messaging about old versions.
- **No migration path may be retrofitted casually.** Adding one is a feature to
  decide, and it is materially harder now the v1 formats have left the tree.
- **Keychain service and account names are fixed strings.** Renaming them
  orphans keys already written by a v2 build (manifest 02, I-10). A
  frozen-identifier test asserts it.
- **The security properties survive the format change.** Fail closed on a wrong
  key or a wrong AAD, never fall back to a plaintext read; the AAD binds item
  identity; key material is zeroized; comparisons are constant-time.
- **Sensitive items never reach the search index**, and no error string shown to
  a user may contain a filesystem path — the daemon socket path discloses the
  local username.
- **The port manifests are the acceptance criteria.** Read
  [`port-manifest/README.md`](port-manifest/README.md) first: it records which
  sections stayed binding and which became reference material. Behaviour stayed;
  formats did not.
