#!/usr/bin/env bash
# verify-checksum.sh — emit SHA256SUMS for all release artefacts in dist/.
#
# Usage: scripts/release/verify-checksum.sh [output-dir]
#   output-dir  defaults to dist
#
# Captures: *.dmg, *.tar.gz, *.zip, *.deb, *.rpm, *.AppImage, *.msi
# Writes:   <output-dir>/SHA256SUMS  (one "<hash>  <basename>" per artefact)
#
# Format is compatible with `shasum -a 256 -c SHA256SUMS` on macOS/Linux.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

OUT_DIR="${1:-dist}"
if [[ ! -d "$OUT_DIR" ]]; then
    echo "ERROR: output dir not found: $OUT_DIR" >&2
    exit 1
fi

OUT_FILE="${OUT_DIR}/SHA256SUMS"

# Pick a hasher: prefer shasum (default on macOS), fall back to sha256sum (Linux CI).
if command -v shasum >/dev/null 2>&1; then
    HASHER=(shasum -a 256)
elif command -v sha256sum >/dev/null 2>&1; then
    HASHER=(sha256sum)
else
    echo "ERROR: neither shasum nor sha256sum is available" >&2
    exit 1
fi

cd "$OUT_DIR"

# Collect artefacts (basenames only). Use find -maxdepth 1 to avoid recursing
# into intermediate build dirs like target/release/deps.
shopt -s nullglob
artefacts=()
for f in *.dmg *.tar.gz *.zip *.deb *.rpm *.AppImage *.msi; do
    [[ -f "$f" ]] && artefacts+=("$f")
done
shopt -u nullglob

if [[ ${#artefacts[@]} -eq 0 ]]; then
    echo "ERROR: no release artefacts (*.dmg, *.tar.gz, *.zip, ...) found in $OUT_DIR" >&2
    exit 1
fi

echo "==> Hashing ${#artefacts[@]} artefact(s) → $OUT_FILE"
: > "$(basename "$OUT_FILE")"  # truncate
for f in "${artefacts[@]}"; do
    "${HASHER[@]}" "$f" >> "$(basename "$OUT_FILE")"
done

echo
cat "$(basename "$OUT_FILE")"
echo
echo "Wrote: $OUT_FILE"
