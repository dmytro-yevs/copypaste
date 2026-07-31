# Module audit — what the 500-line budget cannot see

**Question asked:** the file-size gate is green on every file in the tree. Did
the modules that were split — `daemon/p2p/mod.rs`, `ipc.rs`, `backend/embedded`,
`storage/`, `lib/ipc.ts` — stop being god modules, or did they just stop being
long files?

**Short answer:** mostly they stopped. Six of the seven named candidates measure
as genuinely narrow, and the Rust tree is in better shape than the file sizes
suggest. Four things are too broad, and the one that is worst is the one the gate
literally cannot see: `copypaste-daemon/src/main.rs`, which the script reports as
**38 source lines** when it is **729**. Today's `lib/ipc.ts` split is the one
change that made a module worse rather than better; it introduced the only
import cycle in the frontend.

---

## 0. Method and tools

Two graphs, three ways of cutting each.

| Tool | What it produced | Verdict |
|---|---|---|
| **`cargo-modules` 0.26** (scratchpad install, not added to any manifest) | Item-level `use` graph per crate, as DOT. 964 nodes, 1 397 use-edges across seven crates. Also `--acyclic`. | The primary Rust source. Resolves `pub use` re-exports, visibility and `cfg` correctly — a text scan does not. |
| **`dependency-cruiser` 18** | Frontend module graph, 140 files. Run twice: default, and with `--ts-pre-compilation-deps`. | Primary frontend source. **The default run is wrong for this question** — see §5. |
| **`madge` 8** | Cross-check for cycles only. | Found the cycle depcruise's default missed, out of the box. |
| **`ts-morph` 28** | Per-symbol export/import resolution: which of a file's exports each importer actually names. | The measurement that answered the question. depcruise and madge give edges; only symbol resolution gives *width*. |
| `cargo-depgraph` | Not used. | Crate-level only. Seven crates whose edges are already in `Cargo.toml`; nothing to learn. |
| `git log --name-only` | Co-change and per-file churn over 111 commits. | Weak — the repository is three days old and the maximum churn on any source file is 12. Used only to confirm, never alone. |

Nothing was added to `Cargo.toml`, `package.json` or `Cargo.lock`; the four tools
were installed into a scratch directory and run from there.

**Measured against the working tree at `9e497cb8` + uncommitted changes**, not a
clean checkout, because several agents were mid-edit. Two figures move as a
result: `backend/embedded/mod.rs` crossed 500 lines during the audit, and
`lib/ipc.ts` gained `setAllowScreenshots` — which is itself the point of §2.2.

### The four measurements

1. **Public surface actually used.** For each barrel, the set of names any
   external consumer *names* — not the set it exports.
2. **Fan-in / fan-out** at module and directory level.
3. **Consumer partition.** For a surface of *n* items and *m* consumers, does the
   bipartite graph split into disjoint blocks? A surface whose consumers
   partition cleanly is *k* modules wearing one name; a surface whose consumers
   all touch all of it is one module.
4. **Blast radius per behavioural change** — how many modules one feature commit
   had to touch.

---

## 1. What the measurements say

### Rust barrels are thin

Every `mod.rs` under audit declares its submodules private (`mod x;`), so
external consumers cannot name them. That makes the barrel the whole surface —
and the surfaces are small:

| Barrel | External consumer files | Names they use |
|---|---|---|
| `copypaste-ui::backend` | 17 | **4** — `Backend`, `BackendError`, `SelectedBackend`, `Result` |
| `copypaste-core::storage` | 7 | **6** |
| `copypaste-cloud::sync` | 5 | **7** of 16 exported |
| `copypaste-p2p::peers` | 6 | 2 |
| `copypaste-daemon::server` | **0** | — (`pub(crate)`, reached only from `main`) |
| `copypaste-ui::commands` | **0** | — (named by path from `lib.rs` alone) |

`cargo-modules` agrees at module level: `core::storage` is the widest node in the
tree at fan-in 19 / fan-out 7, and it exposes eight items.

**There are no cycles.** Tarjan over the `cargo-modules` use-edges finds zero
SCCs of size > 1 and zero two-cycles in all seven crates. (`--acyclic` reports
failures, but that is the parent-owns-child edge; the `use` graph alone is
acyclic.)

So: a re-export cap would flag nothing in Rust. **The width in this tree is not
in the barrels. It is inside single exported items.**

### The width is inside four items

| Item | Surface | Consumer files | Consumers partition? |
|---|---|---|---|
| `copypaste-daemon` `AppState` (`main.rs`) | 18 fields + 21 methods | **17** | partly — three blocks |
| `copypaste-core` `Store` | 42 `pub fn` across 11 files | 22 | yes — two blocks |
| `copypaste-ui` `Backend` trait | 25 methods | 11 | **yes — five disjoint blocks** |
| `lib/ipc.ts` | 64 exports | 47 | **yes — eight blocks, 42/48 sites in exactly one** |

Each is one name. No export count, in any language, can see any of them.

### Blast radius: features are vertical, modules are horizontal

Distinct modules touched per commit (111 commits, sweeps over 25 files excluded):

| modules touched | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| commits | 20 | 13 | 10 | 6 | 3 | 1 | 3 | 1 | 2 | 2 | 1 |

`642e77f6` "Page the history by cursor, not by offset" is the clean specimen:
one behavioural change, 20 files, 8 modules. It edited the *same concept* — a
page of items — in four separately-declared types:

```
copypaste_ipc::ItemPage   →  backend::Page   →  model::UiPage   →  ipc.ts ItemPage
```

`backend::Page` is field-for-field identical to `copypaste_ipc::ItemPage`
(`items`, `skipped_undecryptable`, `next_cursor`) with a `From` impl and no added
behaviour. `UiPage` earns its place — it is where a sensitive item's plaintext is
dropped. `backend::Page` does not, and `backend/mod.rs` declares
*"in terms of `copypaste_ipc` types … so there is no second set of DTOs
(CLAUDE.md rule 1)"* thirty lines above defining one. **Reported, not fixed.**

---

## 2. Ranked: the modules that are genuinely too broad

### 1. `crates/copypaste-daemon/src/main.rs` — 729 source lines, invisible to the gate

**`scripts/check-file-size.sh` reports this file at 38 lines.** It truncates at
the first `^#[cfg(test)]`, which at line 39 is `#[cfg(test)] mod testutil;` — a
module *declaration*, not the test module. The real test module starts at 730.
It is the only file in the tree affected, and it is the largest production file
in the tree. `scripts/check-comments.sh` has the same truncation and measures
main.rs's comment ratio over the same 38 lines. **This is a defect in the gate,
reported here and not fixed.**

Behind the blind spot, seven responsibilities:

| | lines |
|---|---|
| `clap` argument parsing (`Args`) | 72–122 |
| cloud deployment resolution | 123–140 |
| **`AppState`** — 18 fields, 21 methods | 141–405 |
| v0.4 database detection | 406–436 |
| `main()` — startup, task supervision | 437–665 |
| tracing init, halt-or-fail, socket removal, shutdown | 666–729 |

`AppState` is reached from **17 files**, one in every daemon subsystem, and 23 of
its 38 distinct member names are used externally. Because it lives in `main.rs`,
every one of those files transitively depends on the file that also holds
`main()` and the CLI parser. That is why `main.rs` is joint second in churn
across the whole repository — 10 of 111 commits, behind only
`copypaste-ipc/src/lib.rs` at 12 — for reasons that have nothing to do with each
other.

**The seam — three of them, in order of value.**

*First,* move `AppState` out of `main.rs` into `state.rs`. Consumers stop
depending on the entry point. This is behaviour-preserving and separates the
17-file fan-in from the startup code entirely.

*Second,* the consumer partition says `AppState` is three handles, not one:

| Handle | Members | Consumers that need only this |
|---|---|---|
| **change bus** | `events`, `publish`, `note_local_change`, `note_remote_change`, `note_peers_changed`, `note_capture`, `note_sensitive_swept`, `subscribe` | `p2p/handlers.rs`, `p2p/poll.rs`, `p2p/mod.rs`, `cloud/poll.rs` — four files that never touch `store`, `keyring` or `detector` |
| **data context** | `store`, `keyring`, `detector`, `settings`, `clipboard`, `db_path` | `capture.rs`, `sync.rs`, `server/transfer.rs`, `server/dbadmin.rs` |
| **process lifecycle** | `ready`, `shutdown`, `backend_name`, `capture_running`, `legacy_history`, `started_at`, `counters` | `server/dispatch.rs` — uses exactly `is_ready`, one member of thirty-eight |

`server/listener.rs` is the one file that straddles two blocks
(`request_shutdown` and `subscribe`), which is what a socket server should do.

Passing a change-bus handle to the four peer/cloud files removes their dependency
on the store, the keyring and the detector outright — the largest single
reduction available in the daemon.

*Third,* `main()` at 229 lines is its own extraction (`startup.rs`), but it has
one consumer and is the least valuable of the three.

### 2. `crates/copypaste-ui/src/lib/ipc.ts` — 64 exports, eight disjoint consumer groups

The widest surface in the repository, by a factor of three over the next
(`lib/layout.ts`, 23). 47 files reference it across 48 import sites — 36 of them
production code. 19 files import at least one value; the other 28 name types
only.

The consumer partition is as clean as this measurement ever comes out. Of 48
import sites, **42 name symbols from exactly one section**:

| Section | Importing files (incl. tests) | Example — everything that file takes |
|---|---|---|
| history | 20 | `components/history/*`, `hooks/useHistory`, `useReveal`, `useSelection` |
| capture | 9 | `components/capture/*`, `hooks/useCapture`, `lib/capture` |
| devices | 6 | `components/devices/*`, `hooks/useDevices` |
| service | 7 | `shell/ServiceOffline`, `PairCreateDialog` |
| config | 3 | `settings/ServiceTab`, `hooks/useServiceConfig` |
| transfer | 2 | `settings/StorageTab` |
| shortcut | 1 | `settings/ShortcutTab` |
| status | 9 | cross-cutting — `StatusData`, `getStatus`, `hasBridge` |

The six sites that span more than one section are five hooks and the test
harness, and four of those span only because a type (`Item`, `StatusData`) is
declared in the barrel.

Seven exports are imported by nothing at all, including tests:
`addItem`, `getDefaultShortcut`, `captureDisarm`, `CaptureRung`,
`CapturedPayload`, `NotGrantedReason`, `NotWorkingReason`. Several are documented
in the file as "not routed yet". The surface is wider than the product.

**The seam.** Split by consumer, not by size: `lib/ipc/history.ts`,
`ipc/devices.ts`, `ipc/capture.ts`, `ipc/service.ts`, `ipc/config.ts`,
`ipc/transfer.ts`, over the existing `ipc/call.ts` gateway, with the shared types
(`Item`, `ItemPage`, `StatusData`, `CURRENT_PROTOCOL_VERSION`, `hasBridge`) in
`ipc/types.ts` that all six import and none re-export.

**And delete the barrel.** Keeping `ipc.ts` as a re-export point is what makes
the split invisible: `components/history/HistoryRow.tsx` would still declare a
dependency on `pairAccept` and `restoreDatabase`. The import sites change from
`@/lib/ipc` to `@/lib/ipc/history` — a mechanical edit at 48 sites, once.

### 3. `backend::Backend` — a 25-method trait whose consumers already partition

The trait is one exported name, so no export count sees it. Its 11 consumers
partition perfectly, and the partition is already written into the filenames:

| Consumer | Methods it uses | Section |
|---|---|---|
| `commands/history.rs` | 9 | exactly the 9 history methods |
| `commands/peers.rs` | 8 | exactly the 8 peer methods |
| `commands/transfer.rs` | 4 | exactly the 4 transfer methods |
| `commands/config.rs` | 2 | exactly the 2 settings methods |
| `commands/status.rs`, `service/mod.rs`, `commands/diagnostics.rs` | 1 each | `status` |
| `service/push.rs` | 1 | `watch` |
| `capture/intake.rs`, `shell/tray/*` | 2 each | `add`+`status`, `list`+`status`, `copy` |

`backend/embedded/` is *already* split along the same line — `peers.rs`,
`rows.rs`, `transfer.rs`. The trait is the only thing in the crate not split that
way.

**The seam.** Five traits — `HistoryBackend`, `PeersBackend`, `TransferBackend`,
`SettingsBackend`, `StatusBackend` — with `SelectedBackend` implementing all
five. The ADR-0002 property that motivates the trait (one declaration both
platforms answer to, so drift is a build error) is preserved exactly: five
declarations, both implementations, same compiler enforcement.

What it buys: `commands/history.rs` stops depending on the peer surface, so
changing `pair_accept` no longer recompiles and re-tests the history commands,
and — the reason that matters — a reader of `commands/peers.rs` no longer has to
establish that it does not touch the store.

**Cost to state honestly:** five `impl` blocks instead of one on each backend,
and `#[tauri::command]` handlers must name the trait they need. This is a
medium-value change, not an urgent one.

### 4. `copypaste-core::storage::Store` — 42 methods, two concerns

The directory split (14 files) is good and the barrel exposes six names. The
`Store` type itself is two objects:

| Concern | Methods | Files that use *only* this |
|---|---|---|
| clipboard history | `items` (7), `page` (2), `pinning` (2), `search` (4), `retention` (3) | `ingest.rs`, `sensitive/purge.rs`, `sensitive/wipe.rs`, `transfer/export.rs`, `transfer/import.rs`, `server/items.rs`, `backend/embedded/rows.rs` |
| sync metadata | `state` (6), `versions` (6), `identity` (4) | `daemon/meta/mod.rs` (**11 methods, zero from history**), `core/sync/source.rs`, `backend/embedded/peers.rs` |

`daemon/src/meta/mod.rs` — the module both transports share — uses eleven `Store`
methods and touches nothing about clipboard items. That is the seam, and it is
already named: `Meta`.

**The seam.** `Store` keeps the history methods; the state/versions/identity
methods move behind a `MetaStore` handle over the same connection pool. This is
the lowest-risk of the four (it moves `impl` blocks that already live in separate
files) and the lowest-value, because `Store`'s consumers are all inside two
crates. Ranked last for that reason.

---

## 3. What is fine, and worth saying so

- **`copypaste-daemon/src/server/`** — the "daemon dispatch" candidate. Not a god
  module. Nine files, `pub(crate)`, **zero** external consumers by path; the
  barrel re-exports three names. `dispatch.rs` is a `match` on a typed enum with
  fan-out 4. The mod.rs header describing the request path is the one layout
  table in the tree that earns its place, because the files are ordered by a
  pipeline rather than by directory listing.
- **`copypaste-cloud/src/sync/`** — 10 files, 16 exports, 7 used, 5 consumers,
  fan-out concentrated in `driver.rs`. Orchestration with a narrow face. Fine.
- **`backend/embedded/`** — became a directory today and split along the same
  line the `Backend` trait's consumers do. That split was on the right axis.
- **`copypaste-ui/src-tauri/src/commands/`** — ten files, zero cross-imports,
  every file the sole consumer of one trait section. The most cohesive directory
  in the repository.
- **settings/config mapping** — one `ConfigData` in `copypaste-ipc`, one
  `ConfigPatch`, one `Liveness`, used by 18 files. High fan-in, zero fan-out,
  no duplicate model. This is a shared value type, which is the opposite of a
  god module. Fine.
- **`crates/copypaste-ui/src/lib/`** — 13 unrelated files (`accelerator`,
  `banners`, `cn`, `errors`, `format`, `layout`, `pairing`, `theme`, `view`, the
  ipc trio). By directory it is a grab-bag. **It has no `index.ts`**, so the file
  is the module and every consumer's declared dependency is honest:
  `lib/cn` fan-in 28, `lib/errors` 23, `lib/pairing` 3, `lib/accelerator` 1.
  It is the counter-example that settles the question — the same directory
  contains the tree's healthiest surface and its widest, and the difference is
  entirely whether there is a barrel.

---

## 4. Verdict on the `lib/ipc.ts` split

**It was made on the wrong axis, and it made the module worse.** Stated plainly
because the instruction that produced it asked for exactly this answer.

What happened: 498 lines went to 397 + 134 + 31 by lifting out the Android
capture surface and the `invoke` gateway, and both were re-exported through
`ipc.ts` "so `@/lib/ipc` remains the single import." The file-size number
improved. Nothing else did.

Three measurements:

1. **The public surface did not narrow.** 64 exports before, 64 after — the
   re-export block at the bottom of `ipc.ts` is what holds it there. 47 files
   still name the barrel.
2. **A real module was created and then hidden.** `ipcCapture.ts` has 17
   exports and a coherent consumer set: nine files, all under
   `components/capture/`, `hooks/useCapture` and `lib/capture`. Its *measured*
   fan-in is **1** — the barrel. The one file in the split that was cut on the
   consumer axis is the one the re-export conceals.
3. **It introduced the only import cycle in the frontend.**

```
lib/ipc.ts  --export-->  lib/ipcCapture.ts  --import type { Item }-->  lib/ipc.ts
```

`madge --circular` reports it: *"Found 1 circular dependency."* Zero elsewhere in
140 files. The cause is mechanical and diagnostic: the extraction moved the
capture functions out but left `Item` behind in the barrel, so the extracted file
has to reach back. A split on the consumer axis would have put `Item` in a shared
types module and no edge would exist.

`ipcCall.ts` is a different case and is **right**. It has one job — be the only
place `invoke` is called — one importer per surface, and it is a leaf. It would
survive the split in §2.2 unchanged.

So: the capture extraction was the correct *content* moved for the wrong
*reason*, stopped one step short. Finish it — `lib/ipc/capture.ts` importing
`lib/ipc/types.ts`, no re-export, the nine consumers importing it directly — and
the cycle disappears along with the barrel.

---

## 5. Whether any mechanical check is worth adding

The repository already carries a file-size gate, a comment-budget gate, a
commit-message gate, a design-contrast gate, a design-usage gate and 244 release
checks. The bar for a seventh is that it catches something those six missed.

### Checks considered and rejected

| Candidate | What it would flag today | Why not |
|---|---|---|
| **Cap on public re-exports per barrel** | `daemon/cloud/mod.rs` (24), `ipc/lib.rs` (21) in Rust — and **`lib/ipc.ts` at 18**, level with `daemon/meta/mod.rs` | **It ranks the worst module in the tree third.** `ipc.ts` re-exports 18 names and *declares* the other 46, so a re-export cap sees the smaller half. Meanwhile `ipc/lib.rs` is the wire contract, where width is the purpose. And three of the four findings in §2 are *one exported name*: a 25-method trait, a 42-method struct, a 39-member struct. |
| **Fan-in threshold** | `i18n/index.ts` (50), `lib/cn.ts` (28), `ui/button.tsx` (25), `core::storage` (19), `ui::backend` (15) | Every one is correct. High fan-in on a stable, narrow item is the shape you want; it is indistinguishable by fan-in alone from the shape you do not. |
| **Method-count cap on a type or trait** | `Backend` (25), `Store` (42), `AppState` (18 fields + 21 methods), `Method` (30 variants) | This one *would* see the real findings — and would also demand splitting `Method`, which is the enum whose exhaustiveness is the entire argument for typed dispatch in `docs/rewrite/target-architecture.md`. A check whose first action is to break the crate's design premise is a check that gets a baseline file and then gets ignored. |
| **Consumer-partition score** (the measurement that actually worked) | `ipc.ts`, `Backend`, `Store`, `AppState` — all four, and nothing else | No maintained tool computes it; it needs `ts-morph`/`cargo-modules` plus a bespoke bipartite analysis, which is a script to maintain — exactly the rule-1 failure applied to governance. And a clean partition is evidence, not a verdict: `commands/` partitions perfectly and is *correct*. A human has to say which. |

### What to do instead

**1. Fix the existing gate. This is not a new check — it is a bug.**
`scripts/check-file-size.sh` and `scripts/check-comments.sh` both truncate at the
first line beginning `#[cfg(test)]`, which catches `#[cfg(test)] mod testutil;`.
Truncate at the *last* such line, or at one followed by `mod tests {`. Today that
turns `daemon/src/main.rs` from a reported 38 lines into a reported 729 — the
largest production file in the tree, currently invisible. One file affected now;
the pattern (`#[cfg(test)] mod testutil;` near the top) is idiomatic and will
recur.

**2. `madge --circular`, and only if the cycle is fixed first.**

This is the one candidate that clears the bar. It would have caught something the
other six missed, that landed today, in the module this audit was asked about.
The honest accounting:

- *Cost:* one devDependency (`madge`, ~3 MB, no transitive risk), one line in the
  existing frontend CI job. Not a new script, not a new baseline file, no new
  entry in `scripts/`.
- *Yield today:* exactly one finding.
- *Precondition:* fix the `ipc.ts` ↔ `ipcCapture.ts` cycle first, so the check
  lands at **zero with no baseline**. A gate that ships with a baseline file is
  how the comment budget acquired 160 exemptions, and a cycle check with an
  exemption list is worth nothing.
- *Do not* add the Rust half. `cargo modules --acyclic` would flag nothing —
  all seven crates are already acyclic — and it is a 15-minute build.

If the cycle is fixed and nobody wants the dependency, that is a defensible
answer too: one cycle in three days is a low base rate, and this one was found by
reading the file.

**3. Nothing else.** The four modules in §2 were found by measurement a person
did once, on a question a person asked. That is the right tool for this, and
running it again after the next big split costs an afternoon and no permanent
maintenance.

### On rule 5 itself

The line count did its job — it is why `server/`, `commands/`, `storage/` and
`backend/embedded/` are the shape they are, and those are genuinely good. Keep
it, and keep the number where it is.

What it needs is not a stricter number but one sentence, because the failure this
audit found is a *split that satisfied the rule while making the module worse*:

> **Splitting is a change of consumers, not of line counts.** Before splitting,
> name which consumers each new file will serve. If the answer is "the same ones,
> through a re-export", the split has not happened yet.

That sentence, plus the bug fix in §5.1, is the whole recommendation.

One inconsistency noticed in passing: `scripts/check-file-size.sh`'s header says
*"Advisory, not a gate"*, while `.github/workflows/ci.yml:105` says *"A gate, not
an annotation"* and fails the job. The workflow is right and the script header is
stale — which matters more than it reads, because a hard gate is what is
currently passing a 729-line file.

---

## 6. Defects found, not fixed

1. **`scripts/check-file-size.sh` and `scripts/check-comments.sh` truncate at the
   wrong `#[cfg(test)]`.** `crates/copypaste-daemon/src/main.rs` is reported at
   38 source lines; it is 729. The first is a hard CI gate, so this is a gate
   passing a file 229 lines over its budget. §5.1.
2. **`lib/ipc.ts` ↔ `lib/ipcCapture.ts` import cycle**, introduced by the split
   in `642e77f6`. `madge --circular` reproduces it; `depcruise` does not without
   `--ts-pre-compilation-deps`, because its default analyses post-transpile
   output where `import type` has been elided. §4.
3. **`backend::Page` is a fourth model of a page of items**, field-identical to
   `copypaste_ipc::ItemPage`, declared 30 lines below a module header asserting
   that no second set of DTOs exists. `crates/copypaste-ui/src-tauri/src/backend/mod.rs`.
   CLAUDE.md rule 1. §1.
4. **52 of 364 frontend exports (14%) are imported by nothing**, tests included.
   Seven of them are in `lib/ipc.ts`. Not urgent; it is the measure of how much
   wider the surface is than the product.
