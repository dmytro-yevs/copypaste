# ADR-0006 — Android release signing

**Status:** accepted · 2026-07-30
**Scope:** what the released Android APK is signed with, what that means for the
person installing it, and what has to change to improve it.

## Context

The release must serve both platforms from one page: a DMG installed through
`brew`, and an APK a user can download and install directly. There is no Play
Store listing and no plan for one — the same reasoning as ADR-0001, one step
sideways.

Android leaves less room than macOS. **An unsigned APK cannot be installed at
all**, so there is no equivalent of "ship it ad-hoc and fix it on the device":
`PackageInstaller` requires a signature before it will consider the file. And
the signature is load-bearing in a second way — Android refuses to install an
update whose signing key differs from the installed app's, with
`INSTALL_FAILED_UPDATE_INCOMPATIBLE`. The user's only recourse is to uninstall,
which takes their data with it.

That is the same shape as the TCC problem in ADR-0001 — an identity that moves
between builds breaks the upgrade path — but with a harsher failure and no
install-time escape, because there is nothing on the device that can sign for
us.

## Decision

**The APK is signed by the release workflow, from a durable keystore supplied
through repository secrets. All four secrets are required. If any are absent,
the release fails before it uploads an Android artifact or creates a GitHub
Release.**

The four secret names are the release workflow's canonical signing interface:

| Secret | Meaning |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | base64 of the release keystore (`.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | store password |
| `ANDROID_KEY_ALIAS` | key alias |
| `ANDROID_KEY_PASSWORD` | key password |

The public SHA-256 certificate fingerprint lives in
`Cargo.toml` under `[workspace.metadata.copypaste]`, beside the public Android
application IDs. The release workflow compares the signed APK's `apksigner`
value to that metadata before it uploads the artifact, so a changed or
accidentally replaced secret cannot create a non-upgradable release.

All four secrets must be configured before tagging. A key change requires
updating the public pin in the same reviewed change; it is an upgrade-breaking
event, not a routine secret rotation.

The one-time exception is v2 alpha.1, or a local/debug APK that used
`com.copypaste.app` with an incompatible key. Android requires that package to
be uninstalled before the durable-key release can be installed, which erases
that install's history, pairings and settings. Future debug builds use
`com.copypaste.app.debug`, leaving the release identity and data separate.

## Why not the alternatives

**A debug keystore, generated per run.** This is useful only for an ephemeral,
never-published emulator test. A release signed this way cannot upgrade an
existing install, so the workflow refuses to create one.

**A keystore committed to the repository.** It would make the key stable with
no secret-management account. The cost is that the private key becomes public,
so anyone can build an APK that Android accepts as an update to an installed
CopyPaste. That does not let an attacker push a file, but it removes the check
that otherwise stops a sideloaded replacement from inheriting the app's data
directory. This ADR rejects that trade.

**Play App Signing.** Needs a Play Console account ($25, one-off). Cheaper than
Apple's $99/yr and worth revisiting if the app is ever listed. It solves nothing
for direct download, which is the channel this ADR is about.

## Consequences

- The Android build and a smoke test of its signed universal artifact are hard
  dependencies of the publish job. A missing signing secret, a broken APK or an
  APK that cannot install and run stops the macOS release too. This is
  deliberate: the alternative is a release page that silently serves one
  platform, and "one release page for both" is the requirement.
- The Rust Android targets and the NDK are pinned in the workflow, for the
  reason `rust-toolchain.toml` exists — an unpinned NDK is a build that changes
  under you.
- A universal APK is built rather than per-ABI splits. It is larger; it is also
  one file that installs on any device, which is what direct download needs.
- On publishable runs, the workflow downloads the signed universal APK it just
  produced, verifies its sidecar checksum, and installs/runs that exact file on
  an x86_64 emulator before publication. The paths in the workflow are written
  against the Tauri scaffold at `crates/copypaste-ui/src-tauri/gen/android`, and
  the job fails with an explicit message rather than a stack trace if that
  scaffold is not where it expects.
- The publish job also creates a detached Tauri updater signature for the APK
  with the same `TAURI_SIGNING_PRIVATE_KEY` used for the Windows updater. One
  canonical `latest.json` contains both `windows-x86_64` and
  `android-universal`; missing keys, signatures, URLs, or placeholder values
  fail closed before the GitHub Release is created.

## What would change this

Set the four secrets. Generate the keystore once, keep it somewhere you will
still have it in five years — losing it means no existing install can ever be
upgraded again:

```sh
keytool -genkeypair -v \
  -keystore copypaste-release.jks \
  -alias copypaste \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 copypaste-release.jks    # the value for ANDROID_KEYSTORE_BASE64
```

Record its public certificate fingerprint, without colons or whitespace, as
`android-release-certificate-sha256` under
`[workspace.metadata.copypaste]` before setting the secrets:

```sh
keytool -list -v -keystore copypaste-release.jks -alias copypaste \
  | sed -n 's/.*SHA256: //p'
```
