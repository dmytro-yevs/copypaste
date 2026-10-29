# CopyPaste

CopyPaste keeps encrypted clipboard history on macOS, Android, Windows, and Linux.
It supports search, Quick Paste, private capture controls, device pairing, and
optional OCR, semantic search, SMS Codes, and Supabase Sync modules.

Download the latest release from [GitHub Releases](https://github.com/dmytro-yevs/copypaste/releases).

## Modules

- **OCR** reads text from images locally.
- **Semantic Search** finds related clips locally.
- **SMS Codes** copies verification codes from new Android SMS messages.
- **Supabase Sync** synchronizes encrypted history through your Supabase project.

### Additional Ubuntu ARM64 CI

The **Ubuntu ARM64** workflow runs portable Rust tests and Clippy on the repository's self-hosted `ubuntu-arm64-copypaste` runner. Select it in Actions to start a manual run as the repository owner. Automatic runs target `main`; pull requests continue to use the existing GitHub-hosted platform validation. This additional check does not qualify desktop, Android, release, or resource behavior.

The runner labels are `self-hosted`, `Linux`, `ARM64`, `ubuntu-server`, and `copypaste`. It is an ARM64 Ubuntu machine with a minimal toolchain, not a GitHub-hosted x86_64 image. Keep platform-specific jobs on their existing runners. A timing budget remains unset until comparable successful run evidence exists.
