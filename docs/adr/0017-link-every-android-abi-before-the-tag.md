# ADR-0017 — Link every Android ABI before the tag

**Status:** accepted · 2026-08-08
**Scope:** the `link-abis` job in `.github/workflows/android-emulator.yml` and
`scripts/release/android-link-abis.sh`.

## Decision

Link `libcopypaste_ui_lib.so` for all four Android ABIs on every push and pull
request that touches Android, with the linker strictness the APK link has.

## The gap

The universal APK is the only artifact that links `armv7`, `i686` and
`aarch64`. Only `release.yml` builds it, and only on a pushed tag. Every other
Android job in this repository is `x86_64`, because that is the emulator's ABI.

So the release path had a stretch that nothing exercised except publishing, and
v2.0.0-alpha.5 failed on it twice, in public, on two defects that had never been
reachable by any check:

1. `sha2-asm`'s 32-bit x86 assembly is not position-independent, and Android
   links every `.so` with `-fPIC`: `R_386_32 cannot be used against local
   symbol`.
2. Under that, vendored OpenSSL's `threads_pthread.c` needs `__atomic_*_8`,
   which only `i686` resolves out of line and which nothing on the link line
   provides — ADR-0007 has the detail.

## Why a cargo link and not an APK

Measured, not assumed. Both defects land at the final `.so` and neither at
crate level: `sha2-asm` compiles clean and fails when its object reaches
`ld.lld`. So the cheap half of the release build — `cargo build --release
--target <triple> --lib`, no Gradle, no R8, no aapt2, no emulator — is where
both are visible.

**But the exit code alone is not enough, and that is the part worth writing
down.** With defect 2 reverted, a plain `cargo build` **succeeds**: it emits a
`.so` carrying seven unresolved `__atomic_*` symbols that would fail at
`dlopen` on the device. `-Wl,--no-undefined` is what turns that back into the
release job's own error, so the script passes it. A guard that only checked
`cargo`'s exit status would have caught defect 1 and shipped defect 2.

| | plain `cargo build --lib` | with `-Wl,--no-undefined` |
|---|---|---|
| `sha2-asm` `R_386_32` | fails | fails |
| OpenSSL `__atomic_load_8` | **exit 0, broken `.so`** | fails |

The flag is not invented strictness. Defect 2 was a *build* failure in the
release job, which is only possible if that link already has it.

## Why here, and not the other two places

`android-emulator.yml`'s path filter is already the list of things that can
break Android, and it already covers both defects' source files:
`crates/copypaste-core/**` held the `sha2` feature gate,
`crates/copypaste-ui/src-tauri/**` holds `build.rs`, and `Cargo.lock` and
`rust-toolchain.toml` are the other two ways a toolchain defect arrives. The job
therefore gates the merge that would introduce one, and inherits the nightly as
well.

**`ci.yml`** would need a third hand-maintained copy of that path list — the
note above the existing two says GitHub rejects YAML anchors and they have to be
kept identical by hand, so a third is a standing bug — plus its own NDK install.

**A release dry run** is worse on the only axis that matters: it answers after
the merge rather than before it, and spends Gradle, R8 and aapt2 to re-ask a
question the cargo link already answers.

**The nightly alone** is worse for the same reason — up to a day late, on
`main`, after the change landed. That is the argument this workflow already made
for itself when it started gating pushes.

## What it costs

Measured with cargo capped at four jobs, to stand in for a 4-vCPU runner. The
host is faster per core than one, so read these as a floor rather than a
forecast:

| | wall | CPU |
|---|---|---|
| cold, no cache | 10m15 | 21m30 |
| warm, nothing changed | 4m18 | 4m04 |
| warm, `build.rs` touched | 4m18 | 4m05 |

There is no "nothing to do" case: the Tauri build script re-runs and the four
`.so` are relinked every time, so four minutes is the floor whatever the cache
holds. It runs in parallel with `apk`, whose 75 minutes are dominated by the
OWASP audit, so it spends runner-minutes rather than wall-clock — and the same
75-minute timeout has room, since even fully serialised the cold run is 21m30.

The rust-cache entry is its own key: four target triples sharing one with a
single-ABI job would overwrite each other's save every night.

Cost is a trade to state, not an exemption (CLAUDE.md rule 1). Four ABIs of Rust
is the cheapest thing that can answer this question at all, it is far below four
ABIs of APK, and it is far below a release that breaks in public.

## Proof it catches the defect it guards against

Both fixes were reverted one at a time against the delivered script, on
`i686-linux-android`:

| | script exit | what it printed |
|---|---|---|
| both fixes present | 0 | `linked …libcopypaste_ui_lib.so` |
| `391b0f2f` reverted | 1 | `R_386_32 cannot be used against local symbol` |
| restored | 0 | linked |
| `9832aa03` reverted | 1 | `undefined symbol: __atomic_load_8` (and five more) |
| restored | 0 | linked |

## Consequences

- Adding an ABI means adding it to `TRIPLES` in `android-ndk-env.sh`;
  `check-wiring.py` already holds that list equal to what the jobs install.
- The job asserts nothing about the APK — packaging, signing and R8 stay the
  release workflow's and the emulator legs' business. It answers one question.
- A dependency whose build breaks only on a non-`x86_64` ABI now fails on the
  pull request that adds it, which is the whole point.
- `-Wl,--no-undefined` will fail a genuinely new undefined symbol too. That is
  correct for a shipped `.so` and is not a false positive; the fix is to put the
  provider on the link line, as ADR-0007 does.
- Unmeasured, and worth watching: the target directory reaches 3.7 GB across the
  four triples. `rust-cache` prunes to dependencies before saving, but GitHub's
  per-repository cache budget is shared with this repository's four other keys,
  so this entry may start evicting them.
