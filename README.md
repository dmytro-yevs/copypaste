# CopyPaste

CopyPaste keeps encrypted clipboard history on macOS, Android, Windows, and Linux.
It supports search, Quick Paste, private capture controls, device pairing, and
optional OCR, semantic search, SMS Codes, and Supabase Sync modules.

Download the latest release from [GitHub Releases](https://github.com/dmytro-yevs/copypaste/releases).

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

### Additional Ubuntu ARM64 CI

The **Ubuntu ARM64** workflow runs portable Rust tests and Clippy on the repository's self-hosted `ubuntu-arm64-copypaste` runner. Select it in Actions to start a manual run as the repository owner. Automatic runs target owner-triggered pushes to `main`; pull requests continue to use the existing GitHub-hosted platform validation. This additional check does not qualify desktop, Android, release, or resource behavior.

The runner labels are `self-hosted`, `Linux`, `ARM64`, `ubuntu-server`, and `copypaste`. It is an ARM64 Ubuntu machine with a minimal toolchain, not a GitHub-hosted x86_64 image. Keep platform-specific jobs on their existing runners. A timing budget remains unset until comparable successful run evidence exists.
