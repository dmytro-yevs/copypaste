# Release Checklist

Reusable runbook for cutting CopyPaste releases. Applies to every tag:
`v0.2.0-beta.1`, `v0.2.0-beta.N`, `v1.0.0`, and all subsequent tags.

Replace `vX.Y.Z[-pre.N]` below with the target tag for the current cut.

---

## 0. Release artefacts location (read first)

**All release artefacts live in `<repo>/dist/` only.** `target/` is for cargo
intermediates, never for shipped artefacts. Build scripts must output finished
`.dmg` / `.apk` / `.zip` to `dist/` together with a matching `.sha256`.

Canonical filename:

```
CopyPaste-v<full-version>-<platform>-<arch>.<ext>
CopyPaste-v<full-version>-<platform>-<arch>.<ext>.sha256
```

See [`dist/README.md`](../../dist/README.md) for the full convention,
allowed token values, and which build scripts write which artefact.

If you ever find a shipped artefact under `target/release/` or `builds/`,
that is a build-script bug — fix the script, do not copy by hand.

---

## 1. Pre-flight (T-7 days)

- [ ] **Scope freeze.** Confirm the milestone scope with maintainers. No new
      features merged into the release branch after this point — only bug fixes,
      docs, and regression patches.
- [ ] **Branch cut decision.** Decide whether to cut from `main` directly or
      from a dedicated `release/vX.Y.Z` branch. For beta tags the convention is
      a long-lived `release/v0.2.0-beta` branch; for stable tags cut a fresh
      `release/vX.Y.0` branch from `main`.
- [ ] **Milestone check.** All issues tagged with the milestone are either
      closed, deferred to a later milestone, or explicitly accepted as known
      issues in the release notes.
- [ ] **Dependency review.** Inspect any pending dependency bumps; defer
      anything risky to the next cycle.
- [ ] **Open PR sweep.** Triage open PRs targeting the release branch; merge
      or defer.

## 2. Code freeze (T-3 days)

Run every check from a clean checkout of the release branch. Every command
must exit zero. Re-run after every fix until green.

- [ ] `cargo test --workspace --all-features` — zero failures, zero ignored
      tests reactivated for regressions.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean. No
      `#[allow]` added solely to silence release-blocking lints.
- [ ] `cargo audit` — no advisories. Documented exceptions must live in
      `deny.toml` with a justification comment and a tracking issue.
- [ ] `bash scripts/check-adr-format.sh` — exit 0. Every ADR has the required
      front matter and status field.
- [ ] `bash scripts/check-license-headers.sh` — exit 0. All source files carry
      the SPDX header.
- [ ] `bash scripts/find-cycles.sh` — zero cycles in the workspace dependency
      graph.
- [ ] **Manual smoke.** Run the desktop binary locally for at least 10 minutes
      of normal clipboard activity. No panics, no leaks, no zombie processes.
- [ ] **CHANGELOG draft.** Draft the release-notes entry locally so the
      generator output (step 4) can be diffed against it.

## 3. Build verification (T-1 day)

Reproducible release artifacts must build successfully on every supported
target before the tag is cut.

- [ ] **macOS arm64 + x86_64 release build** via `bash scripts/build-all.sh`.
      Verify both architectures appear in the output directory and the
      `release-strip` profile was applied (binaries should be substantially
      smaller than `target/release/` debug-info builds).
- [ ] **Android** via `bash scripts/build-in-docker.sh android`. APK/AAB
      installs on a clean emulator and launches without crashes.
- [ ] **(Windows: SKIP)** Frozen as of 2026-05-23 — see ADR-012. `build-windows.sh`
      is a no-op shim and the nightly/CI Windows jobs are removed. Do not
      reintroduce a Windows build verification step until the freeze is
      lifted in a follow-up ADR.
- [ ] **Binary size check vs baseline.** Compare against the previous tag.
      Record any regression >10% in the release notes and open a follow-up
      issue if not justified by feature work. Confirm the `release-strip`
      profile is in effect for every shipping binary.
- [ ] **SBOM dry-run.** Generate an SBOM locally to confirm the tooling
      succeeds before relying on CI in the next step.

## 4. Tag cut (T-0)

- [ ] `bash scripts/release/cut-tag.sh vX.Y.Z[-pre.N]` — creates the annotated
      tag, bumps the workspace version, and pushes to `origin`.
- [ ] **GitHub release workflow** (`.github/workflows/release.yml`) triggers on
      tag push. Watch the run to completion; re-run individual jobs only on
      transient infrastructure failures.
- [ ] `bash scripts/release/verify-checksum.sh` — verify every published
      artifact's checksum matches the workflow output. Mismatch is
      release-blocking.
- [ ] **SBOM generated and attached.** Confirm the SBOM is uploaded as a
      release asset and is signed if the signing key is configured.
- [ ] `bash scripts/gen-changelog.sh` — generate the CHANGELOG entry, diff
      against the draft from step 2, edit the release body on GitHub if the
      generated text needs polish.
- [ ] **Pre-release flag.** Mark the GitHub release as "pre-release" for any
      tag containing `-beta`, `-alpha`, or `-rc`. Stable tags only flip this
      off after distribution checks pass.

## 5. Distribution (T+0)

**Policy (frozen 2026-05-23 — see ADR-010 + ADR-012):** Homebrew Cask only.
Apple notarization is **not** used (no Developer ID per project policy);
users grant trust manually via
`xattr -dr com.apple.quarantine /Applications/CopyPaste.app` if Gatekeeper
flags the install. Sparkle / autoupdate feeds are **not** shipped. A
signed `.dmg` is still attached to every GitHub release for
reproducibility, but the README directs users at the Cask rather than
promoting standalone `.dmg` downloads as the primary install path.

The DMG itself is built and signed (ad-hoc) by
`scripts/release/build-dmg-ci.sh` and `scripts/release/_sign-and-dmg.sh`
during the release workflow (`.github/workflows/release.yml`); the Cask
formula (`scripts/release/gen-cask.sh`) then points at that DMG.

- [ ] **DMG built + checksummed** by `release.yml` and attached to the
      GitHub release. `verify-checksum.sh` exit 0.
- [ ] **Homebrew Cask update** via `bash scripts/release/gen-cask.sh`. Verify
      the generated cask references the new tag, correct SHA-256 sums, and the
      arch-specific URLs.
- [ ] **Tap push** to the `copypaste-tap` repo. Open a PR if branch protection
      requires review; otherwise push directly to `main` of the tap.
- [ ] **Smoke install** on a clean macOS host:
      `brew install --cask copypaste/tap/copypaste`. Launch the app, copy and
      paste between two pasteboards, quit cleanly. Uninstall and reinstall to
      confirm idempotency.
- [ ] **(NOT applicable):** Apple notarization / `notarytool` round-trip,
      Sparkle update feed publication, mirror checks to direct-download
      promo pages. Re-introduce only if a Developer ID is acquired AND
      ADR-010/ADR-012 are amended in a follow-up PR.
- [ ] **Announcement.** Post the release notes to the project README/website
      and any social channels listed in the comms plan.

## 6. Post-release (T+1 day)

- [ ] **Monitor crash reports.** Watch any local crash dump intake for the
      first 24 hours of broader usage. Triage anything new to the milestone or
      open a hotfix issue.
- [ ] **Telemetry opt-in adoption.** Verify the opt-in counter moves only when
      users explicitly enable telemetry. Any unexpected baseline traffic is a
      release-blocking privacy regression.
- [ ] **Sentry stub remains opt-in.** Confirm the Sentry integration ships in
      its stubbed, opt-in-only configuration. No DSN should be active by
      default in shipped binaries.
- [ ] **Issue triage.** Sweep new issues filed within 24 hours of release for
      anything that looks regression-shaped. Open a hotfix milestone if two or
      more independent reports of the same defect arrive.

## 7. Rollback

Use this path if a release-blocking defect surfaces post-tag.

- [ ] **Yank Homebrew Cask.** Revert the tap PR or push a follow-up commit that
      restores the previous cask version. Smoke-install the rolled-back cask
      to confirm users land on the prior tag.
- [ ] **Mark GitHub release as pre-release or draft.** For published stable
      tags, flip to "pre-release" first to discourage downloads; convert to
      "draft" only if assets must be withdrawn entirely. Never delete the tag
      itself — downstream consumers may already depend on it.
- [ ] **Communicate.** Update the release notes with a clear ROLLBACK banner
      pointing users at the previous stable tag and the tracking issue for the
      defect.
- [ ] **Open a hotfix branch.** Cut `hotfix/vX.Y.Z+1` from the rolled-back tag,
      apply the minimal fix, and restart this checklist from section 2 (code
      freeze) for the hotfix release.
- [ ] **Postmortem.** Within one week of the rollback, file a postmortem in
      `docs/postmortems/` covering detection, root cause, and process gaps so
      this checklist can be amended for the next cut.
