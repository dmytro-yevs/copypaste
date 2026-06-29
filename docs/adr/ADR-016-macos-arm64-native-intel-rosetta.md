# ADR-016: macOS Ships arm64-Native; Intel Macs Run Under Rosetta 2

## Status

Accepted

Date: 2026-06-29
Track: v0.3.x — platform / CI scope decision (CopyPaste-crh3.62)

## Context

Every macOS CI runner in this repository is `macos-14` (Apple silicon /
arm64). `release.yml`'s `build-macos` job builds **only**
`aarch64-apple-darwin`, even though `scripts/build-macos.sh` supports a
universal (`lipo` arm64 + x86_64) build. There is no `macos-13` (Intel)
runner in either `ci.yml` or the release matrix.

The practical consequences:

- The shipped macOS binary is arm64-native. On an Intel Mac it runs
  under **Rosetta 2** — Apple's transparent x86_64 translation layer and
  the standard path the broader ecosystem relies on for arm64-only
  binaries.
- Two areas carry platform-specific native code that is therefore
  exercised only on arm64 in CI:
  - `rusqlite` with `bundled-sqlcipher` (a C build of SQLCipher), and
  - mDNS-SD service discovery in `copypaste-p2p`.
  These *could* in principle behave differently when the same binary is
  translated by Rosetta 2, but in practice Rosetta 2 is a faithful
  x86_64 emulator and SQLCipher/mDNS make no arm64-specific assumptions
  in our usage.

We considered three options:

1. **Add a `macos-13` (Intel) job** to the test matrix and ship a
   universal binary. Cost: roughly doubles macOS CI minutes and release
   build time (a second native compile + `lipo`), for a user base that
   is small and shrinking (Apple shipped its last Intel Mac in 2023) and
   already covered transparently by Rosetta 2.
2. **Ship a universal binary** (arm64 + x86_64 via `lipo`) without a
   native Intel test runner. This removes Rosetta 2 from the runtime
   path but still leaves the x86_64 slice **untested** natively — so it
   buys runtime nativeness without buying test confidence, at full build
   cost.
3. **Document the arm64-native + Rosetta-2 stance** and revisit if real
   Intel-specific bug reports arrive. This is the lowest-cost option and
   keeps CI fast; the risk it accepts (an undetected x86_64-under-Rosetta
   divergence) has not materialised.

## Decision

Adopt option 3. macOS releases ship **arm64-native** binaries built and
tested on `macos-14`. Intel Macs are a **supported-via-Rosetta-2**
configuration: the same arm64 build runs under Rosetta 2; we do **not**
build a universal binary and do **not** run a native `macos-13` job by
default.

This mirrors the project's existing platform posture (arm64 macOS is the
primary target; see `CLAUDE.md` "Platform Support") and the cost-driven
scope decisions already recorded for Windows (ADR-012).

## Consequences

- macOS CI stays single-arch (`macos-14`), keeping release and PR build
  times low.
- Intel-Mac users are covered by Rosetta 2 with no native test signal.
  If an Intel-specific defect is reported in SQLCipher bundling, mDNS-SD,
  or elsewhere, the mitigation path is pre-decided and cheap: flip
  `build-macos.sh` to its universal (`lipo`) build in `release.yml` and
  add a `macos-13` job to the matrix — both already supported by the
  script; only the workflow wiring is missing.
- This ADR is referenced from a comment on the `build-macos` job in
  `.github/workflows/release.yml` so the arm64-only target is an
  explicit, documented choice rather than an oversight.

## Revisit triggers

- A credible volume of Intel-Mac-specific bug reports.
- Rosetta 2 deprecation or removal in a future macOS release.
- A dependency adding documented arm64-vs-x86_64 behavioural differences
  relevant to our use (crypto, storage, networking).
