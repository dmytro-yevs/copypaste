## Highlights

- Hardens Windows installer signing: validates the release certificate before
  the build, bounds SignTool execution, verifies the signer and RFC 3161
  timestamp, and removes temporary signing material afterward.
- Keeps Windows update availability in a pending state until the installed
  build's updater configuration has been resolved, avoiding a premature or
  misleading settings message.
- Strengthens release evidence for signed Windows installers and in-place
  updates, including signer identity, updater artifacts, and failure states.
- Pins the frontend toolchain consistently across release and platform CI to
  make packaged builds reproducible.

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

### Android

On the [releases page](https://github.com/dmytro-yevs/copypaste/releases), open
the newest v2 prerelease rather than GitHub's legacy **Latest** release. Download
its `CopyPaste-…-android.apk` asset as `CopyPaste-android.apk` in the directory
where you run `adb`, then install or update it with:

```sh
adb install -r ./CopyPaste-android.apk
```

This is a clean v2 install (`com.copypaste.app`). It starts with new history and
pairings; it does not import or upgrade any prior product install.

Only v2 alpha.1 or a local/debug APK installed as `com.copypaste.app` with an
incompatible key needs a one-time uninstall. If installation reports
`INSTALL_FAILED_UPDATE_INCOMPATIBLE`, inspect every Android user, including a
work profile, before removing anything:

```sh
adb shell pm list users
adb shell pm list packages --user 0 com.copypaste.app
adb shell pm list packages --user 10 com.copypaste.app  # repeat for each listed ID
```

**Uninstalling erases that v2 install's history, pairings and settings for all
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
