# Hardcoded values: what is a decision and what is a defect

A sweep of Supabase credentials, path construction, bounds and intervals,
feature flags, and version/protocol/port constants, prompted by
`backend/embedded/rows.rs` answering `legacy_history_present: false`
unconditionally on Android — a constant standing in for a value the code could
compute, where being wrong is silent.

Most constants in this tree are deliberate and carry their reason. The three
classes below separate them. Class 3 is where the next silent failure comes
from: the value is defensible, and nothing keeps it so.

Three findings were fixed; each is marked. Nothing else was changed.

---

## 1. Defects

Ranked by consequence.

### 1.1 `brew zap` deletes the v0.4 history and leaves every v2 file

`Casks/copypaste.rb:145-149`

```ruby
zap trash: [
  "~/Library/Application Support/CopyPaste",
```

That path is `copypaste_ipc::v1_data_dir()` — where v0.4.x kept `clipboard.db`.
v2's directory is `~/Library/Application Support/com.copypaste.CopyPaste`
(`data_dir()`, via `ProjectDirs::from("com", "copypaste", "CopyPaste")`).
`copypaste-ipc`'s own test asserts the two differ on macOS
(`lib.rs:456-465`, `assert_ne!(v1, data_dir())`).

Both halves are wrong, in opposite directions:

- **`zap` destroys the v0.4 history.** CLAUDE.md rule 3 promises "a user who
  downgrades or reinstalls should find their old data intact on disk". This
  removes it. Unrecoverable — rule 4's worst outcome.
- **`zap` removes nothing of v2.** The history, the device secret and the peer
  PSKs all survive a command whose entire contract is "remove every trace".

`packaging/macos/selfsign.sh:71` builds the same path for the signing keychain,
so install-time tooling *writes* into the directory `paths.rs` documents as
"Read-only, by contract. Nothing may open, create or write anything under the
returned path." The cask and the script agree with each other and disagree with
the application.

Fixed. The signing material moved to `com.copypaste.CopyPaste/signing` and the
cask now zaps that directory. Relocating a certificate normally costs the TCC
grant ADR-0001 exists to preserve, and here it costs nothing: v2 has not
shipped, so no certificate exists at the old path that belongs to this app —
only v0.4's, which v2 must not be signing with anyway. `check.sh` now fails if
either file names the v0.4 directory again; nothing in the Rust tree does,
which is why this survived.

### 1.2 The capture gate is compiled in, and is above the IPC frame cap

`crates/copypaste-daemon/src/clipboard/mod.rs:96-102`

```rust
/// Read gate for text, in bytes (§4, "Max text (default)" = 10 MiB).
///
/// Kept under the 16 MiB wire-frame cap so anything storable is transportable.
const MAX_TEXT_BYTES: usize = 10 * 1024 * 1024;
```

`copypaste_ipc::MAX_FRAME_BYTES` is **8 MiB**, not 16. The comment's number is
wrong and the invariant it asserts — anything storable is transportable — is
false by 2 MiB.

There are two size gates and only one is the user's:

| Gate | Value | Configurable |
|---|---|---|
| capture (`clipboard/mod.rs`) | 10 MiB | no |
| storage (`core/ingest.rs:108`) | `settings.max_item_bytes`, default 4 MiB, bounds 1 KiB – 64 MiB | yes, live |
| IPC frame (`ipc/lib.rs:39`) | 8 MiB | no |

`ingest.rs`'s own comment says "the cap has to be a *user's* number rather than
a compiled-in one"; the gate immediately upstream of it is a compiled-in one.
`clipboard/mod.rs` admits the gap ("Configuration is not wired yet") — so this
is a known hole, but its stated bound is also wrong.

What breaks, once `max_item_bytes` is raised past 8 MiB (the bounds permit 64):

- An 8–10 MiB item is captured, stored and indexed, and then **no client can
  read it**. `Item.content` carries the full content and `list` returns pages of
  `Item`, so the response frame exceeds `MAX_FRAME_BYTES` and the *whole page*
  fails — not just that row — for the CLI and the UI alike.
- Import takes the same path, so an export can plant one without a copy.
- Raising `max_item_bytes` above 10 MiB does nothing at all for captured items,
  and says nothing.

Not fixed: which gate wins, and whether the frame cap rises with it, is a design
decision.

### 1.3 The daemon rebuilt the database and socket filenames — **fixed**

`crates/copypaste-daemon/src/main.rs:431` was

```rust
Some(dir) => (dir.join("copypaste-v2.db"), dir.join("daemon.sock")),
```

Now both come from `copypaste_ipc::database_path()` / `socket_path()` through a
`relocate` helper. `git log -S` puts both literals in `92d69044`, the MVP
commit — an original, not something a wide diff swept in.

Why it matters more than a stale name: `keystore::history_present`
(`crypto/keystore/mod.rs:96`) derives the v2 filename from `database_path()`
and is the F-11 guard that refuses to mint a new device secret when a history is
already present. If the resolver's name and the literal ever diverged,
`--data-dir` would write a database the guard cannot see, the guard would read
"first run", and a **new device secret would be minted over a database sitting
right there** — re-keyed, unreadable, silent. That file already states the rule:
"a second definition of it is a second thing to keep in step."

### 1.4 `clipboard_items` spelled twice in `copypaste-cloud` — **fixed**

`rest/mod.rs:168` (`pub const TABLE`) and `realtime/mod.rs:95`
(`pub(crate) const TABLE`), plus `TOPIC` restating it a third time as
`"realtime:clipboard_items"`.

A Realtime subscription naming a table the REST client does not write receives
nothing — and nothing reports it, because the cursor poll is the correctness
mechanism and simply carries everything. The symptom is lost push latency with
no error on any surface.

`realtime::TABLE` now *is* `rest::TABLE`. `TOPIC` cannot be built from a `const`
in a `const`, so a test pins it to `format!("realtime:{TABLE}")`.

---

## 2. Deliberate, reason recorded, still true

Checked, not merely noted.

**Supabase URL and anon key.** No production value anywhere in the tree. The
deployment resolves in `daemon/src/main.rs:128-139` from `--cloud-url` /
`--cloud-anon-key`, then `COPYPASTE_CLOUD_URL` / `COPYPASTE_CLOUD_ANON_KEY`;
a half-configuration reports unconfigured rather than failing at first request.
Every literal in the tree is a test fixture (`example.invalid`, `127.0.0.1:1`,
`proj.supabase.invalid`, `"anon"`).

The anon key is handled as **configuration**, and `docs/cloud-privacy.md`
agrees: it never claims the key is a secret, and names RLS as the control. Code
and doc are consistent. `SupabaseAuth`'s hand-written `Debug`
(`auth/client.rs:38-46`) keeps it out of log lines anyway, for noise rather than
secrecy, and says so. Manifest 06 §132 lists the anon key among values never
rendered as text; that is a v1 Settings-screen rule and v2's UI has no field for
it, so nothing contradicts it today.

**Feature flags.** There are **zero** `#[cfg(feature = …)]` guards over a
capability. Three `cfg` sites name a feature: two are
`any(target_os = "android", feature = "embedded-backend")`, where the target arm
is what a real build takes, and the third
(`all(not(target_os = "android"), feature = "android-keystore-typecheck")`)
compiles a module it deliberately does not select. The two features
select nothing at runtime; they exist so Android code type-checks on a Linux
host. Release builds pass no `--features` at all
(`scripts/release/build-{macos-app,cli-tarball}.sh`). **Nothing can be shipped
off.** Both Cargo.toml entries name the macOS Keychain incident as the reason
the backends are target-gated rather than feature-gated.

**Retention and caps.** 24 h / 500 rows live in `private.retention_policy`, a
one-row queryable table rather than literals in the function body, with the
`inserted_at`-not-`created_at` defect (`CopyPaste-1uqb`) recorded. Matches
`docs/cloud-privacy.md`'s "forgets within a day … newest 500".

**Version strings.** `env!("CARGO_PKG_VERSION")` in both binaries;
`tauri.conf.json` points `version` at `package.json`; `build-macos-app.sh:104`
fails the release if `package.json` and `[workspace.package]` disagree.

**Bounds that already have a keeper.** `DEDUP_WINDOW_MS` is the index bucket,
deliberately distinct from the configurable window, and `settings.rs:211` pins
the config default to it. `MAX_MESSAGE_BYTES` (p2p protocol vs transport) is
pinned by an assertion at `transport/session.rs:372`.
`too_large_to_sync` is computed from the cloud crate's own caps and pinned by
`the_upload_caps_are_the_cloud_crates_own`.

**Constants carrying a measurement or a defect ID**, all still holding:
`PREFILTER_DFA_SIZE_LIMIT` (8 MiB, with the 55× measurement), `MAX_PAIRINGS`
(16, pinned equal to `MAX_ADVERTISED_PAIRING_IDS`, refusal-not-eviction),
`PAIRING_CODE_TTL` (300 s, against v1's 120), `DEFAULT_PORT` (47654),
`peers-v2.json` and `copypaste-v2.db` (rule 3's distinct names),
`SQLCIPHER_PAGE_BYTES`, `notify.rs`'s `/usr/bin/afplay` (target-gated, and the
reason it is not user-identifying is written down).

---

## 3. Deliberate, reason not recorded — or recorded and unenforced

This is the list to watch.

**3.1 `MAX_FUTURE_SKEW_MS` = 24 h, defined twice.**
`p2p/sync/plan.rs:17` and `cloud/sync/pull.rs:75`. The cloud side records the
mirroring *and* the tradeoff (no crate edge for one `i64`; belongs in
`copypaste-core` if a third transport appears). But nothing enforces the
equality: the only apparent pin, `cloud/sync/mod.rs:150`, asserts the constant
against its own literal, not against p2p. This is the bound that stops a forged
future stamp censoring an item on every device, and the two transports share one
comparator by INV-C2. The daemon links both crates and is already where this
repo puts exactly this kind of pin — two such tests exist there.

**3.2 Page bounds defined twice across crates.** `MAX_PAGE` = 1000,
`DEFAULT_LIST_PAGE` = 50, `DEFAULT_SEARCH_PAGE` = 20 in
`daemon/server/items.rs:24-28` and `ui/src-tauri/…/embedded/rows.rs:19-21`.
`rows.rs` records *why* they must be equal ("the same contract seen from the
other side"). The daemon's are private, so they cannot be imported, and nothing
pins them. Both crates depend on `copypaste-ipc`, which is where one definition
belongs — it already owns `MAX_FRAME_BYTES` and the config bounds.

**3.3 `MAX_REORDER_IDS` = 10 000** (`daemon/server/items.rs:35`) is justified by
"the history cap itself is 10 000 items". That is the config *default*;
`history_limit` is settable to 1 000 000. The value is probably still fine; the
justification is stale.

**3.4 TypeScript restatements of Rust constants**, none pinned:

| TS | Rust | If they drift |
|---|---|---|
| `peerState.ts:19` `MAX_PAIRINGS = 16` | `copypaste_p2p::MAX_PAIRINGS` | The screen refuses a pairing the daemon would take, or promises one it will not |
| `peerState.ts:27` `STALE_AFTER_MS = 15 * 60_000` | derived by hand as 3 × `MAX_POLL_INTERVAL` | Healthy peers reported stalled |
| `lib/ipc.ts:88` `CURRENT_PROTOCOL_VERSION = 1` | `copypaste_ipc::PROTOCOL_VERSION` | Fails loudly (mismatch banner) — low consequence |
| `usePush.ts` / `useCapture.ts` event names | `service/push.rs`, `capture/intake.rs` | Live updates stop; the poll backstop hides it |

The first two carry a note saying where the Rust value lives. That is the right
comment and not a check.

**3.5 `daemon/src/cadence.rs` reimplements the doubling** that
`cloud/sync/cadence.rs` owns. The *bounds* are imported; the algorithm is not.
The module header says so and names the intended end state, which is the honest
version of this — but it is a second implementation of a retry-shaped rule, and
rule 1's opening table is what that grows into.

**3.6 Peer sync idles to the with-push ceiling.** `daemon/p2p/poll.rs` uses
`Idle`, whose ceiling is `MAX_POLL_INTERVAL` (300 s) — the cloud's ceiling *for
when a push channel is confirmed*. Peer sync has no push channel; the cloud
picks `MAX_POLL_INTERVAL_WITHOUT_PUSH` (10 s) in that state. The compensating
mechanism is real (the other side dials in, and `wake()` resets on local
capture), but the choice is not written down anywhere.

**3.7 A stale fact in a comment.** `ui/src-tauri/src/service/diagnostics.rs:88`
states "The real macOS socket path is
`~/Library/Application Support/CopyPaste/daemon.sock`". That is the v0.4
directory; v2's socket is under `com.copypaste.CopyPaste`. The code is correct —
only the claim is wrong, which rule 8 rates as worse than no comment. Same
directory confusion as finding 1.1, in a second place.

---

## What was changed

Two edits, each confined to one file, each a duplicated constant made to
reference its owner. `cargo +1.96 test --workspace` (968 tests), `clippy
--workspace --all-targets -D warnings` and `fmt` are clean.

- `crates/copypaste-daemon/src/main.rs` — `--data-dir` relocates the filenames
  `copypaste_ipc` resolves instead of restating them.
- `crates/copypaste-cloud/src/realtime/mod.rs` — `TABLE` is `rest::TABLE`; a
  test pins `TOPIC` to it.

Everything else is reported rather than touched: 1.1 and 1.2 need a decision,
and the page bounds in 3.2 and the `rows.rs` half of it are held by another
agent.
