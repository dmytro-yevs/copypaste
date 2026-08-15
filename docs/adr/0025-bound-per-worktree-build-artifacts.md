# ADR-0025: Bound per-worktree build artifacts with a fail-closed cleaner

Status: accepted

## Decision

`scripts/worktree-hygiene.py` reports and reclaims build artifacts across Orca
and codex worktrees. It is a dry run unless given `--apply`.

Removal is delegated, not reinvented: `shutil.rmtree` does the work, and
`scripts/clean-target.sh` remains the in-checkout reclaim. What this adds is the
decision of *whether* a path may be removed at all.

Two published markers carry that decision instead of a heuristic:

- **`CACHEDIR.TAG`** ([spec](https://bford.info/cachedir/)) — cargo writes one
  into `target/`. A directory is treated as a regenerable cache only when the
  tag's 43-byte signature matches.
- **`target/<profile>/.cargo-lock`** — a build in progress holds it. The cleaner
  tries the lock (`msvcrt.locking` on Windows, `fcntl.flock` elsewhere) and
  preserves the cache when it cannot take it.

Anything else is proved disposable file by file: content is removable only if
`git hash-object` produces a blob that `git cat-file -e` already has.

## Rule 1 exemption 1 — no maintained package provides the behaviour

The removal and the size accounting come from the standard library.
[`cargo-sweep`](https://crates.io/crates/cargo-sweep) was the closest fit and
does not apply: it prunes *inside* one target directory by toolchain and age,
and this problem is deciding which worktrees may be touched at all. It would
also add a binary to install on three platforms.

`git worktree prune` cannot substitute either, and this is measured: after the
2026-08-15 sweep, `git worktree prune --dry-run` reported nothing while nine
leftover directories sat on disk. Prune removes registrations, and a swept
directory has none. Leftovers are therefore found by walking declared roots and
differencing against `git worktree list --porcelain`.

## Consequences

The primary checkout is never a removal target, so the largest consumer stays
out of reach by design — 47.4 GiB of 58.9 GiB when this was written, 46.1 GiB of
it `target/debug`. Bounding that is
[DMY-189](https://linear.app/dmytro-yevs/issue/DMY-189/bound-targetdebug-growth-in-the-primary-checkout),
not this mechanism.

Fail-closed costs reclaim. A leftover holding one unrecoverable file is
preserved whole, which is why `complete-android-e2e` keeps 0.923 GiB: it holds
`Screenshot_1786337929.png`, whose content is in no git object.

The recoverability proof spawns two git processes per file, so directories named
`node_modules`, `build`, `target` and `__pycache__` are skipped as regenerable
rather than hashed.
