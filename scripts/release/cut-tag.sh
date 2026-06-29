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

# CopyPaste-crh3.64: also bump the Android Gradle dev-default versionName /
# versionCode so a LOCAL `gradlew assembleRelease` after a tag cut produces a
# correctly-versioned APK. (CI release overrides these from the git tag, so
# released APKs were already correct — this fixes the latent local/non-overriding
# path.) versionCode = major*10000 + minor*100 + patch from the numeric core of
# the version (any -prerelease/+build suffix is stripped for the integer code).
GRADLE="android/app/build.gradle.kts"
if [[ -f "$GRADLE" ]]; then
    NUMERIC_VERSION="${VERSION%%-*}"
    IFS='.' read -r VMAJOR VMINOR VPATCH <<< "$NUMERIC_VERSION"
    if [[ -z "$VMAJOR" || -z "$VMINOR" || -z "$VPATCH" ]]; then
        echo "ERROR: cannot derive Android versionCode from '$VERSION' (need major.minor.patch)" >&2
        exit 1
    fi
    VERSION_CODE=$(( VMAJOR * 10000 + VMINOR * 100 + VPATCH ))
    echo "==> Bumping $GRADLE versionName=$VERSION versionCode=$VERSION_CODE"
    GTMP="$(mktemp)"
    sed \
        -e 's/\(versionName = (project\.findProperty("versionName") as String?) ?: \)"[^"]*"/\1"'"$VERSION"'"/' \
        -e 's/\(versionCode = (project\.findProperty("versionCode") as String?)?\.toInt() ?: \)[0-9][0-9]*/\1'"$VERSION_CODE"'/' \
        "$GRADLE" > "$GTMP"
    if rg -qF "?: \"${VERSION}\"" "$GTMP" && rg -qF "?: ${VERSION_CODE}" "$GTMP"; then
        mv "$GTMP" "$GRADLE"
    else
        echo "ERROR: Android version bump failed — $GRADLE unchanged (regex did not match the dev-default lines)" >&2
        rm -f "$GTMP"
        exit 1
    fi
fi

echo "==> Regenerating Cargo.lock"
cargo generate-lockfile

echo "==> Committing"
git add Cargo.toml Cargo.lock "$TAURI_CONF"
if [[ -f "$GRADLE" ]]; then
    git add "$GRADLE"
fi
git commit -m "chore(release): cut $TAG"

echo "==> Tagging $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo
echo "Done. Next steps:"
echo "  git push origin HEAD"
echo "  git push origin $TAG"
