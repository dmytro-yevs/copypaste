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

**Android:** download the `.apk` attached below and open it. Android will ask
you to allow installing from this source.

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
