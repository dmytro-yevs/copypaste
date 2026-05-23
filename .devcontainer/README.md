# CopyPaste Dev Container

Cloud and local dev-container config for working on CopyPaste with a pre-provisioned
Rust toolchain, Slint, SQLCipher, and D-Bus build dependencies — no host setup required.

## Usage

### GitHub Codespaces

1. Open the repo on GitHub.
2. Click **Code -> Codespaces -> Create codespace on `release/v0.2.0-beta`**.
3. Wait ~2-4 min for the image to build and `postCreateCommand` to finish
   (`cargo fetch` warms the registry cache).
4. The Rust toolchain, rust-analyzer, Slint LSP, and LLDB are pre-installed.
5. Relay port `7777` is auto-forwarded — open the **Ports** panel for the URL.

### VS Code Remote Containers (local)

1. Install the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers).
2. Clone the repo locally.
3. `Cmd/Ctrl+Shift+P` -> **Dev Containers: Reopen in Container**.
4. First build downloads `mcr.microsoft.com/devcontainers/rust:1-bullseye`
   (~700 MB) plus features.

## What's included

- **Base image**: `mcr.microsoft.com/devcontainers/rust:1-bullseye` — stable
  Rust + Cargo + clippy + rustfmt.
- **Features**:
  - `docker-in-docker` — needed so `scripts/build-in-docker.sh` works inside
    the container (Android, Windows, Linux cross-builds).
  - `github-cli` — `gh` for PR/issue workflows.
  - `common-utils` — zsh, sudo, standard dev utilities.
- **VS Code extensions**:
  - `rust-lang.rust-analyzer` — Rust LSP.
  - `tamasfe.even-better-toml` — `Cargo.toml` editing.
  - `slint-ui.slint` — Slint UI LSP and preview.
  - `vadimcn.vscode-lldb` — debugger (CodeLLDB).
- **postCreateCommand**: installs native build deps
  (`pkg-config`, `libssl-dev`, `libsqlcipher-dev`, `libdbus-1-dev`), adds
  `clippy` + `rustfmt` components, runs `cargo fetch`.
- **Volumes**: `copypaste-cargo-cache` and `copypaste-target-cache` survive
  container rebuilds — keeps recompile time low.
- **Forwarded ports**: `7777` (CopyPaste relay).

## Notes

- The base image is **not** `docker/Dockerfile.dev` from `docker-compose.yml`.
  That Dockerfile is for `docker compose run dev` workflows; the devcontainer
  uses the upstream MS image because it ships VS Code server prerequisites and
  feature support that a plain Rust image lacks.
- If you prefer the project Dockerfile, swap the `image` field for:
  ```jsonc
  "build": { "dockerfile": "../docker/Dockerfile.dev", "context": ".." }
  ```
  but you will lose the `features` ecosystem and have to install VS Code server
  deps yourself.
- macOS host + Codespaces both work; Linux hosts may need
  `sudo chown -R vscode:vscode target/` once after the first build if you see
  permission errors on the cached volume.
