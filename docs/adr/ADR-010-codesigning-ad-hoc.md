# ADR-010: Ad-hoc Code Signing for macOS Distribution

## Status

Accepted

Date: 2026-05-23
Track: beta-w4.1 (v0.2.0-beta hardening)

## Context

CopyPaste ships a macOS `.dmg` produced in CI (`scripts/release/build-dmg-ci.sh`)
and distributed via a Homebrew Cask (`scripts/release/gen-cask.sh`). macOS
Gatekeeper requires that executables either be:

1. **Notarised** with an Apple Developer ID (`$99/yr`, requires Apple Developer
   account, `notarytool` round-trip in CI), or
2. **Ad-hoc signed** with hardened runtime + entitlements, and have the
   `com.apple.quarantine` xattr removed at install time.

We do **not** have an Apple Developer account during the beta phase, and we
explicitly do not want to gate the project on one.

## Decision

Use **ad-hoc code signing** (`codesign --sign -`) with the hardened runtime
(`--options runtime`) and a restrictive entitlements plist
(`scripts/macos/entitlements.plist`).

The signing invocation is identical in both CI (`release.yml`) and the local
release builder (`build-dmg-ci.sh`):

```sh
codesign --force --deep \
    --sign - \
    --options runtime \
    --entitlements scripts/macos/entitlements.plist \
    --timestamp=none \
    "$APP_DIR"
```

Key flags:

- `--sign -` — ad-hoc identity; no certificate, no keychain, no secrets in CI.
- `--options runtime` — opt into hardened runtime; the entitlements file is
  then the contract for what the binary may do.
- `--timestamp=none` — ad-hoc signatures cannot be timestamped by Apple's TSA
  (no Developer ID), so explicitly disable the timestamp request.
- `--entitlements …` — restrictive plist: JIT off, unsigned-exec-memory off,
  library validation on, sandbox off (clipboard daemon needs full-disk
  semantics outside the App Sandbox container).

## Postflight: xattr quarantine strip

GitHub Actions downloads and re-extracts artifacts, which sets the
`com.apple.quarantine` xattr on the staged `.app` and the final `.dmg`. On a
user's machine, Gatekeeper would then refuse to launch the binary on first
open even though the signature is valid.

We mitigate this in two places:

1. **CI postbuild step** (`.github/workflows/release.yml`):
   `xattr -cr target/release/CopyPaste.dmg` (and `dist/CopyPaste.app`) before
   uploading artifacts and creating the GitHub Release.
2. **User-side installer** (`scripts/release/install.sh`): runs
   `xattr -dr com.apple.quarantine` on the installed `.app` for users who
   download the DMG directly rather than via Homebrew Cask.

For the Homebrew Cask path, Homebrew itself strips the quarantine xattr after
copying the artifact into `/Applications`, so the cask formula needs no
special clauses beyond the standard `app "CopyPaste.app"` stanza.

## Why no Apple Developer ID for the Homebrew Cask flow

- Homebrew Cask does not require Developer ID-signed binaries. Many casks
  ship ad-hoc-signed or unsigned binaries; Homebrew documents this as
  acceptable (`brew audit --cask`) provided the cask is for a project the
  user has explicitly opted into.
- The quarantine xattr (which is what triggers Gatekeeper's "unidentified
  developer" dialog) is removed by `brew install --cask` after staging.
- Notarisation buys us silent first-launch UX, but at the cost of a $99/yr
  Apple account and a `notarytool submit --wait` round-trip in every CI
  release (typically 2–10 minutes of CI wall time). Not worth it during beta.

## Consequences

**Positive:**

- Zero CI secrets required for release. No Apple Developer account needed.
- Reproducible: ad-hoc signature is deterministic given the same binary
  bytes and entitlements file.
- Fast release pipeline (no notarisation wait).

**Negative:**

- Users installing the DMG **manually** (not via Homebrew Cask) must either
  run `scripts/release/install.sh` or right-click → Open the first time, OR
  manually run `xattr -dr com.apple.quarantine /Applications/CopyPaste.app`.
- We must move to Developer ID + notarisation before any non-beta release
  intended for non-technical users.

## Migration path (post-beta)

When we eventually obtain an Apple Developer ID:

1. Add `APPLE_DEVELOPER_ID`, `APPLE_TEAM_ID`, and signing cert (`.p12` +
   passphrase) as GitHub Secrets.
2. Replace `--sign -` with `--sign "Developer ID Application: <Name> (<TEAM>)"`.
3. Keep `--options runtime` and the entitlements file as-is (already
   compatible).
4. Add `xcrun notarytool submit … --wait` + `xcrun stapler staple` steps
   after the DMG is built.
5. Drop the postbuild `xattr -cr` step (notarised + stapled binaries do not
   need quarantine stripping).

The entitlements file and codesign invocation structure are already
forward-compatible — only the identity argument and the additional
notarisation step change.

## References

- `scripts/macos/entitlements.plist` — restrictive entitlements (JIT off,
  sandbox off, network client+server on).
- `scripts/release/build-dmg-ci.sh` — local DMG builder; same flags as CI.
- `.github/workflows/release.yml` — CI release pipeline with postbuild
  xattr strip.
- `scripts/release/install.sh` — user-side installer with quarantine strip.
- Apple Technical Note TN3127, "Inside Code Signing: Requirements" — for
  the semantics of `--options runtime` and entitlement keys.
