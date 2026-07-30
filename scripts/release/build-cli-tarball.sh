#!/usr/bin/env bash
# build-cli-tarball.sh — build, ad-hoc sign and archive the CLI + daemon.
#
#   Usage: scripts/release/build-cli-tarball.sh <version> [arch]
#
# Output: dist/copypaste-cli-v<version>-macos-<arch>.tar.gz  (+ .sha256)
#
# macOS only. Never executed on a Mac by its author.
#
# The signing here is not about Gatekeeper. macOS on Apple Silicon will not
# execute an unsigned Mach-O at all — the kernel requires at least an ad-hoc
# signature — so this is the floor for a binary that has to run, not an extra.
set -euo pipefail

VERSION="${1:-}"
[[ -n "$VERSION" ]] || { echo "ERROR: version required. Usage: $0 <version> [arch]" >&2; exit 1; }
VERSION="${VERSION#v}"

ARCH="${2:-}"
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64)  ARCH="arm64"  ;;
        x86_64) ARCH="x86_64" ;;
        *) echo "ERROR: cannot infer arch from $(uname -m)" >&2; exit 1 ;;
    esac
fi
case "$ARCH" in
    arm64)  TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TRIPLE="x86_64-apple-darwin"  ;;
    *) echo "ERROR: arch must be arm64 or x86_64" >&2; exit 1 ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

[[ "$(uname -s)" == "Darwin" ]] || { echo "ERROR: must run on macOS" >&2; exit 1; }

DIST="dist"
OUT="${DIST}/copypaste-cli-v${VERSION}-macos-${ARCH}.tar.gz"
mkdir -p "$DIST"

echo "==> Building for $TRIPLE"
rustup target add "$TRIPLE" >/dev/null 2>&1 || true
cargo build --release --locked --target "$TRIPLE" -p copypaste-daemon -p copypaste-cli

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

for bin in copypaste copypaste-daemon; do
    src="target/${TRIPLE}/release/${bin}"
    [[ -f "$src" ]] || { echo "ERROR: $src missing" >&2; exit 1; }
    cp "$src" "$STAGE/"
    # A stable --identifier so the signature is not derived from the filename
    # plus a build hash. Under ad-hoc the cdhash still moves every build; the
    # identifier not moving is what keeps anything keyed on it deterministic.
    codesign --force --sign - \
        --identifier "com.copypaste.${bin}" \
        --timestamp=none \
        "$STAGE/${bin}"
    codesign --verify --strict "$STAGE/${bin}"
    echo "    signed $bin"
done

# Licences travel with the binaries — the formula declares
# `license any_of: ["MIT", "Apache-2.0"]` and both texts should be present in
# what the user actually downloaded.
cp LICENSE-MIT LICENSE-APACHE "$STAGE/"

echo "==> Archiving"
# Flat archive: the formula's `bin.install "copypaste"` expects the binaries at
# the archive root, not under a version-named directory.
#
# --no-mac-metadata / COPYFILE_DISABLE stop bsdtar writing ._AppleDouble
# entries for extended attributes. Without it the archive carries resource
# forks that are meaningless off macOS and that change the checksum depending
# on which xattrs happened to be set.
COPYFILE_DISABLE=1 tar --no-mac-metadata -czf "$OUT" -C "$STAGE" .

echo "==> Checksum"
( cd "$DIST" && shasum -a 256 "$(basename "$OUT")" > "$(basename "$OUT").sha256" )
cat "${OUT}.sha256"

echo
ls -lh "$OUT"
