#!/usr/bin/env bash
# gen-cask.sh — stamp Casks/copypaste.rb with a version and a DMG checksum.
#
#   scripts/release/gen-cask.sh <version> <sha256> [--out <dir>]
#
# Rewrites the `version` and `sha256` lines of Casks/copypaste.rb in place, and
# optionally writes a copy to <dir>/Casks/copypaste.rb for publishing to the
# tap.
#
# There is one cask file, not a template plus a generated artefact. Two files
# that must agree is a thing to keep in sync; one file that gets two lines
# rewritten is not.
#
# This script does not commit, push, set a git remote or read a token.
# Publishing is the workflow's job; this script only edits a file.
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

# Reject a leading 'v'. The cask's `version` is bare and its `url` re-adds a
# single 'v'; accepting "v2.0.0" here silently produces a URL with "vv".
if [[ "$VERSION" == v* ]]; then
    echo "ERROR: pass the version without a leading 'v' (got: $VERSION)" >&2
    exit 1
fi
if [[ ! "$SHA256" =~ ^[a-f0-9]{64}$ ]]; then
    echo "ERROR: sha256 must be 64 lowercase hex characters (got: $SHA256)" >&2
    exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

CASK="Casks/copypaste.rb"
[[ -f "$CASK" ]] || { echo "ERROR: $CASK not found" >&2; exit 1; }

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# Rewrite only the first `version`/`sha256` at the top level of the cask.
# Anchored to the start of the line with optional leading whitespace so the
# words appearing inside comments or caveats are untouched — the cask has a
# long comment block that says both.
awk -v ver="$VERSION" -v sha="$SHA256" '
    !done_version && /^[[:space:]]*version[[:space:]]+"/ {
        sub(/version[[:space:]]+"[^"]*"/, "version \"" ver "\"")
        done_version = 1
        print
        next
    }
    !done_sha && /^[[:space:]]*sha256[[:space:]]+/ {
        sub(/sha256[[:space:]]+.*/, "sha256 \"" sha "\"")
        done_sha = 1
        print
        next
    }
    { print }
' "$CASK" > "$TMP"

# Verify rather than trust. A silently-unmodified cask would publish a stale
# checksum, and Homebrew would reject the download with a mismatch that looks
# like a corrupted release rather than a broken release script.
grep -qE "^[[:space:]]*version \"${VERSION}\"$" "$TMP" \
    || { echo "ERROR: failed to rewrite the version line in $CASK" >&2; exit 1; }
grep -qE "^[[:space:]]*sha256 \"${SHA256}\"$" "$TMP" \
    || { echo "ERROR: failed to rewrite the sha256 line in $CASK" >&2; exit 1; }

cat "$TMP" > "$CASK"

echo "==> $CASK"
echo "    version $VERSION"
echo "    sha256  $SHA256"

if [[ -n "$OUT_DIR" ]]; then
    mkdir -p "$OUT_DIR/Casks"
    cp "$CASK" "$OUT_DIR/Casks/copypaste.rb"
    echo "    copied  $OUT_DIR/Casks/copypaste.rb"
fi

if command -v brew >/dev/null 2>&1; then
    # `brew audit` on a third-party tap is advisory: the notarisation rule that
    # closes homebrew/cask to us is one of the things it checks, so a clean
    # pass is not expected and is not required. Run it anyway — it also catches
    # ordinary mistakes like a malformed stanza or an unreachable URL.
    echo "==> brew audit (advisory — see ADR-0001 for why it will complain)"
    brew audit --cask --strict "$REPO_ROOT/$CASK" || true
fi

echo
git --no-pager diff -- "$CASK" || true
