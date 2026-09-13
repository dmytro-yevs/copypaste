## Highlights

- Android background capture and capture from other apps start on by default
  and keep that choice across app restarts. A failed arm or a dismissed
  permission prompt no longer persists as the user turning capture off.
- The capture overlay no longer blocks touches in other apps.
- LAN discovery on Android uses platform DNS-SD plus a multicast lock, so the
  Devices radar is no longer stuck on “unavailable” when raw mDNS is dropped.
- Third-party QR scanners can open `copypaste://pair` and start inbound
  pairing. This is pairing association only, not HTTPS App Links and not the
  Tauri deep-link plugin.
- Edge-to-edge cutouts and system bars publish into the CSS inset tokens.
- Long clips open a swipeable, scrollable compact sheet, and delete toasts use
  the shared toaster tokens.
- Onboarding stacks at the toolbar breakpoint (~748px) and uses larger capture,
  card, and network artwork glyphs.
- The compact library toolbar no longer gains an extra left inset from a hidden
  search field.

## Not verified on this host

Physical Android capture persistence, overlay hit-testing, LAN discovery, QR
association, and cutout insets were not walked on a device or emulator for this
tag. Windows pairing and macOS TCC prompts were not exercised. Missing
same-commit native evidence remains a blocker for claiming this alpha is
qualified. `v2.0.0-alpha.34` was the authorized retry of the failed
`v2.0.0-alpha.33` tag and is not this product wave.

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
