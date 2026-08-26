## Highlights

- Refreshes the app shell, onboarding, settings, history states, and responsive
  navigation so the same product flow works cleanly on desktop and Android.
- Unifies clipboard permission, capture, source-app, and unsupported-state
  handling across macOS, Android, and Windows while preserving native payloads.
- Tightens device pairing and peer sync with shared deadlines, typed UI
  contracts, and smaller protocol and session boundaries.
- Makes v2 use only its own database and current contracts, removing retired
  preference, pairing, deep-link, IPC, and prior-product compatibility paths.
- Binds native release evidence to the exact commit and artifact, strengthens
  redaction and Android manifest checks, and requires physical Android evidence.

## Not verified on this host

macOS TCC permission prompts, the Android Quick Settings tile add flow, and
Windows pairing were not exercised on the machine that cut this tag. Treat
those paths as implemented and CI-covered, not as a live walkthrough.

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

The release package is `com.copypaste.app` and starts with an empty history and
no pairings.

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
