# CopyPaste

CopyPaste keeps encrypted clipboard history on macOS, Android, Windows, and Linux.
It supports search, Quick Paste, private capture controls, device pairing, and
optional OCR, semantic search, SMS Codes, and Supabase Sync modules.

Download the latest release from [GitHub Releases](https://github.com/dmytro-yevs/copypaste/releases).

## Install on macOS

CopyPaste for macOS needs Apple silicon and macOS 14 or later. Install it with Homebrew:

```bash
brew tap dmytro-yevs/copypaste
```

```bash
brew install --cask copypaste
```

Or with the [install script](scripts/install.sh), which downloads the release DMG,
checks it against the published SHA-256 checksum, and copies `CopyPaste.app` into
`/Applications`, or `~/Applications` when `/Applications` is not writable:

```bash
curl -fsSL https://raw.githubusercontent.com/dmytro-yevs/copypaste/main/scripts/install.sh | sh
```

Options go after `sh -s --`: `--version v0.1.0`, `--dir ~/Applications`, or
`--dry-run` to download and verify without installing.

CopyPaste is not notarized, because Apple notarization requires a paid Apple
Developer Program membership. Both install paths re-sign the app with a local
identity on your Mac, and it opens without a Gatekeeper prompt. If you download
the DMG in a browser instead, macOS blocks the first launch: open
**System Settings → Privacy & Security** and choose **Open Anyway**.

## Install on Windows

Once CopyPaste is published to winget, install it on Windows (x64) with:

```powershell
winget install CopyPaste.CopyPaste
```

`winget upgrade CopyPaste.CopyPaste` updates it. Maintainers: see the
[Windows release setup](packaging/windows/README.md) for signing and winget publication.

## History organization

History supports local-calendar date ranges, quick periods, and combined server-side
search, source, device, type, and tag filters before pagination. Pinned clips can
have multiple tags; unpinning retains their tag assignments. Tag definitions,
assignments, pin state, and pin order synchronize through the existing encrypted
P2P and optional cloud transports without changing a clip's capture timestamp.

Organization is stored separately from clipboard content. Each tag, membership,
and pin has an independent last-write-wins version, including removals, so
concurrent edits to different assignments do not overwrite one another. Backups
include organization; text exports contain clipboard content only. Update all
devices to use organization features. Older clients preserve organization as
opaque unsupported binary records rather than treating it as pasteable text.
Cloud synchronization continues to follow the configured server retention policy.

## Modules

- **OCR** reads text from images locally.
- **Semantic Search** finds related clips locally.
- **SMS Codes** copies verification codes from new Android SMS messages. CopyPaste itself never requests SMS access: only **CopyPaste SMS**, a separate small app from the same release signed with the same key, does.
- **Supabase Sync** synchronizes encrypted history through your Supabase project.
