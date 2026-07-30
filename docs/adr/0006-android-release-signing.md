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

**The APK is signed by the release workflow, from a keystore supplied through
repository secrets. When those secrets are absent, the workflow still produces
an installable APK, signed with a keystore generated during that run, and says
so in the artefact's own filename.**

The four secret names are carried over from v0.4.x rather than invented, so a
keystore made for v1 still works:

| Secret | Meaning |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | base64 of the release keystore (`.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | store password |
| `ANDROID_KEY_ALIAS` | key alias |
| `ANDROID_KEY_PASSWORD` | key password |

**None of them is set today.** The workflow does not assume otherwise: absence
is the default path, it is not an error, and it does not fail the release.

What each path produces:

| Secrets | Artefact | Upgrades in place |
|---|---|---|
| present | `CopyPaste-v<version>-android.apk` | yes |
| absent | `CopyPaste-v<version>-android-unstable-key.apk` | **no** |

The workflow prints the SHA-256 of the signing certificate into the run summary
either way. That is the one number that decides whether an upgrade will work,
and it costs nothing to publish it.

### The sentence the user gets

In the release notes, unhedged:

> Android will warn that this app comes from an unknown developer. If the
> filename ends in `-unstable-key`, Android will also refuse to install it over
> an earlier CopyPaste — uninstall the old version first, which deletes its
> clipboard history.

## Why not the alternatives

**A debug keystore, generated per run.** This is the same thing as the fallback
above and is what "just use debug signing" means in practice; naming it
`-unstable-key` and putting the certificate hash in the log is the only
difference, and the difference is that nobody is surprised.

**A keystore committed to the repository.** This is what v1 actually did — it
carried `android/app/debug.keystore` and signed the fallback with it. It has a
real advantage: the key is stable, so in-place upgrades work with no secret and
no account. The cost is that the private key is public, so anyone can build an
APK that Android will accept as an update to an installed CopyPaste. That does
not let an attacker push anything — they still need the user to install their
file — but it removes the one check that would otherwise stop a sideloaded
"update" from silently replacing the real app and inheriting its data
directory. v1 took that trade without writing it down.

It remains the right answer if this project decides it would rather have working
upgrades than that check, and it is a two-line change: commit the keystore under
`packaging/android/` and point the workflow at it. **It is not taken by default
here because it is a security decision and it should be made deliberately, in a
commit that says so, rather than inherited.**

**Play App Signing.** Needs a Play Console account ($25, one-off). Cheaper than
Apple's $99/yr and worth revisiting if the app is ever listed. It solves nothing
for direct download, which is the channel this ADR is about.

## Consequences

- The Android job is a hard dependency of the publish job. A broken APK build
  stops the macOS release too. This is deliberate: the alternative is a release
  page that silently serves one platform, and "one release page for both" is the
  requirement.
- The Rust Android targets and the NDK are pinned in the workflow, for the
  reason `rust-toolchain.toml` exists — an unpinned NDK is a build that changes
  under you.
- A universal APK is built rather than per-ABI splits. It is larger; it is also
  one file that installs on any device, which is what direct download needs.
- **Untested.** No Android build has run in CI and no APK has been installed on
  a device. The paths in the workflow are written against the Tauri scaffold at
  `crates/copypaste-ui/src-tauri/gen/android`, and the job fails with an
  explicit message rather than a stack trace if that scaffold is not where it
  expects.

## What would change this

Setting the four secrets. Generate the keystore once, keep it somewhere you will
still have it in five years — losing it means no existing install can ever be
upgraded again:

```sh
keytool -genkeypair -v \
  -keystore copypaste-release.jks \
  -alias copypaste \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 copypaste-release.jks    # the value for ANDROID_KEYSTORE_BASE64
```
