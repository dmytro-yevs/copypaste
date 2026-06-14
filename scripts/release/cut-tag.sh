#!/usr/bin/env bash
# cut-tag.sh — bump workspace version, regen Cargo.lock, commit, create annotated tag.
#
# Usage: scripts/release/cut-tag.sh <version>
#   <version>  semver string without leading 'v', e.g. 0.2.0-beta.1
#
# Creates tag 'v<version>' on the current branch.
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "ERROR: version required. Usage: $0 <version> (e.g. 0.2.0-beta.1)" >&2
    exit 1
fi

# Reject leading 'v' to avoid double 'vv0.2.0' tags.
if [[ "$VERSION" == v* ]]; then
    echo "ERROR: pass version without leading 'v' (got: $VERSION)" >&2
    exit 1
fi

# Repo root = parent of scripts/release/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

CARGO_TOML="Cargo.toml"
TAURI_CONF="crates/copypaste-ui/src-tauri/tauri.conf.json"
if [[ ! -f "$CARGO_TOML" ]]; then
    echo "ERROR: $CARGO_TOML not found at $REPO_ROOT" >&2
    exit 1
fi
if [[ ! -f "$TAURI_CONF" ]]; then
    echo "ERROR: $TAURI_CONF not found at $REPO_ROOT" >&2
    exit 1
fi

TAG="v${VERSION}"
if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "ERROR: tag $TAG already exists" >&2
    exit 1
fi

# Refuse dirty tree (avoid sweeping unrelated changes into the release commit).
if [[ -n "$(git status --porcelain)" ]]; then
    echo "ERROR: working tree is dirty. Commit or stash before cutting a tag." >&2
    git status --short >&2
    exit 1
fi

echo "==> Bumping [workspace.package].version to $VERSION"
# Replace only the version line inside the [workspace.package] table.
# Use awk to scope the substitution to that table.
awk -v ver="$VERSION" '
    /^\[workspace\.package\]/ { in_wp = 1; print; next }
    /^\[/                     { in_wp = 0 }
    in_wp && /^version[[:space:]]*=/ {
        print "version = \"" ver "\""
        next
    }
    { print }
' "$CARGO_TOML" > "$CARGO_TOML.tmp"
mv "$CARGO_TOML.tmp" "$CARGO_TOML"

# Sanity check the bump landed.
if ! rg -q "^version[[:space:]]*=[[:space:]]*\"$VERSION\"" "$CARGO_TOML"; then
    echo "ERROR: version bump failed — $CARGO_TOML still does not contain $VERSION" >&2
    exit 1
fi

echo "==> Bumping $TAURI_CONF version to $VERSION"
# Replace the top-level "version" field in tauri.conf.json.
# The file has exactly one top-level "version" key.
TMP="$(mktemp)"
sed 's/"version": "[^"]*"/"version": "'"$VERSION"'"/' "$TAURI_CONF" > "$TMP"
if rg -qF "\"version\": \"${VERSION}\"" "$TMP"; then
    mv "$TMP" "$TAURI_CONF"
else
    echo "ERROR: version bump failed — $TAURI_CONF still does not contain $VERSION" >&2
    rm -f "$TMP"
    exit 1
fi

echo "==> Regenerating Cargo.lock"
cargo generate-lockfile

echo "==> Committing"
git add Cargo.toml Cargo.lock "$TAURI_CONF"
git commit -m "chore(release): cut $TAG"

echo "==> Tagging $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo
echo "Done. Next steps:"
echo "  git push origin HEAD"
echo "  git push origin $TAG"
