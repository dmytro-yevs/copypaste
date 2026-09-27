## Highlights

- Copy now reflects native clipboard availability, shows progress, and keeps
  the selected item and reader focus stable. Unsupported file content remains
  selectable but is not presented as a copy action.
- Long clips have an explicit full reader. Library and Quick Paste share the
  same content presentation and copy-availability rules.
- Settings presents direct-device readiness and Cloud connection state
  consistently. Runtime events are under Diagnostics, while self-hosted Cloud
  fields stay in Advanced and open deliberately from search or setup.
- Onboarding keeps actions reachable on short screens, explains permission
  failures, and uses platform-appropriate shortcut hints.
- Android capture and pairing recovery preserve the first attempted action and
  report the exact recovery outcome instead of silently treating an app failure
  as a successful retry.
- macOS update status explains whether automatic updates are available and
  provides the manual release-page fallback when they are not.
- Windows Cloud qualification records the closed overview as diagnostic
  evidence while the revealed configuration form remains the canonical
  unconfigured state.

## Evidence note

`v2.0.0-alpha.36` carries a documented one-alpha release-risk acceptance for
the same 58 pending native-evidence states across history, capture, devices,
settings/service, and Cloud account. They remain explicitly unverified and
excluded from receipt expectations; this release does not claim them complete.

## Not verified on this host

Physical Android capture persistence, overlay hit-testing, LAN discovery, QR
association, cutout insets, OEM process-kill survival, and Shizuku rung 2
were not walked on a device for this tag. Windows pairing and macOS TCC
prompts were not exercised. These limits, including the 58 documented pending
states above, remain visible release risk and are not presented as completed
native qualification.

## Install

**macOS 14 Sonoma or later** (Apple Silicon):

```sh
brew tap dmytro-yevs/copypaste
brew install --cask copypaste     # the app
brew install copypaste-cli        # the CLI and daemon
```

The same release page also includes the Apple Silicon DMG for direct install.

### Windows

On Windows 10 or 11 (x86-64), download and run the
`CopyPaste-…-windows-x86_64-setup.exe` asset from the release page.

### Android

On the [releases page](https://github.com/dmytro-yevs/copypaste/releases), open
the newest prerelease; GitHub's **Latest** link does not select prereleases.
Download its `CopyPaste-…-android.apk` asset as `CopyPaste-android.apk` in the
directory where you run `adb`, then install or update it with:

```sh
adb install -r ./CopyPaste-android.apk
```

The release package is `com.copypaste.app`. A first install starts with an
empty history and no pairings. An in-place update of an existing package with
the same signing key uses `adb install -r` and retains that package's data,
including its history, pairings, and settings.

Only an APK installed as `com.copypaste.app` with an incompatible signing key
needs an uninstall. If installation reports
`INSTALL_FAILED_UPDATE_INCOMPATIBLE`, inspect every Android user, including a
work profile, before removing anything:

```sh
adb shell pm list users
adb shell pm list packages --user 0 com.copypaste.app
adb shell pm list packages --user 10 com.copypaste.app  # repeat for each listed ID
```

**Uninstalling erases the package's history, pairings and settings for all
users and profiles.** Remove only the incompatibly signed `com.copypaste.app`
package, then rerun the install command above:

```sh
adb uninstall com.copypaste.app
```

Future debug builds use `com.copypaste.app.debug`, so they do not replace the
release app.

## Signing and verification caveats

The macOS workflow uses ad-hoc signing and does not notarize artifacts with
Apple. The Homebrew cask removes quarantine only from the installed CopyPaste
bundle and re-signs it locally. Android publication requires the configured
durable release key and fails closed when it is unavailable. Windows
Authenticode on this alpha uses a project-generated code-signing certificate,
not a public CA; SmartScreen may warn until a CA-issued identity is in place.

Verify downloaded artifacts against their attached `.sha256` files.
