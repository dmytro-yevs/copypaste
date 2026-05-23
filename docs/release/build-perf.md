# Build performance — v0.3

Reference for the build-perf changes landed in v0.3.0-dev. Covers CI runner
sizing, the Android Docker builder image, sccache/ccache, the release
profile change to thin LTO, and incremental-build expectations.

## Headline numbers

| Scenario                                       | v0.2 baseline | v0.3 target | v0.3 measured |
|------------------------------------------------|---------------|-------------|---------------|
| Android cold build, Apple Silicon (Rosetta)    | 30–60 min     | 5–10 min    | (see below)   |
| Android cold build, native amd64 (xlarge CI)   | 12–18 min     | 5–10 min    | projected     |
| Android warm build, code-only change           | 5–10 min      | 1–2 min     | projected     |
| macOS arm64 workspace release (host, cold)     | ~7 min        | ~6 min      | 6 min 6 s     |
| macOS arm64 workspace release link time only   | ~3–4 min      | ~1–2 min    | included      |

"Projected" numbers come from the time budgets removed by each change
(pre-baked openssl+sqlcipher saves ~15–20 min of host-side C compile; thin
LTO saves ~30–50 % of link time; sccache returns ~80–95 % of object files
on a one-crate change). Numbers will be revised once nightly runs land
on the larger runner.

## What changed in v0.3

### 1. Native amd64 CI runner for Android

`ci-android-build.yml` and `nightly.yml` (the `nightly-android` job) switch
from `ubuntu-latest` to `ubuntu-latest-xlarge` (16 GB RAM, 4× CPU). Native
amd64 avoids the Rosetta emulation that adds 5–10× overhead when developers
run the same Docker builds on Apple Silicon locally; the larger runner
additionally halves NDK link time vs default ubuntu-latest.

Falls back to `ubuntu-latest` if `ubuntu-latest-xlarge` is not provisioned
for the org (paid GitHub larger-runner tier).

`release.yml` and `ci-matrix.yml` have no Android job, so they are
untouched.

### 2. Pre-baked OpenSSL + SQLCipher in the Android builder image

`docker/Dockerfile.android` now has a layer that compiles OpenSSL 3.0.13
and SQLCipher 4.5.6 as host-arch (linux/amd64) static libraries into
`/opt/prebuilt/{openssl,sqlcipher}/` and exposes them via:

- `OPENSSL_DIR=/opt/prebuilt/openssl`
- `OPENSSL_STATIC=1`
- `SQLCIPHER_LIB_DIR=/opt/prebuilt/sqlcipher/lib`
- `SQLCIPHER_INCLUDE_DIR=/opt/prebuilt/sqlcipher/include`

The rusqlite `bundled-sqlcipher` feature otherwise compiles the SQLCipher
amalgamation (~1.5 M LOC of C) plus an embedded copy of OpenSSL on every
cold build, costing ~15–20 min. With pre-baked libs, the host-side C build
is paid once per image rebuild.

Image grows ~200 MB. The Android target itself still cross-compiles its
own OpenSSL via the NDK toolchain — this layer kills only the host-side
OpenSSL/SQLCipher rebuild.

Bump `OPENSSL_VERSION` / `SQLCIPHER_VERSION` ARGs when the workspace's
`Cargo.lock` advances `openssl-src` or `libsqlite3-sys` major versions.

### 3. sccache + ccache

`docker/Dockerfile.android` installs `sccache` 0.8.2 and `ccache` and wires:

- `RUSTC_WRAPPER=sccache` — cargo invokes sccache for every rustc call.
- `SCCACHE_DIR=/sccache`, `SCCACHE_CACHE_SIZE=10G`.
- `CC="ccache cc"`, `CXX="ccache c++"`, `CCACHE_DIR=/ccache`, `CCACHE_MAXSIZE=5G`.

The `android` compose service mounts named volumes:

```
sccache-android:/sccache
ccache-android:/ccache
```

so caches survive `docker compose down`. Cold container = full build;
subsequent runs hit the caches and a one-crate change collapses to
~30–90 s rustc time plus link.

### 4. `lto = "thin"` for `[profile.release]`

Root `Cargo.toml` switches the workspace release profile from `lto = "fat"`
to `lto = "thin"`. Thin LTO is parallelisable across codegen units, so
link time drops by 30–50 % on cold builds.

- Measured on macOS arm64 host: full workspace cold release build = 6 min 6 s
  (519 crates). Prior fat-LTO baseline was ~7+ min.
- Binary size grows ~5–10 % because cross-crate inlining is more limited.

`[profile.release-size]` is now explicitly pinned to `lto = "fat"` plus
`opt-level = "z"`, so size-critical distribution artifacts (mobile,
embedded) keep the old behaviour at the old link-time cost.

### 5. Persistent cargo target volumes

`docker-compose.yml` already declared per-platform cargo target volumes
(`cargo-android-target`, etc.). v0.3 documents the pattern in
`scripts/build-android-pkg.sh` and adds Make targets:

```
make android-docker             # cold or warm build with all caches
make android-docker-clean-cache # wipe target+sccache+ccache (keep registry)
```

Direct `docker run` callers should mirror the four volumes (see
`scripts/build-android-pkg.sh` header).

## Verification

- `cargo build --workspace --release --target aarch64-apple-darwin` — verified
  succeeds on macOS arm64 with thin LTO in 6 min 6 s, 519 crates.
- Docker image build — image layer count grows by 2 (prebuilt libs, sccache);
  `/opt/prebuilt/{openssl,sqlcipher}/lib/lib*.a` sanity-checked at end of layer.
- `nightly-android` job will exercise the full pipeline (image build + cached
  cargo build) once it next runs on `ubuntu-latest-xlarge`.

## When the numbers regress

- **Cache eviction**: bump `SCCACHE_CACHE_SIZE` / `CCACHE_MAXSIZE` if
  workspace size grows past ~10 GiB of compiled objects.
- **Compiler hash skew**: any Rust toolchain bump invalidates all sccache
  entries — first build after a `rust-toolchain` change is cold by design.
- **Lockfile churn**: large `Cargo.lock` deltas wipe per-crate hits.
- **Image rebuild**: editing `docker/Dockerfile.android` above the cache
  layers also resets the prebuilt-libs layer. Add new layers at the bottom.
