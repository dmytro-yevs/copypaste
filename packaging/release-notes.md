## Highlights

- Adds native device-pairing flows on macOS, Android and Windows.
- Adds the Windows desktop backend and installer/update packaging support.
- Restores cloud account controls and private mode across the app backends.
- Hardens clipboard, IPC, cloud-sync and sensitive-content handling.
- Expands Android accessibility, storage-transfer and dependency release gates.

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

The current publish workflow does not attach a Windows installer.

## Signing and verification caveats

The macOS workflow uses ad-hoc signing and does not notarize artifacts with
Apple. The Homebrew cask removes quarantine only from the installed CopyPaste
bundle and re-signs it locally. Android publication requires the configured
durable release key and fails closed when it is unavailable.

These notes were prepared without running the tagged release workflow. They do
not claim publication, notarization, new physical-device coverage or a
completed cross-platform end-to-end run. The tagged macOS and Android release
gates have not run yet; Windows packaging is present in the tree but is not part
of the current publish workflow. Treat this build as an early alpha.

Verify downloaded artifacts against their attached `.sha256` files.
