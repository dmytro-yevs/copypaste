# ADR-012: Freeze Windows + Homebrew Cask as Sole Distribution Channel

## Status

Accepted

Date: 2026-05-23
Track: v0.3.0-dev — scope decision

## Context

Through v0.2.0-beta the project's stated platform set was macOS,
Android, and Windows (cross-compiled via mingw-w64 in a Linux container,
plus a stub `crates/copypaste-daemon/src/ipc_win.rs` named-pipe IPC
layer and a `platform/windows.rs` shim). In practice, Windows has
carried persistent cost without proportionate value:

- The mingw container (`docker/Dockerfile.windows`) pulls a third-party
  MSYS2 OpenSSL build to satisfy `rusqlite[bundled-sqlcipher]` and is
  the only image that needs Tcl + an out-of-tree `.tar.zst` extraction.
  Cold builds run 8-12 GB RAM and routinely OOM on the standard
  GitHub-hosted runner.
- The Slint UI's Windows backend was never validated end-to-end on a
  Windows host; the existing CI signal was "compiles" via mingw, not
  "runs".
- The daemon's Windows IPC named-pipe implementation
  (`crates/copypaste-daemon/src/ipc_win.rs`) was a skeleton; no
  integration tests exercised the path on a Windows runner.

Concurrently, distribution policy for macOS has been settled by
[ADR-010](./ADR-010-codesigning-ad-hoc.md): ad-hoc signing, no Developer
ID, no `notarytool` round-trip. The repository has not acquired an
Apple Developer account, and there is no committed timeline for doing
so. This rules out Sparkle-style autoupdate (which expects a notarised
binary with a developer-signed feed) and makes direct `.dmg` downloads
fragile because Gatekeeper quarantines them on first launch.

A maintenance-cost audit produced for the v0.3 cut showed Windows
support consuming approximately 30-40% of the cross-platform CI minutes
and weekly maintenance attention for ~1% of the realistically reachable
user base — there is no Windows beta tester, no Windows install
telemetry, and no Windows-tagged issue in the last six months of triage.

## Decision

**Freeze Windows support** for v0.3 and all subsequent releases until a
follow-up ADR explicitly thaws it. Concretely:

1. CI: remove the nightly Windows job and all Windows entries from
   matrix workflows; keep the `docker/Dockerfile.windows` image and
   `scripts/build-windows.sh` script as inert reference material.
2. Build scripts: `build-windows.sh` becomes a no-op shim that exits 0
   with a notice; `build-in-docker.sh windows` and `build-all.sh
   windows` likewise short-circuit with a notice.
3. Docker compose: the `windows` service moves off the `build` profile
   onto a dedicated `frozen-windows` profile so `--profile build` no
   longer brings it up.
4. Source: `crates/copypaste-daemon/src/ipc_win.rs` and
   `crates/copypaste-daemon/src/platform/windows.rs` remain in the
   tree, unchanged, with a freeze header comment. Deletion would
   destroy work that an eventual thaw would need to recreate.
5. Documentation: README, ARCHITECTURE, RELEASE-CHECKLIST, and the v0.3
   plan drop Windows from "supported platforms" and "cut criteria".
6. Threat model: no Windows-specific asset was ever enumerated, so no
   change is required there.

**Adopt Homebrew Cask as the sole distribution channel** for macOS
under the same decision:

1. Apple notarization (`notarytool`, `stapler`) remains out of scope.
2. Sparkle / autoupdate feeds remain out of scope.
3. The DMG continues to be built and ad-hoc signed by
   `scripts/release/build-dmg-ci.sh` + `scripts/release/_sign-and-dmg.sh`
   and is still attached to each GitHub release as a release asset
   (for reproducibility and offline install), but the README directs
   users to `brew install --cask copypaste/tap/copypaste` rather than
   promoting direct-download installation.

## Consequences

Positive:

- Two supported platforms instead of three. CI minute budget drops by
  ~30-40% — that capacity moves to TSAN/MSAN runs and longer fuzz
  cycles instead.
- v0.3 → v1.0 timeline compresses by roughly three weeks because the
  Windows port (T3 in the previous v0.3 plan) is no longer on the
  critical path.
- One distribution channel means one set of trust semantics to explain
  to users, one install path to test, one update path to maintain.

Negative:

- Windows users have no installer. WSL2 is a workable but unsupported
  fallback (the daemon's Unix-socket IPC works inside WSL2; the Slint
  UI does not). This is an acknowledged regression vs. the v0.2-beta
  intent — but not vs. the v0.2-beta reality, which never shipped a
  working Windows binary.
- `ipc_win.rs` and `platform/windows.rs` will gradually bit-rot. The
  freeze header makes this explicit; the thaw ADR will need to budget
  a re-bring-up cost.
- Cask-only means we cannot serve users who refuse Homebrew. Direct
  DMG remains attached to every release as an escape hatch; we do not
  advertise it.

Neutral:

- Android remains unaffected.
- The relay (`copypaste-relay`) is platform-agnostic and continues to
  build and run on any target.

## Alternatives Considered

- **Maintain Windows via WiX/MSI installer.** Rejected: requires a
  Code Signing Certificate ($300-500/yr) and a Windows signing host,
  neither of which the project has.
- **Maintain Windows on a community-supported basis.** Rejected for
  v0.3: no community maintainer has stepped forward and the failure
  mode is "broken nightly with no one to fix it", which is worse than
  an explicit freeze.
- **Acquire Apple Developer ID and ship notarised + Sparkle.**
  Rejected for v0.3: cost is acceptable but onboarding (`$99/yr`,
  Apple account verification, `notarytool` plumbing in CI, Sparkle
  feed hosting) is multi-week and does not align with the v0.3 scope.
  Re-evaluate at v0.4.
- **Drop the DMG entirely and ship only the Cask formula referring to
  a GitHub-released tarball.** Rejected: the DMG carries the
  `.app` bundle layout that Cask consumes; switching to a tarball
  forces every Cask user through a `pkgutil`/`xattr` postinstall and
  fragments the install surface. Cask + DMG is the path of least
  resistance.

## Reversibility

Thaw requires a follow-up ADR (`ADR-NNN-windows-thaw.md`) that:

1. Justifies the thaw (community maintainer, paid Windows VM, or
   acquired tooling budget).
2. Reverts the freeze header in `ipc_win.rs`, `platform/windows.rs`,
   `Dockerfile.windows`, and `build-windows.sh`.
3. Re-adds the nightly Windows job and the v0.3-plan T3 entry.
4. Adds a Windows smoke-install step to the release checklist.

The DMG-only distribution decision is independent and can be revisited
when (a) a Developer ID is acquired or (b) a community-maintained
alternative install surface (e.g. MacPorts) is proposed.
