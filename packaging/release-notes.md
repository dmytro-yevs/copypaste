## Highlights

- Turns sensitive auto-wipe on by default (30s TTL) with a 0.85 wipe floor so
  generic/KV noise is withheld rather than deleted.
- Patches glib 0.18.5 in-tree for RUSTSEC-2024-0429 (VariantStrIter).
- Zeroizes STREAM, binary-open and sync plaintext buffers.
- Adds CI contracts for native pairing, Shizuku fail-closed, FLAG_SECURE and
  OEM-sticky capture restarts. Windows/Android pairing is not manually verified
  on a Mac.

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
durable release key and fails closed when it is unavailable.

Verify downloaded artifacts against their attached `.sha256` files.
