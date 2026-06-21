#!/usr/bin/env bash
# Build copypaste-daemon + copypaste CLI for macOS (arm64, x86_64, or universal).
# Outputs to builds/macos-<arch>/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ARCH="${1:-arm64}"
OUT_BASE="$ROOT/builds"

build_target() {
  local arch="$1"
  local rust_target="$2"
  local out_dir="$OUT_BASE/macos-${arch}"

  echo "  -> target: ${rust_target}"
  if ! rustup target list --installed | grep -q "^${rust_target}$"; then
    echo "  !! rust target '${rust_target}' not installed."
    echo "     Install: rustup target add ${rust_target}"
    return 1
  fi

  cargo build --release --target "$rust_target" \
    -p copypaste-daemon -p copypaste-cli -p copypaste-relay \
    --features copypaste-daemon/cloud-sync,copypaste-daemon/relay-sync

  mkdir -p "$out_dir"
  cp "target/${rust_target}/release/copypaste-daemon" "$out_dir/"
  cp "target/${rust_target}/release/copypaste"        "$out_dir/"
  cp "target/${rust_target}/release/copypaste-relay"  "$out_dir/"
  echo "  -> wrote $out_dir/{copypaste-daemon,copypaste,copypaste-relay}"
}

build_universal() {
  local out_dir="$OUT_BASE/macos-universal"
  local arm_dir="$OUT_BASE/macos-arm64"
  local x86_dir="$OUT_BASE/macos-x86_64"

  if [[ ! -x "$arm_dir/copypaste-daemon" || ! -x "$x86_dir/copypaste-daemon" ]]; then
    echo "  !! universal build needs both arm64 + x86_64 binaries first."
    echo "     Run: bash scripts/build-macos.sh arm64 && bash scripts/build-macos.sh x86_64"
    return 1
  fi

  mkdir -p "$out_dir"
  lipo -create -output "$out_dir/copypaste-daemon" \
    "$arm_dir/copypaste-daemon" "$x86_dir/copypaste-daemon"
  lipo -create -output "$out_dir/copypaste" \
    "$arm_dir/copypaste" "$x86_dir/copypaste"
  lipo -create -output "$out_dir/copypaste-relay" \
    "$arm_dir/copypaste-relay" "$x86_dir/copypaste-relay"
  echo "  -> wrote universal binaries to $out_dir/"
  lipo -info "$out_dir/copypaste-daemon"
}

bundle_app() {
  local arch="$1"
  # make_app_bundle.sh reads from target/release/, not from builds/.
  # Only safe to bundle when last `cargo build --release` matches our arch
  # (i.e. arch == host). Skip otherwise.
  local host_arch
  host_arch="$(uname -m)"
  case "$host_arch" in
    arm64)  host_arch="arm64"   ;;
    x86_64) host_arch="x86_64"  ;;
  esac

  if [[ "$arch" != "$host_arch" && "$arch" != "universal" ]]; then
    echo "  -> skipping .app bundle for $arch (host is $host_arch; make_app_bundle reads target/release/)"
    return 0
  fi

  # Stage host-arch binaries (or universal) into target/release/ so make_app_bundle.sh picks them up.
  local src_dir="$OUT_BASE/macos-${arch}"
  if [[ ! -x "$src_dir/copypaste-daemon" ]]; then
    echo "  -> no binaries at $src_dir, skipping .app bundle"
    return 0
  fi
  mkdir -p target/release
  cp "$src_dir/copypaste-daemon"   target/release/
  cp "$src_dir/copypaste"          target/release/
  cp "$src_dir/copypaste-relay"    target/release/

  # Derive version from the workspace Cargo.toml ([workspace.package] version).
  local cargo_version
  cargo_version="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
  bash scripts/make_app_bundle.sh "$cargo_version" || {
    echo "  !! make_app_bundle.sh failed (non-fatal)"
    return 0
  }

  if [[ -d "dist/CopyPaste.app" ]]; then
    rm -rf "$OUT_BASE/macos-${arch}/CopyPaste.app"
    cp -R "dist/CopyPaste.app" "$OUT_BASE/macos-${arch}/CopyPaste.app"
    echo "  -> bundled $OUT_BASE/macos-${arch}/CopyPaste.app"
  fi
}

case "$ARCH" in
  arm64)
    build_target arm64 aarch64-apple-darwin
    bundle_app arm64
    ;;
  x86_64)
    build_target x86_64 x86_64-apple-darwin
    bundle_app x86_64
    ;;
  universal)
    build_universal
    bundle_app universal
    ;;
  *)
    echo "Unknown arch: $ARCH"
    echo "Usage: $0 [arm64|x86_64|universal]"
    exit 1
    ;;
esac
