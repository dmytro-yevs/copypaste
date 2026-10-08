# ADR-0006 — Android release signing

**Status:** accepted · 2026-07-30
**Scope:** what the released Android APK is signed with, what that means for the
person installing it, and what has to change to improve it.

## Context

The release serves macOS, Android, and Windows from one version stream. Android
uses an APK a user can download and install directly. There is no Play Store
listing and no plan for one.

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
application IDs. The same metadata pins version code `300000000`: the reset to
marketing version 1.0.0 must still install over the retired prerelease line,
whose last published version code was `200000038`. The release workflow
compares the signed APK's `apksigner`
value to that metadata before it uploads the artifact, so a changed or
accidentally replaced secret cannot create a non-upgradable release.

All four secrets must be configured before tagging. A key change requires
updating the public pin in the same reviewed change; it is an upgrade-breaking
event, not a routine secret rotation.

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
  dependencies of the publish job. A missing signing secret, a broken APK, or
  an APK that cannot install and run stops the shared release.
- The Rust Android targets and the NDK are pinned in the workflow, for the
  reason `rust-toolchain.toml` exists — an unpinned NDK is a build that changes
  under you.
- Universal, arm64, and armv7 APKs are published. Universal keeps x86_64 support
  and the original filename for older updaters. ARM variants reduce download
  size. Every variant uses the same durable certificate and version code;
  Flutter's per-ABI version-code offsets are disabled for GitHub distribution.
- The Flutter workflow builds the signed universal APK, verifies its package,
  version, debuggable flag, ABI libraries, certificate fingerprint, and
  checksum, then installs and starts that exact artifact on an x86_64 emulator.
- The workflow creates a detached minisign-compatible updater signature for the
  APK with the same private updater key used for Windows. The Flutter client
  discovers the signed artifacts directly from GitHub Releases; missing keys,
  signatures, digests, or artifacts fail closed before publication.
- The Flutter client preserves that independent updater signature boundary. It
  requires the APK and `.sig` asset digests from the GitHub Releases API,
  verifies the detached signature with the public key embedded in the client,
  and then asks Android to confirm the package name, version code, and signing
  certificate before showing the system installer.

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
