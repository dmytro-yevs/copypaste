# ADR-0026: Bound the primary checkout's target directory against cargo's own unit set

Status: accepted

## Decision

`scripts/target-budget.py` records which compilation units a configuration
actually uses and removes generations no recorded configuration named. It is a
dry run unless given `--apply`, and nothing invokes it on a schedule.

The live set comes from cargo, not from file times:
`cargo build --message-format=json` reports every unit it considered, fresh ones
included, so a no-op build enumerates the live set for the price of a
fingerprint check. On this workspace it is exact — 696 `compiler-artifact` plus
87 `build-script-executed` messages against exactly 783 generations on disk.

Two refusals, both fail-closed:

- **`target/CACHEDIR.TAG`** ([spec](https://bford.info/cachedir/)) must carry
  the 43-byte signature. Gradle's `build/` has no tag, which is what makes this
  a discriminator rather than a name check.
- **`.cargo-lock`** is probed wherever it is found under `target/`, never at a
  named profile. Hardcoding `debug` and `release` reports "idle" during a
  `--target x86_64-linux-android` or `--profile evidence` build.

A generation newer than the newest mark is kept regardless: output made after we
last asked cargo is work we have no knowledge of.

## The premise this replaces

`[profile.dev]` records that `target/debug` reached 22 GB and blames "a test
binary per code revision". Measured on 2026-08-15, that mechanism does not
exist. Six code revisions in an isolated tree, rebuilding 19 units each time,
moved the directory 0.35 GiB on the first and then held it flat to within 141
bytes:

```
base 4.66 GiB   1 5.01   2 5.00   3 5.00   4 5.00   5 5.00   6 5.00 GiB
```

Cargo's `-Cmetadata` covers package name, version, features, profile, dependency
hashes, target triple and rustc version — not source text — so an edit
overwrites in place and cannot mint a generation. What accumulates is a
generation per *configuration*: version bumps, `Cargo.lock` changes, differing
feature sets between gates, profile and toolchain changes. Six workspace version
bumps grew the same tree from 4.66 GiB to 15.75 GiB, about 1.57 GiB each.

`line-tables-only` therefore was not an outgrown partial fix, and this is the
part to keep: **it shrinks each artifact, and the problem is how many artifacts
are retained.** No size knob can reach that, so the next one will fail the same
way. The measured distribution says the same thing — the mass is not in fat
artifacts but in ordinary ones, many times over:

| `target/debug/deps` | files | size |
|---|---|---|
| over 50 MB | 42 | 3.67 GiB |
| 10–50 MB | 451 | 8.36 GiB |
| 1–10 MB | 2,690 | 8.03 GiB |

15,106 files across 1,270 package stems is **11.9 artifacts per stem**, and
`build/` holds 904 directories across 93 stems, **9.7 per stem**. A single
configuration needs about 1.6. The mitigation stays in force for the reason it
was added; it was never going to bound this.

## Rule 1 exemption 1 — no maintained package provides the behaviour

[`cargo-sweep`](https://github.com/holmgr/cargo-sweep) is the closest fit and
was rejected on measurement. Its README opens "cargo-sweep is currently
unmaintained"; last commit 2026-05-26. Decisively, `src/fingerprint.rs`
selects on `metadata().accessed()`, so `--time`, `--maxsize` and
`--stamp`/`--file` all rest on access time. Access time does not survive here:
14,898 of 15,106 files under `target/debug/deps` shared a single atime minute,
because reading a file's content updates it and something scans the tree.
Enumerating a directory does not. An atime-based selector consequently sees the
whole 46 GiB as used moments ago and reclaims nothing.

[`cargo-clean-all`](https://github.com/dnlmlr/cargo-clean-all), which
cargo-sweep's README recommends, was last pushed 2025-04-19 and solves the other
problem: sweeping many target directories, which is ADR-0025's.

Cargo itself has no target GC. `-Z gc` exists in 1.97.1 but the
[Cargo Book](https://doc.rust-lang.org/cargo/reference/unstable.html#gc) scopes
it to "cargo's global cache within the cargo home directory".
[rust-lang/cargo#6229](https://github.com/rust-lang/cargo/issues/6229) is closed
as a duplicate of [#13060](https://github.com/rust-lang/cargo/issues/13060),
which is open, `S-needs-design`, and treats target-dir GC as a future question.
`cargo clean` offers no age or size selection.

Removal and size accounting come from the standard library.

## The trap: do not group generations by package name

**Anyone writing a target cleaner reaches for "keep the newest N per crate"
first. Against real data it deletes live output.**

One package legitimately holds many live hashes at once. In a single clean build
`copypaste-ipc` has four — the lib, its test binary, and the `wire_roundtrip`
and `method_contract` integration tests — and `getrandom` has eleven across the
host and target graphs. A "keep the newest N per crate" rule was written first
and, against real data, would have deleted 136 generations and 1.40 GiB of
entirely current output after one clean build, forcing exactly the rebuild this
work exists to avoid. The unit hash is the only identity that separates them.

## Consequences

The retained set is the union of the marked configurations, which is bounded by
how many are marked rather than by how long the session runs. That is the bound.

**The churn this costs**, measured rather than argued. Sweeping a tree grown to
15.04 GiB reclaimed 10.74 GiB and cost one build of 4 units in 10.9 s; the build
after that recompiled nothing. Across six further waves the tree stayed at
3.69 GiB and 781 generations instead of growing 1.57 GiB a wave, with 27 units
recompiled per wave either way.

A configuration that was never marked pays a full rebuild, and configurations
overlap far less than they look. `cargo build --workspace --tests` and
`cargo clippy --workspace --all-targets` share 164 unit hashes of 781 and 859:
marking only the first sweeps 695 of the second's units. Mark every gate that
runs, or expect it to rebuild.

Dropping incremental scratch showed no separable build-time cost over six
alternating waves — 10.6/17.1/18.8 s dropped against 18.5/10.4/10.6 s kept, one
machine under uncontrolled load — for a consistent 0.97 GiB. The ranges overlap,
so this claims no direction on time; `--keep-incremental` is there for anyone
whose workload measures otherwise.

Incremental scratch is dropped by default when the marked build did not write to
it. It is keyed by session id, so no mark can name it, and it was the largest
single area measured — 15.82 GiB of 46.13 GiB — so retaining it means the sweep
does not bound anything. Losing it cannot make a build wrong, only slower, and
only for a crate that is not currently being edited.

This owns the primary checkout. ADR-0025 owns worktrees and refuses to touch
this directory; the two do not overlap and neither should grow into the other.
The cargo markers both rely on — `CACHEDIR.TAG` and `.cargo-lock` — live in
`scripts/cargo_target.py`, shared, because two copies of a fail-closed check
drift into one strict and one permissive and the permissive one deletes.

## Measured, and deliberately not addressed here

`scripts/clean-target.sh` removes `target/debug/incremental` and files over
50 MB in `target/debug/deps`. On the measured tree that is 15.82 + 3.67 =
19.49 GiB of 46.13 — a partial reclaim, not a bound, because the 50 MB
threshold sits above where the mass actually is. It also takes no build lock and
has no dry run. Left as it is rather than fixed alongside; whoever needs it
should decide whether it survives `target-budget.py` at all.

All `target/` directories under the WSL `$HOME` total **100 GiB**, against the
58.9 GiB ADR-0025 was written from. That is neither the primary checkout nor a
git worktree — it is sandbox trees agents create outside both mechanisms — so it
belongs to neither ADR and needs its own issue rather than a quiet widening of
this one.
