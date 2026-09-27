## Highlights

- Pairing uses one QR link format and shared state messages across macOS,
  Android and Windows. Cancelling a pairing no longer blocks the next attempt.
- Android discovery uses the same metadata, validation, pairing identity and
  expiry rules as desktop, and respects network visibility changes.
- Android background capture uses Shizuku without starting app-owned device-log
  requests. Keep Shizuku running while background capture is enabled.
- Android applies system-bar and cutout insets at startup. Capture setup lives
  in Clipboard settings, and saved-copy system notifications are removed.
- Devices and update checks keep a stable layout. Android and Windows update
  checks have a bounded deadline and recoverable error state.
- Quick Paste has compact rows, visible shortcuts, virtualized results and
  on-demand access to older history. Search uses the shared bounded backend.
- Capture and pairing updates use shared events; idle retention work waits for
  the next deadline. Test databases use injected keys instead of the macOS
  login Keychain.

## Release verification

The release pipeline checks native installation, startup, capture, persistence,
protection and signed artifacts before publication. The macOS DMG, Android APK
and Windows installer are published from the exact files used by their native
qualification jobs. This alpha does not claim comprehensive validation of every
OEM background-process policy or every historical feature-evidence state.

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
