#!/usr/bin/env bash
# gen-cask.sh — update Casks/copypaste.rb with new version + sha256.
#
# Usage: scripts/release/gen-cask.sh <version> <sha256>
#
# Used by the release maintainer AFTER artefacts are uploaded to the GitHub
# release. Prints a PR-ready diff and leaves the change uncommitted so the
# maintainer can inspect before opening a PR against the Casks repo.
#
# The cask file is owned by worktree W1.5; this script only edits two fields:
#   version "<...>"
#   sha256  "<...>"
set -euo pipefail

VERSION="${1:-}"
SHA256="${2:-}"

if [[ -z "$VERSION" || -z "$SHA256" ]]; then
    echo "ERROR: usage: $0 <version> <sha256>" >&2
    exit 1
fi

# Reject leading 'v' on version to keep cask string clean.
if [[ "$VERSION" == v* ]]; then
    echo "ERROR: pass version without leading 'v' (got: $VERSION)" >&2
    exit 1
fi

# Validate sha256 shape: 64 lowercase hex chars.
if [[ ! "$SHA256" =~ ^[a-f0-9]{64}$ ]]; then
    echo "ERROR: sha256 must be 64 lowercase hex chars (got: $SHA256)" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

CASK="Casks/copypaste.rb"
if [[ ! -f "$CASK" ]]; then
    echo "ERROR: cask not found at $CASK" >&2
    echo "       The cask file is owned by worktree W1.5. Run this only" >&2
    echo "       after that work is merged." >&2
    exit 1
fi

echo "==> Updating $CASK"
echo "    version → $VERSION"
echo "    sha256  → $SHA256"

# Edit version "...":  matches `  version "anything"` (Ruby DSL).
# Edit sha256 "...":   matches `  sha256 "anything"`.
# Use a portable sed invocation that works on BSD sed (macOS) and GNU sed.
TMP="$(mktemp)"
awk -v ver="$VERSION" -v sha="$SHA256" '
    {
        if (match($0, /^([[:space:]]*)version[[:space:]]+"[^"]*"/, m)) {
            print m[1] "version \"" ver "\""
            next
        }
        if (match($0, /^([[:space:]]*)sha256[[:space:]]+"[^"]*"/, m)) {
            print m[1] "sha256 \"" sha "\""
            next
        }
        print
    }
' "$CASK" > "$TMP"

# Verify both fields actually changed.
if ! grep -E "^[[:space:]]*version[[:space:]]+\"$VERSION\"" "$TMP" >/dev/null; then
    echo "ERROR: failed to update version line in $CASK" >&2
    rm -f "$TMP"
    exit 1
fi
if ! grep -E "^[[:space:]]*sha256[[:space:]]+\"$SHA256\"" "$TMP" >/dev/null; then
    echo "ERROR: failed to update sha256 line in $CASK" >&2
    rm -f "$TMP"
    exit 1
fi

mv "$TMP" "$CASK"

echo
echo "==> Diff:"
git --no-pager diff -- "$CASK" || true

echo
echo "Done. Review then commit:"
echo "  git add $CASK"
echo "  git commit -m \"chore(cask): bump to $VERSION\""
