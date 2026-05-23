# Docker Builds — CopyPaste

Container-based cross-compilation for **non-macOS** platforms.
Host machine stays clean; no NDK / mingw / musl-tools clutter.

## Why containers

| Platform | Where it builds | Why |
|----------|-----------------|-----|
| **macOS arm64 / x86_64 / universal** | **Host only** | Apple SDK cannot legally run in a Linux container. Use the host Mac or a GitHub Actions `macos-14` runner. |
| **Android arm64-v8a / armeabi-v7a** | Docker (`docker/Dockerfile.android`) | NDK + cargo-ndk + JDK = ~3 GB host pollution. Image owns it. |
| **Windows x86_64** | Docker (`docker/Dockerfile.windows`) | mingw-w64 cross toolchain. Best-effort — daemon Windows IPC is still a stub. |
| **Linux x86_64-musl** | Docker (`docker/Dockerfile.linux`) | Runtime support is **frozen** per project rules; image exists for cross-build sanity only. |

`lipo` (universal binary creation) is also host/CI-only — it's an Apple tool.

## Quickstart

```bash
# First time: build the image (~3-5 min per platform — deps download once)
docker compose --profile build build android

# Run the build (re-uses image; recompiles only changed source)
docker compose --profile build run --rm android

# Outputs land here, mounted from host:
ls builds/android-arm64-v8a/
```

Or use the wrapper:

```bash
bash scripts/build-in-docker.sh android        # one platform
bash scripts/build-in-docker.sh all            # android + windows + linux
```

## Full release flow (macOS dev box)

```bash
# 1. macOS on host (universal binary, .app, .dmg)
bash scripts/build-all.sh macos

# 2. Everything else in containers
bash scripts/build-in-docker.sh all

# 3. Inspect outputs
ls -la builds/
```

Or combined via the `--docker` flag on `build-all.sh`:

```bash
bash scripts/build-all.sh macos               # host
bash scripts/build-all.sh --docker android    # container
bash scripts/build-all.sh --docker windows    # container
```

## How it works

- Source tree is bind-mounted at `/workspace` (live; edits on host are visible
  inside the container instantly — no rebuild needed for source changes).
- Each platform uses a **separate cargo target dir** (`target-android`,
  `target-windows`, `target-linux`) so they don't fight your host `target/`
  or each other.
- Cargo registry is cached in a **named docker volume** per platform
  (`cargo-android-cache`, etc.) so deps download once across builds.
- Build outputs land in `builds/<platform>-<arch>/` via the bind mount.

## Images

| File | Image tag | Size (approx, after build) |
|------|-----------|----------------------------|
| `docker/Dockerfile.android` | `copypaste-builder-android:latest` | ~4 GB (NDK + SDK + JDK) |
| `docker/Dockerfile.windows` | `copypaste-builder-windows:latest` | ~1.5 GB (rust + mingw) |
| `docker/Dockerfile.linux`   | `copypaste-builder-linux:latest`   | ~1.3 GB (rust + musl) |

## Layer caching

Each Dockerfile is layered so that:

1. **Base + system deps** — cached unless the Dockerfile changes
2. **Toolchain install** (NDK / mingw / musl) — cached unless ARG changes
3. **Rust targets** — cached unless rust-toolchain changes

Source code is **not** copied into the image — it's bind-mounted at runtime.
So a `cargo build` rerun only recompiles changed source.

## Reproducibility

Pinned via `ARG`:

- `NDK_VERSION=26.3.11579264` (Dockerfile.android)
- `CMDLINE_TOOLS_VERSION=11076708` (Dockerfile.android)
- Rust toolchain pinned to `1.75-slim-bookworm` in every base.

Override at build time:

```bash
docker build -f docker/Dockerfile.android --build-arg NDK_VERSION=27.0.12077973 -t copypaste-builder-android .
```

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `docker: Cannot connect to the Docker daemon` | Start Docker Desktop / `sudo systemctl start docker` |
| Android build hangs at `sdkmanager --licenses` | First build only; needs network. Retry with `--no-cache`. |
| Windows link errors (`undefined reference to …`) | Expected — `copypaste-daemon` Windows IPC is a stub. See `scripts/build-windows.sh`. |
| `builds/` empty after build | Check the mount: `docker compose --profile build run --rm android ls /workspace/builds/`. If empty there too, the build script failed silently — re-run without `-q`. |
| Stale registry / cache | `docker compose --profile build down -v` wipes named volumes. |

## CI integration (future)

For releases, GitHub Actions runs:

- `macos-14` runner → host build for macOS (lipo, .app, .dmg, .pkg)
- `ubuntu-latest` runner → reuses `docker/Dockerfile.android` & `.windows` for parity with local
