## Install

**macOS** (Apple Silicon):

```sh
brew tap dmytro-yevs/copypaste
brew install --cask copypaste     # the app
brew install copypaste-cli        # the CLI and daemon
```

**Android:** download the `.apk` attached below and open it. Android will ask
you to allow installing from this source.

## Signing

**macOS.** These builds are **not notarised by Apple** — the project has no
Apple Developer ID. On install the cask removes the `com.apple.quarantine`
attribute from `/Applications/CopyPaste.app`, and only from that bundle, and
re-signs the app with a certificate generated on your own machine. That
certificate stays in your keychain, is never sent anywhere, and is what makes
macOS treat each update as the same app rather than as a new one.

See [ADR-0001](https://github.com/dmytro-yevs/copypaste/blob/main/docs/adr/0001-macos-distribution-without-a-developer-id.md)
for the reasoning and for what it costs: the app deliberately requires no
Accessibility or Input Monitoring permission.

**Android.** Android will warn that this app comes from an unknown developer. If
the filename ends in `-unstable-key`, Android will also refuse to install it
over an earlier CopyPaste — uninstall the old version first, which deletes its
clipboard history. See
[ADR-0006](https://github.com/dmytro-yevs/copypaste/blob/main/docs/adr/0006-android-release-signing.md).

macOS builds are Apple Silicon only. Verify what you downloaded against the
`.sha256` files attached below.

## Not verified on real hardware

This project is developed on Linux. The macOS clipboard backend, the Keychain
device-secret store, the install-time signing path, the Android build and the
app itself have been compiled but never observed running on a Mac or on a
phone. Treat this as an alpha in the literal sense.
