#!/usr/bin/env bash
# Build the native Linux Flutter bundle and stage the daemon and CLI beside it.
set -euo pipefail

VERSION="${1:-}"
BUILD_NUMBER="${2:-}"
ARCHITECTURE="${3:-}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "ERROR: usage: $0 <stable-version> <build-number> <x86_64|aarch64>" >&2
  exit 1
}
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || {
  echo "ERROR: build-number must be a positive integer" >&2
  exit 1
}
case "$ARCHITECTURE" in
  x86_64|aarch64) ;;
  *) echo "ERROR: architecture must be x86_64 or aarch64" >&2; exit 1 ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_ROOT="$ROOT/apps/copypaste_flutter"
CONTRACT="$ROOT/packaging/linux/release-contract.json"
[[ "$(uname -s)" == Linux ]] || { echo "ERROR: Linux packaging requires Linux" >&2; exit 1; }
case "$(uname -m)" in
  x86_64) HOST_ARCHITECTURE=x86_64 ;;
  aarch64|arm64) HOST_ARCHITECTURE=aarch64 ;;
  *) echo "ERROR: unsupported native Linux architecture: $(uname -m)" >&2; exit 1 ;;
esac
[[ "$HOST_ARCHITECTURE" == "$ARCHITECTURE" ]] || {
  echo "ERROR: Linux release builds are native; requested $ARCHITECTURE on $HOST_ARCHITECTURE" >&2
  exit 1
}
[[ -f "$APP_ROOT/linux/CMakeLists.txt" ]] || {
  echo "ERROR: the committed Flutter Linux runner is required for a Linux release" >&2
  exit 1
}
[[ -f "$CONTRACT" ]] || { echo "ERROR: missing Linux release contract" >&2; exit 1; }

workspace_version="$(awk '/^\[workspace\.package\]/{inside=1; next} inside && /^version[[:space:]]*=/{gsub(/[\"[:space:]]/, "", $3); print $3; exit}' "$ROOT/Cargo.toml")"
pubspec_version="$(awk '/^version:[[:space:]]*/ {print $2; exit}' "$APP_ROOT/pubspec.yaml")"
[[ "$workspace_version" == "$VERSION" && "${pubspec_version%%+*}" == "$VERSION" ]] || {
  echo "ERROR: Cargo.toml and pubspec.yaml must both identify version $VERSION" >&2
  exit 1
}

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export CARGO_BUILD_JOBS=2
export CMAKE_BUILD_PARALLEL_LEVEL=2
cargo build --jobs 2 --manifest-path "$ROOT/Cargo.toml" --release --locked \
  -p copypaste-daemon -p copypaste-cli

cd "$APP_ROOT"
flutter pub get --enforce-lockfile
flutter build linux --release --build-name "$VERSION" --build-number "$BUILD_NUMBER"

mapfile -t bundles < <(find build/linux -type d -path '*/release/bundle' -print | sort)
[[ "${#bundles[@]}" == 1 ]] || {
  echo "ERROR: Flutter must produce exactly one Linux release bundle" >&2
  exit 1
}
BUNDLE="${bundles[0]}"
[[ -x "$BUNDLE/copypaste" ]] || {
  echo "ERROR: Flutter Linux release bundle has no copypaste executable" >&2
  exit 1
}
install -m 755 "$ROOT/target/release/copypaste-daemon" "$BUNDLE/copypaste-daemon"
install -m 755 "$ROOT/target/release/copypaste" "$BUNDLE/copypaste-cli"

OUTPUT="$ROOT/dist/linux-$ARCHITECTURE/bundle"
rm -rf "$OUTPUT"
mkdir -p "$(dirname "$OUTPUT")"
cp -a "$BUNDLE" "$OUTPUT"
[[ -x "$OUTPUT/copypaste-daemon" && -x "$OUTPUT/copypaste-cli" ]] || {
  echo "ERROR: staged Linux bundle is missing a runtime helper" >&2
  exit 1
}
