# CopyPaste

CopyPaste keeps encrypted clipboard history on macOS, Android, Windows, and Linux.
It supports search, Quick Paste, private capture controls, device pairing, and
optional OCR, semantic search, SMS Codes, and Supabase Sync modules.

Download the latest release from [GitHub Releases](https://github.com/dmytro-yevs/copypaste/releases).

On Windows, install with WinGet. Windows SmartScreen warns about the installer
when it is downloaded in a browser, because it is not signed by a commercial
certificate authority; WinGet verifies the installer hash and does not show
that warning.

```powershell
winget install DmytroYevs.CopyPaste
```

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
- **SMS Codes** copies verification codes from new Android SMS messages.
- **Supabase Sync** synchronizes encrypted history through your Supabase project.
