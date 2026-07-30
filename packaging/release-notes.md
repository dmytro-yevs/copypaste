## Install

```sh
brew tap dmytro-yevs/copypaste
brew install --cask copypaste     # the app
brew install copypaste-cli        # the CLI and daemon
```

## Signing

These builds are **ad-hoc signed and not notarised** — the project has no Apple
Developer ID. The cask removes the `com.apple.quarantine` attribute from
`/Applications/CopyPaste.app`, and only from that bundle, on install.

See [ADR-0001](https://github.com/dmytro-yevs/copypaste/blob/main/docs/adr/0001-macos-distribution-without-a-developer-id.md)
for the reasoning and for what it costs: the app deliberately requires no
Accessibility or Input Monitoring permission, because an ad-hoc signature has no
Team ID and macOS would revoke any such grant on every update.

Apple Silicon only. Verify what you downloaded against the `.sha256` files
attached below.

## Not verified on macOS

This project is developed on Linux. The macOS clipboard backend, the Keychain
device-secret store, and the app itself have been compiled but never observed
running on a Mac. Treat this as an alpha in the literal sense.
