## What changed

**The p2p wire protocol is now v3.** A device still on an older build will stop
syncing with this one until both are updated. Deliberate — v2 keeps no backward
compatibility.

**`copypaste list --json` now returns a 1 KB preview of each item body** rather
than the whole body. A script that needs full contents must use
`copypaste export`, which is unchanged.

The rest is a performance pass. Sync no longer re-sends history a peer already
has: a converged round over 10,000 items went from ~5 MB to under 1 KB, and idle
cost per peer from ~62 MB/hour to ~11 KB/hour. Capture, search and the history
list are all faster; the measurements are in
[docs/rewrite/performance.md](https://github.com/dmytro-yevs/copypaste/blob/main/docs/rewrite/performance.md).

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

**Android.** Android will warn that this app comes from an unknown developer.
Every published APK is signed with the durable release key; the release fails
closed if that key is unavailable. The signed universal APK is installed and
smoke-tested on an emulator before publication. See
[ADR-0006](https://github.com/dmytro-yevs/copypaste/blob/main/docs/adr/0006-android-release-signing.md).

macOS builds are Apple Silicon only. Verify what you downloaded against the
`.sha256` files attached below.

## Not verified on real hardware

This project is developed on Linux. The macOS clipboard backend, the Keychain
device-secret store and the install-time signing path have been compiled but
never observed running on a user's Mac or phone. On a publishable tag, the
signed universal Android APK is installed and smoke-tested on an emulator before
publication. Treat this as an alpha in the literal sense.
