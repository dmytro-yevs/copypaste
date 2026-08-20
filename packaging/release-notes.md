## Highlights

- Turns sensitive auto-wipe on by default (30s TTL) with a 0.85 wipe floor so
  generic/KV noise is withheld rather than deleted.
- Zeroizes STREAM, binary-open and sync plaintext buffers.
- Patches glib 0.18.5 in-tree for RUSTSEC-2024-0429 (VariantStrIter) and keeps
  Dependabot on for remaining moderate advisories.
- Adds a first-run setup wizard: permissions tap-to-prompt, pairing, hosted
  Supabase as the easy cloud path, and Android-only capture. Skip never blocks
  later setup.
- Drops the physical arm64 Android self-hosted publish gate. Signed APK
  smoke on GitHub-hosted emulators is the Android release evidence.
- Unblocks release publish: authenticated clipboard column privileges,
  Android upgrade-fixture metadata deps, and macOS cloud evidence launch.
- Restores payload_metadata grants after the authenticated revoke and
  hardens macOS cloud AX reach (ready wait, contains press, always-upload).
- Opt-in COPYPASTE_EVIDENCE_AX so WKWebView publishes its AX tree on CI.
- Report (do not enforce) macOS cloud UI AX on hosted runners; uncredit its p95.
- Import the Windows release cert via X509Store so Root trust cannot hang.
- Adds CI contracts for native pairing, Shizuku fail-closed, FLAG_SECURE and
  OEM-sticky capture restarts.

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
