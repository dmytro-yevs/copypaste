#!/usr/bin/env bash
# gen-formula.sh — stamp packaging/homebrew/copypaste-cli.rb with a version and
# the CLI tarball's checksum.
#
#   scripts/release/gen-formula.sh <version> <sha256> [--out <dir>]
#
# Same contract as gen-cask.sh: rewrites two lines in the one checked-in file,
# verifies the rewrite happened, optionally drops a copy into a tap checkout.
# Publishing is not this script's business and it reads no credential.
set -euo pipefail

usage() {
    echo "Usage: $0 <version> <sha256> [--out <dir>]" >&2
    exit 1
}

VERSION="${1:-}"
SHA256="${2:-}"
shift 2 2>/dev/null || usage
[[ -n "$VERSION" && -n "$SHA256" ]] || usage

OUT_DIR=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --out) [[ $# -ge 2 ]] || usage; OUT_DIR="$2"; shift 2 ;;
        *) echo "ERROR: unknown argument: $1" >&2; usage ;;
    esac
done

[[ "$VERSION" != v* ]] || { echo "ERROR: pass the version without a leading 'v'" >&2; exit 1; }
[[ "$SHA256" =~ ^[a-f0-9]{64}$ ]] || { echo "ERROR: sha256 must be 64 lowercase hex characters" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FORMULA="packaging/homebrew/copypaste-cli.rb"
[[ -f "$FORMULA" ]] || { echo "ERROR: $FORMULA not found" >&2; exit 1; }

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

awk -v ver="$VERSION" -v sha="$SHA256" '
    !done_version && /^[[:space:]]*version[[:space:]]+"/ {
        sub(/version[[:space:]]+"[^"]*"/, "version \"" ver "\"")
        done_version = 1
        print
        next
    }
    !done_sha && /^[[:space:]]*sha256[[:space:]]+"/ {
        sub(/sha256[[:space:]]+"[^"]*"/, "sha256 \"" sha "\"")
        done_sha = 1
        print
        next
    }
    { print }
' "$FORMULA" > "$TMP"

grep -qE "^[[:space:]]*version \"${VERSION}\"$" "$TMP" \
    || { echo "ERROR: failed to rewrite the version line in $FORMULA" >&2; exit 1; }
grep -qE "^[[:space:]]*sha256 \"${SHA256}\"$" "$TMP" \
    || { echo "ERROR: failed to rewrite the sha256 line in $FORMULA" >&2; exit 1; }

cat "$TMP" > "$FORMULA"

echo "==> $FORMULA"
echo "    version $VERSION"
echo "    sha256  $SHA256"

if [[ -n "$OUT_DIR" ]]; then
    mkdir -p "$OUT_DIR/Formula"
    cp "$FORMULA" "$OUT_DIR/Formula/copypaste-cli.rb"
    echo "    copied  $OUT_DIR/Formula/copypaste-cli.rb"
fi

if command -v brew >/dev/null 2>&1; then
    echo "==> brew audit (advisory)"
    brew audit --formula --strict "$REPO_ROOT/$FORMULA" || true
fi

echo
git --no-pager diff -- "$FORMULA" || true
